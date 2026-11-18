//! The note's typed projection: sections, tasks, tables, properties and
//! links, computed from the text and given ids an agent can name them by. The body text stays the
//! one stored truth and the commands the only way to write; nothing here is stored, and no id is
//! ever written into the Markdown.
//!
//! What an id promises. An id (`section:3`, `task:1`, `table:2`, `property:1`, `link:4`) names one
//! object of this projection for as long as the server holds the note open:
//!
//! - when the note is opened, ids are given in document order (`section:1` is the first heading);
//! - every change of the text the server sees (a command of any writer, an Undo, a peer's change)
//!   is followed: an object keeps its id while the place it starts at survives the change (a
//!   section renamed, a task edited, lines added above it), and when its text is found again
//!   whole and alone elsewhere (a section moved), or its name is found again alone (a section
//!   moved and edited). An object these do not find again is gone; a new one gets a new number,
//!   never one given before;
//! - an id is not saved: reopening the note gives ids in document order again.
//!
//! An id whose object cannot be told from look-alikes after a change (two sections of the same
//! path and text, one deleted) is *unproven* from that change on: a write that names it with a
//! `basis` before that change is refused `unknown` rather than guessed. After a change made in the
//! file, an object found again only by where it starts (neither its name nor its text after its
//! first line the same) is unproven too.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use kition_note_kdm::file::merge::{TextEdit, diff_strings};
use kition_note_markdown::{index, sub};
use serde_json::Value;

/// A type of the projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Section,
    Task,
    Table,
    Property,
    Link,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Section, Kind::Task, Kind::Table, Kind::Property, Kind::Link];

    /// The word an id starts with.
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Section => "section",
            Kind::Task => "task",
            Kind::Table => "table",
            Kind::Property => "property",
            Kind::Link => "link",
        }
    }

    fn slot(self) -> usize {
        match self {
            Kind::Section => 0,
            Kind::Task => 1,
            Kind::Table => 2,
            Kind::Property => 3,
            Kind::Link => 4,
        }
    }

    fn parse(noun: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.noun() == noun)
    }
}

/// An object's id: its type and a number from 1, spelled `section:3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    pub kind: Kind,
    pub number: u32,
}

impl Id {
    /// `section:3` and the like; `None` for anything else.
    pub fn parse(text: &str) -> Option<Id> {
        let (noun, number) = text.split_once(':')?;
        let number: u32 = number.parse().ok().filter(|number| *number > 0)?;
        Some(Id { kind: Kind::parse(noun)?, number })
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.noun(), self.number)
    }
}

/// One projected object as the text holds it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: Kind,
    /// Its own text, in bytes: a section from its heading to its first subsection (or its end), a
    /// task's line, a table, a property's lines, a link.
    pub span: Range<usize>,
    /// Where it starts, in Unicode scalar values (what change sets and edits count in).
    pub at: usize,
    /// Its first line, from 1.
    pub line: usize,
    /// What names it: a heading path, a task's text, a table's header row, a property key, a
    /// link's destination.
    pub key: String,
    /// How a person reads it: `Launch plan › Tasks`, the task's text, the header's cells.
    pub label: String,
    /// Its whole text, hashed.
    pub hash: u64,
    /// Its text after its first line (a section's body without the heading), hashed.
    pub body: u64,
    /// A task's box.
    pub done: Option<bool>,
}

fn hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn item(kind: Kind, text: &str, ix: &kition_note_markdown::Index, span: Range<usize>, key: String, label: String) -> Item {
    // (Blank lines around it belong to where it stands, not to what it is.)
    let own = sub(text, span.clone()).trim_end();
    let body = own.split_once('\n').map_or("", |(_, rest)| rest).trim();
    Item {
        kind,
        at: ix.lines.scalar(text, span.start),
        line: ix.lines.line_of(span.start) + 1,
        span,
        key,
        label,
        hash: hash(own),
        body: hash(body),
        done: None,
    }
}

