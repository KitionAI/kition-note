//! What an agent's write rests on, for the premise guard (Kit Process Protocol `context.under`,
//! `context.covered`).
//!
//! A note is one object of the object model, the root `note`, whose `body` is the whole Markdown
//! file, and comment threads beside it. Headings, list items, tasks, tables, properties and links
//! are text inside `body`, not objects: Markdown has no place for an id that a reader does not
//! see, and the kit never adds text to a file it did not ask to change (the file is the text), so
//! they get no tool ids. A write names its place in the text by a position (`at`, `from`/`to`), a
//! line, a table's number, an occurrence, or a name (a heading path, a property key, a block id).
//!
//! Every write that edits the body or anchors a comment in it declares those parameters places
//! ([`ADDRESSES`]) and answers `under` with the root under the place, `covers: ["body"]`, and a
//! premise on the root holding only its `eol` register (inserted line endings take it). When others
//! changed the body since the agent's `basis`, the guard asks [`covered`] with the body as it was:
//! the kit works the write out ([`crate::run::plan`]) on the text then and on the text now and
//! compares, by the rule of its address ([`Address`]):
//!
//! - by position (an offset, a line, a table's number, an occurrence): refused `changed` when
//!   others' edits touch what the write reads or replaces, `relocated` when they moved it (an
//!   insertion or deletion before it), applied when they are all after it or elsewhere;
//! - by name (a heading path, a property key, a block id): applied when the name finds the same
//!   text, wherever it is now; `relocated` when it finds other text elsewhere or none (renamed
//!   away, deleted); `changed` when others edited it in place.
//!
//! A write by no place (`block.append` and `table.add` at the end of the note) holds the same
//! comparison as a content premise ([`content_at`]). `note.write` and `file.setLineEndings`
//! rewrite the whole body and hold the root whole. Comment commands hold the thread or comment
//! they name by id.

use std::ops::Range;

use kition_kit::{ContentAsk, ContentPremise, Covered, CoveredAsk, CoveredAt, CoveredContent, Covers, Premise, Under};
use kition_note_kdm::file::merge::{TextEdit, apply_edits, diff_strings, merge_edits};
use kition_note_markdown::index;
use serde_json::{Value, json};

use crate::projection::{self, Snapshot, Tracked};
use crate::run;

/// The root's object id, as change sets and premises name it.
pub const ROOT: &str = "note";

/// How a command's parameters address what it acts on (`KitCommand::targets`, `objects`,
/// `object_types`, `places`), read from its handler in `run.rs`.
pub struct Addresses {
    pub command: &'static str,
    /// The parameter naming the object it acts on by id (`on` fills it).
    pub targets: Option<&'static str>,
    /// Other parameters naming objects by id.
    pub objects: &'static [&'static str],
    /// The types those parameters name.
    pub object_types: &'static [(&'static str, &'static [&'static str])],
    /// The parameters naming a place in the body: a position, a line, a table, an occurrence, a
    /// heading path, a property key, a block id, a search.
    pub places: &'static [&'static str],
}

