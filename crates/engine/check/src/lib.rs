//! `kition-note-check`: the note's check (`note_check`, KHP A21–A23 and A38), the twelve codes of
//! the design note (section 4.2) over the Markdown index.
//!
//! The check reads and changes nothing. Whether a linked file exists is asked of a
//! [`FileLookup`] (the host's lent files); when it cannot answer, a link's state is unknown and
//! no diagnostic is raised, so the check never reports a false error for want of an answer.

use std::collections::BTreeMap;
use std::ops::Range;

use kition_kit::diagnostics::{Diagnostic, Severity, Target};
use kition_note_markdown::{Anchor, Index, Link, LinkKind, Style, index, sub};
use serde_json::json;

/// Every code the check reports, with its severity and a one-sentence summary
/// (`agent/checks.json` lists the same).
pub const CODES: &[(&str, Severity, &str)] = &[
    ("note.invalid-utf8", Severity::Error, "The file is not valid UTF-8, so the note is open read-only and saves its bytes unchanged."),
    ("note.unclosed-fence", Severity::Error, "A code fence or a $$ math block is never closed, so the rest of the note renders as code."),
    ("note.frontmatter-invalid", Severity::Error, "The property block at the top does not parse as YAML, so no property is read."),
    ("note.frontmatter-duplicate-key", Severity::Error, "A property key is written twice, and readers disagree on which value counts."),
    ("note.broken-link", Severity::Error, "A wikilink or Markdown link names a file that does not exist, or has no target."),
    ("note.broken-embed", Severity::Error, "An embed names a file that does not exist."),
    ("note.broken-anchor", Severity::Error, "A link's #heading or #^block part names no heading or block id in its target."),
    ("note.duplicate-block-id", Severity::Error, "Two blocks end with the same ^id, so links to it are ambiguous."),
    ("note.table-column-mismatch", Severity::Warning, "A table row has more or fewer cells than its header."),
    ("note.heading-level-skip", Severity::Warning, "A heading is more than one level below the heading before it."),
    (
        "note.property-complex",
        Severity::Info,
        "A property's value is a nested map, a block scalar, or carries comments; the engine reads it but does not rewrite it.",
    ),
    ("note.mixed-line-endings", Severity::Info, "The file mixes line endings or has a lone CR; it is kept as it is."),
    ("note.truncated", Severity::Info, "More than 100 diagnostics of one code; the rest are counted, not listed."),
];

/// The most diagnostics listed per code.
pub const PER_CODE: usize = 100;

/// The note's root object id (`kition-note-kdm`), the target of every diagnostic.
const NOTE_OBJECT: &str = "note";

/// Asks the host about other files. `None`: the host cannot say.
pub trait FileLookup {
    /// Whether `path` (as the note names it: relative to its folder, or `/` from the root) exists.
    fn exists(&self, path: &str) -> Option<bool>;
    /// The text of a Markdown file, to check anchors in it.
    fn read_text(&self, path: &str) -> Option<String>;
}

/// A lookup that can answer nothing (no host files).
pub struct NoFiles;

impl FileLookup for NoFiles {
    fn exists(&self, _path: &str) -> Option<bool> {
        None
    }
    fn read_text(&self, _path: &str) -> Option<String> {
        None
    }
}

/// What the check needs to know besides the text.
pub struct Input<'a> {
    pub text: &'a str,
    pub index: &'a Index,
    /// The file is not valid UTF-8: the byte where decoding failed.
    pub invalid_utf8: Option<usize>,
    /// The note's `eol` register is `mixed`.
    pub mixed_line_endings: bool,
    pub files: &'a dyn FileLookup,
}

struct Found {
    by_code: BTreeMap<&'static str, Vec<Diagnostic>>,
}

impl Found {
    fn push(&mut self, diagnostic: Diagnostic) {
        let code = CODES.iter().find(|(code, _, _)| *code == diagnostic.code).map_or("note.truncated", |(code, _, _)| *code);
        self.by_code.entry(code).or_default().push(diagnostic);
    }
}

fn severity(code: &str) -> Severity {
    CODES.iter().find(|(known, _, _)| *known == code).map_or(Severity::Info, |(_, severity, _)| *severity)
}