/// The objects `text` holds, by type, each in document order.
pub fn project(text: &str) -> Vec<Item> {
    let ix = index(text);
    let mut items = Vec::new();
    for (number, heading) in ix.headings.iter().enumerate() {
        // Its own text ends where the next heading inside it starts.
        let end =
            ix.headings.get(number + 1).map(|next| next.range.start).filter(|start| *start < heading.section.end).unwrap_or(heading.section.end);
        let start = ix.lines.line(ix.lines.line_of(heading.range.start)).map_or(heading.range.start, |line| line.start);
        items.push(item(Kind::Section, text, &ix, start..end.max(start), heading.path.join("\u{1f}"), heading.path.join(" › ")));
    }
    for task in &ix.tasks {
        let line = ix.lines.content(text, ix.lines.line_of(task.marker.start)).unwrap_or(task.marker.clone());
        let words = sub(text, task.marker.end.min(line.end)..line.end).trim().to_owned();
        let mut found = item(Kind::Task, text, &ix, line, words.clone(), words);
        found.done = Some(task.done);
        items.push(found);
    }
    for table in &ix.tables {
        let header =
            table.rows.first().map(|row| row.cells.iter().map(|cell| sub(text, cell.content.clone()).trim()).collect::<Vec<_>>().join(" | "));
        let header = header.unwrap_or_default();
        items.push(item(Kind::Table, text, &ix, table.range.clone(), header.clone(), header));
    }
    if let Some(fm) = &ix.frontmatter {
        for entry in &fm.entries {
            items.push(item(Kind::Property, text, &ix, entry.lines.clone(), entry.key.clone(), entry.key.clone()));
        }
    }
    for link in &ix.links {
        items.push(item(Kind::Link, text, &ix, link.range.clone(), link.destination.clone(), link.destination.clone()));
    }
    items
}

/// The next number of `kind`, never one given before.
fn bump(next: &mut [u32; 5], kind: Kind) -> Id {
    let number = next.get_mut(kind.slot()).map_or(u32::MAX, |number| {
        *number = number.saturating_add(1);
        *number
    });
    Id { kind, number }
}

/// The projection of one state of the text, with its ids.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Counts the states this session followed.
    pub seq: u64,
    pub text: Arc<str>,
    pub items: Vec<(Id, Item)>,
}

impl Snapshot {
    /// The object `id` names in this state.
    pub fn find(&self, id: Id) -> Option<&Item> {
        self.items.iter().find(|(held, _)| *held == id).map(|(_, item)| item)
    }

    /// The id of the object of `kind` that starts at byte `start`.
    pub fn id_at(&self, kind: Kind, start: usize) -> Option<Id> {
        self.items.iter().find(|(id, item)| id.kind == kind && item.span.start == start).map(|(id, _)| *id)
    }

    /// The id of the object of `kind` on line `line` (from 1).
    pub fn id_on_line(&self, kind: Kind, line: usize) -> Option<Id> {
        self.items.iter().find(|(id, item)| id.kind == kind && item.line == line).map(|(id, _)| *id)
    }
}

/// States kept for writes whose `basis` is an earlier one.
const HISTORY: usize = 16;

/// The projection followed through every change of the text (see the module's promise).
#[derive(Clone, Debug)]
pub struct Tracked {
    now: Snapshot,
    next: [u32; 5],
    history: VecDeque<Snapshot>,
    /// Ids not proven since the state of that `seq`: (the first state they are doubtful in, id).
    unproven: Vec<(u64, Id)>,
}

impl Tracked {
    /// `text` as opened: ids in document order.
    pub fn open(text: &str) -> Self {
        let mut next = [0u32; 5];
        let items = project(text).into_iter().map(|item| (bump(&mut next, item.kind), item)).collect();
        let now = Snapshot { seq: 0, text: Arc::from(text), items };
        Self { history: VecDeque::from([now.clone()]), now, next, unproven: Vec::new() }
    }

    /// The projection now.
    pub fn now(&self) -> &Snapshot {
        &self.now
    }

    /// The text the projection was last followed to.
    pub fn text(&self) -> &str {
        &self.now.text
    }

    /// The kept state whose text is `text` (newest first).
    pub fn at_text(&self, text: &str) -> Option<&Snapshot> {
        self.history.iter().rev().find(|snapshot| snapshot.text.len() == text.len() && *snapshot.text == *text)
    }

