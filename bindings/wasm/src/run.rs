//! Runs the commands of `commands.rs`: reads answer from the text and its index; writes turn the
//! minimal-span edits of `kition-note-edit` into one KDM transaction.

use kition_kit::kdm::comment::{self, AnchorRange, CommentAnchor, NewComment};
use kition_kit::kdm::{ObjectId, Origin, Replica, Tx};
use kition_kit::session::{KitError, Result};
use kition_note_check::FileLookup;
use kition_note_edit::{Edit, EditError, HeadingRef, block, find as search, links, props, section, table};
use kition_note_kdm::file::merge::TextEdit;
use kition_note_kdm::file::{Eol, LineStyle, normalize_inserted};
use kition_note_kdm::{BODY, NoteModel, apply_text_edits, root_id, set_style, write_body};
use kition_note_markdown::{Align, Anchor, Index, LinkKind, Style, Typed, index, sub};
use serde_json::{Map, Value, json};

use crate::clock;
use crate::files::HostFiles;
use crate::positions::{Unit, byte_range};
use crate::projection::{Kind, Snapshot};

/// Sets `key` of a JSON object.
fn put(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_owned(), value);
    }
}

fn err(error: EditError) -> KitError {
    KitError::new(error.to_string())
}

fn invalid(message: impl Into<String>) -> KitError {
    KitError::new(format!("invalid-params: {}", message.into()))
}

fn number(params: &Value, key: &str) -> Result<Option<usize>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} must be a whole number from 0, not {value}"))),
    }
}

fn need_number(params: &Value, key: &str) -> Result<usize> {
    number(params, key)?.ok_or_else(|| invalid(format!("{key} is required")))
}

fn text_of<'a>(params: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(other) => Err(invalid(format!("{key} must be a string, not {other}"))),
    }
}

fn need_text<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    text_of(params, key)?.ok_or_else(|| invalid(format!("{key} is required")))
}

fn flag(params: &Value, key: &str) -> Result<Option<bool>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(other) => Err(invalid(format!("{key} must be true or false, not {other}"))),
    }
}

fn texts(params: &Value, key: &str) -> Result<Option<Vec<String>>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| invalid(format!("{key} must be a list of strings"))))
            .collect::<Result<_>>()
            .map(Some),
        Some(other) => Err(invalid(format!("{key} must be a list of strings, not {other}"))),
    }
}

/// `heading` (a path) or `line`; one of them when `needed`.
fn heading_ref(params: &Value, heading: &str, line: &str, needed: bool) -> Result<Option<HeadingRef>> {
    match (texts(params, heading)?, number(params, line)?) {
        (Some(_), Some(_)) => Err(invalid(format!("give {heading} or {line}, not both"))),
        (Some(path), None) => Ok(Some(HeadingRef::Path(path))),
        (None, Some(line)) => Ok(Some(HeadingRef::Line(line))),
        (None, None) if needed => Err(invalid(format!("{heading} (a heading path) or {line} is required"))),
        (None, None) => Ok(None),
    }
}

fn need_heading(params: &Value) -> Result<HeadingRef> {
    heading_ref(params, "heading", "line", true)?.ok_or_else(|| invalid("heading or line is required"))
}

fn position_param(params: &Value, needed: bool) -> Result<section::Position> {
    match text_of(params, "position")? {
        Some(name) => section::Position::parse(name).ok_or_else(|| invalid(format!("position must be before, after, start or end, not {name:?}"))),
        None if needed => Err(invalid("position is required")),
        None => Ok(section::Position::End),
    }
}

/// Positions of the text in the request's unit.
struct Positions<'a> {
    text: &'a str,
    ix: &'a Index,
    unit: Unit,
}

impl Positions<'_> {
    fn at(&self, byte: usize) -> usize {
        match self.unit {
            Unit::Scalar => self.ix.lines.scalar(self.text, byte),
            Unit::Utf16 => self.ix.lines.utf16(self.text, byte),
        }
    }

    /// A position of the request as a byte offset.
    fn byte(&self, position: usize) -> Result<usize> {
        let found = match self.unit {
            Unit::Scalar => self.ix.lines.byte_of_scalar(self.text, position),
            Unit::Utf16 => self.ix.lines.byte_of_utf16(self.text, position),
        };
        found.ok_or_else(|| KitError::new(format!("out-of-range: {position} is past the end of the text or inside a character")))
    }

    fn line(&self, byte: usize) -> usize {
        self.ix.lines.line_of(byte) + 1
    }
}

fn typed_value(typed: &Typed) -> Value {
    match typed {
        Typed::String(s) | Typed::Date(s) | Typed::DateTime(s) => json!(s),
        Typed::Number(n) => n.parse::<f64>().ok().and_then(|n| serde_json::Number::from_f64(n).map(Value::Number)).unwrap_or_else(|| json!(n)),
        Typed::Boolean(b) => json!(b),
        Typed::Null => Value::Null,
        Typed::List(items) => json!(items),
        Typed::Other => Value::Null,
    }
}

fn property_value(params: &Value) -> Result<props::Value> {
    match params.get("value") {
        None => Err(invalid("value is required")),
        Some(Value::Null) => Ok(props::Value::Null),
        Some(Value::Bool(b)) => Ok(props::Value::Bool(*b)),
        Some(Value::Number(n)) => Ok(props::Value::Number(n.to_string())),
        Some(Value::String(s)) => Ok(props::Value::Str(s.clone())),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| invalid("a list value holds strings only")))
            .collect::<Result<_>>()
            .map(props::Value::List),
        Some(other) => Err(invalid(format!("value must be a string, number, boolean, null or a list of strings, not {other}"))),
    }
}