const fn places(command: &'static str, places: &'static [&'static str]) -> Addresses {
    Addresses { command, targets: None, objects: &[], object_types: &[], places }
}

const THREAD: &[(&str, &[&str])] = &[("thread", &["CommentThread"])];
const ANCHOR: &[&str] = &["from", "to", "quote", "occurrence", "heading", "line"];
const TABLE: &[&str] = &["table"];

/// Every write's addresses. A command missing here names nothing (`note.write`,
/// `file.setLineEndings`, `edit.undo`, `edit.redo`, and the reads).
pub const ADDRESSES: &[Addresses] = &[
    places("text.insert", &["at"]),
    places("text.delete", &["from", "to"]),
    places("text.replace", &["from", "to"]),
    places("text.findReplace", &["find", "occurrence"]),
    places("section.replace", &["heading", "line", "section"]),
    places("section.insert", &["heading", "line", "section", "position"]),
    places("section.delete", &["heading", "line", "section"]),
    places("section.move", &["heading", "line", "section", "toHeading", "toLine", "toSection", "position"]),
    places("heading.setLevel", &["heading", "line", "section"]),
    places("block.replace", &["blockId"]),
    places("block.append", &["heading", "line", "section"]),
    places("block.setId", &["line", "blockId", "occurrence"]),
    places("task.toggle", &["line", "blockId", "task"]),
    places("property.set", &["key"]),
    places("property.delete", &["key", "occurrence"]),
    places("property.rename", &["key"]),
    places("table.add", &["line", "heading", "position"]),
    places("table.setCell", &["table", "row", "column"]),
    places("table.insertRow", &["table", "at"]),
    places("table.insertColumn", &["table", "at"]),
    places("table.deleteRow", &["table", "row"]),
    places("table.deleteColumn", &["table", "column"]),
    places("table.moveRow", &["table", "from", "to"]),
    places("table.moveColumn", &["table", "from", "to"]),
    places("table.align", &["table", "column"]),
    places("table.format", TABLE),
    places("link.insert", &["at"]),
    places("link.update", &["line", "index"]),
    places("link.retarget", &["from"]),
    places("comment.add", ANCHOR),
    Addresses { command: "comment.reply", targets: Some("thread"), objects: &[], object_types: THREAD, places: &[] },
    Addresses { command: "comment.resolve", targets: Some("thread"), objects: &[], object_types: THREAD, places: &[] },
    Addresses { command: "comment.move", targets: Some("thread"), objects: &[], object_types: THREAD, places: ANCHOR },
    Addresses {
        command: "comment.delete",
        targets: Some("comment"),
        objects: &["thread"],
        object_types: &[("thread", &["CommentThread"]), ("comment", &["Comment"])],
        places: &[],
    },
];

/// The addresses of `command`, if it names anything.
pub fn addresses(command: &str) -> Option<&'static Addresses> {
    ADDRESSES.iter().find(|addresses| addresses.command == command)
}

/// Whether a step of `command` with `params` names a place (one of its place parameters given).
fn placed(command: &str, params: &Value) -> bool {
    addresses(command).is_some_and(|addresses| addresses.places.iter().any(|name| params.get(*name).is_some_and(|value| !value.is_null())))
}

/// Whether `command` edits the body or anchors at it by [`run::plan`].
fn planned(command: &str) -> bool {
    addresses(command).is_some_and(|addresses| !addresses.places.is_empty())
}

/// `context.under` for a write (reads answer `None`: the guard never asks for them).
pub fn under(command: &str, params: &Value) -> Option<Under> {
    let thread = || params.get("thread").cloned().unwrap_or(Value::Null);
    let premises = match command {
        // The whole body and its line style: every field of the root.
        "note.write" | "file.setLineEndings" => vec![Premise::on(ROOT)],
        "comment.reply" => vec![Premise::on(thread()).fields(Vec::<String>::new()).positions(Vec::<String>::new())],
        "comment.resolve" => vec![Premise::on(thread()).fields(["resolved"]).positions(Vec::<String>::new())],
        "comment.delete" => vec![
            Premise::on(thread()).fields(Vec::<String>::new()).positions(Vec::<String>::new()),
            Premise::on(params.get("comment").cloned().unwrap_or(Value::Null)).positions(Vec::<String>::new()),
        ],
        _ if planned(command) => Vec::new(),
        _ => return None,
    };
    let mut under = Under { premises, ..Under::default() };
    if !planned(command) {
        return Some(under);
    }
    // A comment command anchors at the body and writes none of it; every other writes text whose
    // line endings take the root's `eol`.
    let held: Vec<&str> = if command.starts_with("comment.") { vec![] } else { vec!["eol"] };
    under.premises.push(Premise::on(ROOT).fields(held).positions(Vec::<String>::new()));
    match command {
        "comment.move" => under.premises.push(Premise::on(thread()).fields(["anchor"]).positions(Vec::<String>::new())),
        // Without a place, a comment anchors at the whole note and rests on none of its text.
        "comment.add" if !placed(command, params) => return Some(under),
        _ => {}
    }
    // An object of the projection named by id is held as it is: deleted, the write is refused
    // `deleted`; what the write reads of it is compared by `covered`.
    for (_, id) in projection::named(params) {
        if let Some(id) = id {
            under.premises.push(Premise::on(id.to_string()).fields(Vec::<String>::new()).positions(Vec::<String>::new()));
        }
    }
    if placed(command, params) {
        under.objects = vec![json!(ROOT)];
        under.covers = Some(Covers { fields: vec!["body".to_owned()] });
    } else {
        under.content = vec![ContentPremise { id: json!(ROOT), field: "body".to_owned(), region: json!({ "command": command, "params": params }) }];
    }
    Some(under)
}