    /// The projection as it was in the kept state `state` (its history up to then), to be
    /// followed again from there.
    pub fn rewound(&self, state: &Snapshot) -> Option<Tracked> {
        let at = self.history.iter().position(|kept| kept.seq == state.seq)?;
        let mut rewound = self.clone();
        rewound.history.truncate(at + 1);
        rewound.now = state.clone();
        Some(rewound)
    }

    /// Whether `id` became doubtful after the state `seq`.
    pub fn unproven_since(&self, seq: u64, id: Id) -> bool {
        self.unproven.iter().any(|(from, doubtful)| *doubtful == id && *from > seq)
    }

    /// Follows the text to `text`. `outside`: a change made in the file (read again), where an
    /// object found again only by where it starts is unproven.
    pub fn follow(&mut self, text: &str, outside: bool) {
        if *self.now.text == *text {
            return;
        }
        let edits = diff_strings(&self.now.text, text);
        let fresh = project(text);
        let (ids, doubtful) = matched(&self.now.items, &edits, &fresh, outside);
        let seq = self.now.seq + 1;
        let mut items = Vec::with_capacity(fresh.len());
        for (item, id) in fresh.into_iter().zip(ids) {
            let id = id.unwrap_or_else(|| bump(&mut self.next, item.kind));
            items.push((id, item));
        }
        self.unproven.extend(doubtful.into_iter().map(|id| (seq, id)));
        self.now = Snapshot { seq, text: Arc::from(text), items };
        self.history.push_back(self.now.clone());
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
    }
}

/// Where scalar position `at` of the text before `edits` is after them; `None` when an edit
/// deletes or replaces the character there. An insertion right at `at` counts as before it when
/// `before_inserts`.
pub(crate) fn shifted(edits: &[TextEdit], at: usize, before_inserts: bool) -> Option<usize> {
    let mut shift: isize = 0;
    for edit in edits {
        if edit.from <= at && at < edit.to {
            return None;
        }
        let insertion_here = edit.from == at && edit.to == at;
        if edit.to <= at && (before_inserts || !insertion_here) {
            shift += edit.insert.chars().count() as isize - (edit.to - edit.from) as isize;
        }
    }
    Some(at.saturating_add_signed(shift))
}

/// For each new item, the old id it continues (see the module's promise), and the old ids that
/// could not be told from look-alikes (or, `outside`, were found again by where they start alone).
fn matched(old: &[(Id, Item)], edits: &[TextEdit], new: &[Item], outside: bool) -> (Vec<Option<Id>>, Vec<Id>) {
    let mut ids: Vec<Option<Id>> = vec![None; new.len()];
    let mut doubtful = Vec::new();
    for kind in Kind::ALL {
        let olds: Vec<&(Id, Item)> = old.iter().filter(|(id, _)| id.kind == kind).collect();
        let news: Vec<usize> = new.iter().enumerate().filter(|(_, item)| item.kind == kind).map(|(at, _)| at).collect();
        let mut used: Vec<Id> = Vec::new();
        // Another object of the type had the same name or text: after a change made in the file,
        // which of them is which is a guess.
        let look_alike = |id: &Id, was: &Item| olds.iter().any(|(other, item)| other != id && (item.key == was.key || item.hash == was.hash));
        // 1. Where it starts survived the change.
        for (id, was) in &olds {
            for before_inserts in [true, false] {
                let Some(at) = shifted(edits, was.at, before_inserts) else { break };
                if let Some(&index) =
                    news.iter().find(|index| new.get(**index).is_some_and(|item| item.at == at) && ids.get(**index).is_some_and(Option::is_none))
                {
                    if let Some(slot) = ids.get_mut(index) {
                        *slot = Some(*id);
                    }
                    used.push(*id);
                    if outside && (new.get(index).is_some_and(|item| item.key != was.key && item.body != was.body) || look_alike(id, was)) {
                        doubtful.push(*id);
                    }
                    break;
                }
            }
        }
        // 2. Its text found again, alone; 3. its name found again, alone.
        let rules: [fn(&Item, &Item) -> bool; 2] = [|a, b| a.key == b.key && a.hash == b.hash, |a, b| a.key == b.key];
        for same in rules {
            let left: Vec<&&(Id, Item)> = olds.iter().filter(|(id, _)| !used.contains(id)).collect();
            for (id, was) in left.iter().map(|pair| (pair.0, &pair.1)) {
                let candidates: Vec<usize> = news
                    .iter()
                    .copied()
                    .filter(|index| ids.get(*index).is_some_and(Option::is_none) && new.get(*index).is_some_and(|item| same(was, item)))
                    .collect();
                let rivals = left
                    .iter()
                    .filter(|(other, item)| *other != id && candidates.iter().any(|index| new.get(*index).is_some_and(|new| same(item, new))))
                    .count();
                if let ([index], 0) = (candidates.as_slice(), rivals) {
                    if let Some(slot) = ids.get_mut(*index) {
                        *slot = Some(id);
                    }
                    used.push(id);
                }
            }
        }
        // Not found again: gone, or doubtful when look-alikes remain.
        for (id, was) in olds.iter().map(|pair| (pair.0, &pair.1)) {
            if used.contains(&id) {
                continue;
            }
            let alike = news.iter().any(|index| {
                ids.get(*index).is_some_and(Option::is_none) && new.get(*index).is_some_and(|item| item.key == was.key || item.hash == was.hash)
            });
            if alike || (outside && look_alike(&id, was)) {
                doubtful.push(id);
            }
        }
    }
    (ids, doubtful)
}