/// A link's state: whether its destination exists (`unknown` when the host cannot say).
fn link_state(files: &dyn FileLookup, ix: &Index, link: &kition_note_markdown::Link) -> &'static str {
    if link.is_external() {
        return "external";
    }
    if link.file.trim().is_empty() {
        return match &link.anchor {
            None => "missing",
            Some(Anchor::Heading(name))
                if ix.headings.iter().any(|heading| heading.text.to_lowercase() == name.rsplit('#').next().unwrap_or(name).trim().to_lowercase()) =>
            {
                "resolved"
            }
            Some(Anchor::Block(id)) if ix.block_ids.iter().any(|block| block.id == *id) => "resolved",
            Some(_) => "missing-anchor",
        };
    }
    let file = link.file.trim();
    let named = if link.kind.is_wiki() && !file.rsplit('/').next().unwrap_or(file).contains('.') { format!("{file}.md") } else { file.to_owned() };
    let mut candidates = vec![named.clone()];
    if link.kind.is_wiki() && !named.starts_with('/') {
        candidates.push(format!("/{named}"));
    }
    let answers: Vec<Option<bool>> = candidates.iter().map(|path| files.exists(path)).collect();
    if answers.contains(&Some(true)) {
        "resolved"
    } else if answers.iter().all(|answer| *answer == Some(false)) {
        "missing"
    } else {
        "unknown"
    }
}