/// Where a diagnostic is: the note, `path` = `body:<line>:<column>` (both from 1) until targets
/// carry text positions.
fn at(input: &Input<'_>, byte: usize) -> (Target, String) {
    let line = input.index.lines.line_of(byte);
    let start = input.index.lines.line(line).map_or(0, |range| range.start);
    let column = sub(input.text, start..byte).chars().count() + 1;
    let heading = input.index.headings.iter().rev().find(|heading| heading.range.start <= byte).map(|heading| heading.path.join(" / "));
    let place = match heading {
        Some(path) => format!("line {} (under {path})", line + 1),
        None => format!("line {}", line + 1),
    };
    (Target { object_id: Some(NOTE_OBJECT.into()), path: Some(format!("body:{}:{column}", line + 1)), ..Target::default() }, place)
}

fn diagnostic(input: &Input<'_>, code: &'static str, byte: usize, message: impl FnOnce(&str) -> String) -> Diagnostic {
    let (target, place) = at(input, byte);
    Diagnostic::new(code, severity(code), message(&place)).on(target)
}

/// Runs the check: errors first, at most [`PER_CODE`] per code, the rest counted in
/// `note.truncated`.
pub fn check(input: &Input<'_>) -> Vec<Diagnostic> {
    let mut found = Found { by_code: BTreeMap::new() };
    if let Some(byte) = input.invalid_utf8 {
        found.push(
            Diagnostic::new(
                "note.invalid-utf8",
                Severity::Error,
                format!("The file is not valid UTF-8 (byte {byte}), so the note is read-only and saves its bytes unchanged."),
            )
            .on(Target { object_id: Some(NOTE_OBJECT.into()), ..Target::default() }),
        );
    }
    fences(input, &mut found);
    properties(input, &mut found);
    links(input, &mut found);
    block_ids(input, &mut found);
    tables(input, &mut found);
    headings(input, &mut found);
    if input.mixed_line_endings {
        found.push(
            Diagnostic::new(
                "note.mixed-line-endings",
                Severity::Info,
                "The file mixes line endings or has a lone CR; it is kept as it is, and new lines are written with LF.",
            )
            .on(Target { object_id: Some(NOTE_OBJECT.into()), ..Target::default() })
            .fix("file.setLineEndings", Some(json!({ "eol": "lf" }))),
        );
    }
    let mut out = Vec::new();
    let mut truncated = 0usize;
    for (_, mut list) in found.by_code {
        if list.len() > PER_CODE {
            truncated += list.len() - PER_CODE;
            list.truncate(PER_CODE);
        }
        out.extend(list);
    }
    if truncated > 0 {
        out.push(
            Diagnostic::new(
                "note.truncated",
                Severity::Info,
                format!("{truncated} more diagnostics were found and not listed (at most {PER_CODE} per code)."),
            )
            .on(Target { object_id: Some(NOTE_OBJECT.into()), ..Target::default() }),
        );
    }
    out.sort_by_key(|diagnostic| diagnostic.severity);
    out
}

fn fences(input: &Input<'_>, found: &mut Found) {
    let end = input.text.chars().count();
    let newline = if input.text.is_empty() || input.text.ends_with('\n') { "" } else { "\n" };
    for code in input.index.code.iter().filter(|code| code.unclosed) {
        found.push(
            diagnostic(input, "note.unclosed-fence", code.range.start, |place| {
                format!("The code fence at {place} is never closed, so the rest of the note renders as code.")
            })
            .fix("text.insert", Some(json!({ "at": end, "text": format!("{newline}{}\n", code.fence) }))),
        );
    }
    if let Some(open) = &input.index.unclosed_math {
        found.push(
            diagnostic(input, "note.unclosed-fence", open.start, |place| {
                format!("The $$ math block at {place} is never closed, so the rest of the note renders as math.")
            })
            .fix("text.insert", Some(json!({ "at": end, "text": format!("{newline}$$\n") }))),
        );
    }
}