/// The id parameters, the type each names and the parameter it stands for.
pub const ID_PARAMS: &[(&str, Kind, &str, &[&str])] = &[
    ("section", Kind::Section, "line", &["heading", "line"]),
    ("toSection", Kind::Section, "toLine", &["toHeading", "toLine"]),
    ("task", Kind::Task, "line", &["line", "blockId"]),
];

/// The ids `params` gives (`None` when it gives none).
pub fn named(params: &Value) -> Vec<(&'static str, Option<Id>)> {
    ID_PARAMS
        .iter()
        .filter_map(|(name, kind, _, _)| {
            let value = params.get(*name).filter(|value| !value.is_null())?;
            Some((*name, value.as_str().and_then(Id::parse).filter(|id| id.kind == *kind)))
        })
        .collect()
}

/// `params` with each id replaced by the line of the object it names in `state`. Errors:
/// `invalid-params` for a malformed id or an id given with the address it stands for,
/// `not-found` for an id `state` does not hold.
pub fn resolve(params: &Value, state: &Snapshot) -> Result<Value, String> {
    let mut out = params.clone();
    for (name, kind, stands_for, instead) in ID_PARAMS {
        let Some(value) = params.get(*name).filter(|value| !value.is_null()) else { continue };
        let id = value.as_str().and_then(Id::parse).filter(|id| id.kind == *kind).ok_or_else(|| {
            format!(
                "invalid-params: {name} is a {noun} id such as \"{noun}:1\" (outline.read and tasks.list list them), not {value}",
                noun = kind.noun()
            )
        })?;
        if let Some(both) = instead.iter().find(|other| params.get(**other).is_some_and(|value| !value.is_null())) {
            return Err(format!("invalid-params: give {name} or {both}, not both"));
        }
        let found = state.find(id).ok_or_else(|| {
            format!("not-found: the note has no {id} now: it was deleted, or no id this session gave (outline.read and tasks.list list the ids)")
        })?;
        if let Some(object) = out.as_object_mut() {
            object.remove(*name);
            object.insert((*stands_for).to_owned(), Value::from(found.line));
        }
    }
    Ok(out)
}