/// Runs a read-only command; listings name each object by its id in the projection `ids` (of
/// the text now).
pub fn read(model: &NoteModel, replica: &Replica, id: &str, params: &Value, ids: &Snapshot) -> Result<Value> {
    let text = model.body();
    let unit = Unit::of(params)?;
    if id == "text.read" {
        let len = unit.len(text);
        let from = unit.to_scalar(text, number(params, "from")?.unwrap_or(0))?;
        let to = unit.to_scalar(text, number(params, "to")?.unwrap_or(len))?;
        if from > to {
            return Err(invalid(format!("from ({from}) is after to ({to})")));
        }
        return Ok(json!({ "text": text.get(byte_range(text, from, to)).unwrap_or_default(), "length": len }));
    }
    if id == "template.expand" {
        let markdown = need_text(params, "markdown")?;
        let title = text_of(params, "title")?.unwrap_or_default();
        let when = match text_of(params, "date")? {
            Some(date) => clock::DateTime::parse(date)
                .ok_or_else(|| invalid(format!("date must be ISO 8601 (2026-10-23 or 2026-10-23T14:05), not {date:?}")))?,
            None => clock::now(),
        };
        return Ok(json!({ "markdown": clock::expand_template(markdown, title, &when) }));
    }
    let ix = index(text);
    let at = Positions { text, ix: &ix, unit };
    match id {
        "outline.read" => {
            let limit = number(params, "limit")?.unwrap_or(200).clamp(1, 1000);
            let start = match text_of(params, "cursor")? {
                Some(cursor) => cursor.parse::<usize>().map_err(|_| invalid(format!("cursor {cursor:?} is not one this command answered")))?,
                None => 0,
            };
            let headings: Vec<Value> = ix
                .headings
                .iter()
                .skip(start)
                .take(limit)
                .map(|heading| {
                    let words = index(sub(text, heading.range.end..heading.section.end)).words;
                    let line = at.line(heading.range.start);
                    json!({ "path": heading.path, "level": heading.level, "line": line, "from": at.at(heading.section.start), "to": at.at(heading.section.end), "words": words })
                })
                .collect();
            let next = start + headings.len();
            let mut answer = json!({ "headings": headings, "total": ix.headings.len() });
            if next < ix.headings.len() {
                put(&mut answer, "next", json!(next.to_string()));
            }
            Ok(answer)
        }
        "section.read" => {
            let heading = need_heading(params)?;
            let range = section::read(&ix, &heading, flag(params, "withHeading")?.unwrap_or(true)).map_err(err)?;
            Ok(json!({ "markdown": sub(text, range.clone()), "from": at.at(range.start), "to": at.at(range.end), "line": at.line(range.start) }))
        }
        "block.read" => {
            let found = block::find(&ix, need_text(params, "blockId")?, None).map_err(err)?;
            Ok(
                json!({ "markdown": sub(text, found.block.clone()), "line": at.line(found.block.start), "from": at.at(found.block.start), "to": at.at(found.block.end) }),
            )
        }
        "property.list" => {
            let Some(fm) = &ix.frontmatter else { return Ok(json!({ "properties": [] })) };
            let properties: Vec<Value> = fm
                .entries
                .iter()
                .map(|entry| {
                    let mut property = json!({
                        "key": entry.key,
                        "type": entry.typed.as_ref().map_or("unknown", Typed::kind),
                        "value": entry.typed.as_ref().map_or(Value::Null, typed_value),
                        "style": entry.style.name(),
                        "lines": [at.line(entry.lines.start), at.line(entry.lines.end.saturating_sub(1).max(entry.lines.start))],
                    });
                    if let Style::Complex(why) = entry.style {
                        put(&mut property, "complex", json!(why));
                    }
                    property
                })
                .collect();
            let mut answer = json!({ "properties": properties });
            if let Some(invalid) = &fm.invalid {
                put(&mut answer, "invalid", json!({ "line": invalid.line, "message": invalid.message }));
            }
            if !fm.duplicates.is_empty() {
                put(&mut answer, "duplicates", json!(fm.duplicates));
            }
            Ok(answer)
        }
        "table.read" => match number(params, "table")? {
            None => Ok(json!({
                "tables": ix.tables.len(),
                "list": ix.tables.iter().enumerate().map(|(number, found)| json!({ "table": number, "line": at.line(found.range.start), "rows": found.rows.len(), "columns": found.columns() })).collect::<Vec<_>>(),
            })),
            Some(number) => {
                let found = table::table(&ix, number).map_err(err)?;
                let cells = |row: &kition_note_markdown::Row| {
                    row.cells.iter().map(|cell| table::cell_value(sub(text, cell.content.clone()))).collect::<Vec<_>>()
                };
                Ok(json!({
                    "table": number,
                    "header": found.rows.first().map(cells).unwrap_or_default(),
                    "align": found.align.iter().map(|align| align.name()).collect::<Vec<_>>(),
                    "rows": found.rows.iter().skip(1).map(cells).collect::<Vec<_>>(),
                    "lines": [at.line(found.range.start), at.line(found.range.end.saturating_sub(1).max(found.range.start))],
                }))
            }
        },
        "links.list" => {
            let kinds = texts(params, "kinds")?;
            let wanted = |kind: &str| kinds.as_ref().is_none_or(|kinds| kinds.iter().any(|wanted| wanted == kind));
            let files = HostFiles;
            let mut out = Vec::new();
            for link in &ix.links {
                let kind = match link.kind {
                    LinkKind::Reference | LinkKind::Autolink => "markdown",
                    other => other.name(),
                };
                if !wanted(kind) {
                    continue;
                }
                let line = at.line(link.range.start);
                let index_in_line = links::on_line(&ix, line).iter().position(|other| other.range == link.range).unwrap_or(0);
                let mut found = json!({ "kind": kind, "line": line, "index": index_in_line, "destination": link.destination, "state": link_state(&files, &ix, link) });
                if let Some(shown) = &link.text {
                    put(&mut found, "text", json!(shown));
                }
                match &link.anchor {
                    Some(Anchor::Heading(name)) => put(&mut found, "anchor", json!(format!("#{name}"))),
                    Some(Anchor::Block(id)) => put(&mut found, "anchor", json!(format!("#^{id}"))),
                    None => {}
                }
                out.push(found);
            }
            if wanted("tag") {
                out.extend(ix.tags.iter().map(|tag| json!({ "kind": "tag", "line": at.line(tag.range.start), "name": tag.name })));
            }
            if wanted("blockId") {
                out.extend(ix.block_ids.iter().map(|found| json!({ "kind": "blockId", "line": at.line(found.range.start), "id": found.id })));
            }
            if let Some(fm) = &ix.frontmatter
                && wanted("tag")
            {
                // Tags written as the `tags` property count as tags too, as in Obsidian.
                for entry in fm.entries.iter().filter(|entry| entry.key == "tags" || entry.key == "tag") {
                    if let Some(Typed::List(names)) = &entry.typed {
                        out.extend(names.iter().map(|name| json!({ "kind": "tag", "line": at.line(entry.lines.start), "name": name.trim_start_matches('#'), "property": true })));
                    }
                }
            }
            Ok(json!({ "links": out }))
        }
        "tasks.list" => {
            let within = match heading_ref(params, "heading", "line", false)? {
                Some(heading) => Some(kition_note_edit::find_heading(&ix, &heading).map_err(err)?.section.clone()),
                None => None,
            };
            let tasks: Vec<Value> = ix
                .tasks
                .iter()
                .filter(|task| within.as_ref().is_none_or(|range| range.contains(&task.marker.start)))
                .map(|task| {
                    let line_range = ix.lines.content(text, ix.lines.line_of(task.marker.start)).unwrap_or(task.marker.clone());
                    let item_text = sub(text, task.marker.end..line_range.end).trim();
                    let block_id = ix
                        .block_ids
                        .iter()
                        .find(|found| line_range.start <= found.range.start && found.range.end <= line_range.end)
                        .map(|found| found.id.clone());
                    let shown = block_id.as_ref().map_or(item_text, |id| item_text.trim_end_matches(&format!("^{id}")).trim_end());
                    let line = at.line(task.marker.start);
                    json!({ "line": line, "done": task.done, "text": shown, "blockId": block_id })
                })
                .collect();
            Ok(json!({ "tasks": tasks }))
        }
        "text.find" => {
            let limit = number(params, "limit")?.unwrap_or(100).clamp(1, search::MAX_MATCHES);
            // The front matter is the properties' (`property.list`); find reads the body.
            let body = ix.frontmatter.as_ref().map_or(0, |found| found.range.end);
            let found = search::find_all(
                text,
                body,
                need_text(params, "find")?,
                flag(params, "regex")?.unwrap_or(false),
                flag(params, "caseSensitive")?.unwrap_or(true),
                limit + 1,
            )
            .map_err(err)?;
            let more = found.len() > limit;
            let matches: Vec<Value> = found.iter().take(limit).map(|range| json!({ "from": at.at(range.start), "to": at.at(range.end), "line": at.line(range.start), "text": sub(text, range.clone()) })).collect();
            Ok(json!({ "count": matches.len(), "more": more, "matches": matches }))
        }
        "comment.threads" => {
            let threads = comment::threads(replica).map_err(|error| KitError::new(error.to_string()))?;
            let out: Vec<Value> = threads
                .iter()
                .map(|thread| {
                    let mut found = json!({
                        "id": thread.id.as_str(),
                        "resolved": thread.resolved,
                        "comments": thread.comments.iter().map(|comment| json!({ "id": comment.id.as_str(), "author": comment.author, "name": comment.name, "body": comment.body, "created": comment.created })).collect::<Vec<_>>(),
                    });
                    if let Some(anchor) = thread.anchor() {
                        if let Some(quote) = &anchor.quote {
                            put(&mut found, "quote", json!(quote));
                        }
                        match relocate(text, &ix, &anchor) {
                            Some(range) => {
                                put(&mut found, "from", json!(at.at(range.start)));
                                put(&mut found, "to", json!(at.at(range.end)));
                                put(&mut found, "line", json!(at.line(range.start)));
                            }
                            None if anchor.range.is_some() => put(&mut found, "gone", json!(true)),
                            None => {}
                        }
                    }
                    found
                })
                .collect();
            Ok(json!({ "threads": out }))
        }
        "projection.read" => {
            let kinds = texts(params, "kinds")?;
            let objects: Vec<Value> = ids
                .items
                .iter()
                .filter(|(id, _)| kinds.as_ref().is_none_or(|kinds| kinds.iter().any(|kind| kind == id.kind.noun())))
                .map(|(id, item)| {
                    let mut object = json!({ "id": id.to_string(), "kind": id.kind.noun(), "label": item.label, "line": item.line });
                    if id.kind == Kind::Section {
                        put(&mut object, "path", json!(item.key.split('\u{1f}').collect::<Vec<_>>()));
                    }
                    if let Some(done) = item.done {
                        put(&mut object, "done", json!(done));
                    }
                    object
                })
                .collect();
            Ok(json!({ "objects": objects }))
        }
        other => Err(KitError::new(format!("the note has no read command {other}"))),
    }
}