/// `context.under` for `edit.undo` and `edit.redo`: what the step it takes back or applies again
/// wrote (`touches`, kition-sdk's undo footprint), each object by the fields the step wrote. The
/// body is held whole: where the step writes it now cannot be told without taking the step (a copy
/// of the note has no undo history), so a change of the text anywhere refuses it.
pub fn history_under(touches: &[kition_kit::kdm::UndoTouch]) -> Under {
    let premises = touches
        .iter()
        .map(|touch| {
            let premise = Premise::on(touch.id.clone()).positions(Vec::<String>::new());
            match &touch.fields {
                Some(fields) => premise.fields(fields.iter().cloned().collect::<Vec<_>>()),
                None => premise,
            }
        })
        .collect();
    Under { premises, ..Under::default() }
}

/// How a write fares against others' change of the body from `before` to `now`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    /// Nothing it reads or replaces changed, nor where it is: the write lands as it would have.
    Same,
    /// What it addresses is elsewhere or gone (`relocated`).
    Moved,
    /// What it reads or replaces changed in place (`changed`).
    Changed,
    /// It could not be worked out on the text then (the call would have failed at `basis`).
    Unknown,
    /// What it names by id is gone: the object was deleted (`deleted`, from its own premise).
    Gone,
}

/// Writes that move text as it is when they run, rather than write what the agent read.
const CARRIES: &[&str] = &["section.move"];

/// Writes whose place is a section's extent or end ([`landing`]).
const SECTIONED: &[&str] = &["section.replace", "section.insert", "section.delete", "section.move", "block.append", "table.add"];

/// The rule of a step's address: by name when every place it gives is a name (a heading path, a
/// property key, a block id) and no occurrence counts among names.
fn by_name(params: &Value) -> bool {
    const NAMES: &[&str] = &["heading", "toHeading", "key", "blockId"];
    const POSITIONS: &[&str] = &["at", "from", "to", "line", "toLine", "table", "row", "column", "index", "occurrence", "find", "quote"];
    let given = |name: &&str| params.get(*name).is_some_and(|value| !value.is_null());
    NAMES.iter().any(given) && !POSITIONS.iter().any(given)
}

/// A plan in scalar indexes: the ranges it reads to find its place (`reads`), all it reads or
/// replaces (`footprint`, `reads` first), what its names stand for (`names`), and its edits.
struct Worked {
    reads: Vec<Range<usize>>,
    footprint: Vec<Range<usize>>,
    names: Vec<Range<usize>>,
    inserts: Vec<String>,
    /// The sections where it lands ([`landing`]).
    landing: Vec<Vec<u8>>,
}

/// Where a write that names a heading lands among the note's sections: for each of its edits, the
/// levels of the headings between the heading it names (the last one read before the edit) and the
/// edit's end. A section's extent is up to the next heading of its level or higher, and a section's
/// end is after its last subsection: another writer who demotes the heading that ended it, promotes
/// one inside it, or adds a subsection where it lands changes what the write lands on, though no
/// character it reads changed. (Renaming a heading changes none of it.)
fn landing(ix: &kition_note_markdown::Index, reads: &[Range<usize>], edits: &[kition_note_edit::Edit]) -> Vec<Vec<u8>> {
    // The headings it reads: their lines' starts.
    let starts: Vec<usize> = reads
        .iter()
        .filter(|read| ix.headings.iter().any(|heading| ix.lines.line_of(heading.range.start) == ix.lines.line_of(read.start)))
        .map(|read| read.start)
        .collect();
    if starts.is_empty() {
        return Vec::new();
    }
    edits
        .iter()
        .map(|edit| {
            let from = starts.iter().copied().filter(|start| *start <= edit.range.start).max().unwrap_or(0);
            ix.headings
                .iter()
                .filter(|heading| heading.range.start > from && heading.range.start < edit.range.end.max(edit.range.start))
                .map(|heading| heading.level)
                .collect()
        })
        .collect()
}