/// The projection of `state` as objects of the object model (`Section`, `Task`, `Table`, `Link`,
/// `Property`, kition-sdk projected types): sections form the root's `sections` tree by heading
/// level, and each task, table and link sits in the innermost section holding it (the root's
/// lists before the first heading); properties are the root's. Computed, never stored.
pub fn objects(state: &Snapshot, root: &str) -> Vec<kition_kit::kdm::projection::ProjectedObject> {
    use kition_kit::kdm::projection::{ProjectedObject, ProjectedPlace};
    use serde_json::{Map, json};
    let text: &str = &state.text;
    let ix = index(text);
    // Each section's id, extent and level, in document order (the index's headings).
    let sections: Vec<(Id, Range<usize>, u8)> = state
        .items
        .iter()
        .filter(|(id, _)| id.kind == Kind::Section)
        .zip(ix.headings.iter())
        .map(|((id, _), heading)| (*id, heading.section.clone(), heading.level))
        .collect();
    // The innermost section holding byte `at`, other than `except`.
    let holder = |at: usize, except: Option<Id>| -> Option<Id> {
        sections
            .iter()
            .filter(|(id, extent, _)| Some(*id) != except && extent.contains(&at))
            .max_by_key(|(_, extent, _)| extent.start)
            .map(|(id, _, _)| *id)
    };
    let mut counts: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    let mut place = |parent: String, field: &'static str, node: Option<String>| -> ProjectedPlace {
        let key = (node.clone().unwrap_or_else(|| parent.clone()), field);
        let index = counts.entry(key).or_insert(0);
        let at = ProjectedPlace { parent, field: field.into(), index: *index, node };
        *index += 1;
        at
    };
    let mut out = Vec::with_capacity(state.items.len());
    let mut headings = ix.headings.iter();
    let mut tables = ix.tables.iter();
    for (id, item) in &state.items {
        // Its own text: a section's after its heading (to its first subsection), a table's whole,
        // a property's after its key.
        let own = sub(text, item.span.clone()).trim_end();
        let rest = own.split_once('\n').map_or("", |(_, rest)| rest).trim();
        let mut fields = Map::new();
        fields.insert("number".into(), json!(id.number.to_string()));
        let inside = |field: &'static str, place: &mut dyn FnMut(String, &'static str, Option<String>) -> ProjectedPlace| match holder(
            item.span.start,
            None,
        ) {
            Some(section) => place(section.to_string(), field, None),
            None => place(root.to_owned(), field, None),
        };
        let at = match id.kind {
            Kind::Section => {
                let heading = headings.next();
                fields.insert("heading".into(), json!(heading.map_or("", |heading| heading.text.as_str())));
                fields.insert("path".into(), json!(item.label));
                fields.insert("level".into(), json!(heading.map_or(1, |heading| heading.level)));
                fields.insert("body".into(), json!(rest));
                match holder(item.span.start, Some(*id)) {
                    Some(parent) => place(parent.to_string(), "sections", None),
                    None => place(root.to_owned(), "sections", None),
                }
            }
            Kind::Task => {
                fields.insert("text".into(), json!(item.label));
                fields.insert("checked".into(), json!(item.done.unwrap_or(false)));
                inside("tasks", &mut place)
            }
            Kind::Table => {
                let table = tables.next();
                fields.insert("header".into(), json!(item.label));
                fields.insert("rows".into(), json!(table.map_or(0, |table| table.rows.len())));
                fields.insert("columns".into(), json!(table.map_or(0, kition_note_markdown::Table::columns)));
                fields.insert("text".into(), json!(own));
                inside("tables", &mut place)
            }
            Kind::Link => {
                fields.insert("destination".into(), json!(item.key));
                inside("links", &mut place)
            }
            Kind::Property => {
                fields.insert("key".into(), json!(item.key));
                let value = own.split_once(':').map_or("", |(_, value)| value).trim();
                fields.insert("value".into(), json!(value));
                place(root.to_owned(), "properties", None)
            }
        };
        out.push(ProjectedObject { id: id.to_string(), ty: kind_type(id.kind).into(), fields, at: Some(at) });
    }
    out
}