/// The text the open comment threads are anchored to now (bytes), for the view to tint.
pub fn open_anchors(text: &str, replica: &Replica) -> Vec<std::ops::Range<usize>> {
    let Ok(threads) = comment::threads(replica) else { return Vec::new() };
    let anchors: Vec<CommentAnchor> = threads.iter().filter(|thread| !thread.resolved).filter_map(|thread| thread.anchor()).collect();
    if anchors.is_empty() {
        return Vec::new();
    }
    let ix = index(text);
    let mut ranges: Vec<std::ops::Range<usize>> =
        anchors.iter().filter_map(|anchor| relocate(text, &ix, anchor)).filter(|range| !range.is_empty()).collect();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges
}

/// Where a thread's quote is now: its stored range when it still holds the quote, else the
/// occurrence nearest to it; `None` when the quote is gone.
fn relocate(text: &str, ix: &Index, anchor: &CommentAnchor) -> Option<std::ops::Range<usize>> {
    let range = anchor.range?;
    let start = ix.lines.byte_of_scalar(text, usize::try_from(range.start).ok()?).unwrap_or(text.len());
    let end = ix.lines.byte_of_scalar(text, usize::try_from(range.end).ok()?).unwrap_or(text.len());
    let Some(quote) = anchor.quote.as_deref().filter(|quote| !quote.is_empty()) else { return Some(start..end.max(start)) };
    if text.get(start..end) == Some(quote) {
        return Some(start..end);
    }
    text.match_indices(quote).map(|(at, _)| at).min_by_key(|at| at.abs_diff(start)).map(|at| at..at + quote.len())
}

/// A thread's `anchor` (as stored) of `before` for the text `now`, a change made in the file:
/// where the text it quotes is now, nearest to where its range went (as [`relocate`] finds it
/// after the kit's own edits); when the text is gone, its range follows the change and the
/// thread is kept, its quote not found (`comment.threads` says `gone`).
pub fn reanchor(before: &str, now: &str, anchor: &Value) -> Value {
    let Ok(mut parsed) = serde_json::from_value::<CommentAnchor>(anchor.clone()) else { return anchor.clone() };
    let (Some(range), Some(quote)) = (parsed.range, parsed.quote.clone().filter(|quote| !quote.is_empty())) else { return anchor.clone() };
    let edits = kition_note_kdm::file::merge::diff_strings(before, now);
    let goes = |at: u64| -> usize {
        let at = usize::try_from(at).unwrap_or(usize::MAX);
        crate::projection::shifted(&edits, at, false).unwrap_or_else(|| {
            // Inside an edit: where that edit starts.
            let from = edits.iter().find(|edit| edit.from <= at && at < edit.to).map_or(at, |edit| edit.from);
            crate::projection::shifted(&edits, from, false).unwrap_or(from)
        })
    };
    let (start, end) = (goes(range.start), goes(range.end));
    let ix = index(now);
    let near = ix.lines.byte_of_scalar(now, start).unwrap_or(now.len());
    match now.match_indices(quote.as_str()).map(|(at, _)| at).min_by_key(|at| at.abs_diff(near)) {
        Some(at) => {
            parsed.range = Some(AnchorRange { start: ix.lines.scalar(now, at) as u64, end: ix.lines.scalar(now, at + quote.len()) as u64 });
        }
        None => parsed.range = Some(AnchorRange { start: start as u64, end: end.max(start) as u64 }),
    }
    serde_json::to_value(&parsed).unwrap_or_else(|_| anchor.clone())
}

/// The threads of `replica` whose quoted text is not in `text` (orphaned), with that text.
pub fn orphaned(text: &str, replica: &Replica) -> Vec<(String, String)> {
    let Ok(threads) = comment::threads(replica) else { return Vec::new() };
    orphaned_of(text, threads.iter().map(|thread| (thread.id.as_str().to_owned(), thread.anchor.clone())))
}

/// The threads (id, stored anchor) whose quoted text is not in `text`, with that text.
pub fn orphaned_of(text: &str, threads: impl Iterator<Item = (String, Value)>) -> Vec<(String, String)> {
    let ix = index(text);
    threads
        .filter_map(|(id, anchor)| {
            let anchor: CommentAnchor = serde_json::from_value(anchor).ok()?;
            let quote = anchor.quote.clone().filter(|quote| !quote.is_empty() && anchor.range.is_some())?;
            relocate(text, &ix, &anchor).is_none().then_some((id, quote))
        })
        .collect()
}

/// The byte range a comment command names: `from`/`to`, `quote`, or a heading's line.
fn comment_range(text: &str, ix: &Index, params: &Value, unit: Unit) -> Result<Option<std::ops::Range<usize>>> {
    let at = Positions { text, ix, unit };
    if let (Some(from), Some(to)) = (number(params, "from")?, number(params, "to")?) {
        let (from, to) = (at.byte(from)?, at.byte(to)?);
        if from > to {
            return Err(invalid("from is after to"));
        }
        return Ok(Some(from..to));
    }
    if number(params, "from")?.is_some() || number(params, "to")?.is_some() {
        return Err(invalid("give both from and to"));
    }
    if let Some(quote) = text_of(params, "quote")? {
        if quote.is_empty() {
            return Err(invalid("quote is empty"));
        }
        let occurrence = number(params, "occurrence")?.unwrap_or(1).max(1);
        let found = text.match_indices(quote).nth(occurrence - 1).map(|(start, _)| start..start + quote.len());
        return found.map(Some).ok_or_else(|| KitError::new(format!("not-found: {quote:?} does not occur {occurrence} time(s) in the note")));
    }
    if let Some(heading) = heading_ref(params, "heading", "line", false)? {
        let found = kition_note_edit::find_heading(ix, &heading).map_err(err)?;
        let line = ix.lines.content(text, ix.lines.line_of(found.range.start)).unwrap_or(found.range.clone());
        return Ok(Some(line));
    }
    Ok(None)
}

fn anchor_at(text: &str, ix: &Index, range: Option<std::ops::Range<usize>>) -> CommentAnchor {
    let mut anchor = CommentAnchor::object(&root_id());
    anchor.field = Some(BODY.to_owned());
    if let Some(range) = range {
        anchor.range = Some(AnchorRange { start: ix.lines.scalar(text, range.start) as u64, end: ix.lines.scalar(text, range.end) as u64 });
        anchor.quote = Some(sub(text, range).to_owned());
    }
    anchor
}