/// Where `command` with `params` reads on `text` to find its place, in scalar indexes (`None` when
/// it fails there), for the declaration table's check (`tests/table_check.rs`).
#[doc(hidden)]
pub fn reads_of(command: &str, params: &Value, text: &str) -> Option<Vec<Range<usize>>> {
    work(command, &opened(params, text)?, text).map(|worked| worked.reads)
}

/// `params` with the projection's ids (as a note opened with `text` gives them) put as lines.
fn opened(params: &Value, text: &str) -> Option<Value> {
    if projection::named(params).is_empty() {
        return Some(params.clone());
    }
    projection::resolve(params, Tracked::open(text).now()).ok()
}

/// Where `command` with `params` reads or writes on `text` (`None` when it fails there).
#[doc(hidden)]
pub fn footprint_of(command: &str, params: &Value, text: &str) -> Option<Vec<Range<usize>>> {
    work(command, &opened(params, text)?, text).map(|worked| worked.footprint)
}

/// Works `command` out on `text` in scalar indexes; `None` when it fails there.
fn work(command: &str, params: &Value, text: &str) -> Option<Worked> {
    let ix = index(text);
    let plan = run::plan(text, &ix, command, params).ok()??;
    let scalar = |range: &Range<usize>| ix.lines.scalar(text, range.start)..ix.lines.scalar(text, range.end);
    let reads: Vec<Range<usize>> = plan.reads.iter().chain(plan.anchor.iter()).map(scalar).collect();
    let mut footprint = reads.clone();
    footprint.extend(plan.edits.iter().map(|edit| scalar(&edit.range)));
    let landing = if SECTIONED.contains(&command) { landing(&ix, &plan.reads, &plan.edits) } else { Vec::new() };
    Some(Worked {
        reads,
        footprint,
        names: plan.names.iter().map(scalar).collect(),
        inserts: plan.edits.into_iter().map(|edit| edit.insert).collect(),
        landing,
    })
}

/// The span an edit of `before` (scalar indexes) could have been made at: an insertion or a
/// deletion in repeated text could sit anywhere along the repeat, so it is widened to all of it.
fn reach(before: &[char], edit: &TextEdit) -> Range<usize> {
    let (mut from, mut to) = (edit.from, edit.to);
    let inserted: Vec<char> = edit.insert.chars().collect();
    if inserted.is_empty() && to > from {
        // A deletion slides while the character before it equals its last, or after it its first.
        while from > 0 && before.get(from - 1) == before.get(to - 1) {
            from -= 1;
            to -= 1;
        }
        let (mut right_from, mut right_to) = (edit.from, edit.to);
        while right_to < before.len() && before.get(right_from) == before.get(right_to) {
            right_from += 1;
            right_to += 1;
        }
        return from..right_to;
    }
    if to == from && !inserted.is_empty() {
        let mut left = from;
        let mut rotated = inserted.clone();
        while left > 0 && before.get(left - 1) == rotated.last() {
            left -= 1;
            rotated.rotate_right(1);
        }
        let mut right = to;
        let mut rotated = inserted;
        while right < before.len() && before.get(right) == rotated.first() {
            right += 1;
            rotated.rotate_left(1);
        }
        return left..right;
    }
    from..to
}

/// Whether span `edit` touches `range` (closed at both ends: an edit right at an edge counts).
fn touches(edit: &Range<usize>, range: &Range<usize>) -> bool {
    edit.start <= range.end && range.start <= edit.end
}

/// Where position `at` of `before` is after `edits` (none of which touches it).
fn mapped(edits: &[TextEdit], at: usize) -> usize {
    let mut shift: isize = 0;
    for edit in edits.iter().filter(|edit| edit.to <= at) {
        shift += edit.insert.chars().count() as isize - (edit.to - edit.from) as isize;
    }
    at.saturating_add_signed(shift)
}