/// The object model's type of a kind.
pub fn kind_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Section => "Section",
        Kind::Task => "Task",
        Kind::Table => "Table",
        Kind::Property => "Property",
        Kind::Link => "Link",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\nstatus: draft\n---\n\n# Plan\n\nIntro [[a]].\n\n## Tasks\n\n- [ ] One\n- [ ] Two\n\n## Notes\n\nSome notes.\n\n| A | B |\n| - | - |\n| 1 | 2 |\n";

    fn ids(tracked: &Tracked, kind: Kind) -> Vec<(String, String)> {
        tracked.now().items.iter().filter(|(id, _)| id.kind == kind).map(|(id, item)| (id.to_string(), item.label.clone())).collect()
    }

    #[test]
    fn ids_are_given_in_document_order() {
        let tracked = Tracked::open(NOTE);
        assert_eq!(
            ids(&tracked, Kind::Section),
            [("section:1".into(), "Plan".into()), ("section:2".into(), "Plan › Tasks".into()), ("section:3".into(), "Plan › Notes".into())]
        );
        assert_eq!(ids(&tracked, Kind::Task), [("task:1".into(), "One".into()), ("task:2".into(), "Two".into())]);
        assert_eq!(ids(&tracked, Kind::Table).len(), 1);
        assert_eq!(ids(&tracked, Kind::Property), [("property:1".into(), "status".into())]);
        assert_eq!(ids(&tracked, Kind::Link), [("link:1".into(), "a".into())]);
    }

    #[test]
    fn a_section_keeps_its_id_through_a_rename_a_move_and_lines_above() {
        let mut tracked = Tracked::open(NOTE);
        tracked.follow(&NOTE.replace("## Notes", "## Closing"), false);
        assert_eq!(tracked.now().find(Id { kind: Kind::Section, number: 3 }).unwrap().label, "Plan › Closing");
        let text = tracked.text().replace("Intro [[a]].", "Intro [[a]].\n\nMore.\n\nAnd more.");
        tracked.follow(&text, false);
        assert_eq!(tracked.now().find(Id { kind: Kind::Section, number: 3 }).unwrap().label, "Plan › Closing");
        // Moved: Tasks after Closing.
        let moved = text.replace("## Tasks\n\n- [ ] One\n- [ ] Two\n\n", "") + "\n## Tasks\n\n- [ ] One\n- [ ] Two\n";
        tracked.follow(&moved, false);
        assert_eq!(tracked.now().find(Id { kind: Kind::Section, number: 2 }).unwrap().label, "Plan › Tasks");
        assert_eq!(tracked.now().find(Id { kind: Kind::Task, number: 2 }).unwrap().label, "Two");
    }

    #[test]
    fn a_deleted_section_is_gone_and_a_new_one_gets_a_new_number() {
        let mut tracked = Tracked::open(NOTE);
        tracked.follow(&NOTE.replace("## Tasks\n\n- [ ] One\n- [ ] Two\n\n", ""), false);
        assert!(tracked.now().find(Id { kind: Kind::Section, number: 2 }).is_none());
        tracked.follow(&format!("{}\n## Tasks\n\nNew.\n", tracked.text()), false);
        assert_eq!(ids(&tracked, Kind::Section).last().unwrap().0, "section:4");
        assert!(!tracked.unproven_since(0, Id { kind: Kind::Section, number: 2 }));
    }

    #[test]
    fn look_alikes_make_an_id_unproven() {
        let note = "# A\n\nSame.\n\n# A\n\nSame.\n\n# A\n\nSame.\n";
        let mut tracked = Tracked::open(note);
        // Rewritten as two: which of three went is not known.
        tracked.follow("# A\n\nSame.\n\n# A\n\nSame.\n", true);
        let doubtful: Vec<u32> = (1..=3).filter(|number| tracked.unproven_since(0, Id { kind: Kind::Section, number: *number })).collect();
        assert!(!doubtful.is_empty(), "{:?}", tracked.now().items);
    }

    #[test]
    fn resolve_puts_the_line_in_place_of_the_id() {
        let tracked = Tracked::open(NOTE);
        let params = serde_json::json!({ "section": "section:2", "position": "end", "markdown": "x" });
        assert_eq!(resolve(&params, tracked.now()).unwrap(), serde_json::json!({ "line": 9, "position": "end", "markdown": "x" }));
        assert!(resolve(&serde_json::json!({ "section": "section:9" }), tracked.now()).unwrap_err().starts_with("not-found"));
        assert!(resolve(&serde_json::json!({ "section": "task:1" }), tracked.now()).unwrap_err().starts_with("invalid-params"));
        assert!(resolve(&serde_json::json!({ "section": "section:1", "line": 3 }), tracked.now()).unwrap_err().starts_with("invalid-params"));
    }
}