fn new_comment(params: &Value, origin: &Origin) -> Result<NewComment> {
    let body = need_text(params, "body")?;
    if body.trim().is_empty() {
        return Err(invalid("body is empty"));
    }
    let created = match text_of(params, "created")? {
        Some(created) => created.to_owned(),
        None => clock::now().iso(),
    };
    // A comment is written as its caller. The page runs as the local person, whose principal the
    // host knows and passes in `author`; any other caller has a principal of its own and cannot
    // write as someone else.
    let author = match text_of(params, "author")? {
        Some(author) if *origin == crate::local_person() || author == origin.principal => author.to_owned(),
        Some(author) => {
            return Err(invalid(format!("author {author:?} is not the caller ({}); a comment is written as its caller", origin.principal)));
        }
        None => origin.principal.clone(),
    };
    Ok(NewComment { author, name: text_of(params, "name")?.map(str::to_owned), body: body.to_owned(), created })
}

fn thread_id(tx: &Tx<'_>, params: &Value) -> Result<ObjectId> {
    let id = ObjectId::from_string(need_text(params, "thread")?);
    if !tx.exists(&id) || tx.object_type(&id).ok().as_deref() != Some(comment::COMMENT_THREAD_TYPE) {
        return Err(KitError::new(format!("not-found: no comment thread {}", id.as_str())));
    }
    Ok(id)
}

fn kdm(error: kition_kit::kdm::KdmError) -> KitError {
    KitError::new(error.to_string())
}

/// Applies byte `edits` of `text` as one change of the body; inserted line endings take the
/// note's.
fn apply(tx: &mut Tx<'_>, text: &str, ix: &Index, eol: Eol, edits: Vec<Edit>) -> Result<bool> {
    let edits = kition_note_edit::normalize(edits).map_err(err)?;
    let scalar: Vec<TextEdit> = edits
        .iter()
        .map(|edit| {
            TextEdit::new(
                ix.lines.scalar(text, edit.range.start),
                ix.lines.scalar(text, edit.range.end),
                normalize_inserted(&edit.insert, eol).into_owned(),
            )
        })
        .collect();
    apply_text_edits(tx, &scalar)?;
    carry_anchors(tx, &scalar)?;
    Ok(!scalar.is_empty())
}

/// Moves the comment threads' anchors with a write's scalar `edits` (sorted, of the text before
/// them), so a thread stays on the text it quotes: without this an anchor kept its old place and
/// [`relocate`] took the occurrence of its quote nearest to it, which after a long insertion above
/// can be another one. An anchor whose text an edit replaces moves to where that edit starts, and
/// [`relocate`] looks for its quote from there.
fn carry_anchors(tx: &mut Tx<'_>, edits: &[TextEdit]) -> Result<()> {
    if edits.is_empty() {
        return Ok(());
    }
    let goes = |at: u64, before_inserts: bool| -> u64 {
        let at = usize::try_from(at).unwrap_or(usize::MAX);
        let moved = crate::projection::shifted(edits, at, before_inserts).unwrap_or_else(|| {
            let from = edits.iter().find(|edit| edit.from <= at && at < edit.to).map_or(at, |edit| edit.from);
            crate::projection::shifted(edits, from, true).unwrap_or(from)
        });
        u64::try_from(moved).unwrap_or(u64::MAX)
    };
    for thread in tx.objects_of_type(comment::COMMENT_THREAD_TYPE) {
        let Ok(stored) = tx.get(&thread, "anchor") else { continue };
        let Ok(mut anchor) = serde_json::from_value::<CommentAnchor>(stored) else { continue };
        let Some(range) = anchor.range else { continue };
        // Text typed right before the quote is outside it, and so is text typed right after it.
        let start = goes(range.start, true);
        let moved = AnchorRange { start, end: goes(range.end, false).max(start) };
        if moved == range {
            continue;
        }
        anchor.range = Some(moved);
        tx.set(&thread, "anchor", serde_json::to_value(&anchor).map_err(|error| KitError::new(error.to_string()))?)?;
    }
    Ok(())
}

/// What a write that edits the body does, worked out from the text alone (no transaction): the
/// byte `edits` it makes, the byte ranges it reads to find its place (`reads`: a heading's line, a
/// table, a property's lines; the edits' own ranges are read too), the range a comment command
/// anchors (`anchor`), and the members of its answer. [`write`] applies it; the premise guard
/// (`crate::premise`) works it out on the text an agent observed and on the text now, so that
/// what a write rests on and what it does are one computation.
#[derive(Debug, Default)]
pub struct Plan {
    pub edits: Vec<Edit>,
    pub reads: Vec<std::ops::Range<usize>>,
    /// What each name the write gives stands for: a heading path's whole section, a block id's
    /// block, a property key's lines (the guard compares them when the name is found elsewhere).
    pub names: Vec<std::ops::Range<usize>>,
    pub anchor: Option<std::ops::Range<usize>>,
    pub result: Map<String, Value>,
}

/// The line `line` (from 1) of the note, without its ending.
fn line_range(text: &str, ix: &Index, line: usize) -> Option<std::ops::Range<usize>> {
    ix.lines.content(text, line.checked_sub(1)?)
}

/// The heading or line `at` names: its line's range.
fn heading_line(text: &str, ix: &Index, at: &HeadingRef) -> Result<std::ops::Range<usize>> {
    let found = kition_note_edit::find_heading(ix, at).map_err(err)?;
    Ok(ix.lines.content(text, ix.lines.line_of(found.range.start)).unwrap_or(found.range.clone()))
}

/// The section of the heading a path names (nothing for a heading named by line).
fn named_section(ix: &Index, at: &HeadingRef) -> Option<std::ops::Range<usize>> {
    match at {
        HeadingRef::Path(_) => kition_note_edit::find_heading(ix, at).ok().map(|heading| heading.section.clone()),
        HeadingRef::Line(_) => None,
    }
}

/// The table `number` names: its range.
/// The lines of the rows of the table `params` names that its parameters `rows` give (row 0 is the
/// header).
fn table_rows(ix: &Index, params: &Value, rows: &[&str]) -> Result<Vec<std::ops::Range<usize>>> {
    let Some(table) = ix.tables.get(need_number(params, "table")?) else { return Ok(Vec::new()) };
    let mut lines = Vec::new();
    for name in rows {
        if let Some(row) = table.rows.get(need_number(params, name)?) {
            lines.push(row.line.clone());
        }
    }
    Ok(lines)
}