/// The range `range` of `before` covers after `edits`, an edit inside or across it taken whole.
fn mapped_range(edits: &[TextEdit], range: &Range<usize>) -> Range<usize> {
    let mut start = mapped(edits, range.start);
    let mut end = mapped(edits, range.end);
    for edit in edits.iter().filter(|edit| touches(&(edit.from..edit.to), range)) {
        let at = mapped(edits, edit.from);
        start = start.min(at);
        end = end.max(at + edit.insert.chars().count());
    }
    start..end.max(start)
}

/// The scalar characters of `text` in `range`.
fn slice(chars: &[char], range: &Range<usize>) -> String {
    chars.get(range.start.min(chars.len())..range.end.min(chars.len())).map(|part| part.iter().collect()).unwrap_or_default()
}

/// Works `command` with `params` out on the body as it was (`before`) and as it is (`now`) and
/// says how it fares (see the module's rules).
pub fn classify(command: &str, params: &Value, before: &str, now: &str) -> Fact {
    let Some(then) = work(command, params, before) else { return Fact::Unknown };
    if before == now {
        return Fact::Same;
    }
    let (b, n): (Vec<char>, Vec<char>) = (before.chars().collect(), now.chars().collect());
    let changes = diff_strings(before, now);
    let spans: Vec<Range<usize>> = changes.iter().map(|change| reach(&b, change)).collect();
    let touch = |ranges: &[Range<usize>]| spans.iter().any(|span| ranges.iter().any(|range| touches(span, range)));
    let touched = touch(&then.footprint);
    let Some(later) = work(command, params, now) else { return Fact::Moved };
    if then.landing != later.landing {
        return Fact::Changed;
    }
    let texts = |chars: &[char], ranges: &[Range<usize>]| -> Vec<String> { ranges.iter().map(|range| slice(chars, range)).collect() };
    if by_name(params) {
        // The name finds the text it found (its section, block or lines unchanged, wherever they are
        // now), or the same place with its own text untouched (an edit elsewhere inside a section).
        // (Line breaks around a section, block or insertion follow where it stands, not what it is.)
        let bare = |texts: Vec<String>| -> Vec<String> { texts.iter().map(|text| text.trim_matches(['\n', '\r']).to_owned()).collect() };
        // A move carries the section's text as it is when it runs: an edit inside it moves with it,
        // so only the headings it finds by name must be the ones observed.
        let same = if CARRIES.contains(&command) {
            texts(&b, &then.reads) == texts(&n, &later.reads)
        } else {
            texts(&b, &then.footprint) == texts(&n, &later.footprint) && bare(then.inserts.clone()) == bare(later.inserts.clone())
        };
        let intact = bare(texts(&b, &then.names)) == bare(texts(&n, &later.names));
        if same && (intact || !touch(&then.reads)) {
            return Fact::Same;
        }
        let first = |worked: &Worked| worked.footprint.first().cloned();
        let here = match (first(&then), first(&later)) {
            (Some(was), Some(is)) => touches(&mapped_range(&changes, &was), &is),
            _ => true,
        };
        return if touched && here { Fact::Changed } else { Fact::Moved };
    }
    if touched {
        // A line is a whole: an edit right at its edge (a line added right below it, or right
        // above it) that leaves its characters as they were did not change it. Whether it moved
        // is where those characters went.
        let given = |name: &str| params.get(name).is_some_and(|value| !value.is_null());
        let by_line = given("line") && !["at", "from", "to"].into_iter().any(given);
        // Each range's characters where they went, and each line it reads whole there.
        let whole_line = |chars: &[char], at: usize| -> Option<String> {
            let starts = at == 0 || chars.get(at - 1) == Some(&'\n');
            starts.then(|| chars.get(at..).unwrap_or_default().iter().take_while(|c| **c != '\n' && **c != '\r').collect())
        };
        let intact = then.footprint.iter().all(|range| {
            let at = mapped(&changes, range.start);
            slice(&b, range) == slice(&n, &(at..at + range.len()))
        }) && then.reads.iter().all(|range| whole_line(&b, range.start) == whole_line(&n, mapped(&changes, range.start)));
        if !(by_line && intact) {
            return Fact::Changed;
        }
        let starts = then.footprint.iter().map(|range| mapped(&changes, range.start)).eq(later.footprint.iter().map(|range| range.start));
        return if starts && then.inserts == later.inserts { Fact::Same } else { Fact::Moved };
    }
    let moved: Vec<(usize, usize)> = then.footprint.iter().map(|range| (mapped(&changes, range.start), mapped(&changes, range.end))).collect();
    let now_at: Vec<(usize, usize)> = later.footprint.iter().map(|range| (range.start, range.end)).collect();
    if moved != now_at || then.inserts != later.inserts {
        return Fact::Moved;
    }
    Fact::Same
}