fn properties(input: &Input<'_>, found: &mut Found) {
    let Some(fm) = &input.index.frontmatter else { return };
    if let Some(invalid) = &fm.invalid {
        let byte = input.index.lines.line(invalid.line.saturating_sub(1)).map_or(0, |range| range.start);
        found.push(diagnostic(input, "note.frontmatter-invalid", byte, |_| {
            format!("The property block does not parse as YAML at line {} ({}), so no property is read.", invalid.line, invalid.message)
        }));
    }
    for key in &fm.duplicates {
        for (occurrence, entry) in fm.entries.iter().filter(|entry| entry.key == *key).enumerate().skip(1) {
            found.push(
                diagnostic(input, "note.frontmatter-duplicate-key", entry.lines.start, |place| {
                    format!("The property {key:?} is written again at {place}; readers disagree on which value counts.")
                })
                .fix("property.delete", Some(json!({ "key": key, "occurrence": occurrence + 1 }))),
            );
        }
    }
    // An invalid block is reported as such; its keys are not judged one by one.
    for entry in fm.entries.iter().filter(|_| fm.invalid.is_none()) {
        if let Style::Complex(why) = entry.style {
            if why == "not a key" {
                continue;
            }
            found.push(diagnostic(input, "note.property-complex", entry.lines.start, |place| {
                format!("The property {:?} at {place} is a {why}; the engine reads it but does not rewrite it, so edit it in the text.", entry.key)
            }));
        }
    }
}

/// A heading text as anchors compare it: lowercase letters and digits only.
fn loose(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Whether `anchor` names a heading or a block of the indexed text; for a broken heading anchor,
/// the one heading it loosely matches.
fn anchor_state(ix: &Index, anchor: &Anchor) -> Result<(), Option<String>> {
    match anchor {
        Anchor::Block(id) => {
            if ix.block_ids.iter().any(|block| block.id == *id) {
                Ok(())
            } else {
                Err(None)
            }
        }
        Anchor::Heading(name) => {
            // `#A#B` names a heading B under A; the last part has to exist.
            let last = name.rsplit('#').next().unwrap_or(name).trim();
            if ix.headings.iter().any(|heading| heading.text.eq_ignore_ascii_case(last) || heading.text.to_lowercase() == last.to_lowercase()) {
                return Ok(());
            }
            let matches: Vec<&str> =
                ix.headings.iter().filter(|heading| loose(&heading.text) == loose(last)).map(|heading| heading.text.as_str()).collect();
            Err(if let [one] = matches.as_slice() { Some((*one).to_owned()) } else { None })
        }
    }
}

/// The paths a link may name: the file as written, and with `.md` for a wikilink without an
/// extension; a wikilink also from the root.
fn candidates(link: &Link) -> Vec<String> {
    let file = link.file.trim();
    let named = if link.kind.is_wiki() && !file.rsplit('/').next().unwrap_or(file).contains('.') { format!("{file}.md") } else { file.to_owned() };
    let mut out = vec![named.clone()];
    if link.kind.is_wiki() && !named.starts_with('/') {
        out.push(format!("/{named}"));
    }
    out
}

/// Whether the link's file exists: `Some(path)` found, `None` unknown, `Err` missing.
fn resolve(files: &dyn FileLookup, link: &Link) -> Result<Option<String>, ()> {
    let mut unknown = false;
    for path in candidates(link) {
        match files.exists(&path) {
            Some(true) => return Ok(Some(path)),
            Some(false) => {}
            None => unknown = true,
        }
    }
    if unknown { Ok(None) } else { Err(()) }
}

fn link_line_index(input: &Input<'_>, link: &Link) -> (usize, usize) {
    let line = input.index.lines.line_of(link.range.start);
    let range = input.index.lines.line(line).unwrap_or(0..0);
    let index = input.index.links.iter().filter(|other| range.start <= other.range.start && other.range.start < link.range.start).count();
    (line + 1, index)
}

fn links(input: &Input<'_>, found: &mut Found) {
    for link in &input.index.links {
        if link.is_external() || matches!(link.kind, LinkKind::Reference) && link.destination.is_empty() {
            continue;
        }
        let embed = matches!(link.kind, LinkKind::Embed | LinkKind::Image);
        let code = if embed { "note.broken-embed" } else { "note.broken-link" };
        if link.file.trim().is_empty() && link.anchor.is_none() {
            if !embed {
                found.push(diagnostic(input, "note.broken-link", link.range.start, |place| format!("The link at {place} has no target.")));
            }
            continue;
        }
        let target_text: Option<String> = if link.file.trim().is_empty() {
            Some(input.text.to_owned())
        } else {
            match resolve(input.files, link) {
                Ok(Some(path)) if path.ends_with(".md") || path.ends_with(".markdown") => input.files.read_text(&path),
                Ok(_) => None,
                Err(()) => {
                    found.push(diagnostic(input, code, link.range.start, |place| {
                        format!("The {} at {place} names {:?}, which does not exist.", if embed { "embed" } else { "link" }, link.file)
                    }));
                    continue;
                }
            }
        };
        let (Some(anchor), Some(target)) = (&link.anchor, target_text) else { continue };
        let target_index = if link.file.trim().is_empty() { None } else { Some(index(&target)) };
        let ix = target_index.as_ref().unwrap_or(input.index);
        if let Err(hint) = anchor_state(ix, anchor) {
            let (line, index) = link_line_index(input, link);
            let named = match anchor {
                Anchor::Heading(name) => format!("#{name}"),
                Anchor::Block(id) => format!("#^{id}"),
            };
            let mut found_one = diagnostic(input, "note.broken-anchor", link.range.start, |place| {
                format!(
                    "The link at {place} points at {named}, which {} has no heading or block for.",
                    if link.file.trim().is_empty() { "this note" } else { "its target" }
                )
            });
            if let Some(heading) = hint {
                found_one =
                    found_one.fix("link.update", Some(json!({ "line": line, "index": index, "destination": format!("{}#{heading}", link.file) })));
            }
            found.push(found_one);
        }
    }
    // `[[]]` is not a link to pulldown-cmark: find it in the text outside code.
    let mut from = 0;
    while let Some(offset) = sub(input.text, from..input.text.len()).find("[[]]") {
        let at = from + offset;
        let in_code =
            input.index.code.iter().map(|code| &code.range).chain(input.index.inline_code.iter()).any(|range: &Range<usize>| range.contains(&at));
        if !in_code {
            found.push(diagnostic(input, "note.broken-link", at, |place| format!("The wikilink at {place} has no target.")));
        }
        from = at + 4;
    }
}

fn block_ids(input: &Input<'_>, found: &mut Found) {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for block in &input.index.block_ids {
        let count = seen.entry(block.id.as_str()).or_default();
        *count += 1;
        if *count > 1 {
            let occurrence = *count;
            found.push(
                diagnostic(input, "note.duplicate-block-id", block.range.start, |place| {
                    format!("The block id ^{} at {place} is used before, so links to it are ambiguous.", block.id)
                })
                .fix("block.setId", Some(json!({ "blockId": block.id, "occurrence": occurrence, "id": format!("{}-{occurrence}", block.id) }))),
            );
        }
    }
}