fn table_range(ix: &Index, params: &Value) -> Result<Vec<std::ops::Range<usize>>> {
    Ok(ix.tables.get(need_number(params, "table")?).map(|table| table.range.clone()).into_iter().collect())
}

/// The plan of `text.insert`, `text.delete` and `text.replace`: the scalar range and the text.
fn text_edit<'a>(text: &str, id: &str, params: &'a Value, unit: Unit) -> Result<(usize, usize, &'a str)> {
    let range = |params: &Value| -> Result<(usize, usize)> {
        let (from, to) = (unit.to_scalar(text, need_number(params, "from")?)?, unit.to_scalar(text, need_number(params, "to")?)?);
        if from > to {
            return Err(invalid(format!("from ({from}) is after to ({to})")));
        }
        Ok((from, to))
    };
    let (from, to, insert) = match id {
        "text.insert" => {
            let at = unit.to_scalar(text, need_number(params, "at")?)?;
            (at, at, need_text(params, "text")?)
        }
        "text.delete" => {
            let (from, to) = range(params)?;
            (from, to, "")
        }
        _ => {
            let (from, to) = range(params)?;
            (from, to, need_text(params, "text")?)
        }
    };
    if let Some(expected) = text_of(params, "expected")? {
        let holds = text.get(byte_range(text, from, to)).unwrap_or_default();
        if holds != expected {
            return Err(KitError::new(format!("stale: the range holds {holds:?}, not {expected:?}")));
        }
    }
    Ok((from, to, insert))
}