/// How a write that names objects of the projection by id (`section`, `toSection`, `task`) fares
/// against others' change of the body from `before` to `now`, with `tracked` the projection
/// followed to `now`. The id stands for what it named at `basis` (the kept state of `before`) and
/// for what it names now: unknown when `before` is no state this session kept, or the id became
/// unproven since; gone when the id names nothing now (the object was deleted); else the same
/// unless what the write reads or replaces changed. A section's heading line is its name, not what
/// a write by its id rests on (it follows a rename); `heading.setLevel` rests on the heading's
/// marker it rewrites; a move by ids carries its sections as they are.
pub fn classify_ids(command: &str, params: &Value, before: &str, now: &str, basis: &Snapshot, tracked: &Tracked) -> Fact {
    if projection::named(params).iter().any(|(_, id)| id.is_some_and(|id| tracked.unproven_since(basis.seq, id))) {
        return Fact::Unknown;
    }
    let Ok(then_params) = projection::resolve(params, basis) else { return Fact::Unknown };
    let Ok(now_params) = projection::resolve(params, tracked.now()) else { return Fact::Gone };
    let Some(then) = work(command, &then_params, before) else { return Fact::Unknown };
    let Some(later) = work(command, &now_params, now) else { return Fact::Moved };
    if then.landing != later.landing {
        return Fact::Changed;
    }
    if CARRIES.contains(&command) {
        return Fact::Same;
    }
    let by_section = projection::named(params).iter().any(|(name, _)| *name != "task");
    let rest = |worked: &Worked| -> Vec<Range<usize>> {
        if by_section { worked.footprint.get(worked.reads.len()..).unwrap_or_default().to_vec() } else { worked.footprint.clone() }
    };
    let (b, n): (Vec<char>, Vec<char>) = (before.chars().collect(), now.chars().collect());
    let texts = |chars: &[char], ranges: &[Range<usize>]| -> Vec<String> { ranges.iter().map(|range| slice(chars, range)).collect() };
    let bare = |texts: &[String]| -> Vec<String> { texts.iter().map(|text| text.trim_matches(['\n', '\r']).to_owned()).collect() };
    if texts(&b, &rest(&then)) == texts(&n, &rest(&later)) && bare(&then.inserts) == bare(&later.inserts) { Fact::Same } else { Fact::Changed }
}

/// The ids among `objects` that name no object of the document now (a comment thread or comment
/// a reread of the file dropped: comments live in the object model, not in the `.md`, and a
/// document read again from its file has none). Answered so the guard refuses such a write
/// `unknown` instead of letting it fail at the kit.
pub fn unproven(objects: &[Value], exists: impl Fn(&str) -> bool) -> Vec<Value> {
    // (An id of the projection is no stored object: whether it is proven since a basis is
    // `covered`'s to say, and a deleted one is `deleted`.)
    objects
        .iter()
        .filter(|object| object.as_str().is_some_and(|id| id != ROOT && projection::Id::parse(id).is_none() && !exists(id)))
        .cloned()
        .collect()
}