fn tables(input: &Input<'_>, found: &mut Found) {
    for (number, table) in input.index.tables.iter().enumerate() {
        let columns = table.columns();
        for row in table.rows.iter().skip(1).filter(|row| row.cells.len() != columns) {
            found.push(
                diagnostic(input, "note.table-column-mismatch", row.body.start, |place| {
                    format!(
                        "The table row at {place} has {} cells, but its header has {columns}; cells are dropped or added when it renders.",
                        row.cells.len()
                    )
                })
                .fix("table.format", Some(json!({ "table": number }))),
            );
        }
    }
}

fn headings(input: &Input<'_>, found: &mut Found) {
    let mut previous: Option<u8> = None;
    for heading in &input.index.headings {
        if let Some(before) = previous
            && heading.level > before + 1
        {
            let line = input.index.lines.line_of(heading.range.start) + 1;
            found.push(
                diagnostic(input, "note.heading-level-skip", heading.range.start, |place| {
                    format!(
                        "The heading {:?} at {place} is level {} right after a level {before} heading, which breaks the outline.",
                        heading.text, heading.level
                    )
                })
                .fix("heading.setLevel", Some(json!({ "line": line, "level": before + 1 }))),
            );
        }
        previous = Some(heading.level);
    }
}

#[cfg(test)]
mod tests;