/// The plan of write `id` on `text` (see [`Plan`]); `Ok(None)` for a command that does not edit or
/// anchor at the body (`note.write`, `file.setLineEndings` and the comment commands by thread).
pub fn plan(text: &str, ix: &Index, id: &str, params: &Value) -> Result<Option<Plan>> {
    let unit = Unit::of(params)?;
    let at = Positions { text, ix, unit };
    let mut plan = Plan::default();
    plan.edits = match id {
        "text.insert" | "text.delete" | "text.replace" => {
            let (from, to, insert) = text_edit(text, id, params, unit)?;
            vec![Edit::new(byte_range(text, from, to), insert)]
        }
        "comment.add" | "comment.move" => {
            let range = comment_range(text, ix, params, unit)?;
            if id == "comment.move" && range.is_none() {
                return Err(invalid("quote, or from and to, is required"));
            }
            plan.anchor = range;
            Vec::new()
        }
        "text.findReplace" => {
            let occurrence = match params.get("occurrence") {
                None | Some(Value::Null) => search::Occurrence::Nth(1),
                Some(Value::String(all)) if all == "all" => search::Occurrence::All,
                Some(value) => search::Occurrence::Nth(
                    value
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .filter(|n| *n >= 1)
                        .ok_or_else(|| invalid("occurrence must be a number from 1 or \"all\""))?,
                ),
            };
            // The front matter is the properties' (`property.set`); find and replace edits the body.
            let body = ix.frontmatter.as_ref().map_or(0, |found| found.range.end);
            let (edits, count) = search::replace(
                text,
                body,
                need_text(params, "find")?,
                need_text(params, "replace")?,
                occurrence,
                flag(params, "regex")?.unwrap_or(false),
                flag(params, "caseSensitive")?.unwrap_or(true),
            )
            .map_err(err)?;
            plan.result.insert("replaced".into(), json!(count));
            edits
        }
        "section.replace" => {
            let heading = need_heading(params)?;
            plan.reads.push(heading_line(text, ix, &heading)?);
            plan.names.extend(named_section(ix, &heading));
            section::replace(text, ix, &heading, need_text(params, "markdown")?, flag(params, "withHeading")?.unwrap_or(false)).map_err(err)?
        }
        "section.insert" => {
            let heading = need_heading(params)?;
            plan.reads.push(heading_line(text, ix, &heading)?);
            plan.names.extend(named_section(ix, &heading));
            section::insert(text, ix, &heading, position_param(params, true)?, need_text(params, "markdown")?).map_err(err)?
        }
        "section.delete" => {
            let heading = need_heading(params)?;
            plan.reads.push(heading_line(text, ix, &heading)?);
            plan.names.extend(named_section(ix, &heading));
            section::delete(ix, &heading).map_err(err)?
        }
        "section.move" => {
            let heading = need_heading(params)?;
            let to = heading_ref(params, "toHeading", "toLine", true)?.ok_or_else(|| invalid("toHeading or toLine is required"))?;
            plan.reads.push(heading_line(text, ix, &heading)?);
            plan.reads.push(heading_line(text, ix, &to)?);
            plan.names.extend(named_section(ix, &heading));
            plan.names.extend(named_section(ix, &to));
            section::move_to(text, ix, &heading, &to, position_param(params, true)?).map_err(err)?
        }
        "heading.setLevel" => {
            let level = u8::try_from(need_number(params, "level")?).map_err(|_| invalid("level must be 1 to 6"))?;
            let atx = match text_of(params, "style")? {
                None => false,
                Some("atx") => true,
                Some(other) => return Err(invalid(format!("style must be \"atx\", not {other:?}"))),
            };
            let heading = need_heading(params)?;
            plan.reads.push(heading_line(text, ix, &heading)?);
            plan.names.extend(named_section(ix, &heading));
            section::set_level(text, ix, &heading, level, atx).map_err(err)?
        }
        "block.replace" => {
            let id = need_text(params, "blockId")?;
            plan.reads.push(block::find(ix, id, None).map_err(err)?.block.clone());
            plan.names = plan.reads.clone();
            block::replace(text, ix, id, need_text(params, "markdown")?).map_err(err)?
        }
        "block.append" => {
            let heading = heading_ref(params, "heading", "line", false)?;
            if let Some(heading) = &heading {
                plan.reads.push(heading_line(text, ix, heading)?);
                plan.names.extend(named_section(ix, heading));
            }
            block::append(text, ix, heading.as_ref(), need_text(params, "markdown")?).map_err(err)?
        }
        "block.setId" => {
            let id = need_text(params, "id")?;
            match (number(params, "line")?, text_of(params, "blockId")?) {
                (Some(line), None) => {
                    plan.reads.extend(line_range(text, ix, line));
                    block::set_id_on_line(text, ix, line, id).map_err(err)?
                }
                (None, Some(old)) => {
                    let occurrence = number(params, "occurrence")?;
                    // It rewrites the block's id, and the id names the block: what the block says
                    // besides is not what it rests on.
                    let found = block::find(ix, old, occurrence).map_err(err)?;
                    plan.reads.push(found.range.clone());
                    plan.names.push(found.block.clone());
                    block::set_id(text, ix, old, occurrence, id).map_err(err)?
                }
                _ => return Err(invalid("give line or blockId")),
            }
        }
        "task.toggle" => {
            let task = match (number(params, "line")?, text_of(params, "blockId")?) {
                (Some(line), None) => block::task_on_line(ix, line).map_err(err)?,
                (None, Some(id)) => {
                    let found = block::find(ix, id, None).map_err(err)?;
                    plan.reads.push(found.block.clone());
                    plan.names.push(found.block.clone());
                    block::task_of(ix, found).map_err(err)?
                }
                _ => return Err(invalid("give line or blockId")),
            };
            plan.reads.extend(ix.lines.content(text, ix.lines.line_of(task.marker.start)));
            let done = flag(params, "done")?.unwrap_or(!task.done);
            plan.result.insert("done".into(), json!(done));
            block::toggle(text, task, Some(done))
        }
        "property.set" | "property.delete" | "property.rename" => {
            let key = need_text(params, "key")?;
            let occurrence = if id == "property.delete" { number(params, "occurrence")? } else { None };
            let entries: Vec<_> = ix.frontmatter.iter().flat_map(|fm| fm.entries.iter()).filter(|entry| entry.key == key).collect();
            match occurrence {
                Some(n) => plan.reads.extend(n.checked_sub(1).and_then(|index| entries.get(index)).map(|entry| entry.lines.clone())),
                None => plan.reads.extend(entries.iter().map(|entry| entry.lines.clone())),
            }
            plan.names = plan.reads.clone();
            match id {
                "property.set" => props::set(text, ix, key, &property_value(params)?).map_err(err)?,
                "property.delete" => props::delete(ix, key, occurrence).map_err(err)?,
                _ => props::rename(ix, key, need_text(params, "to")?).map_err(err)?,
            }
        }
        "table.create" => {
            let columns = need_number(params, "columns")?;
            let header = texts(params, "header")?.unwrap_or_default();
            let place = match (number(params, "line")?, texts(params, "heading")?) {
                (Some(_), Some(_)) => return Err(invalid("give line or heading, not both")),
                (Some(line), None) => {
                    let range = ix
                        .lines
                        .line(line.checked_sub(1).ok_or_else(|| invalid("line counts from 1"))?)
                        .ok_or_else(|| KitError::new(format!("out-of-range: the note has {} lines", ix.lines.count())))?;
                    plan.reads.push(range.clone());
                    table::block_place(ix, range.start, line).map_err(err)?;
                    range.start
                }
                (None, Some(path)) => {
                    let path = HeadingRef::Path(path);
                    plan.reads.push(heading_line(text, ix, &path)?);
                    plan.names.extend(named_section(ix, &path));
                    let heading = kition_note_edit::find_heading(ix, &path).map_err(err)?;
                    section::position_at(text, ix, heading, position_param(params, false)?)
                }
                (None, None) => text.len(),
            };
            plan.result.insert("table".into(), json!(ix.tables.iter().filter(|found| found.range.start < place).count()));
            table::create(place, text, columns, number(params, "rows")?.unwrap_or(1), &header).map_err(err)?
        }
        "table.setCell" => {
            plan.reads = table_range(ix, params)?;
            table::set_cell(
                text,
                ix,
                need_number(params, "table")?,
                need_number(params, "row")?,
                need_number(params, "column")?,
                need_text(params, "text")?,
                text_of(params, "expected")?,
            )
            .map_err(err)?
        }
        "table.insertRow" => {
            plan.reads = table_range(ix, params)?;
            table::insert_row(text, ix, need_number(params, "table")?, need_number(params, "at")?, &texts(params, "cells")?.unwrap_or_default())
                .map_err(err)?
        }
        "table.insertColumn" => {
            plan.reads = table_range(ix, params)?;
            let align = align_param(params, false)?;
            table::insert_column(text, ix, need_number(params, "table")?, need_number(params, "at")?, text_of(params, "header")?, align)
                .map_err(err)?
        }
        "table.deleteRow" => {
            // The row it deletes: an edit of another row or of the header leaves it as it was.
            plan.reads = table_rows(ix, params, &["row"])?;
            table::delete_row(ix, need_number(params, "table")?, need_number(params, "row")?).map_err(err)?
        }
        "table.deleteColumn" => {
            plan.reads = table_range(ix, params)?;
            table::delete_column(ix, need_number(params, "table")?, need_number(params, "column")?).map_err(err)?
        }
        "table.moveRow" => {
            plan.reads = table_rows(ix, params, &["from", "to"])?;
            table::move_row(text, ix, need_number(params, "table")?, need_number(params, "from")?, need_number(params, "to")?).map_err(err)?
        }
        "table.moveColumn" => {
            plan.reads = table_range(ix, params)?;
            table::move_column(text, ix, need_number(params, "table")?, need_number(params, "from")?, need_number(params, "to")?).map_err(err)?
        }
        "table.align" => {
            // The column's cell of the delimiter row, which holds its alignment.
            plan.reads = ix
                .tables
                .get(need_number(params, "table")?)
                .and_then(|table| table.delimiter.cells.get(need_number(params, "column").ok()?))
                .map(|cell| cell.segment.clone())
                .into_iter()
                .collect();
            table::align(text, ix, need_number(params, "table")?, need_number(params, "column")?, align_param(params, true)?).map_err(err)?
        }
        "table.format" => {
            plan.reads = table_range(ix, params)?;
            table::format(text, ix, need_number(params, "table")?).map_err(err)?
        }
        "link.insert" => {
            let position = at.byte(need_number(params, "at")?)?;
            let style = match text_of(params, "style")? {
                None | Some("wikilink") => links::LinkStyle::Wikilink,
                Some("markdown") => links::LinkStyle::Markdown,
                Some(other) => return Err(invalid(format!("style must be wikilink or markdown, not {other:?}"))),
            };
            let written = links::render(need_text(params, "destination")?, text_of(params, "text")?, style, flag(params, "embed")?.unwrap_or(false))
                .map_err(err)?;
            vec![Edit::insert(position, written)]
        }
        "link.update" => {
            let (line, index) = (need_number(params, "line")?, need_number(params, "index")?);
            plan.reads.push(links::find(ix, line, index).map_err(err)?.range.clone());
            links::update(text, ix, line, index, need_text(params, "destination")?, text_of(params, "text")?).map_err(err)?
        }
        "links.retarget" => {
            let (edits, count) = links::retarget(text, ix, need_text(params, "from")?, need_text(params, "to")?).map_err(err)?;
            plan.result.insert("links".into(), json!(count));
            edits
        }
        _ => return Ok(None),
    };
    Ok(Some(plan))
}