/// The body before others' change, as `context.covered` and `context.content` give it: the whole
/// text; `None` for an excerpt (a body over the SDK's retained size), which cannot be told.
fn whole(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// The body a step meant, and the projection's state then: the body at `basis` (`before`) with
/// the run's earlier steps applied. On a planning copy (`planning`, the text the note held when it
/// was copied) the earlier steps ran from that text to `now`; they are carried over to `before`
/// with others' changes since `basis` taken out (`merge_edits`), and the projection followed
/// through them from its state at `basis` (`None` when `basis` is no state this session kept).
/// `None` when the earlier steps cannot be carried over.
fn with_prefix(planning: Option<&str>, before: &str, now: &str, tracked: &Tracked) -> Option<(String, Option<Snapshot>)> {
    let base = tracked.at_text(before).cloned();
    let Some(live) = planning.filter(|live| *live != now) else {
        return Some((before.to_owned(), base));
    };
    let (live, was, current): (Vec<char>, Vec<char>, Vec<char>) = (live.chars().collect(), before.chars().collect(), now.chars().collect());
    let meant = apply_edits(&current, &merge_edits(&live, &was, &current));
    let state = base.and_then(|base| tracked.rewound(&base)).map(|mut followed| {
        followed.follow(&meant, false);
        followed.now().clone()
    });
    Some((meant, state))
}

/// `context.covered` for a write by place in the body: the root was under the place at `basis`
/// unless others' change moved what the write addresses; its content is `changed` when they
/// edited it in place.
pub fn covered(planning: Option<&str>, command: &str, params: &Value, asks: &[CoveredAsk], now: &str, tracked: &Tracked) -> Option<CoveredAt> {
    let by_ids = !projection::named(params).is_empty();
    let root = asks.iter().find(|ask| ask.id.as_str() == Some(ROOT));
    let fact = match root {
        None => Fact::Same,
        Some(ask) if ask.created || ask.deleted || !ask.unknown.is_empty() => Fact::Unknown,
        Some(ask) => match ask.before.get("body") {
            None => Fact::Same,
            Some(before) => whole(before).map_or(Fact::Unknown, |before| {
                let Some((before, basis)) = with_prefix(planning, &before, now, tracked) else { return Fact::Unknown };
                match (by_ids, basis) {
                    (false, _) => classify(command, params, &before, now),
                    (true, Some(basis)) => classify_ids(command, params, &before, now, &basis, tracked),
                    (true, None) => Fact::Unknown,
                }
            }),
        },
    };
    let (under, covered, content) = match fact {
        Fact::Same => (vec![json!(ROOT)], Some(true), Some(CoveredContent::Same)),
        Fact::Changed => (vec![json!(ROOT)], Some(true), Some(CoveredContent::Changed)),
        Fact::Moved => (Vec::new(), Some(false), None),
        // The object named by id is gone: its own premise (`under`) refuses the write `deleted`;
        // the root it was in is still there.
        Fact::Gone => (vec![json!(ROOT)], Some(true), None),
        Fact::Unknown => (Vec::new(), None, None),
    };
    Some(CoveredAt {
        under,
        covered: asks
            .iter()
            .map(|ask| {
                if ask.id.as_str() == Some(ROOT) {
                    Covered { id: ask.id.clone(), covered, content, gone: false }
                } else {
                    Covered { id: ask.id.clone(), covered: Some(false), content: None, gone: false }
                }
            })
            .collect(),
    })
}

/// `content_at` for a write by no place: whether others' change of the body touched or moved
/// what it would write; `None` when it cannot be told.
pub fn content_at(planning: Option<&str>, ask: &ContentAsk, now: &str, tracked: &Tracked) -> Option<bool> {
    if ask.premise.id.as_str() != Some(ROOT) || ask.premise.field != "body" {
        return None;
    }
    let command = ask.premise.region.get("command")?.as_str()?;
    let params = ask.premise.region.get("params")?;
    let (before, _) = with_prefix(planning, &whole(&ask.before)?, now, tracked)?;
    match classify(command, params, &before, now) {
        Fact::Same => Some(false),
        Fact::Moved | Fact::Changed | Fact::Gone => Some(true),
        Fact::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "# Plan\n\nIntro text.\n\n## Tasks\n\n- [ ] One\n- [ ] Two\n\n## Notes\n\nSome notes.\n";

    fn fact(command: &str, params: Value, now: &str) -> Fact {
        classify(command, &params, NOTE, now)
    }

    #[test]
    fn a_write_by_position_is_moved_by_an_edit_before_it_and_not_after() {
        let toggle = json!({ "line": 7 });
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("Intro text.", "Intro text.\nMore.")), Fact::Moved);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("Intro text.", "Intro, longer.")), Fact::Same);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] One", "- [ ] Uno")), Fact::Changed);
        assert_eq!(fact("task.toggle", toggle, &NOTE.replace("Some notes.", "Some notes.\nAnd more.")), Fact::Same);
        let insert = json!({ "at": 10, "text": "X" });
        assert_eq!(fact("text.insert", insert.clone(), &NOTE.replace("# Plan", "# The plan")), Fact::Moved);
        assert_eq!(fact("text.insert", insert, &NOTE.replace("Some notes.", "Notes.")), Fact::Same);
    }

    #[test]
    fn a_write_by_name_follows_its_text_and_refuses_a_rename_or_an_edit() {
        let section = json!({ "heading": ["Plan", "Tasks"], "markdown": "- [x] Done" });
        // Moved elsewhere unchanged: the name finds the same text.
        let moved = NOTE.replace("## Tasks\n\n- [ ] One\n- [ ] Two\n\n", "") + "\n## Tasks\n\n- [ ] One\n- [ ] Two\n";
        assert_eq!(fact("section.replace", section.clone(), &moved), Fact::Same);
        assert_eq!(fact("section.replace", section.clone(), &NOTE.replace("Intro text.", "Intro text.\nMore.")), Fact::Same);
        assert_eq!(fact("section.replace", section.clone(), &NOTE.replace("- [ ] Two", "- [ ] Deux")), Fact::Changed);
        assert_eq!(fact("section.replace", section.clone(), &NOTE.replace("## Tasks", "## Todo")), Fact::Moved);
        // Swapped names: the name finds the other section's text.
        let swapped = NOTE.replace("## Tasks", "## Swap").replace("## Notes", "## Tasks").replace("## Swap", "## Notes");
        assert_eq!(fact("section.replace", section, &swapped), Fact::Moved);
    }

    /// A write by line is moved (`relocated`, not `changed`) by lines
    /// added above it, and applies after lines added right below it or further down.
    #[test]
    fn a_write_by_line_is_moved_by_lines_above_and_applies_after_lines_below() {
        let toggle = json!({ "line": 8, "done": true });
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] One", "- [ ] Zero\n- [ ] One")), Fact::Moved);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] Two", "- [ ] Zero\n- [ ] Two")), Fact::Moved);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] Two", "- [ ] Two, edited")), Fact::Changed);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] Two", "- [ ] Two\n- [ ] Three")), Fact::Same);
        assert_eq!(fact("task.toggle", toggle.clone(), &NOTE.replace("- [ ] Two\n", "- [ ] Two\n\nA paragraph.\n")), Fact::Same);
        assert_eq!(fact("task.toggle", toggle, &NOTE.replace("Some notes.", "Some notes.\nMore.")), Fact::Same);
    }

    /// Item 5: an edit before a position that keeps its length leaves the position on the same
    /// text, so a write there applies, even when that edit renamed the section the position is in:
    /// a position promises the characters at it, not the section around them (a write that rests
    /// on the section names it, by path or id). An edit that changes the length refuses it.
    #[test]
    fn an_equal_length_edit_before_a_position_applies_and_a_longer_one_moves_it() {
        let at = NOTE.find("Some notes.").unwrap();
        let insert = json!({ "at": at, "text": "Really " });
        assert_eq!(fact("text.insert", insert.clone(), &NOTE.replace("## Tasks", "## Tests")), Fact::Same);
        assert_eq!(fact("text.insert", insert.clone(), &NOTE.replace("## Notes", "## Votes")), Fact::Same);
        assert_eq!(fact("text.insert", insert, &NOTE.replace("## Notes", "## Closing")), Fact::Moved);
        // By name the rename refuses (the name finds nothing).
        let by_name = json!({ "heading": ["Plan", "Notes"], "position": "end", "markdown": "x" });
        assert_eq!(fact("section.insert", by_name, &NOTE.replace("## Notes", "## Votes")), Fact::Moved);
    }

    #[test]
    fn repeated_text_widens_an_edit_to_where_it_could_have_been_made() {
        let before: Vec<char> = "aaaa".chars().collect();
        assert_eq!(reach(&before, &TextEdit::new(4, 4, "a")), 0..4);
        assert_eq!(reach(&before, &TextEdit::new(2, 3, "")), 0..4);
    }
}