/// Writes one command into `tx`; answers its result.
pub fn write(model: &NoteModel, tx: &mut Tx<'_>, id: &str, params: &Value, origin: &Origin) -> Result<Value> {
    let text = model.body();
    let eol = model.style().eol;
    match id {
        "text.insert" | "text.delete" | "text.replace" => return text_command(model, tx, id, params),
        "note.write" => {
            let markdown = normalize_inserted(need_text(params, "markdown")?, eol).into_owned();
            let changed = write_body(tx, text, &markdown)?;
            if changed {
                carry_anchors(tx, &kition_note_kdm::file::merge::diff_strings(text, &markdown))?;
            }
            return Ok(json!({ "changed": changed }));
        }
        "file.setLineEndings" => {
            let target = match need_text(params, "eol")? {
                "lf" => Eol::Lf,
                "crlf" => Eol::Crlf,
                other => return Err(invalid(format!("eol must be \"lf\" or \"crlf\", not {other:?}"))),
            };
            let style = LineStyle { eol: target, bom: flag(params, "bom")?.unwrap_or(model.style().bom) };
            let mut edits = Vec::new();
            let mut chars = text.char_indices().peekable();
            let mut scalar = 0usize;
            while let Some((_, c)) = chars.next() {
                if c == '\r' {
                    // `\r\n` becomes `\n` (the register writes the ending); a lone CR is a line break.
                    let lone = chars.peek().is_none_or(|(_, next)| *next != '\n');
                    edits.push(TextEdit::new(scalar, scalar + 1, if lone { "\n" } else { "" }));
                }
                scalar += 1;
            }
            apply_text_edits(tx, &edits)?;
            carry_anchors(tx, &edits)?;
            set_style(tx, model.style(), style)?;
            return Ok(json!({ "changed": !edits.is_empty() || model.style() != style, "eol": style.eol.as_str(), "bom": style.bom }));
        }
        "comment.reply" => {
            let thread = thread_id(tx, params)?;
            let comment = new_comment(params, origin)?;
            let id = comment::reply(tx, &thread, &comment).map_err(kdm)?;
            return Ok(json!({ "thread": thread.as_str(), "comment": id.as_str() }));
        }
        "comment.resolve" => {
            let thread = thread_id(tx, params)?;
            comment::set_resolved(tx, &thread, flag(params, "resolved")?.unwrap_or(true)).map_err(kdm)?;
            return Ok(json!({ "thread": thread.as_str() }));
        }
        "comment.delete" => {
            let thread = thread_id(tx, params)?;
            let comment = ObjectId::from_string(need_text(params, "comment")?);
            if !tx.exists(&comment) {
                return Err(KitError::new(format!("not-found: no comment {}", comment.as_str())));
            }
            comment::delete_comment(tx, &thread, &comment).map_err(kdm)?;
            return Ok(json!({ "thread": thread.as_str() }));
        }
        _ => {}
    }
    let ix = index(text);
    match id {
        "comment.add" => {
            let comment = new_comment(params, origin)?;
            let range = plan(text, &ix, id, params)?.and_then(|plan| plan.anchor);
            let anchor = anchor_at(text, &ix, range);
            let (thread, first) = comment::add_thread(tx, &anchor, &comment).map_err(kdm)?;
            return Ok(json!({ "thread": thread.as_str(), "comment": first.as_str() }));
        }
        "comment.move" => {
            let thread = thread_id(tx, params)?;
            let range = plan(text, &ix, id, params)?.and_then(|plan| plan.anchor);
            let anchor = anchor_at(text, &ix, range);
            tx.set(&thread, "anchor", serde_json::to_value(&anchor).map_err(|error| KitError::new(error.to_string()))?)?;
            return Ok(json!({ "thread": thread.as_str() }));
        }
        _ => {}
    }
    let Some(Plan { edits, mut result, .. }) = plan(text, &ix, id, params)? else {
        return Err(KitError::new(format!("the note has no command {id}")));
    };
    let changed = apply(tx, text, &ix, eol, edits)?;
    result.insert("changed".into(), json!(changed));
    Ok(Value::Object(result))
}

fn align_param(params: &Value, needed: bool) -> Result<Align> {
    match text_of(params, "align")? {
        Some(name) => Align::parse(name).ok_or_else(|| invalid(format!("align must be left, center, right or none, not {name:?}"))),
        None if needed => Err(invalid("align is required")),
        None => Ok(Align::None),
    }
}

/// `text.insert`, `text.delete`, `text.replace` at positions of the request's unit.
fn text_command(model: &NoteModel, tx: &mut Tx<'_>, id: &str, params: &Value) -> Result<Value> {
    let text = model.body();
    let unit = Unit::of(params)?;
    let (from, to, insert) = text_edit(text, id, params, unit)?;
    let insert = normalize_inserted(insert, model.style().eol);
    let edits = [TextEdit::new(from, to, insert.as_ref())];
    apply_text_edits(tx, &edits)?;
    carry_anchors(tx, &edits)?;
    let length = unit.len(text) - unit.from_scalar(text, to) + unit.from_scalar(text, from) + unit.len(&insert);
    Ok(json!({ "length": length }))
}
