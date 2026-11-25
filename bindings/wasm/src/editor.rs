//! Editing the note on the engine's own view (Word's model): the selection, the one `text` tool
//! (a click places the caret, a drag selects, a double click selects a word), editing keys, typed
//! and pasted text, and the commands that act on the selection (`format.*`, `list.*`,
//! `selection.*`). Every change is a `text.replace` transaction of the replica, run as the local
//! person; typing runs in one undo group until something else happens, so Undo takes back what
//! was typed in one go.
//!
//! The selection is the view's state, not the document's: byte offsets into the text that follow
//! other people's and the agent's edits ([`Editor::follow`]).

use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolInfo, ToolMods, ToolOverlay, ToolPointer, ToolRequest, ToolResponse};
use serde_json::{Value, json};
use unicode_segmentation::GraphemeCursor;

use kition_note_markdown::LinkKind;

use crate::NoteDocument;

/// The caret's color: the text's.
/// The caret's overlay is clear: the editor draws the caret itself over the canvas, blinking as
/// Obsidian's does (the overlay still moves with it, for the editor to follow).
const CARET: [u8; 4] = [31, 35, 40, 0];
/// The selection: the project purple, translucent, as light as Obsidian's (its accent at 20%).
const SELECTION: [u8; 4] = [86, 69, 212, 36];

/// How long a change may follow the one before and still join its undo step (CodeMirror's
/// `newGroupDelay`), in milliseconds.
const JOIN_DELAY: i64 = 500;

/// The open undo step, as CodeMirror's history keeps one: the byte ranges its changes touched (in
/// the text now) and when it last changed. A change that touches it soon after joins it.
#[derive(Clone, Debug, Default)]
struct Step {
    ranges: Vec<(usize, usize)>,
    last: i64,
    /// The replica's undo group is open for it.
    grouped: bool,
}

/// How the next change takes the open undo step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Join {
    /// Typing, deleting, cutting, and the format commands: in the open step when they touch it
    /// within [`JOIN_DELAY`] (as Obsidian joins a Cmd+B to the word typed before it).
    #[default]
    Adjacent,
    /// Enter and pasted lines: a step of their own, which the typing after them joins.
    Fresh,
}

/// An undo step the editor made: the selection before it and after it (what Undo and Redo put
/// back, as CodeMirror's history does), and the revision after it, which tells the step is still
/// the replica's last one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Record {
    before: (usize, usize),
    after: (usize, usize),
    revision: u64,
    /// The text's hash when the step began, and whether the step ended with the same text: the
    /// replica keeps no step for changes that come to nothing (Enter then Backspace).
    start: u64,
    pub(crate) empty: bool,
}

/// A text's hash, to tell a step that changed nothing.
fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// CodeMirror's selection history (Undo selection, Cmd+U): the selections moves left, moves
/// within [`JOIN_DELAY`] of each other one step, and whether an edit came after the last one.
#[derive(Clone, Debug, Default)]
struct SelectionHistory {
    done: Vec<(usize, usize)>,
    undone: Vec<(usize, usize)>,
    last: i64,
    edited: bool,
}

/// The most undo steps whose selections are kept.
const RECORDS: usize = 500;

/// The selection and the state of the gesture in progress.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    /// Where the selection started and where it ends (the caret), as byte offsets.
    pub anchor: usize,
    pub focus: usize,
    /// The column Up and Down keep to, in document pixels.
    goal: Option<f32>,
    /// The open undo step, and how the next change takes it.
    step: Option<Step>,
    join: Join,
    /// Inside a command of several changes: whether its first change has taken a step (its
    /// later changes go in the same one), and the selection before the command.
    batch: Option<bool>,
    batch_before: Option<(usize, usize)>,
    /// Offsets the editor follows through its own follow-up edits (the renumbering after a change).
    marks: Vec<usize>,
    /// The ordered lists are being renumbered: their edits renumber nothing more.
    renumbering: bool,
    /// The editor's undo steps (oldest first) and the steps undone, for their selections.
    pub(crate) done: Vec<Record>,
    pub(crate) undone: Vec<Record>,
    /// The text the offsets refer to, to follow changes made elsewhere.
    text: String,
    /// Set while the editor runs a command itself, so a command from elsewhere (an accepted
    /// suggestion, a menu, an agent) can end the typing run first.
    pub(crate) inside: bool,
    /// The person has clicked, typed, or moved the caret: until then no line shows its Markdown,
    /// so a note opens drawn.
    pub(crate) engaged: bool,
    /// The folded headings' first bytes (view state, moved with edits like the selection).
    pub(crate) folded: Vec<usize>,
    /// The press opened a link, folded a heading, or checked a task: its drag selects nothing.
    pub(crate) consumed: bool,
    /// The closers typed with their opener (`(` then `)`), by offset: typing the closer there
    /// steps over it, as Obsidian's paired brackets do.
    pairs: Vec<usize>,
    /// The last double click's time (milliseconds) and place, for a triple click.
    double: Option<(i64, f64, f64)>,
    /// The pointer's shape over what is under it: the pointing hand over what a click opens or
    /// folds (a link, a fold chevron), the arrow over a task's box (as Obsidian's), else the I-beam.
    cursor: &'static str,
    /// The selections the caret moved from, for Undo selection (Cmd+U), and those undone.
    selections: SelectionHistory,
    /// The caret is at a wrapped row's end: at the next row's first byte, drawn after the row's
    /// last character (CodeMirror's caret association, as End leaves it).
    row_end: bool,
    /// A press in the selection: the selected bytes, where the press was, and where a drag
    /// would drop them (the selected text moves there on release, as Obsidian drags it).
    dragged: Option<(usize, usize, usize, Option<usize>)>,
}

/// Brackets and quotes typed in pairs.
const PAIRS: [(char, char); 8] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`'), ('*', '*'), ('_', '_')];

/// Markdown marks that wrap a selection when typed over it (and the brackets and quotes).
const WRAPS: [char; 2] = ['=', '~'];

/// The closer typed with `c`, if `c` opens a pair.
fn closer_of(c: char) -> Option<char> {
    PAIRS.iter().find(|(open, _)| *open == c).map(|(_, close)| *close)
}

/// A pasted web or mail address (one, with nothing else).
fn is_address(text: &str) -> bool {
    let text = text.trim();
    (text.starts_with("https://") || text.starts_with("http://") || text.starts_with("mailto:"))
        && !text.chars().any(char::is_whitespace)
        && text.len() > 8
}

/// How many bytes `a` and `b` share at their start: whole blocks compare as slices (memcmp).
fn shared_start(a: &[u8], b: &[u8]) -> usize {
    const BLOCK: usize = 256;
    let blocks = a.as_chunks::<BLOCK>().0.iter().zip(b.as_chunks::<BLOCK>().0).take_while(|(x, y)| x == y).count() * BLOCK;
    blocks + a.iter().skip(blocks).zip(b.iter().skip(blocks)).take_while(|(x, y)| x == y).count()
}

/// How many bytes `a` and `b` share at their end, at most `room`.
fn shared_end(a: &[u8], b: &[u8], room: usize) -> usize {
    const BLOCK: usize = 256;
    let blocks = a.rchunks_exact(BLOCK).zip(b.rchunks_exact(BLOCK)).take(room / BLOCK).take_while(|(x, y)| x == y).count() * BLOCK;
    blocks + a.iter().rev().skip(blocks).zip(b.iter().rev().skip(blocks)).take(room - blocks).take_while(|(x, y)| x == y).count()
}

/// The longest common prefix and suffix of two texts, in bytes on character boundaries.
fn common(old: &str, new: &str) -> (usize, usize) {
    let mut prefix = shared_start(old.as_bytes(), new.as_bytes());
    while !(old.is_char_boundary(prefix) && new.is_char_boundary(prefix)) {
        prefix -= 1;
    }
    let max = old.len().min(new.len()) - prefix;
    let mut suffix = shared_end(old.as_bytes(), new.as_bytes(), max);
    while suffix > 0 && !(old.is_char_boundary(old.len() - suffix) && new.is_char_boundary(new.len() - suffix)) {
        suffix -= 1;
    }
    (prefix, suffix)
}

/// One edit of a remote change, in bytes of the old text: `from..to` became `inserted` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ByteEdit {
    from: usize,
    to: usize,
    inserted: usize,
}

/// The edits from `old` to `new`, sorted and in bytes of `old`: the common start and end are
/// skipped, and only the text between them is diffed (a line diff refined to characters, the
/// one file merges use), so a small change to a large note costs little.
fn remote_edits(old: &str, new: &str) -> Vec<ByteEdit> {
    let (prefix, suffix) = common(old, new);
    let (Some(old_middle), Some(new_middle)) = (old.get(prefix..old.len() - suffix), new.get(prefix..new.len() - suffix)) else {
        return vec![ByteEdit { from: 0, to: old.len(), inserted: new.len() }];
    };
    if old_middle.is_empty() || new_middle.is_empty() {
        return vec![ByteEdit { from: prefix, to: prefix + old_middle.len(), inserted: new_middle.len() }];
    }
    let edits = kition_note_kdm::file::merge::diff_strings(old_middle, new_middle);
    // Character indices of the middle to bytes, in one pass (the edits are sorted).
    let mut wanted: Vec<usize> = edits.iter().flat_map(|edit| [edit.from, edit.to]).collect();
    wanted.sort_unstable();
    wanted.dedup();
    let mut bytes = std::collections::HashMap::with_capacity(wanted.len());
    let mut next = wanted.iter().peekable();
    for (index, (byte, _)) in old_middle.char_indices().chain(std::iter::once((old_middle.len(), ' '))).enumerate() {
        while next.peek().is_some_and(|want| **want == index) {
            bytes.insert(index, byte);
            next.next();
        }
        if next.peek().is_none() {
            break;
        }
    }
    let byte = |index: usize| prefix + bytes.get(&index).copied().unwrap_or(old_middle.len());
    edits.iter().map(|edit| ByteEdit { from: byte(edit.from), to: byte(edit.to), inserted: edit.insert.len() }).collect()
}

/// Where the byte offset `at` of the old text is after `edits` (sorted): text inserted at it goes
/// after it, and an offset inside replaced text goes to the end of the replacement.
fn map_through(edits: &[ByteEdit], at: usize) -> usize {
    let mut shift: isize = 0;
    for edit in edits {
        if at <= edit.from {
            break;
        }
        let start = edit.from.saturating_add_signed(shift);
        if at < edit.to {
            return start + edit.inserted;
        }
        shift += isize::try_from(edit.inserted).unwrap_or(isize::MAX) - isize::try_from(edit.to.saturating_sub(edit.from)).unwrap_or(isize::MAX);
    }
    at.saturating_add_signed(shift)
}

/// The file's text `base` with another person's change from `old` (the note's text, holding this
/// person's unsaved edits since `base`) to `new` taken in: the base of a later file merge, so a
/// co-editor's save of what this note already holds brings nothing in twice
/// (`NoteDocument::merge_file`). Edits outside the span where `old` and `base` differ (this
/// person's unsaved edits) go straight onto the base; only those inside it are merged, over that
/// span alone, keeping every insertion as a file merge does.
pub(crate) fn rebase(base: &str, old: &str, new: &str) -> String {
    if base == old {
        return new.to_owned();
    }
    let (prefix, suffix) = common(old, base);
    let middle_end = old.len() - suffix;
    // Each edit with the text it inserts (in `new`).
    let mut shift: isize = 0;
    let mut placed = Vec::new();
    for edit in remote_edits(old, new) {
        let at = edit.from.saturating_add_signed(shift);
        let inserted = new.get(at..at + edit.inserted).unwrap_or("");
        placed.push((edit, inserted));
        shift += isize::try_from(edit.inserted).unwrap_or(isize::MAX) - isize::try_from(edit.to.saturating_sub(edit.from)).unwrap_or(isize::MAX);
    }
    let (mut before, mut inside, mut after) = (Vec::new(), Vec::new(), Vec::new());
    for (edit, text) in placed {
        if edit.to <= prefix {
            before.push((edit, text));
        } else if edit.from >= middle_end {
            after.push((edit, text));
        } else if edit.from >= prefix && edit.to <= middle_end {
            inside.push((edit, text));
        } else {
            return merged(base, old, new);
        }
    }
    // `text` (which starts at byte `start` of `old`) with `edits` applied.
    let apply = |text: &str, start: usize, edits: &[(ByteEdit, &str)]| {
        let mut out = String::with_capacity(text.len());
        let mut from = 0;
        for (edit, inserted) in edits {
            out.push_str(text.get(from..edit.from - start).unwrap_or(""));
            out.push_str(inserted);
            from = edit.to - start;
        }
        out.push_str(text.get(from..).unwrap_or(""));
        out
    };
    let (Some(head), Some(old_middle), Some(base_middle), Some(tail)) =
        (old.get(..prefix), old.get(prefix..middle_end), base.get(prefix..base.len() - suffix), old.get(middle_end..))
    else {
        return merged(base, old, new);
    };
    let middle = if inside.is_empty() { base_middle.to_owned() } else { merged(base_middle, old_middle, &apply(old_middle, prefix, &inside)) };
    let mut out = apply(head, 0, &before);
    out.push_str(&middle);
    out.push_str(&apply(tail, middle_end, &after));
    out
}

/// `base` with the change `old → new` merged in (`old` and `base` both from one ancestor).
fn merged(base: &str, old: &str, new: &str) -> String {
    use kition_note_kdm::file::merge::{apply_edits, merge_edits};
    let (old, new, base): (Vec<char>, Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect(), base.chars().collect());
    apply_edits(&base, &merge_edits(&old, &new, &base))
}

impl Editor {
    /// Moves the selection along with a change from `self.text` to `new`: offsets before the
    /// change stay, offsets after it shift, offsets inside it go to its end.
    pub fn follow(&mut self, new: &str) {
        if self.text == new {
            return;
        }
        self.row_end = false;
        let (prefix, suffix) = common(&self.text, new);
        let (old_len, new_len) = (self.text.len(), new.len());
        let map = |at: usize| {
            if at <= prefix {
                at
            } else if at >= old_len - suffix {
                at + new_len - old_len
            } else {
                new_len - suffix
            }
        };
        self.anchor = map(self.anchor.min(old_len));
        self.focus = map(self.focus.min(old_len));
        for fold in &mut self.folded {
            *fold = map((*fold).min(old_len));
        }
        for mark in &mut self.marks {
            *mark = map((*mark).min(old_len));
        }
        self.folded.sort_unstable();
        self.folded.dedup();
        if let Some(step) = &mut self.step {
            for range in &mut step.ranges {
                *range = (map(range.0.min(old_len)), map(range.1.min(old_len)));
            }
        }
        // A paired closer moves with text typed before it, and goes with a change over it.
        self.pairs = self
            .pairs
            .iter()
            .filter_map(|at| {
                if *at < prefix {
                    Some(*at)
                } else if *at >= old_len - suffix {
                    Some(at + new_len - old_len)
                } else {
                    None
                }
            })
            .collect();
        new.clone_into(&mut self.text);
    }

    /// Moves every offset the editor keeps along with another person's change from `self.text` to
    /// `new` (a `replica/import`): unlike [`Editor::follow`], which takes a change as one span (a
    /// local edit is one), the change may touch several places at once, so it is diffed into its
    /// edits and each offset moves by the edits before it. Text inserted at an offset goes after
    /// it (the caret stays before what another person types there, so neither person's typing
    /// splits the other's); an offset inside text the change replaced goes to the end of the
    /// replacement. The undo records and the selection history move too: they still name the
    /// same text.
    pub fn follow_remote(&mut self, new: &str) {
        if self.text == new {
            return;
        }
        let edits = remote_edits(&self.text, new);
        let old_len = self.text.len();
        let map = |at: &mut usize| *at = map_through(&edits, (*at).min(old_len)).min(new.len());
        map(&mut self.anchor);
        map(&mut self.focus);
        self.folded.iter_mut().for_each(map);
        self.folded.sort_unstable();
        self.folded.dedup();
        self.marks.iter_mut().for_each(map);
        self.pairs.iter_mut().for_each(map);
        if let Some(step) = &mut self.step {
            for (from, to) in &mut step.ranges {
                map(from);
                map(to);
            }
        }
        if let Some((anchor, focus)) = &mut self.batch_before {
            map(anchor);
            map(focus);
        }
        for record in self.done.iter_mut().chain(self.undone.iter_mut()) {
            map(&mut record.before.0);
            map(&mut record.before.1);
            map(&mut record.after.0);
            map(&mut record.after.1);
        }
        for (anchor, focus) in self.selections.done.iter_mut().chain(self.selections.undone.iter_mut()) {
            map(anchor);
            map(focus);
        }
        if let Some((from, to, at, drop)) = &mut self.dragged {
            map(from);
            map(to);
            map(at);
            if let Some(drop) = drop {
                map(drop);
            }
        }
        self.row_end = false;
        self.goal = None;
        new.clone_into(&mut self.text);
    }

    /// Records made at `old` (the revision before a remote change) still describe the replica's
    /// last local steps after it, at `new`.
    pub(crate) fn keep_records(&mut self, old: u64, new: u64) {
        for record in self.done.iter_mut().chain(self.undone.iter_mut()).filter(|record| record.revision == old) {
            record.revision = new;
        }
    }

    /// Puts back a selection and folds taken when the note held `from` (`KitDocument::editor_state`),
    /// the text now when `None`: each offset moves along the change from `from` to the text now as
    /// another person's change moves it ([`Editor::follow_remote`]). Refuses, changing nothing, an
    /// offset `from` does not have (past its end or inside a character).
    pub(crate) fn put_back(&mut self, from: Option<&str>, anchor: usize, focus: usize, folded: &[usize]) -> Result<(), String> {
        let from = from.unwrap_or(&self.text);
        let resolves = |at: usize| at <= from.len() && from.is_char_boundary(at);
        if !resolves(anchor) || !resolves(focus) {
            return Err(format!("selection out of range: {anchor}..{focus} in a text of {} bytes", from.len()));
        }
        if let Some(fold) = folded.iter().find(|at| !resolves(**at)) {
            return Err(format!("fold out of range: {fold} in a text of {} bytes", from.len()));
        }
        let edits = if from == self.text { Vec::new() } else { remote_edits(from, &self.text) };
        let map = |at: usize| map_through(&edits, at).min(self.text.len());
        let (anchor, focus) = (map(anchor), map(focus));
        let mut folds: Vec<usize> = folded.iter().map(|at| map(*at)).collect();
        folds.sort_unstable();
        folds.dedup();
        if (anchor, focus) != (self.anchor, self.focus) {
            self.anchor = anchor;
            self.focus = focus;
            self.row_end = false;
            self.goal = None;
        }
        self.folded = folds;
        Ok(())
    }

    /// Whether an undo step is open (a typing run).
    #[cfg(test)]
    pub(crate) fn typing(&self) -> bool {
        self.step.is_some()
    }

    /// The text the offsets refer to.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub fn ordered(&self) -> (usize, usize) {
        (self.anchor.min(self.focus), self.anchor.max(self.focus))
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    /// The lines the selection touches, from the first one's start to the last one's end: the
    /// live preview shows their Markdown syntax.
    /// The selection's bytes (clamped to the text), or past its end when the editor is not engaged.
    pub fn selection_bytes(&self, text: &str) -> std::ops::Range<usize> {
        if !self.engaged {
            return text.len() + 1..text.len() + 1;
        }
        let (from, to) = self.ordered();
        from.min(text.len())..to.min(text.len())
    }

    pub fn active_lines(&self, text: &str) -> std::ops::Range<usize> {
        if !self.engaged {
            return text.len() + 1..text.len() + 1;
        }
        let (from, to) = self.ordered();
        let (from, to) = (from.min(text.len()), to.min(text.len()));
        let start = text.get(..from).and_then(|before| before.rfind('\n')).map_or(0, |at| at + 1);
        let end = text.get(to..).and_then(|after| after.find('\n')).map_or(text.len(), |at| to + at);
        start..end
    }
}

fn next_grapheme(text: &str, at: usize) -> usize {
    GraphemeCursor::new(at, text.len(), true).next_boundary(text, 0).ok().flatten().unwrap_or(text.len())
}

fn previous_grapheme(text: &str, at: usize) -> usize {
    GraphemeCursor::new(at, text.len(), true).prev_boundary(text, 0).ok().flatten().unwrap_or(0)
}

/// The character group of a grapheme (by its first character; a line break is a space).
fn group_of_part(part: &str) -> Group {
    part.chars().next().map_or(Group::Space, group_of)
}

/// Where Alt+Left (`forward`: Alt+Right) goes, as CodeMirror moves by groups: over the first
/// character, then on while the characters are of its group; spaces take the group of what follows
/// them, and a line break counts as a space (so the move goes on into the next line's word).
fn by_group(text: &str, at: usize, forward: bool) -> usize {
    // The format marks of spans the caret does not touch are hidden in Obsidian's live preview, one
    // atomic range each: a move by group stops at them, or steps over one alone.
    let (line_start, line_end) = line_around(text, at);
    let line = text.get(line_start..line_end).unwrap_or("");
    let atomic: Vec<(usize, usize)> = ["**", "*", "~~", "==", "`"]
        .iter()
        .flat_map(|mark| mark_spans(line, mark))
        .map(|span| span.shifted(line_start))
        .filter(|span| at < span.open.0 || at > span.close.1)
        .flat_map(|span| [span.open, span.close])
        .collect();
    let token_from = |pos: usize| atomic.iter().find(|(start, end)| if forward { *start == pos } else { *end == pos }).copied();
    if let Some((start, end)) = token_from(at) {
        return if forward { end } else { start };
    }
    let step = |pos: usize| {
        if forward { (pos < text.len()).then(|| next_grapheme(text, pos)) } else { (pos > 0).then(|| previous_grapheme(text, pos)) }
    };
    let part = |pos: usize, next: usize| text.get(pos.min(next)..pos.max(next)).unwrap_or("");
    let Some(mut pos) = step(at) else { return at };
    let mut group = group_of_part(part(at, pos));
    while let Some(next) = step(pos) {
        if token_from(pos).is_some() {
            break;
        }
        let found = group_of_part(part(pos, next));
        if group == Group::Space {
            group = found;
        } else if group != found {
            break;
        }
        pos = next;
    }
    pos
}

/// Where Alt+Backspace (`forward`: Alt+Delete) deletes to, as CodeMirror deletes by groups: the
/// characters of one group on the caret's line (a single space at the caret goes with the group
/// after it); at the line's edge, the line break.
fn delete_group(text: &str, at: usize, forward: bool) -> usize {
    let (line_start, line_end) = line_around(text, at);
    let mut pos = at;
    let mut group: Option<Group> = None;
    loop {
        if (!forward && pos <= line_start) || (forward && pos >= line_end) {
            if pos == at {
                return if forward { next_grapheme(text, pos) } else { previous_grapheme(text, pos) };
            }
            return pos;
        }
        let next = if forward { next_grapheme(text, pos) } else { previous_grapheme(text, pos) };
        let part = text.get(pos.min(next)..pos.max(next)).unwrap_or("");
        let found = group_of_part(part);
        if group.as_ref().is_some_and(|group| *group != found) {
            return pos;
        }
        if part != " " || pos != at {
            group = Some(found);
        }
        pos = next;
    }
}

/// What a double click groups, as Obsidian's editor (CodeMirror) groups characters: a word
/// character (a letter, a digit, `_`, so a CJK run too), a space, or anything else.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Group {
    Word,
    Space,
    Other,
}

fn group_of(c: char) -> Group {
    if c.is_alphanumeric() || c == '_' {
        Group::Word
    } else if c.is_whitespace() {
        Group::Space
    } else {
        Group::Other
    }
}

/// The run around `at` that a double click selects: the characters of the clicked one's group on
/// its line (a CJK run up to its punctuation, `123` of `123.45`, `don` of `don't`).
fn word_at(text: &str, at: usize) -> (usize, usize) {
    let (line_start, line_end) = line_around(text, at);
    let line = text.get(line_start..line_end).unwrap_or("");
    let offset = at - line_start;
    // The character clicked: the one at the point, or before it at the line's end.
    let clicked =
        line.get(offset..).and_then(|after| after.chars().next()).or_else(|| line.get(..offset).and_then(|before| before.chars().next_back()));
    let Some(clicked) = clicked else { return (at, at) };
    let group = group_of(clicked);
    let start = line
        .get(..offset)
        .map_or(0, |before| before.char_indices().rev().take_while(|(_, c)| group_of(*c) == group).last().map_or(offset, |(index, _)| index));
    let end = line
        .get(offset..)
        .map_or(offset, |after| offset + after.char_indices().find(|(_, c)| group_of(*c) != group).map_or(after.len(), |(index, _)| index));
    // At the line's end the run is the one before the point.
    let start = if end == offset && offset == line.len() { start } else { start.min(offset) };
    (line_start + start, line_start + end.max(start))
}

/// The source line around `at`: its start and its end before the `\n` (and before a `\r` a
/// mixed-ending note keeps there, which stays at the line's end).
/// Where the front matter ends (0 without one): the line commands leave its lines alone, as the
/// editor shows them as the properties block.
fn front_matter_end(text: &str) -> usize {
    kition_note_markdown::frontmatter::scan(text, |_| 0).map_or(0, |found| found.range.end)
}

fn line_around(text: &str, at: usize) -> (usize, usize) {
    let start = text.get(..at).and_then(|before| before.rfind('\n')).map_or(0, |found| found + 1);
    let mut end = text.get(at..).and_then(|after| after.find('\n')).map_or(text.len(), |found| at + found);
    if end > start.max(at) && text.as_bytes().get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    (start, end)
}

/// The length of a line's quote prefix: its indentation and every `>` with the space after it
/// (0 when the line is not quoted).
fn quote_prefix(line: &str) -> usize {
    let mut rest = line.trim_start_matches([' ', '\t']);
    if !rest.starts_with('>') {
        return 0;
    }
    while let Some(after) = rest.strip_prefix("> ").or_else(|| rest.strip_prefix('>')) {
        rest = after;
    }
    line.len() - rest.len()
}

/// A list or quote line's prefix: (its length in bytes, what the next line starts with), for
/// Enter. `None` when the line is neither.
fn continuation(line: &str) -> Option<(usize, String)> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let mut rest = line.get(indent..)?;
    let mut prefix = line.get(..indent)?.to_owned();
    let mut quoted = false;
    while let Some(after) = rest.strip_prefix("> ").or_else(|| rest.strip_prefix('>')) {
        prefix.push_str("> ");
        rest = after;
        quoted = true;
    }
    // A nested item in a quote: its indentation comes after the `>`.
    if quoted {
        let inner = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        prefix.push_str(rest.get(..inner).unwrap_or(""));
        rest = rest.get(inner..).unwrap_or("");
    }
    let mut next = prefix.clone();
    let marker_len;
    if let Some(after) = ["- ", "* ", "+ "].iter().find_map(|marker| rest.strip_prefix(marker).map(|after| (*marker, after))) {
        next.push_str(after.0);
        marker_len = after.0.len();
        rest = after.1;
    } else {
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let after_digits = rest.get(digits..).unwrap_or("");
        if digits > 0 && digits <= 9 && (after_digits.starts_with(". ") || after_digits.starts_with(") ")) {
            let number: u64 = rest.get(..digits).and_then(|n| n.parse().ok()).unwrap_or(1);
            next.push_str(&format!("{}{}", number + 1, after_digits.get(..2).unwrap_or(". ")));
            marker_len = digits + 2;
            rest = after_digits.get(2..).unwrap_or("");
        } else if quoted {
            let used = line.len() - rest.len();
            return Some((used, next));
        } else {
            return None;
        }
    }
    let mut used = line.len() - rest.len();
    let _ = marker_len;
    for task in ["[ ] ", "[x] ", "[X] "] {
        if rest.starts_with(task) {
            next.push_str("[ ] ");
            used += task.len();
            break;
        }
    }
    Some((used, next))
}

/// A numbered list item's line: its indentation (after a quote's `>`, in bytes), its number, and
/// where its digits are.
struct OrderedItem {
    indent: usize,
    number: u64,
    digits: usize,
    digits_len: usize,
}

/// The line as a numbered item (`12. ` or `3) `, also at the line's end), if it is one.
fn ordered_item(line: &str) -> Option<OrderedItem> {
    let quote = quote_prefix(line);
    let inner = line.get(quote..)?;
    let indent = inner.len() - inner.trim_start_matches([' ', '\t']).len();
    let body = inner.get(indent..)?;
    let digits_len = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let after = body.get(digits_len..)?;
    let marked = after.starts_with(". ") || after.starts_with(") ") || after == "." || after == ")";
    (digits_len > 0 && digits_len <= 9 && marked).then(|| OrderedItem {
        indent,
        number: body.get(..digits_len).and_then(|n| n.parse().ok()).unwrap_or(1),
        digits: quote + indent,
        digits_len,
    })
}

/// A list item's indentation (after a quote's `>`), or `None` for a line that is no item.
fn item_indent(line: &str) -> Option<usize> {
    if ordered_item(line).is_some() {
        return ordered_item(line).map(|item| item.indent);
    }
    let quote = quote_prefix(line);
    let inner = line.get(quote..)?;
    let indent = inner.len() - inner.trim_start_matches([' ', '\t']).len();
    let body = inner.get(indent..)?;
    (["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker)) || matches!(body, "-" | "*" | "+")).then_some(indent)
}

/// How a line stands to a list at `indent` walked through from line `index`'s neighbour.
enum Walk {
    /// An item at the list's indentation (numbered or not).
    Sibling,
    /// Inside the list: a blank line, a deeper line, a lazy continuation.
    Inside,
    /// Past the list: a parent, a bulleted list at its level, a paragraph after a blank line.
    End,
}

/// Line `index` seen from the list at `indent` above or below it.
fn sibling_step(lines: &[&str], index: usize, indent: usize) -> Walk {
    let Some(line) = lines.get(index) else { return Walk::End };
    let quote = quote_prefix(line);
    let inner = line.get(quote..).unwrap_or("");
    if inner.trim().is_empty() {
        return Walk::Inside;
    }
    match item_indent(line) {
        Some(found) if found < indent => Walk::End,
        Some(found) if found == indent => {
            if ordered_item(line).is_some() {
                Walk::Sibling
            } else {
                Walk::End
            }
        }
        Some(_) => Walk::Inside,
        None => {
            let own = inner.len() - inner.trim_start_matches([' ', '\t']).len();
            // Deeper text, or a line right under a non-blank one (a lazy continuation).
            let above = index
                .checked_sub(1)
                .and_then(|above| lines.get(above))
                .is_some_and(|above| !above.get(quote_prefix(above)..).unwrap_or("").trim().is_empty());
            if own > indent || above { Walk::Inside } else { Walk::End }
        }
    }
}

/// The number of the numbered item before line `index` at `indent` in the same list.
fn previous_sibling(lines: &[&str], index: usize, indent: usize) -> Option<u64> {
    for line in (0..index).rev() {
        match sibling_step(lines, line, indent) {
            Walk::Sibling => return lines.get(line).and_then(|found| ordered_item(found)).map(|item| item.number),
            Walk::Inside => {}
            Walk::End => return None,
        }
    }
    None
}

/// Where a deletion of `from..to` starts: before a format's opening mark right before `from`
/// when the deletion takes the span's closing mark and text after it (Obsidian takes the orphaned
/// opener too, so `a **‹b** c›` deleted leaves `a `, but `a **‹b**›` leaves `a **`).
fn orphan_start(text: &str, from: usize, to: usize) -> usize {
    let (line_start, line_end) = line_around(text, from);
    let line = text.get(line_start..line_end).unwrap_or("");
    ["**", "*", "~~", "==", "`"]
        .iter()
        .flat_map(|mark| mark_spans(line, mark))
        .map(|span| span.shifted(line_start))
        .filter(|span| span.open.1 == from && span.close.0 >= from && span.close.1 < to)
        .map(|span| span.open.0)
        .min()
        .unwrap_or(from)
}

/// A formatted span on a line: its opening and closing marks' byte ranges.
#[derive(Clone, Copy, Debug)]
struct Span {
    open: (usize, usize),
    close: (usize, usize),
}

impl Span {
    fn shifted(self, by: usize) -> Self {
        Self { open: (self.open.0 + by, self.open.1 + by), close: (self.close.0 + by, self.close.1 + by) }
    }
}

/// The spans of `mark`'s format on `line`, as Obsidian reads its marks: bold is `**` or `__`
/// (a run of two or three), italic `*` or `_` (a run of one or three), the others their own run of
/// their length; runs of the same character pair up in turn, the marks on the text's side.
fn mark_spans(line: &str, mark: &str) -> Vec<Span> {
    let (chars, lengths): (&[char], &[usize]) = match mark {
        "**" => (&['*', '_'], &[2, 3]),
        "*" => (&['*', '_'], &[1, 3]),
        "~~" => (&['~'], &[2]),
        "==" => (&['='], &[2]),
        "`" => (&['`'], &[1]),
        "$" => (&['$'], &[1]),
        _ => return Vec::new(),
    };
    let k = mark.len();
    let bytes = line.as_bytes();
    let mut runs: Vec<(u8, usize, usize)> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let Some(&byte) = bytes.get(at) else { break };
        if byte == b'\\' {
            at += 2;
            continue;
        }
        if chars.iter().any(|c| *c as u32 == u32::from(byte)) {
            let start = at;
            while bytes.get(at) == Some(&byte) {
                at += 1;
            }
            if lengths.contains(&(at - start)) {
                runs.push((byte, start, at));
            }
            continue;
        }
        at += 1;
    }
    let mut spans = Vec::new();
    let mut open: Option<(u8, usize, usize)> = None;
    for run in runs {
        match open {
            Some(first) if first.0 == run.0 => {
                spans.push(Span { open: (first.2 - k, first.2), close: (run.1, run.1 + k) });
                open = None;
            }
            Some(_) => {}
            None => open = Some(run),
        }
    }
    spans
}

/// For a list item's continuation line starting at `line_start` (indented text under the item,
/// not an item itself): what the next item starts with (its marker, number, and unchecked box).
fn owner_item_next(text: &str, line_start: usize) -> Option<String> {
    let (_, line_end) = line_around(text, line_start);
    let line = text.get(line_start..line_end)?;
    let quote = quote_prefix(line);
    let inner = line.get(quote..)?;
    if inner.trim().is_empty() || item_indent(line).is_some() {
        return None;
    }
    let own = inner.len() - inner.trim_start_matches([' ', '\t']).len();
    let mut at = line_start;
    while at > 0 {
        let (above, above_end) = line_around(text, at - 1);
        let line = text.get(above..above_end)?;
        let inner = line.get(quote_prefix(line)..)?;
        if inner.trim().is_empty() {
            return None;
        }
        if let Some(indent) = item_indent(line) {
            return (indent < own).then(|| continuation(line).map(|(_, next)| next)).flatten();
        }
        at = above;
    }
    None
}

/// The scalar index of byte `at` in `text`.
fn scalar(text: &str, at: usize) -> usize {
    text.get(..at).map_or_else(|| text.chars().count(), |before| before.chars().count())
}

/// Unicode character offsets of byte offsets asked in increasing order, counted in one pass (a
/// large note's thousands of code blocks each counted from the start was quadratic).
struct Scalars<'a> {
    text: &'a str,
    byte: usize,
    count: usize,
}

impl<'a> Scalars<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, byte: 0, count: 0 }
    }

    fn at(&mut self, at: usize) -> usize {
        if at < self.byte {
            self.byte = 0;
            self.count = 0;
        }
        self.count += self.text.get(self.byte..at).map_or(0, |part| part.chars().count());
        self.byte = at.max(self.byte);
        self.count
    }
}

impl NoteDocument {
    /// Takes in a change the person accepts whole (a merged draft, a restored state; `before`: the
    /// selection before it). It may touch several places at once, so the selection, folds and undo
    /// records move with it as with another person's change ([`Editor::follow_remote`]), not as one
    /// span. When it is a step of the person's Undo (`persons_step`) the editor records it, so
    /// Undo puts back the selection from before it and the steps below keep theirs (unmoved: they
    /// are undone on the text before it); otherwise the
    /// records stay the replica's last steps, as after another person's change.
    pub(crate) fn took_in(&mut self, damage: &kition_kit::session::Damage, before: (usize, usize), persons_step: bool) {
        let start = text_hash(self.editor.text());
        let revision = self.revision;
        let stale = self.editor.done.last().is_some_and(|record| record.revision != revision);
        let text = self.session.model().body().to_owned();
        // The person's steps below this one stay on the text before it: Undo takes this one back
        // first, so their selections are not moved with it (another person's change moves them).
        let below = persons_step.then(|| self.editor.done.clone());
        self.editor.follow_remote(&text);
        if let Some(below) = below {
            self.editor.done = below;
        }
        self.changed(damage);
        if *damage == kition_kit::session::Damage::None {
            return;
        }
        if !persons_step {
            self.editor.keep_records(revision, self.revision);
            return;
        }
        if stale {
            self.editor.done.clear();
        }
        if self.editor.done.len() >= RECORDS {
            self.editor.done.remove(0);
        }
        self.editor.undone.clear();
        let after = (self.editor.anchor, self.editor.focus);
        self.editor.done.push(Record { before, after, revision: self.revision, start, empty: false });
    }

    /// The note's text, shared until it changes (see `shared_text`).
    pub(crate) fn editor_text(&self) -> std::sync::Arc<str> {
        if let Some((revision, text)) = self.shared_text.borrow().as_ref()
            && *revision == self.revision
        {
            return text.clone();
        }
        let text: std::sync::Arc<str> = std::sync::Arc::from(self.body());
        *self.shared_text.borrow_mut() = Some((self.revision, text.clone()));
        text
    }

    /// Replaces the bytes `from..to` with `insert` as the local person, and puts the caret after
    /// what was inserted (or selects it with `select`).
    fn replace(&mut self, from: usize, to: usize, insert: &str, select: bool) -> Result<(), String> {
        let (start, end) = self.transact(from, to, insert)?;
        self.editor.anchor = if select { start } else { end };
        self.editor.focus = end;
        self.editor.goal = None;
        Ok(())
    }

    /// Replaces the bytes `from..to` with `insert` as the local person, in the undo step it
    /// joins; answers where the inserted text is now.
    fn transact(&mut self, from: usize, to: usize, insert: &str) -> Result<(usize, usize), String> {
        self.editor.row_end = false;
        self.editor.selections.edited = true;
        let text = self.body();
        let (from, to) = (from.min(text.len()), to.min(text.len()));
        let params = json!({ "from": scalar(text, from), "to": scalar(text, to), "text": insert });
        let before = text.len();
        self.change(from, to, |doc| doc.execute("text.replace", params).map(|_| ()))?;
        let inserted = (self.body().len() + (to - from)).saturating_sub(before);
        if let Some(step) = &mut self.editor.step {
            step.ranges.push((from, from + inserted));
        }
        if self.editor.renumbering {
            return Ok((from, from + inserted));
        }
        // The ordered lists the change is in count on again (as Obsidian renumbers after a change);
        // the inserted text's place follows those edits.
        self.editor.marks = vec![from, from + inserted];
        self.renumber_changed(from, from + inserted)?;
        let marks = std::mem::take(&mut self.editor.marks);
        Ok((marks.first().copied().unwrap_or(from), marks.get(1).copied().unwrap_or(from + inserted)))
    }

    /// After a change of `from..to`: the ordered list item on each changed line and on the line
    /// after them takes the number after its previous sibling's, and the items after it count on
    /// (measured in Obsidian: a list's first item keeps its number; a blank line or a deeper line
    /// does not end the list, a parent or a paragraph after a blank line does).
    fn renumber_changed(&mut self, from: usize, to: usize) -> Result<(), String> {
        let text = self.editor_text();
        let (first, _) = line_around(&text, from);
        let (_, last_end) = line_around(&text, to);
        let next_end = if last_end < text.len() { line_around(&text, last_end + 1).1 } else { last_end };
        // Nothing to do unless an ordered item is among those lines.
        if !text.get(first..next_end).unwrap_or("").split('\n').any(|line| ordered_item(line).is_some()) {
            return Ok(());
        }
        let first_line = text.get(..first).map_or(0, |before| before.matches('\n').count());
        let lines = text.get(first..next_end).map_or(1, |part| part.matches('\n').count() + 1);
        self.editor.renumbering = true;
        let batch = self.editor.batch.replace(true);
        let done = (first_line..first_line + lines).try_for_each(|line| self.renumber_from(line));
        self.editor.batch = batch;
        self.editor.renumbering = false;
        done
    }

    /// Renumbers the ordered item on line `index` (from 0) after its previous sibling, and the
    /// siblings after it.
    fn renumber_from(&mut self, index: usize) -> Result<(), String> {
        let text = self.editor_text();
        let lines: Vec<&str> = text.split('\n').collect();
        let Some(item) = lines.get(index).and_then(|line| ordered_item(line)) else { return Ok(()) };
        // In a code block a numbered line is code.
        let start_of = |line: usize| lines.iter().take(line).map(|part| part.len() + 1).sum::<usize>();
        if self.view().styles().lines.get(index).is_some_and(|style| matches!(style.block, kition_note_view::style::Block::Code { .. })) {
            return Ok(());
        }
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let mut number = match previous_sibling(&lines, index, item.indent) {
            Some(previous) => previous + 1,
            None => item.number,
        };
        if number != item.number {
            let at = start_of(index) + item.digits;
            edits.push((at, at + item.digits_len, number.to_string()));
        }
        let mut line = index + 1;
        while let Some(current) = lines.get(line) {
            match sibling_step(&lines, line, item.indent) {
                Walk::Sibling => {
                    let Some(found) = ordered_item(current) else { break };
                    number += 1;
                    if found.number != number {
                        let at = start_of(line) + found.digits;
                        edits.push((at, at + found.digits_len, number.to_string()));
                    }
                }
                Walk::Inside => {}
                Walk::End => break,
            }
            line += 1;
        }
        for (from, to, digits) in edits.into_iter().rev() {
            self.transact(from, to, &digits)?;
        }
        Ok(())
    }

    /// Runs `edit`, a change of the bytes `from..to` by the local person, in the undo step it
    /// joins (see [`Join`]).
    fn change<T>(&mut self, from: usize, to: usize, edit: impl FnOnce(&mut Self) -> Result<T, String>) -> Result<T, String> {
        self.take_step(from, to);
        self.editor.inside = true;
        let done = edit(self);
        self.editor.inside = false;
        let revision = self.revision;
        if let Some(step) = &mut self.editor.step {
            step.last = crate::clock::now_millis();
        }
        if let Some(record) = self.editor.done.last_mut() {
            record.revision = revision;
        }
        done
    }

    /// Before a change of `from..to`: the open undo step goes on when the change joins it (see
    /// [`Join`]), else it ends and another opens.
    fn take_step(&mut self, from: usize, to: usize) {
        match self.editor.batch {
            Some(true) => return,
            Some(false) => self.editor.batch = Some(true),
            None => {}
        }
        let join = std::mem::take(&mut self.editor.join);
        let now = crate::clock::now_millis();
        let joins = join == Join::Adjacent
            && self.editor.step.as_ref().is_some_and(|step| now - step.last < JOIN_DELAY && step.ranges.iter().any(|(a, b)| to >= *a && from <= *b));
        if joins {
            return;
        }
        let before = self.editor.batch_before.take().unwrap_or((self.editor.anchor, self.editor.focus));
        self.stop_typing();
        let grouped = self.session.begin_undo_group().is_ok();
        self.editor.step = Some(Step { ranges: Vec::new(), last: now, grouped });
        // The steps before are the replica's only while nothing else changed the note since.
        let revision = self.revision;
        if self.editor.done.last().is_some_and(|record| record.revision != revision) {
            self.editor.done.clear();
        }
        if self.editor.done.len() >= RECORDS {
            self.editor.done.remove(0);
        }
        self.editor.undone.clear();
        let start = text_hash(self.body());
        self.editor.done.push(Record { before, after: before, revision, start, empty: false });
    }

    /// Ends the open undo step: the next change is a step of its own (a move, a click, a command
    /// from elsewhere). The step keeps the selection it ends with, for Redo.
    pub(crate) fn stop_typing(&mut self) {
        if let Some(step) = self.editor.step.take() {
            if step.grouped {
                self.session.end_undo_group();
            }
            let selection = (self.editor.anchor, self.editor.focus);
            let now = text_hash(self.session.model().body());
            if let Some(record) = self.editor.done.last_mut()
                && record.revision == self.revision
            {
                record.after = selection;
                record.empty = record.start == now;
            }
        }
    }

    /// Runs `edit` (several changes) as one change: its first change takes the undo step, and
    /// the rest go in it.
    fn grouped<T>(&mut self, edit: impl FnOnce(&mut Self) -> Result<T, String>) -> Result<T, String> {
        if self.editor.batch.is_some() {
            return edit(self);
        }
        self.editor.batch = Some(false);
        self.editor.batch_before = Some((self.editor.anchor, self.editor.focus));
        let done = edit(self);
        self.editor.batch = None;
        self.editor.batch_before = None;
        self.editor.join = Join::Adjacent;
        done
    }

    /// Puts back the selection an undo step had before it (`undo`) or after it (Redo), when the
    /// step undone or redone was the editor's; answers whether it was.
    pub(crate) fn restore_selection(&mut self, undo: bool, record: Option<Record>) -> bool {
        let Some(record) = record else {
            self.editor.done.clear();
            self.editor.undone.clear();
            return false;
        };
        let revision = self.revision;
        let (from, to) = if undo { (&mut self.editor.done, &mut self.editor.undone) } else { (&mut self.editor.undone, &mut self.editor.done) };
        from.pop();
        // The step below is the replica's last one again.
        if let Some(below) = from.last_mut() {
            below.revision = revision;
        }
        to.push(Record { revision, ..record });
        let (anchor, focus) = if undo { record.before } else { record.after };
        let anchor = self.view().clamp(anchor);
        let focus = self.view().clamp(focus);
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        self.editor.goal = None;
        true
    }

    /// Undo or Redo of a step that changed nothing: the replica has no step for it, so it changes
    /// nothing either (the step goes to the other list). Answers whether it took one.
    pub(crate) fn skip_empty(&mut self, undo: bool) -> bool {
        if !self.next_record(undo).is_some_and(|record| record.empty) {
            return false;
        }
        let (from, to) = if undo { (&mut self.editor.done, &mut self.editor.undone) } else { (&mut self.editor.undone, &mut self.editor.done) };
        if let Some(record) = from.pop() {
            to.push(record);
        }
        true
    }

    /// The editor's record of the step Undo (`undo`) or Redo would take, when it is still the
    /// replica's last one.
    pub(crate) fn next_record(&self, undo: bool) -> Option<Record> {
        let list = if undo { &self.editor.done } else { &self.editor.undone };
        list.last().copied().filter(|record| record.revision == self.revision)
    }

    /// Folds or unfolds the heading or list item starting at byte `at`.
    fn toggle_fold(&mut self, at: usize) {
        if let Some(index) = self.editor.folded.iter().position(|fold| *fold == at) {
            self.editor.folded.remove(index);
        } else {
            self.editor.folded.push(at);
            self.editor.folded.sort_unstable();
            // The caret leaves the section it folds, for the heading's line (at the fold's end it
            // stays, drawn at the line's end).
            let at_end = self.fold_spans().iter().any(|(_, end)| *end == self.editor.focus);
            if !at_end && self.view().is_hidden(self.editor.focus) {
                let text = self.editor_text();
                let (_, end) = line_around(&text, at);
                self.stop_typing();
                self.editor.anchor = end;
                self.editor.focus = end;
            }
        }
    }

    /// Moves the selection out of the front matter, to the line after it: the front matter is the
    /// properties block, edited in it, as Obsidian's live preview keeps the caret out of it.
    pub(crate) fn leave_front_matter(&mut self) {
        let after = {
            let view = self.view();
            let styles = view.styles();
            let count = styles
                .lines
                .iter()
                .take_while(|line| matches!(line.block, kition_note_view::style::Block::Frontmatter | kition_note_view::style::Block::Property))
                .count();
            if count < 2 {
                return;
            }
            styles.starts.get(count).copied().unwrap_or(view.text().len())
        };
        if self.editor.focus < after {
            self.editor.focus = after;
        }
        if self.editor.anchor < after {
            self.editor.anchor = after;
        }
    }

    /// Each folded line's end and the end of what its fold hides (the section's last character):
    /// Obsidian's fold is one range between them, and its end is a caret stop drawn at the folded
    /// line's end.
    pub(crate) fn fold_spans(&self) -> Vec<(usize, usize)> {
        let text = self.editor_text();
        let view = self.view();
        let mut spans = Vec::new();
        for fold in &self.editor.folded {
            let (_, line_end) = line_around(&text, (*fold).min(text.len()));
            let mut end = line_end;
            let mut at = line_end + 1;
            while at <= text.len() && view.is_hidden(at) {
                end = line_around(&text, at).1;
                at = end + 1;
            }
            if end > line_end {
                spans.push((line_end, end));
            }
        }
        spans
    }

    /// Where the view draws byte `at`: a fold's end at its folded line's end.
    pub(crate) fn shown_at(&self, at: usize) -> usize {
        self.fold_spans().into_iter().find(|(_, end)| *end == at).map_or(at, |(line_end, _)| line_end)
    }

    /// A folded line's end, as the caret reaches it from the line's text (End, Down past the
    /// text): the fold's end, after what it hides.
    fn past_fold(&self, at: usize) -> usize {
        self.fold_spans().into_iter().find(|(line_end, _)| *line_end == at).map_or(at, |(_, end)| end)
    }

    /// Unfolds what hides the caret (a search hit, a reveal, a move into a folded section); a
    /// caret at a fold's end stays out of it.
    pub(crate) fn unfold_at_caret(&mut self) {
        self.leave_front_matter();
        if self.fold_spans().iter().any(|(_, end)| *end == self.editor.focus) {
            return;
        }
        for _ in 0..64 {
            if !self.view().is_hidden(self.editor.focus) {
                return;
            }
            let focus = self.editor.focus;
            let Some(index) = self.editor.folded.iter().rposition(|fold| *fold < focus) else { return };
            self.editor.folded.remove(index);
        }
    }

    /// Moves the caret (extending the selection with `extend`).
    fn move_to(&mut self, at: usize, extend: bool) {
        self.stop_typing();
        self.editor.row_end = false;
        self.record_selection();
        let at = self.view().clamp(at);
        self.editor.focus = at;
        if !extend {
            self.editor.anchor = at;
        }
    }

    /// Typed or pasted text at the selection.
    fn type_text(&mut self, text: &str) -> Result<(), String> {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if text.is_empty() || self.is_read_only() {
            return Ok(());
        }
        if self.type_paired(&text)? {
            return Ok(());
        }
        // Pasted lines are a step of their own (CodeMirror's paste), which typing after them joins.
        if text.contains('\n') {
            self.editor.join = Join::Fresh;
        }
        let (from, to) = self.editor.ordered();
        let body = self.editor_text();
        let from = if from == to { from } else { orphan_start(&body, from, to) };
        self.replace(from, to, &text, false)
    }

    /// Obsidian's typing helps: an opening bracket or quote types its closer too (when the caret
    /// is before a space, a closer, or the line's end), a closer steps over the one typed with
    /// its opener, a Markdown mark or a bracket typed over a selection wraps it, and an address
    /// pasted over a selection links it. Answers whether it handled `typed`.
    fn type_paired(&mut self, typed: &str) -> Result<bool, String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let selected = text.get(from..to).unwrap_or("").to_owned();
        if from != to && is_address(typed) && !selected.contains('\n') {
            // The link is selected (as Obsidian selects it), an address over an address too.
            let link = format!("[{selected}]({})", typed.trim());
            self.replace(from, to, &link, true)?;
            return Ok(true);
        }
        let mut chars = typed.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else { return Ok(false) };
        if from != to {
            let Some(close) = closer_of(c).or_else(|| WRAPS.contains(&c).then_some(c)) else { return Ok(false) };
            // Over several lines a bracket or a quote types its pair in place of the selection; a
            // backtick and the Markdown marks wrap it (measured in Obsidian).
            if selected.contains('\n') && !WRAPS.contains(&c) && c != '`' {
                self.replace(from, to, &format!("{c}{close}"), false)?;
                let inside = from + c.len_utf8();
                self.editor.anchor = inside;
                self.editor.focus = inside;
                self.editor.pairs.push(inside);
                return Ok(true);
            }
            self.replace(from, to, &format!("{c}{selected}{close}"), false)?;
            self.editor.anchor = from + c.len_utf8();
            self.editor.focus = self.editor.anchor + selected.len();
            return Ok(true);
        }
        let next = text.get(from..).and_then(|after| after.chars().next());
        let previous = text.get(..from).and_then(|before| before.chars().next_back());
        if next == Some(c) && self.editor.pairs.contains(&from) && PAIRS.iter().any(|(_, close)| *close == c) {
            self.editor.pairs.retain(|at| *at != from);
            // Stepping over moves the caret only, which ends the undo step (as in Obsidian).
            self.stop_typing();
            self.editor.anchor = from + c.len_utf8();
            self.editor.focus = self.editor.anchor;
            return Ok(true);
        }
        // A third backtick opens a code block: its closing fence goes on the next line, under the
        // line's list or quote prefix, before the rest of the line (as Obsidian closes it).
        let before = text.get(..from).unwrap_or("");
        if c == '`' && before.ends_with("``") && !before.ends_with("```") {
            let (line_start, line_end) = line_around(&text, from);
            let line = text.get(line_start..line_end).unwrap_or("");
            let under: String = match continuation(line) {
                Some((used, _)) if from >= line_start + used => {
                    line.get(..used).unwrap_or("").chars().map(|c| if c == '>' || c == '\t' { c } else { ' ' }).collect()
                }
                _ => String::new(),
            };
            self.replace(from, to, &format!("`\n{under}```"), false)?;
            self.editor.anchor = from + 1;
            self.editor.focus = from + 1;
            return Ok(true);
        }
        let Some(close) = closer_of(c) else { return Ok(false) };
        let before_space = next.is_none_or(|next| next.is_whitespace() || matches!(next, ')' | ']' | '}'));
        // A quote or a Markdown mark pairs only at a word's start (after a space or the line's
        // start): after a letter it is an apostrophe or an emphasis's end, after punctuation a
        // closer (measured in Obsidian). Brackets pair anywhere.
        let quote = matches!(c, '"' | '\'' | '`' | '*' | '_');
        let word_start = previous.is_none_or(char::is_whitespace);
        if !before_space || (quote && !word_start) {
            return Ok(false);
        }
        self.replace(from, to, &format!("{c}{close}"), false)?;
        let inside = from + c.len_utf8();
        self.editor.anchor = inside;
        self.editor.focus = inside;
        self.editor.pairs.push(inside);
        Ok(true)
    }

    /// Moves the lines the selection touches up or down past the next line, the selection with them.
    fn move_lines(&mut self, up: bool) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (start, _) = line_around(&text, from);
        let (_, end) = line_around(&text, if to > from && text.get(..to).is_some_and(|before| before.ends_with('\n')) { to - 1 } else { to });
        let block = text.get(start..end).unwrap_or("").to_owned();
        let (range, moved, shift): (std::ops::Range<usize>, String, isize) = if up {
            if start == 0 {
                return Ok(());
            }
            let (other, _) = line_around(&text, start - 1);
            let above = text.get(other..start - 1).unwrap_or("");
            (other..end, format!("{block}\n{above}"), -((start - other) as isize))
        } else {
            if end >= text.len() {
                return Ok(());
            }
            let (_, other_end) = line_around(&text, end + 1);
            let below = text.get(end + 1..other_end).unwrap_or("");
            (start..other_end, format!("{below}\n{block}"), (other_end - end) as isize)
        };
        let (anchor, focus) = (self.editor.anchor, self.editor.focus);
        self.grouped(|doc| doc.replace(range.start, range.end, &moved, false))?;
        let place = |at: usize| at.saturating_add_signed(shift);
        self.editor.anchor = place(anchor);
        self.editor.focus = place(focus);
        Ok(())
    }

    /// Checks or unchecks the task on each line the selection touches; a list item becomes a
    /// task, and a plain line an unchecked task item (Obsidian's Toggle checkbox status).
    fn toggle_checkbox(&mut self) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (start, _) = line_around(&text, from);
        let (_, end) = line_around(&text, to);
        let lines: Vec<(usize, String)> = {
            let mut at = start;
            text.get(start..end)
                .unwrap_or("")
                .split('\n')
                .map(|line| {
                    let found = (at, line.to_owned());
                    at += line.len() + 1;
                    found
                })
                .collect()
        };
        // Each line: where its text starts after the list marker (None: no item), and its
        // task box's status character.
        let parsed: Vec<(usize, Option<usize>, Option<char>)> = lines
            .iter()
            .map(|(line_start, line)| {
                // In a quote, the task goes after the `>`.
                let quote = quote_prefix(line);
                let indent = quote + (line.len() - quote - line.get(quote..).map_or(0, |rest| rest.trim_start().len()));
                let body = line.get(indent..).unwrap_or("");
                let marker = ["- ", "* ", "+ "].iter().find(|marker| body.starts_with(**marker)).map(|marker| marker.len()).or_else(|| {
                    let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                    (digits > 0 && (body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")))).then_some(digits + 2)
                });
                let status = marker.and_then(|length| {
                    let after = body.get(length..).unwrap_or("");
                    let mut chars = after.chars();
                    match (chars.next(), chars.next(), chars.next(), chars.next()) {
                        (Some('['), Some(status), Some(']'), None | Some(' ')) => Some(status),
                        _ => None,
                    }
                });
                (line_start + indent, marker, status)
            })
            .collect();
        // Measured in Obsidian: when every line is a task, all are checked unless none is open
        // (then all are opened; a custom status counts as checked); else each line goes a step:
        // text becomes a task, a list item gets a box, and a task loses its box.
        let tasks: Vec<_> = parsed.iter().zip(&lines).filter(|(_, (_, line))| !line.trim().is_empty()).map(|(found, _)| found).collect();
        let all_tasks = !tasks.is_empty() && tasks.iter().all(|(_, _, status)| status.is_some());
        let check = tasks.iter().any(|(_, _, status)| *status == Some(' '));
        let (anchor, focus) = (self.editor.anchor, self.editor.focus);
        let mut moved: Vec<(usize, isize)> = Vec::new();
        self.grouped(|doc| {
            for ((at, marker, status), (_, line)) in parsed.iter().zip(&lines).rev() {
                if lines.len() > 1 && line.trim().is_empty() {
                    continue;
                }
                match (marker, status) {
                    (Some(length), Some(status)) if all_tasks => {
                        let at = at + length + 1;
                        let wanted = if check { "x" } else { " " };
                        doc.replace(at, at + status.len_utf8(), wanted, false)?;
                    }
                    (Some(length), Some(status)) => {
                        // `[x] ` goes, the item stays.
                        let box_at = at + length;
                        let close = box_at + 1 + status.len_utf8() + 1;
                        let gone = close - box_at + usize::from(doc.body().get(close..).is_some_and(|rest| rest.starts_with(' ')));
                        doc.replace(box_at, box_at + gone, "", false)?;
                        moved.push((box_at, -(gone as isize)));
                    }
                    (Some(length), None) => {
                        let box_at = at + length;
                        doc.replace(box_at, box_at, "[ ] ", false)?;
                        moved.push((box_at, 4));
                    }
                    (None, _) => {
                        doc.replace(*at, *at, "- [ ] ", false)?;
                        moved.push((*at, 6));
                    }
                }
            }
            Ok(())
        })?;
        let place = |offset: usize| offset.saturating_add_signed(moved.iter().filter(|(at, _)| *at <= offset).map(|(_, by)| *by).sum());
        self.editor.anchor = place(anchor);
        self.editor.focus = place(focus);
        Ok(())
    }

    /// Indents (or outdents) the lines the selection touches, wherever the caret is in them.
    fn indent_selection(&mut self, outdent: bool) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (line_start, _) = line_around(&text, from);
        self.grouped(|doc| doc.indent_lines(line_start, to, outdent))
    }

    /// Backspace (`forward`: Delete): the selection, or what `unit` reaches from the caret.
    fn delete(&mut self, forward: bool, mods: ToolMods) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        // In a table cell, Backspace at the cell text's start and Delete at its end do nothing:
        // the pipes and the padding stay (as Obsidian's cell editor keeps them).
        if from == to
            && let Some((table, row, column)) = self.cell_at(from)
        {
            let content = self
                .view()
                .index_tables()
                .get(table)
                .and_then(|found| found.rows.get(row))
                .and_then(|cells| cells.cells.get(column))
                .map(|cell| cell.content.clone());
            if content.is_some_and(|content| if forward { from >= content.end } else { from <= content.start }) {
                return Ok(());
            }
        }
        // Delete at a folded line's end takes what the fold hides (as Obsidian's atomic fold).
        if forward && from == to && text.get(from..).is_some_and(|rest| rest.starts_with('\n')) && self.view().is_hidden(from + 1) {
            let mut end = from + 1;
            while end < text.len() && self.view().is_hidden(end) {
                end = line_around(&text, end).1 + 1;
            }
            let end = end.saturating_sub(1).min(text.len());
            if end > from {
                return self.replace(from, end, "", false);
            }
        }
        // Backspace at a fold's end takes what the fold hides (Obsidian's fold is one range).
        if !forward
            && from == to
            && let Some((line_end, _)) = self.fold_spans().into_iter().find(|(_, end)| *end == from)
        {
            return self.replace(line_end, from, "", false);
        }
        let (from, to) = if from != to {
            (orphan_start(&text, from, to), to)
        } else if forward && mods.command && !mods.alt {
            // Cmd+Delete: to the row's end (the line break at its end), as Obsidian's.
            let end = self.view().row_edges(from).1;
            (from, if end > from { end } else { next_grapheme(&text, from) })
        } else if forward {
            let to = if mods.alt || (mods.control && !mods.command) { delete_group(&text, from, true) } else { next_grapheme(&text, from) };
            (from, to)
        } else if mods.command {
            (self.view().row_edges(from).0, from)
        } else if mods.alt || mods.control {
            (delete_group(&text, from, false), from)
        } else if let (Some(open), Some(next)) =
            (text.get(..from).and_then(|before| before.chars().next_back()), text.get(from..).and_then(|after| after.chars().next()))
            && closer_of(open) == Some(next)
        {
            // Between an empty pair, Backspace takes both.
            (from - open.len_utf8(), from + next.len_utf8())
        } else {
            // A character back, as Obsidian's (a list marker goes a character at a time too).
            (previous_grapheme(&text, from), from)
        };
        if from == to {
            return Ok(());
        }
        self.replace(from, to, "", false)
    }

    /// Enter: a new line that goes on with the list or quote it is in; on an empty item, the
    /// item's marker is taken away instead (the list ends).
    fn enter(&mut self) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        // A step of its own (with the renumbering), which the typing after it joins.
        self.editor.join = Join::Fresh;
        self.grouped(Self::enter_now)
    }

    fn enter_now(&mut self) -> Result<(), String> {
        if let Some(cell) = self.cell_at(self.editor.focus)
            && self.cell_at(self.editor.anchor) == Some(cell)
            && self.enter_in_table(cell)?
        {
            return Ok(());
        }
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        // Over a selection, Enter is a plain new line (as Obsidian's).
        if from != to {
            return self.replace(from, to, "\n", false);
        }
        let (line_start, line_end) = line_around(&text, from);
        let line = text.get(line_start..line_end).unwrap_or("");
        let inside_code = self
            .view()
            .styles()
            .lines
            .get(self.view().styles().line_of(from))
            .is_some_and(|style| matches!(style.block, kition_note_view::style::Block::Code { .. } | kition_note_view::style::Block::Indented));
        // On an item's continuation line (its text under the item's), Enter starts the next item
        // (as Obsidian's).
        if !inside_code
            && from == to
            && let Some(next) = owner_item_next(&text, line_start)
        {
            self.replace(from, to, &format!("\n{next}"), false)?;
            return Ok(());
        }
        if !inside_code && let Some((used, next)) = continuation(line) {
            if line.len() == used && from == line_end {
                // In a quote, the item's marker and indentation come after the quote's `>`.
                let quote = quote_prefix(line);
                let inner = line.get(quote..).unwrap_or("");
                let indent = inner.len() - inner.trim_start_matches([' ', '\t']).len();
                let list = !inner.trim().is_empty();
                if indent > 0 && list {
                    // An empty nested item goes out a level.
                    let remove = if inner.starts_with('\t') { 1 } else { indent.min(4) };
                    let at = line_start + quote;
                    return self.replace(at, at + remove, "", false).map(|()| {
                        self.editor.anchor = line_end - remove;
                        self.editor.focus = line_end - remove;
                    });
                }
                if !list {
                    // An empty quote line: the quote ends, after a blank line.
                    return self.replace(line_start, line_end, "\n", false);
                }
                // An empty item: the list ends; in a quote, the quote goes on (as Obsidian's).
                let keep = line.get(..quote).unwrap_or("").to_owned();
                return self.replace(line_start, line_end, &keep, false);
            }
            if from >= line_start + used {
                self.replace(from, to, &format!("\n{next}"), false)?;
                return self.renumber_after(from + 1);
            }
        }
        // In a code block, the new line keeps the line's indentation.
        let indent: String = if inside_code { line.chars().take_while(|c| *c == ' ' || *c == '\t').collect() } else { String::new() };
        // Before a mixed-ending note's `\r\n`, the new line goes after the `\r`, so the line keeps its ending.
        if from == to && text.get(to..).is_some_and(|rest| rest.starts_with("\r\n")) {
            return self.replace(to + 1, to + 1, &format!("\n{indent}"), false);
        }
        self.replace(from, to, &format!("\n{indent}"), false)
    }

    /// After a numbered item was added on the line starting at `line_start`, the items after it
    /// at its indentation are numbered on from it (as Obsidian renumbers).
    fn renumber_after(&mut self, line_start: usize) -> Result<(), String> {
        let text = self.editor_text();
        let parse = |line: &str| {
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            let body = line.get(indent..)?;
            let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let after = body.get(digits..)?;
            (digits > 0 && (after.starts_with(". ") || after.starts_with(") ")))
                .then(|| (indent, body.get(..digits).and_then(|n| n.parse::<u64>().ok()), digits))
        };
        let (_, line_end) = line_around(&text, line_start);
        let Some((indent, Some(mut number), _)) = text.get(line_start..line_end).and_then(parse) else { return Ok(()) };
        let mut at = line_end + 1;
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        while at <= text.len() {
            let (start, end) = line_around(&text, at);
            let line = text.get(start..end).unwrap_or("");
            let line_indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            match parse(line) {
                Some((found, Some(current), digits)) if found == indent => {
                    number += 1;
                    if current != number {
                        edits.push((start + indent, start + indent + digits, number.to_string()));
                    }
                }
                // Deeper items and their lines belong to an item; anything else ends the list.
                _ if line_indent > indent && !line.trim().is_empty() => {}
                _ => break,
            }
            if end >= text.len() {
                break;
            }
            at = end + 1;
        }
        let (anchor, focus) = (self.editor.anchor, self.editor.focus);
        for (from, to, number) in edits.into_iter().rev() {
            self.replace(from, to, &number, false)?;
        }
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        Ok(())
    }

    /// Shift+Enter: a new line in the same item, its text under the item's text (the marker's
    /// width in spaces), as Obsidian's; elsewhere a plain new line.
    fn shift_enter(&mut self) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        self.editor.join = Join::Fresh;
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (line_start, line_end) = line_around(&text, from);
        let line = text.get(line_start..line_end).unwrap_or("");
        let under: String = match continuation(line) {
            Some((used, _)) if from >= line_start + used => {
                line.get(..used).unwrap_or("").chars().map(|c| if c == '>' || c == '\t' { c } else { ' ' }).collect()
            }
            _ => String::new(),
        };
        self.replace(from, to, &format!("\n{under}"), false)
    }

    /// Alt+Up and Alt+Down (macOS): the caret to its line's start or end, or the line before's
    /// start or the line after's end when already there, as Obsidian moves by paragraphs.
    fn paragraph_move(&mut self, up: bool, extend: bool) {
        let text = self.editor_text();
        let focus = self.editor.focus;
        let (start, end) = line_around(&text, focus);
        let target = if up {
            if focus > start { start } else { line_around(&text, start.saturating_sub(1)).0 }
        } else if focus < end {
            end
        } else {
            line_around(&text, (end + 1).min(text.len())).1
        };
        self.move_to(target, extend);
    }

    /// Cmd+D: the lines the selection touches deleted (Obsidian's Delete paragraph).
    fn delete_paragraph(&mut self) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (start, _) = line_around(&text, from);
        let (_, end) = line_around(&text, to);
        let (start, end) = if end < text.len() { (start, end + 1) } else { (start.saturating_sub(usize::from(start > 0)), end) };
        // The caret goes down a line, keeping its column (as CodeMirror's Delete line moves it).
        let below = {
            let view = self.view();
            let focus = self.editor.focus;
            view.vertical(focus, view.caret(focus).x, 1)
        };
        self.replace(start, end, "", false)?;
        if below >= end {
            let at = below - (end - start);
            self.editor.anchor = at;
            self.editor.focus = at;
        }
        Ok(())
    }

    /// Cmd+/: the selection in a comment (`%%…%%`), or out of one; with no selection, the
    /// comment the caret is in comes away, else the word at the caret goes in one, selected (an
    /// empty comment around the caret with no word), as Obsidian's.
    fn toggle_comment(&mut self) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        if from == to {
            let (line_start, line_end) = line_around(&text, from);
            let line = text.get(line_start..line_end).unwrap_or("");
            let caret = from - line_start;
            let marks: Vec<usize> = line.match_indices("%%").map(|(at, _)| at).collect();
            // The comment around the caret: a pair of marks (counted from the line's start).
            let pairs = marks.as_chunks::<2>().0;
            if let Some([open, close]) = pairs.iter().find(|[open, close]| open + 2 <= caret && caret <= *close) {
                let (open, close) = (line_start + open, line_start + close);
                self.grouped(|doc| {
                    doc.replace(close, close + 2, "", false)?;
                    doc.replace(open, open + 2, "", false)
                })?;
                self.editor.anchor = from - 2;
                self.editor.focus = from - 2;
                return Ok(());
            }
            let (start, end) = word_at(&text, from);
            let word = text.get(start..end).unwrap_or("");
            if start < end && word.chars().all(|c| group_of(c) == Group::Word) {
                let word = word.to_owned();
                self.replace(start, end, &format!("%%{word}%%"), false)?;
                self.editor.anchor = start + 2;
                self.editor.focus = start + 2 + word.len();
                return Ok(());
            }
        }
        let inner = text.get(from..to).unwrap_or("").to_owned();
        let wrapped = from >= 2 && text.get(from - 2..from) == Some("%%") && text.get(to..to + 2) == Some("%%");
        if wrapped {
            self.replace(from - 2, to + 2, &inner, false)?;
            self.editor.anchor = from - 2;
            self.editor.focus = to - 2;
            return Ok(());
        }
        self.replace(from, to, &format!("%%{inner}%%"), false)?;
        self.editor.anchor = from + 2;
        self.editor.focus = to + 2;
        Ok(())
    }

    /// Tab and Shift+Tab: a list item (or every selected line) goes in or out a level; elsewhere
    /// Tab types a tab.
    /// The table cell the caret is in: (table index, row with 0 the header, column from 0).
    fn cell_at(&self, at: usize) -> Option<(usize, usize, usize)> {
        let view = self.view();
        view.index_tables().iter().enumerate().find_map(|(index, table)| {
            table.rows.iter().enumerate().find_map(|(row, cells)| {
                cells
                    .cells
                    .iter()
                    .position(|cell| cell.segment.start <= at && at <= cell.segment.end)
                    .filter(|_| cells.line.start <= at && at <= cells.line.end)
                    .map(|column| (index, row, column))
            })
        })
    }

    /// Selects a table cell's text (an empty cell: the caret before its closing pipe), as
    /// Obsidian's table editor does going to a cell. A row shorter than the table gets the empty
    /// cells it lacks, on its own line only.
    fn select_cell(&mut self, table: usize, row: usize, column: usize) -> Result<bool, String> {
        let found = self.view().index_tables().get(table).and_then(|found| found.rows.get(row)).map(|cells| {
            let last = cells.cells.last().map(|cell| cell.segment.end);
            (cells.cells.get(column).map(|cell| (cell.segment.clone(), cell.content.clone())), cells.cells.len(), last, cells.trailing_pipe)
        });
        let Some((cell, count, last, trailing)) = found else { return Ok(false) };
        if let Some((segment, content)) = cell {
            let (anchor, focus) = if content.is_empty() { (segment.end, segment.end) } else { (content.start, content.end) };
            self.editor.anchor = anchor;
            self.editor.focus = focus;
            self.editor.goal = None;
            return Ok(true);
        }
        let Some(last) = last else { return Ok(false) };
        if self.is_read_only() {
            return Ok(false);
        }
        // The missing cells after the row's last one, then the caret in the cell.
        let missing = column + 1 - count;
        let (at, insert) = if trailing { (last + 1, "  |".repeat(missing)) } else { (last, " | ".repeat(missing)) };
        self.replace(at, at, &insert, false)?;
        let cell = self
            .view()
            .index_tables()
            .get(table)
            .and_then(|found| found.rows.get(row))
            .and_then(|cells| cells.cells.get(column))
            .map(|cell| cell.segment.end);
        if let Some(end) = cell {
            self.editor.anchor = end;
            self.editor.focus = end;
        }
        Ok(true)
    }

    /// Where a table cell's text starts (an empty cell: before its closing pipe).
    fn cell_text_start(&self, (table, row, column): (usize, usize, usize)) -> Option<usize> {
        let view = self.view();
        let cell = view.index_tables().get(table)?.rows.get(row)?.cells.get(column)?;
        Some(if cell.content.is_empty() { cell.segment.end } else { cell.content.start })
    }

    /// Tab and Shift+Tab in a table: the next or previous cell (row by row), its text selected;
    /// past the last cell a new row (as Obsidian's table editor).
    fn tab_in_table(&mut self, (table, row, column): (usize, usize, usize), back: bool) -> Result<(), String> {
        let Some((columns, rows)) = self.view().index_tables().get(table).map(|found| (found.columns().max(1), found.rows.len())) else {
            return Ok(());
        };
        let flat = row * columns + column;
        let Some(next) = (if back { flat.checked_sub(1) } else { Some(flat + 1) }) else { return Ok(()) };
        if next >= rows * columns {
            let at = self.editor.focus;
            self.grouped(|doc| doc.change(at, at, |doc| doc.execute("table.insertRow", json!({ "table": table, "at": rows })).map(|_| ())))?;
        }
        self.select_cell(table, next / columns, next % columns)?;
        Ok(())
    }

    /// Enter in a table: the cell below, its text selected; a new last row past the end. In a
    /// table without outer pipes, a plain new line (Obsidian edits no such table as a table).
    fn enter_in_table(&mut self, (table, row, column): (usize, usize, usize)) -> Result<bool, String> {
        let Some((rows, piped)) =
            self.view().index_tables().get(table).map(|found| (found.rows.len(), found.rows.first().is_some_and(|header| header.leading_pipe)))
        else {
            return Ok(false);
        };
        if !piped {
            return Ok(false);
        }
        if row + 1 >= rows {
            let at = self.editor.focus;
            self.grouped(|doc| doc.change(at, at, |doc| doc.execute("table.insertRow", json!({ "table": table, "at": rows })).map(|_| ())))?;
        }
        self.select_cell(table, row + 1, column)?;
        Ok(true)
    }

    fn tab(&mut self, outdent: bool) -> Result<(), String> {
        if self.is_read_only() {
            return Ok(());
        }
        self.stop_typing();
        if let Some(cell) = self.cell_at(self.editor.focus)
            && self.cell_at(self.editor.anchor) == Some(cell)
        {
            return self.tab_in_table(cell, outdent);
        }
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (line_start, line_end) = line_around(&text, from);
        let is_list = text.get(line_start..line_end).and_then(continuation).is_some();
        // Tab with the caret in a line indents the line (Obsidian indents, it never types a tab mid-line).
        let _ = is_list;
        self.grouped(|doc| doc.indent_lines(line_start, to, outdent))
    }

    /// Indents (or outdents) every line from the one starting at `line_start` to the one holding `to`.
    fn indent_lines(&mut self, line_start: usize, to: usize, outdent: bool) -> Result<(), String> {
        let text = self.editor_text();
        let (last_start, _) = line_around(&text, to);
        let mut starts: Vec<usize> = vec![line_start];
        starts.extend(text.get(line_start..last_start).unwrap_or("").match_indices('\n').map(|(at, _)| line_start + at + 1));
        // Last first, so earlier offsets stay right.
        let (mut anchor, mut focus) = (self.editor.anchor, self.editor.focus);
        for start in starts.into_iter().rev() {
            let current = self.editor_text();
            // A quoted line is indented after its `>` (as Obsidian indents in a quote).
            let (_, end) = line_around(&current, start);
            let start = start + current.get(start..end).map_or(0, quote_prefix);
            let (edit_from, edit_to, insert): (usize, usize, &str) = if outdent {
                let line = current.get(start..).unwrap_or("");
                let remove = if line.starts_with('\t') { 1 } else { line.len() - line.trim_start_matches(' ').len().min(line.len()) };
                let remove = remove.min(if line.starts_with('\t') { 1 } else { 4 });
                if remove == 0 {
                    continue;
                }
                (start, start + remove, "")
            } else {
                (start, start, "\t")
            };
            let delta = insert.len() as isize - (edit_to - edit_from) as isize;
            self.transact(edit_from, edit_to, insert)?;
            let shift = |at: usize| {
                if at >= edit_to {
                    (at as isize + delta).max(edit_from as isize) as usize
                } else if at > edit_from {
                    edit_from
                } else {
                    at
                }
            };
            anchor = shift(anchor);
            focus = shift(focus);
        }
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        // Numbered items take their number in the list they are now in, and the lists they left
        // and joined count on (as Obsidian renumbers them).
        let text = self.editor_text();
        let first_line = text.get(..line_start).map_or(0, |before| before.matches('\n').count());
        let (last_start, _) = line_around(&text, self.editor.ordered().1.min(text.len()));
        let last_line = text.get(..last_start).map_or(0, |before| before.matches('\n').count());
        for line in first_line..=last_line + 1 {
            self.renumber_line(line)?;
        }
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        Ok(())
    }

    /// Gives the numbered item on line `index` (from 0) the number after its previous sibling's
    /// (1 when it has none), then counts on the items after it.
    fn renumber_line(&mut self, index: usize) -> Result<(), String> {
        let text = self.editor_text();
        let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1)).collect();
        let Some(&start) = starts.get(index) else { return Ok(()) };
        // In a quote, the indentation and the number come after the `>`.
        let parse = |at: usize| {
            let (from, to) = line_around(&text, at);
            let line = text.get(from..to).unwrap_or("");
            let quote = quote_prefix(line);
            let inner = line.get(quote..).unwrap_or("");
            let indent = inner.len() - inner.trim_start_matches([' ', '\t']).len();
            let body = inner.get(indent..).unwrap_or("");
            let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let numbered = digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") "));
            (
                from + quote,
                indent,
                numbered.then(|| (body.get(..digits).and_then(|n| n.parse::<u64>().ok()).unwrap_or(1), digits)),
                inner.trim().is_empty(),
            )
        };
        let (from, indent, Some((current, digits)), _) = parse(start) else { return Ok(()) };
        let mut wanted = 1;
        for line in (0..index).rev() {
            let Some(&at) = starts.get(line) else { break };
            let (_, other, number, blank) = parse(at);
            if blank {
                break;
            }
            if other < indent {
                break;
            }
            if other == indent {
                if let Some((number, _)) = number {
                    wanted = number + 1;
                }
                break;
            }
        }
        if wanted != current {
            self.replace(from + indent, from + indent + digits, &wanted.to_string(), false)?;
        }
        let (line_start, _) = line_around(&text, from);
        self.renumber_after(line_start)
    }

    /// Arrows, Home, End, Page Up and Down.
    fn navigate(&mut self, key: &str, mods: ToolMods) {
        self.editor.pairs.clear();
        let text = self.editor_text();
        let extend = mods.shift;
        let (from, to) = self.editor.ordered();
        let stored = self.editor.focus;
        // A caret at a fold's end moves as from its folded line's end (where it is drawn).
        let focus = self.shown_at(stored);
        let spans = self.fold_spans();
        let word = mods.alt || (mods.control && !mods.command);
        let target = match key {
            "ArrowLeft" if !extend && from != to => from,
            "ArrowRight" if !extend && from != to => to,
            // Over a fold in one step, as over Obsidian's folded range.
            "ArrowRight" if !word && !mods.command && spans.iter().any(|(line_end, end)| *line_end == stored && *end > stored) => {
                self.past_fold(stored)
            }
            "ArrowLeft" if !word && !mods.command && spans.iter().any(|(_, end)| *end == stored) => focus,
            "ArrowLeft" if !word && !mods.command && spans.iter().any(|(_, end)| *end + 1 == stored) => stored - 1,
            "ArrowRight" if !word && !mods.command && spans.iter().any(|(_, end)| *end == stored) => next_grapheme(&text, stored),
            "ArrowLeft" if mods.command => self.line_start_key(&text, focus),
            "ArrowRight" if mods.command => return self.line_end_key(&text, stored, extend),
            "ArrowLeft" if word => self.view().snap(by_group(&text, focus, false), false),
            "ArrowRight" if word => self.view().snap(by_group(&text, focus, true), true),
            "ArrowLeft" => self.view().snap(previous_grapheme(&text, focus), false),
            "ArrowRight" => self.view().snap(next_grapheme(&text, focus), true),
            "ArrowUp" if mods.command => 0,
            "ArrowDown" if mods.command => text.len(),
            "Home" if mods.command || mods.control => 0,
            "End" if mods.command || mods.control => text.len(),
            "Home" => self.line_start_key(&text, focus),
            "End" => return self.line_end_key(&text, stored, extend),
            "ArrowUp" | "ArrowDown" | "PageUp" | "PageDown" => {
                // At a wrapped row's end the caret is on that row, after its last character.
                let at_row_end = self.row_end_caret();
                let focus = if at_row_end.is_some() { focus.saturating_sub(1) } else { focus };
                let goal = self.editor.goal.unwrap_or_else(|| at_row_end.map_or_else(|| self.view().caret(focus).x, |caret| caret.x));
                let rows = match key {
                    "ArrowUp" => -1,
                    "ArrowDown" => 1,
                    "PageUp" => -20,
                    _ => 20,
                };
                let mut at = self.view().vertical(focus, goal, rows);
                // A page past the note's first or last row goes to its start or end (CodeMirror's).
                if rows.abs() > 1 && self.view().vertical(at, goal, rows.signum()) == at {
                    at = if rows > 0 { text.len() } else { 0 };
                }
                // Past a folded line's text, the caret goes after its fold (Obsidian's widget).
                let from_fold_end = spans.iter().any(|(_, end)| *end == stored);
                // From a fold's end the caret stays past folds (Obsidian's goal is after the fold's
                // widget); from elsewhere it stops before them.
                let at = if from_fold_end && self.view().row_edges(at).1 == at { self.past_fold(at) } else { at };
                let at = match spans.iter().find(|(_, end)| *end == at) {
                    Some((line_end, _)) if !from_fold_end => *line_end,
                    _ => at,
                };
                // Up or Down into another row of a table goes to its cell's text start (Obsidian's
                // cell editor takes the caret there); out of a table, to the line's start.
                let from_cell = self.cell_at(focus);
                let (at, goal) = match (from_cell, self.cell_at(at)) {
                    (from, Some(cell)) if from.is_none_or(|from| (from.0, from.1) != (cell.0, cell.1)) => {
                        let start = self.cell_text_start(cell).unwrap_or(at);
                        (start, self.view().caret(start).x)
                    }
                    (Some(_), None) => {
                        let start = line_around(&text, at).0;
                        (start, self.view().caret(start).x)
                    }
                    _ => (at, goal),
                };
                // Past a wrapped row's last character, the caret goes to the row's end after its
                // trailing space, drawn on that row (as CodeMirror places it).
                let (_, row_end) = self.view().row_edges(at);
                let (_, line_end) = line_around(&text, at);
                let trailing = text.get(row_end..line_end).map_or(0, |rest| rest.len() - rest.trim_start_matches(' ').len());
                let wrapped_end = at == row_end
                    && trailing > 0
                    && row_end + trailing < line_end
                    && self.cell_at(at).is_none()
                    && goal > self.view().caret(at).x + 1.0;
                self.move_to(if wrapped_end { row_end + trailing } else { at }, extend);
                self.editor.row_end = wrapped_end;
                self.editor.goal = Some(goal);
                return;
            }
            _ => return,
        };
        self.move_to(target, extend);
        self.editor.goal = None;
    }

    /// macOS's Control editing keys, as Obsidian takes them (CodeMirror's Emacs-style bindings):
    /// A and E the row's start and end, B, F, P, N, and V the caret back, forward, up, down, and a
    /// page down (Shift moves too, selecting nothing), D and H delete forward and back, K deletes to
    /// the row's end (the line break at its end; a selection), O splits the line before the caret,
    /// and T swaps the characters around the caret (the two before it at a line's end).
    fn control_key(&mut self, name: &str) -> Result<(), String> {
        let plain = ToolMods::default();
        let text = self.editor_text();
        let focus = self.editor.focus;
        match name {
            "a" => {
                let start = self.view().row_edges(focus).0;
                self.move_to(start, false);
            }
            "e" => {
                let end = self.view().row_edges(focus).1;
                self.move_to(end, false);
            }
            "b" => self.navigate("ArrowLeft", plain),
            "f" => self.navigate("ArrowRight", plain),
            "p" => self.navigate("ArrowUp", plain),
            "n" => self.navigate("ArrowDown", plain),
            "v" => self.navigate("PageDown", plain),
            "d" => self.delete(true, plain)?,
            "h" => self.delete(false, plain)?,
            // Ctrl+Alt+H: a group back, as Alt+Backspace.
            "grouph" => self.delete(false, ToolMods { alt: true, ..plain })?,
            "undoselection" => self.undo_selection(true)?,
            "redoselection" => self.undo_selection(false)?,
            "k" if !self.is_read_only() => {
                let (from, to) = self.editor.ordered();
                let to = if from != to {
                    to
                } else {
                    let end = self.view().row_edges(focus).1;
                    if end > focus { end } else { next_grapheme(&text, focus) }
                };
                if to > from {
                    self.replace(from, to, "", false)?;
                }
            }
            "o" if !self.is_read_only() => {
                let (from, to) = self.editor.ordered();
                self.editor.join = Join::Fresh;
                self.replace(from, to, "\n", false)?;
                self.editor.anchor = from;
                self.editor.focus = from;
            }
            "t" if !self.is_read_only() && self.editor.is_collapsed() => {
                let (line_start, line_end) = line_around(&text, focus);
                if focus == line_start || line_end - line_start < 2 {
                    return Ok(());
                }
                // At the line's end the two characters before the caret swap.
                let middle = if focus == line_end { previous_grapheme(&text, focus) } else { focus };
                let before = previous_grapheme(&text, middle);
                let after = next_grapheme(&text, middle);
                let swapped = format!("{}{}", text.get(middle..after).unwrap_or(""), text.get(before..middle).unwrap_or(""));
                self.replace(before, after, &swapped, false)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Before the caret moves: the selection it leaves goes in the selection history (a move soon
    /// after the one before joins it).
    fn record_selection(&mut self) {
        let now = crate::clock::now_millis();
        let current = (self.editor.anchor, self.editor.focus);
        let history = &mut self.editor.selections;
        if history.edited || now - history.last >= JOIN_DELAY || history.done.is_empty() {
            if history.done.last() != Some(&current) {
                history.done.push(current);
                if history.done.len() > RECORDS {
                    history.done.remove(0);
                }
            }
            history.undone.clear();
        }
        history.last = now;
        history.edited = false;
    }

    /// Cmd+U (`undo`) and Cmd+Shift+U, as CodeMirror's Undo and Redo selection: the selection
    /// before the last move (or after it, again); after an edit, the edit is undone (redone).
    fn undo_selection(&mut self, undo: bool) -> Result<(), String> {
        if undo && (self.editor.selections.edited || self.editor.selections.done.is_empty()) {
            self.execute("edit.undo", json!({}))?;
            self.editor.selections.edited = false;
            return Ok(());
        }
        let current = (self.editor.anchor, self.editor.focus);
        let history = &mut self.editor.selections;
        let (from, to) = if undo { (&mut history.done, &mut history.undone) } else { (&mut history.undone, &mut history.done) };
        let Some((anchor, focus)) = from.pop() else {
            if !undo {
                self.execute("edit.redo", json!({}))?;
            }
            return Ok(());
        };
        to.push(current);
        let (anchor, focus) = (self.view().clamp(anchor), self.view().clamp(focus));
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        self.editor.goal = None;
        self.editor.row_end = false;
        Ok(())
    }

    /// Where Cmd+Left and Home go: the row's start; on a list item's or a quote's first row, the
    /// start of its text first (after the marker, the task's box, the `>`), as Obsidian goes.
    fn line_start_key(&self, text: &str, focus: usize) -> usize {
        // At a wrapped row's end the caret is on that row; at a wrapped row's start a second press
        // goes on to the line's start (CodeMirror's line boundary moves).
        let visual = if self.row_end_caret().is_some() { focus.saturating_sub(1) } else { focus };
        let row = self.view().row_edges(visual).0;
        let (line_start, line_end) = line_around(text, focus);
        if row != line_start && (row != focus || self.row_end_caret().is_some()) {
            return row;
        }
        let used = text.get(line_start..line_end).and_then(continuation).map_or(0, |(used, _)| used);
        let after = line_start + used;
        if used > 0 && focus > after { after } else { line_start }
    }

    /// End and Cmd+Right, as Obsidian's: a wrapped row's end is after its trailing space (the next
    /// row's first byte, the caret drawn on this row); from there, or on a line's last row, the
    /// line's end (past a fold).
    fn line_end_key(&mut self, text: &str, stored: usize, extend: bool) {
        let at_row_end = self.row_end_caret().is_some();
        let visual = if at_row_end { stored.saturating_sub(1) } else { self.shown_at(stored) };
        let (_, end) = self.view().row_edges(visual);
        let (_, line_end) = line_around(text, visual);
        let trailing = text.get(end..line_end).map_or(0, |rest| rest.len() - rest.trim_start_matches(' ').len());
        // A table cell's row ends where its text does (its pipes are no wrap).
        let wrapped = end + trailing < line_end && self.cell_at(visual).is_none();
        if wrapped && !at_row_end {
            self.move_to(end + trailing, extend);
            self.editor.row_end = true;
        } else {
            let target = if wrapped { line_end } else { end };
            self.move_to(self.past_fold(target), extend);
        }
        self.editor.goal = None;
    }

    /// Where the caret is drawn when it is at a wrapped row's end: after that row's last character.
    fn row_end_caret(&self) -> Option<kition_note_view::Caret> {
        let focus = self.editor.focus;
        if !self.editor.row_end || focus == 0 {
            return None;
        }
        let view = self.view();
        let rect = view.selection(focus - 1, focus).into_iter().next()?;
        Some(kition_note_view::Caret { x: rect.x1, top: rect.y0, height: rect.y1 - rect.y0 })
    }

    /// The caret and the selection as overlays.
    fn overlays(&self) -> Vec<ToolOverlay> {
        let view = self.view();
        // Where a dragged selection would drop: a caret there, over the selection.
        let drop_caret = self.editor.dragged.and_then(|(_, _, _, drop)| drop).map(|drop| {
            let caret = view.caret(drop);
            let x = f64::from(caret.x);
            ToolOverlay::Line {
                ax: x,
                ay: f64::from(caret.top + 2.0),
                bx: x,
                by: f64::from(caret.top + caret.height - 2.0),
                color: [CARET[0], CARET[1], CARET[2], 255],
                dashed: false,
            }
        });
        if self.editor.is_collapsed() {
            // A caret at a fold's end is drawn at its folded line's end.
            let shown = self.fold_spans().into_iter().find(|(_, end)| *end == self.editor.focus).map_or(self.editor.focus, |(line_end, _)| line_end);
            let caret = self.row_end_caret().unwrap_or_else(|| view.caret(shown));
            let x = f64::from(caret.x);
            return vec![ToolOverlay::Line {
                ax: x,
                ay: f64::from(caret.top + 2.0),
                bx: x,
                by: f64::from(caret.top + caret.height - 2.0),
                color: CARET,
                dashed: false,
            }];
        }
        let (from, to) = self.editor.ordered();
        view.selection(from, to)
            .into_iter()
            .map(|rect| {
                let (x0, y0, x1, y1) = (f64::from(rect.x0), f64::from(rect.y0), f64::from(rect.x1), f64::from(rect.y1));
                ToolOverlay::Highlight { quad: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], color: SELECTION }
            })
            .chain(drop_caret)
            .collect()
    }

    fn respond(&self) -> ToolResponse {
        let cursor = if self.editor.cursor.is_empty() { "text" } else { self.editor.cursor };
        ToolResponse { requests: Vec::new(), overlays: self.overlays(), cursor: cursor.into(), busy: true, wants_text: true }
    }

    /// The caret's rectangle (for the editor to keep it in view), with `remote: true` while the
    /// caret is where another person's change moved it: the editor then keeps it where it was on
    /// screen, and never scrolls to it.
    pub fn caret_rect(&self) -> Value {
        let caret = self.row_end_caret().unwrap_or_else(|| self.view().caret(self.shown_at(self.editor.focus)));
        if self.remote_selection == Some((self.editor.anchor, self.editor.focus)) {
            json!({ "x": caret.x, "y": caret.top, "height": caret.height, "remote": true })
        } else {
            json!({ "x": caret.x, "y": caret.top, "height": caret.height })
        }
    }

    /// What this person shares as presence (Kit UI protocol section 7e): the caret and the
    /// selection's other end as replica text positions (`{ object: "note", field: "body", offset }`,
    /// offsets in Unicode characters, the same on every replica), and the `Note` object as the
    /// selection when text is selected. Before the person clicks or types (the note just opened)
    /// nothing is shared but where they look.
    pub fn presence_caret(&self) -> Value {
        if !self.editor.engaged {
            return json!({ "selection": [], "caret": null, "anchor": null });
        }
        let text = self.body();
        let at = |offset: usize| json!({ "object": kition_note_kdm::root_id().to_string(), "field": kition_note_kdm::BODY, "offset": scalar(text, offset) });
        let selection = if self.editor.is_collapsed() { json!([]) } else { json!([kition_note_kdm::root_id().to_string()]) };
        json!({ "selection": selection, "caret": at(self.editor.focus), "anchor": at(self.editor.anchor) })
    }

    /// Where other people's carets and selections are in this view (`presence.locate`): `places`
    /// is `[{ caret, anchor? }]`, as [`NoteDocument::presence_caret`] shares them, and the answer
    /// is one `{ caret: { x, y, height } | null, rects: [{ x, y, width, height }] }` per place, in
    /// document pixels. A position of another object, past the text, or in a folded section is
    /// not shown (`null`, no rectangles).
    pub fn presence_locate(&self, places: &Value) -> Value {
        /// At most this many people are placed (the host sends at most a room's peers).
        const MOST: usize = 64;
        let text = self.body();
        let root = kition_note_kdm::root_id().to_string();
        let byte = |position: &Value| -> Option<usize> {
            let object = position.get("object").and_then(Value::as_str)?;
            let field = position.get("field").and_then(Value::as_str)?;
            if object != root || field != kition_note_kdm::BODY {
                return None;
            }
            let offset = usize::try_from(position.get("offset").and_then(Value::as_u64)?).ok()?;
            text.char_indices().map(|(at, _)| at).chain(std::iter::once(text.len())).nth(offset)
        };
        let view = self.view();
        let placed: Vec<Value> = places
            .as_array()
            .into_iter()
            .flatten()
            .take(MOST)
            .map(|place| {
                let Some(focus) = place.get("caret").and_then(byte) else { return json!({ "caret": null, "rects": [] }) };
                let caret = (!view.is_hidden(focus)).then(|| {
                    let caret = view.caret(focus);
                    json!({ "x": caret.x, "y": caret.top, "height": caret.height })
                });
                let rects: Vec<Value> = match place.get("anchor").and_then(byte) {
                    Some(anchor) if anchor != focus => view
                        .selection(anchor.min(focus), anchor.max(focus))
                        .into_iter()
                        .map(|rect| json!({ "x": rect.x0, "y": rect.y0, "width": rect.x1 - rect.x0, "height": rect.y1 - rect.y0 }))
                        .collect(),
                    _ => Vec::new(),
                };
                json!({ "caret": caret, "rects": rects })
            })
            .collect();
        Value::Array(placed)
    }

    /// The selected text.
    pub fn selected_text(&self) -> String {
        let (from, to) = self.editor.ordered();
        self.body().get(from..to).unwrap_or("").to_owned()
    }

    /// The editor's commands (`docs/design.md` section 3.4: the editor's, never an agent's).
    pub(crate) fn run_editor(&mut self, command: &str, params: &Value) -> Result<Value, String> {
        let text = self.editor_text();
        match command {
            "select.get" => {
                let (from, to) = self.editor.ordered();
                let line = text.get(..self.editor.focus).map_or(0, |before| before.matches('\n').count()) + 1;
                let (line_start, _) = line_around(&text, self.editor.focus);
                let column = text.get(line_start..self.editor.focus).map_or(0, |part| part.chars().count()) + 1;
                Ok(json!({
                    "anchor": scalar(&text, self.editor.anchor),
                    "focus": scalar(&text, self.editor.focus),
                    "from": scalar(&text, from),
                    "to": scalar(&text, to),
                    "text": self.selected_text(),
                    "line": line,
                    "column": column,
                    // The line's text before the caret (at most 80 characters), for completions.
                    "before": text.get(line_start..self.editor.focus).map(|part| part.chars().rev().take(80).collect::<Vec<_>>().into_iter().rev().collect::<String>()).unwrap_or_default(),
                    "caret": self.caret_rect(),
                }))
            }
            "select.set" => {
                let byte = |name: &str| -> Result<usize, String> {
                    let index = params.get(name).and_then(Value::as_u64).ok_or_else(|| format!("invalid-params: {name} is required"))?;
                    let index = usize::try_from(index).unwrap_or(usize::MAX);
                    Ok(text.char_indices().nth(index).map_or(text.len(), |(at, _)| at))
                };
                // A line (from 1) is selected, as a search hit or a diagnostic is shown; with a
                // column (from 1, in Unicode characters) the caret goes there, at most the line's end.
                let point = params.get("x").and_then(Value::as_f64).zip(params.get("y").and_then(Value::as_f64));
                let (anchor, focus) = if let Some((x, y)) = point {
                    // A point of the document (a drop): the byte there.
                    let at = self.view().hit(x as f32, y as f32);
                    (at, at)
                } else if let Some(line) = params.get("line").and_then(Value::as_u64) {
                    let index = usize::try_from(line.max(1) - 1).unwrap_or(usize::MAX);
                    let start = if index == 0 { Some(0) } else { text.match_indices('\n').nth(index - 1).map(|(at, _)| at + 1) };
                    let start = start.unwrap_or(text.len());
                    let (_, end) = line_around(&text, start);
                    match params.get("column").and_then(Value::as_u64) {
                        Some(column) => {
                            let skip = usize::try_from(column.max(1) - 1).unwrap_or(usize::MAX);
                            let line_text = text.get(start..end).unwrap_or("");
                            let at = line_text.char_indices().nth(skip).map_or(end, |(offset, _)| start + offset);
                            (at, at)
                        }
                        None => (start, end),
                    }
                } else {
                    let anchor = byte("anchor")?;
                    (anchor, params.get("focus").map_or(Ok(anchor), |_| byte("focus"))?)
                };
                self.stop_typing();
                self.editor.anchor = anchor;
                self.editor.focus = focus;
                self.editor.goal = None;
                self.editor.row_end = false;
                self.editor.engaged = true;
                Ok(json!({}))
            }
            "image.wanted" => Ok(json!({ "images": self.view().wanted_pictures(), "drawn": self.view().drawn_pictures() })),
            "select.controlKey" => {
                let name = params.get("key").and_then(Value::as_str).ok_or("invalid-params: key is required")?.to_ascii_lowercase();
                self.editor.engaged = true;
                // Page Up and Page Down (which the frame keeps to itself), Shift selecting.
                if name == "pageup" || name == "pagedown" {
                    let shift = params.get("shift").and_then(Value::as_bool) == Some(true);
                    self.navigate(if name == "pageup" { "PageUp" } else { "PageDown" }, ToolMods { shift, ..ToolMods::default() });
                    self.unfold_at_caret();
                    return Ok(json!({}));
                }
                self.control_key(&name)?;
                self.unfold_at_caret();
                Ok(json!({}))
            }
            "view.blockAt" => {
                // Whether a point is in a callout, and whether the caret is in that callout (a
                // right click in a callout drawn as one opens no menu, as in Obsidian).
                let x = params.get("x").and_then(Value::as_f64).ok_or("invalid-params: x is required")? as f32;
                let y = params.get("y").and_then(Value::as_f64).ok_or("invalid-params: y is required")? as f32;
                let view = self.view();
                let styles = view.styles();
                let line = styles.line_of(view.hit(x, y));
                let caret = styles.line_of(self.editor.focus);
                let in_callout = |index: usize| styles.lines.get(index).is_some_and(|style| style.callout != 0);
                let callout = in_callout(line);
                let active = callout && (line.min(caret)..=line.max(caret)).all(in_callout);
                Ok(json!({ "callout": callout, "active": active }))
            }
            "select.where" => {
                let index = params.get("at").and_then(Value::as_u64).ok_or("invalid-params: at is required")?;
                let at = text.char_indices().nth(usize::try_from(index).unwrap_or(usize::MAX)).map_or(text.len(), |(byte, _)| byte);
                let caret = self.view().caret(at);
                Ok(json!({ "x": caret.x, "y": caret.top, "height": caret.height }))
            }
            "image.set" => {
                let name = params.get("name").and_then(Value::as_str).ok_or("invalid-params: name is required")?.to_owned();
                let data = params.get("data").and_then(Value::as_str).unwrap_or("");
                use base64::Engine as _;
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .ok()
                    .and_then(|bytes| image::load_from_memory(&bytes).ok())
                    .map(|picture| picture.to_rgba8());
                {
                    let _ = self.view();
                    let mut view = self.view.borrow_mut();
                    match &decoded {
                        Some(rgba) => view.set_picture(&name, rgba.width(), rgba.height(), rgba.as_raw()),
                        None => view.set_picture(&name, 0, 0, &[]),
                    }
                }
                self.pictures.set(self.pictures.get() + 1);
                Ok(json!({ "drawn": decoded.is_some() }))
            }
            "select.replace" => {
                let insert = params.get("text").and_then(Value::as_str).ok_or("invalid-params: text is required")?.to_owned();
                let (from, to) = self.editor.ordered();
                // A paste: a step of its own, which the typing after it joins (as Obsidian's). Plain
                // text pasted (`paste`) is pasted as Obsidian pastes it: an address over a
                // selection links it.
                self.editor.join = Join::Fresh;
                let plain = params.get("paste").and_then(Value::as_bool) == Some(true);
                let insert = if plain { insert.replace("\r\n", "\n").replace('\r', "\n") } else { insert };
                if plain && from != to && is_address(&insert) && self.type_paired(&insert)? {
                    return Ok(json!({}));
                }
                self.replace(from, to, &insert, false)?;
                Ok(json!({}))
            }
            "view.fold" => {
                // The caret's own line, as Obsidian's Toggle fold takes it: a heading, or a list
                // item with children; from any other line nothing folds. The caret stays put.
                let heading = {
                    let view = self.view();
                    let styles = view.styles();
                    let line = styles.line_of(self.editor.focus);
                    let folds = match styles.lines.get(line).map(|style| style.block) {
                        Some(kition_note_view::style::Block::Heading(_)) => true,
                        _ => kition_note_view::layout::item_children(view.text(), styles, line).is_some(),
                    };
                    folds.then(|| styles.starts.get(line).copied()).flatten()
                };
                if let Some(at) = heading {
                    self.toggle_fold(at);
                }
                Ok(json!({ "folded": self.editor.folded.len() }))
            }
            "view.foldAll" => {
                let starts: Vec<usize> = {
                    let view = self.view();
                    let styles = view.styles();
                    // Every heading and every list item with children, as Obsidian's Fold all.
                    styles
                        .lines
                        .iter()
                        .zip(&styles.starts)
                        .enumerate()
                        .filter(|(index, (style, _))| {
                            matches!(style.block, kition_note_view::style::Block::Heading(_))
                                || kition_note_view::layout::item_children(view.text(), styles, *index).is_some()
                        })
                        .map(|(_, (_, start))| *start)
                        .collect()
                };
                self.editor.folded = starts;
                // The caret leaves what the folds hide (a caret at a fold's end stays there).
                let at_end = self.fold_spans().iter().any(|(_, end)| *end == self.editor.focus);
                if !at_end && self.view().is_hidden(self.editor.focus) {
                    let at = self.editor.folded.iter().rev().find(|fold| **fold <= self.editor.focus).copied().unwrap_or(0);
                    let (_, end) = line_around(&text, at);
                    self.editor.anchor = end;
                    self.editor.focus = end;
                }
                Ok(json!({ "folded": self.editor.folded.len() }))
            }
            "view.foldProperties" => {
                // Obsidian's Toggle fold properties: the block folds to its heading (nothing is
                // written); the view is laid out again.
                let folded = !self.view().properties_folded();
                self.view.borrow_mut().set_properties_folded(folded);
                self.view_revision.set(None);
                Ok(json!({ "folded": folded }))
            }
            "view.hoverOut" => {
                // The pointer left the note: no heading or item shows its chevron, no hand.
                self.view.borrow_mut().set_hover(None);
                self.editor.cursor = "text";
                Ok(json!({}))
            }
            "view.foldMore" | "view.foldLess" => {
                self.fold_step(command == "view.foldMore");
                Ok(json!({ "folded": self.editor.folded.len() }))
            }
            "view.unfoldAll" => {
                self.editor.folded.clear();
                Ok(json!({ "folded": 0 }))
            }
            "view.width" => {
                let width = params.get("width").and_then(Value::as_f64).ok_or("invalid-params: width is required")?;
                // The visible height, when given, sets the room below the note (half of it).
                let height = params.get("height").and_then(Value::as_f64).filter(|height| height.is_finite() && *height > 0.0);
                let changed = {
                    let _ = self.view();
                    let mut view = self.view.borrow_mut();
                    let wider = view.set_width(width as f32);
                    let taller = height.is_some_and(|height| view.set_visible_height(height as f32));
                    wider || taller
                };
                if changed {
                    self.pictures.set(self.pictures.get() + 1);
                }
                Ok(json!({ "changed": changed, "width": self.view().width() }))
            }
            "view.scheme" => {
                // The host's color scheme: the note paints with its palette; nothing moves.
                let name = params.get("scheme").and_then(Value::as_str).ok_or("invalid-params: scheme is required")?;
                let scheme =
                    kition_note_view::Scheme::parse(name).ok_or_else(|| format!("invalid-params: scheme must be light or dark, not {name}"))?;
                let changed = self.view.borrow_mut().set_scheme(scheme);
                if changed {
                    self.pictures.set(self.pictures.get() + 1);
                }
                Ok(json!({ "changed": changed, "scheme": name }))
            }
            "property.view" => {
                // The front matter as Obsidian's properties block, for the editor to draw: null
                // while it is edited as its source. Asking hands its drawing to the editor.
                if self.view.borrow_mut().set_properties_overlaid(true) {
                    self.paint_epoch.set(self.paint_epoch.get() + 1);
                }
                let view = self.view();
                let Some(block) = view.properties_block() else { return Ok(Value::Null) };
                let rows: Vec<Value> = block
                    .rows
                    .iter()
                    .map(|row| {
                        json!({
                            "key": row.key,
                            "kind": row.kind,
                            "values": row.values,
                            "y": row.y,
                            "height": row.height,
                            "value": scalar(&text, row.value_at),
                        })
                    })
                    .collect();
                Ok(json!({
                    "top": block.top,
                    "bottom": block.bottom,
                    "left": view.theme().margin,
                    "width": view.theme().content_width(),
                    "end": scalar(&text, block.end),
                    "rows": rows,
                    "collapsed": block.folded,
                }))
            }
            "code.blocks" => {
                // The code blocks between `top` and `bottom` (document heights), for their labels.
                let bound = |name: &str, default: f32| params.get(name).and_then(Value::as_f64).map_or(default, |value| value as f32);
                let blocks = self.view().code_blocks_in(bound("top", f32::NEG_INFINITY), bound("bottom", f32::INFINITY));
                let mut scalars = Scalars::new(&text);
                Ok(json!({ "blocks": blocks.iter().map(|block| json!({
                    "top": block.top,
                    "bottom": block.bottom,
                    "right": block.right,
                    "language": block.language,
                    "callout": block.callout,
                    "left": block.left,
                    "from": scalars.at(block.code.start),
                    "to": scalars.at(block.code.end),
                })).collect::<Vec<_>>() }))
            }
            "diagram.view" => {
                // The blocks the editor draws (Mermaid diagrams, HTML blocks): those away from the
                // caret not sized yet, and those drawn between `top` and `bottom` (document
                // heights).
                let bound = |name: &str, default: f32| params.get(name).and_then(Value::as_f64).map_or(default, |value| value as f32);
                let view = self.view();
                let drawn = view.diagrams_in(bound("top", f32::NEG_INFINITY), bound("bottom", f32::INFINITY));
                Ok(json!({
                    "wanted": view.wanted_diagrams().into_iter().map(|(kind, source)| json!({ "kind": kind, "source": source })).collect::<Vec<_>>(),
                    "column": view.theme().content_width(),
                    "diagrams": drawn.iter().map(|diagram| json!({
                        "kind": diagram.kind,
                        "source": diagram.source,
                        "top": diagram.top,
                        "left": diagram.left,
                        "width": diagram.width,
                        "height": diagram.height,
                        "code": scalar(&text, diagram.code),
                    })).collect::<Vec<_>>(),
                }))
            }
            "diagram.set" => {
                let source = params.get("source").and_then(Value::as_str).ok_or("invalid-params: source is required")?.to_owned();
                let size = |name: &str| {
                    params.get(name).and_then(Value::as_f64).filter(|value| value.is_finite() && *value >= 0.0).map(|value| value as f32)
                };
                let (Some(width), Some(height)) = (size("width"), size("height")) else {
                    return Err("invalid-params: width and height are required".into());
                };
                let changed = {
                    let _ = self.view();
                    self.view.borrow_mut().set_diagram(&source, width, height)
                };
                if changed {
                    self.pictures.set(self.pictures.get() + 1);
                }
                Ok(json!({ "changed": changed }))
            }
            "table.geometry" => {
                // Only the tables between `top` and `bottom` (document heights) when given.
                let bound = |name: &str, default: f32| params.get(name).and_then(Value::as_f64).map_or(default, |value| value as f32);
                let (top, bottom) = (bound("top", f32::NEG_INFINITY), bound("bottom", f32::INFINITY));
                // A large note's tables far from what was drawn are guessed: laid out first.
                let _ = self.view();
                self.view.borrow_mut().realize_between(top, bottom);
                let grids = self.view().tables_in(top, bottom);
                Ok(json!({ "tables": grids.iter().map(|grid| json!({
                    "table": grid.index,
                    "columns": grid.columns,
                    "rows": grid.rows.iter().zip(&grid.lines).map(|((top, bottom), line)| json!({ "top": top, "bottom": bottom, "line": line })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>() }))
            }
            "edit.selectAll" => {
                self.stop_typing();
                self.editor.anchor = 0;
                self.editor.focus = text.len();
                Ok(json!({}))
            }
            "edit.cut" => {
                // The clipboard gets exactly what goes: with a span's closer, its opener goes too.
                let (from, to) = self.editor.ordered();
                let start = if from == to { from } else { orphan_start(&text, from, to) };
                let cut = text.get(start..to).unwrap_or("").to_owned();
                if from != to {
                    self.replace(start, to, "", false)?;
                }
                Ok(json!({ "text": cut }))
            }
            "format.bold" => self.wrap("**").map(|()| json!({})),
            "format.italic" => self.wrap("*").map(|()| json!({})),
            "format.strike" => self.wrap("~~").map(|()| json!({})),
            "format.code" => self.wrap("`").map(|()| json!({})),
            "format.highlight" => self.highlight_color(None).map(|()| json!({})),
            "format.highlightColor" => {
                let color = params.get("color").and_then(Value::as_str).ok_or("invalid-params: color is required")?.to_owned();
                self.highlight_color(Some(&color)).map(|()| json!({}))
            }
            "format.math" => self.wrap("$").map(|()| json!({})),
            "format.clear" => self.clear_formatting().map(|()| json!({})),
            "format.link" => {
                let (from, to) = self.editor.ordered();
                let label = text.get(from..to).unwrap_or("").to_owned();
                self.replace(from, to, &format!("[{label}]()"), false)?;
                // The caret goes between the parentheses for a selection's address, or between the
                // brackets for the text when nothing was selected (as Obsidian's).
                self.editor.anchor = if label.is_empty() { from + 1 } else { self.editor.focus.saturating_sub(1) };
                self.editor.focus = self.editor.anchor;
                Ok(json!({}))
            }
            "format.paragraph" | "format.heading1" | "format.heading2" | "format.heading3" | "format.heading4" | "format.heading5"
            | "format.heading6" => {
                let level = command.strip_prefix("format.heading").and_then(|n| n.parse::<u64>().ok()).unwrap_or(0).min(6);
                let prefix = if level == 0 { String::new() } else { format!("{} ", "#".repeat(usize::try_from(level).unwrap_or(1))) };
                self.prefix_lines(|line| {
                    let hashes = line.len() - line.trim_start_matches('#').len();
                    let used = if hashes > 0 && line.get(hashes..).is_some_and(|rest| rest.starts_with(' ') || rest.is_empty()) {
                        (hashes + 1).min(line.len())
                    } else {
                        0
                    };
                    Some((used, prefix.clone()))
                })
                .map(|()| json!({}))
            }
            "list.bullet" => self.toggle_prefix("- ").map(|()| json!({})),
            "list.numbered" => self.toggle_prefix("1. ").map(|()| json!({})),
            "list.task" => self.toggle_prefix("- [ ] ").map(|()| json!({})),
            "list.toggleCheckbox" => self.toggle_checkbox().map(|()| json!({})),
            "edit.indent" => self.indent_selection(false).map(|()| json!({})),
            "edit.outdent" => self.indent_selection(true).map(|()| json!({})),
            "edit.moveLineUp" => self.move_lines(true).map(|()| json!({})),
            "edit.moveLineDown" => self.move_lines(false).map(|()| json!({})),
            "edit.deleteParagraph" => self.delete_paragraph().map(|()| json!({})),
            "format.comment" => self.toggle_comment().map(|()| json!({})),
            "format.quote" => self.toggle_prefix("> ").map(|()| json!({})),
            "wikilink.insert" => self.surround_to_end("[[", "]]").map(|()| json!({})),
            "embed.insert" => self.surround_to_end("![[", "]]").map(|()| json!({})),
            "image.insert" => self.surround("![", "]()").map(|()| json!({})),
            "tag.insert" => {
                // A `#` before the selection, the caret after it (as Obsidian's Add tag).
                let (from, _) = self.editor.ordered();
                self.replace(from, from, "#", false)?;
                Ok(json!({}))
            }
            "footnote.insert" => {
                let used = text.matches("[^").count();
                let label = format!("[^{}]", used / 2 + 1);
                let (_, to) = self.editor.ordered();
                // The note's end gets the footnote's text; the caret goes there.
                let end = text.len();
                let tail = if text.ends_with('\n') { "" } else { "\n" };
                let caret = self.grouped(|doc| {
                    doc.replace(end, end, &format!("{tail}\n{label}: "), false)?;
                    let caret = doc.editor.focus;
                    doc.replace(to, to, &label, false)?;
                    Ok(caret)
                })?;
                self.editor.anchor = caret + label.len();
                self.editor.focus = caret + label.len();
                Ok(json!({}))
            }
            "mathBlock.insert" => self.fence_lines("$$").map(|()| json!({})),
            "callout.insert" => self.insert_callout().map(|()| json!({})),
            "date.insert" | "timestamp.insert" => {
                let pattern = if command == "date.insert" { "YYYY-MM-DD" } else { "YYYY-MM-DD HH:mm" };
                // The local date: the page's time zone, or the offset the caller gives.
                let offset = match params.get("utcOffset") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(
                        value
                            .as_i64()
                            .filter(|minutes| minutes.abs() <= crate::clock::MAX_OFFSET)
                            .ok_or_else(|| format!("invalid-params: utcOffset must be whole minutes from -{0} to {0}", crate::clock::MAX_OFFSET))?,
                    ),
                };
                let stamp = crate::clock::local_now(offset).format(pattern);
                let (from, to) = self.editor.ordered();
                self.replace(from, to, &stamp, false).map(|()| json!({}))
            }
            "format.headingUp" | "format.headingDown" => {
                let up = command == "format.headingUp";
                self.prefix_lines(move |line| {
                    let hashes = line.len() - line.trim_start_matches('#').len();
                    let heading = hashes > 0 && line.get(hashes..).is_some_and(|rest| rest.starts_with(' '));
                    let level = if heading { hashes } else { 7 };
                    let next = if up { level.saturating_sub(1).max(1) } else { level + 1 };
                    let used = if heading { hashes + 1 } else { 0 };
                    if next > 6 { Some((used, String::new())) } else { Some((used, format!("{} ", "#".repeat(next)))) }
                })
                .map(|()| json!({}))
            }
            "edit.sortLines" | "edit.trimWhitespace" => {
                let (from, to) = self.editor.ordered();
                let (first, _) = line_around(&text, from);
                let (_, last) = line_around(&text, to);
                let block = text.get(first..last).unwrap_or("");
                let mut lines: Vec<&str> = block.split('\n').collect();
                let rewritten = if command == "edit.sortLines" {
                    lines.sort_by_key(|line| line.to_lowercase());
                    lines.join("\n")
                } else {
                    lines.iter().map(|line| line.trim_end_matches([' ', '\t'])).collect::<Vec<_>>().join("\n")
                };
                if rewritten != block {
                    self.replace(first, last, &rewritten, true)?;
                }
                Ok(json!({}))
            }
            "edit.selectSection" => {
                let ix = kition_note_markdown::index(&text);
                let (from, _) = self.editor.ordered();
                let section = ix.headings.iter().rev().find(|heading| heading.range.start <= from).map(|heading| heading.section.clone());
                if let Some(section) = section {
                    self.stop_typing();
                    self.editor.anchor = section.start;
                    self.editor.focus = section.end;
                }
                Ok(json!({}))
            }
            "codeBlock.insert" => self.fence_lines("```").map(|()| json!({})),
            "rule.insert" => self.insert_at_caret("---\n", 4).map(|()| json!({})),
            "table.insert" => self.insert_at_caret("|     |     |\n| --- | --- |\n|     |     |\n", 1).map(|()| json!({})),
            other => Err(format!("unknown command {other}")),
        }
    }

    /// Wraps the selection in `mark` (Cmd+B and the like), or takes the mark away when the
    /// selection already has it on both sides.
    fn wrap(&mut self, mark: &str) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let n = mark.len();
        if from == to {
            return self.wrap_at_caret(&text, from, mark);
        }
        // The spaces at the selection's ends stay outside the marks (as Obsidian's).
        let selected = text.get(from..to).unwrap_or("");
        let trimmed = selected.trim_matches([' ', '\t']);
        let (from, to) = if trimmed.is_empty() {
            (from, to)
        } else {
            let lead = selected.len() - selected.trim_start_matches([' ', '\t']).len();
            (from + lead, from + lead + trimmed.len())
        };
        let selected = text.get(from..to).unwrap_or("").to_owned();
        let (line_start, line_end) = line_around(&text, from);
        if to > line_end {
            // Over several lines: the marks go around the whole selection.
            self.replace(from, to, &format!("{mark}{selected}{mark}"), false)?;
            self.editor.anchor = from + n;
            self.editor.focus = from + n + selected.len();
            return Ok(());
        }
        let line = text.get(line_start..line_end).unwrap_or("");
        let spans: Vec<Span> = mark_spans(line, mark).into_iter().map(|span| span.shifted(line_start)).collect();
        // The selection is the text of a span (or the span with its marks): the marks go.
        if let Some(span) = spans.iter().find(|span| (span.open.1 == from && span.close.0 == to) || (span.open.0 == from && span.close.1 == to)) {
            let (open_len, close_len) = (span.open.1 - span.open.0, span.close.1 - span.close.0);
            let (inner_from, inner_to) = (span.open.1, span.close.0);
            let (close, open) = (span.close, span.open);
            self.grouped(|doc| {
                doc.replace(close.0, close.1, "", false)?;
                doc.replace(open.0, open.1, "", false)
            })?;
            let _ = close_len;
            self.editor.anchor = inner_from - open_len;
            self.editor.focus = inner_to - open_len;
            return Ok(());
        }
        // Spans the selection overlaps join it: their marks go, and the marks go around them all
        // (as Obsidian bolds `**b** c` into `**b c**`).
        let touched: Vec<&Span> = spans.iter().filter(|span| span.open.0 < to && span.close.1 > from).collect();
        if !touched.is_empty() {
            let union_from = touched.iter().map(|span| span.open.0).min().unwrap_or(from).min(from);
            let union_to = touched.iter().map(|span| span.close.1).max().unwrap_or(to).max(to);
            let tokens: Vec<(usize, usize)> = touched.iter().flat_map(|span| [span.open, span.close]).collect();
            let removed_before = |at: usize| tokens.iter().map(|(start, end)| at.clamp(*start, *end) - start).sum::<usize>();
            let mut inner = String::new();
            let mut at = union_from;
            let mut sorted = tokens.clone();
            sorted.sort_unstable();
            for (start, end) in &sorted {
                inner.push_str(text.get(at..*start).unwrap_or(""));
                at = *end;
            }
            inner.push_str(text.get(at..union_to).unwrap_or(""));
            let (new_from, new_to) = (from - union_from - removed_before(from), to - union_from - removed_before(to));
            self.replace(union_from, union_to, &format!("{mark}{inner}{mark}"), false)?;
            self.editor.anchor = union_from + n + new_from;
            self.editor.focus = union_from + n + new_to;
            return Ok(());
        }
        self.replace(from, to, &format!("{mark}{selected}{mark}"), false)?;
        self.editor.anchor = from + n;
        self.editor.focus = from + n + selected.len();
        Ok(())
    }

    /// Fold more and Fold less, as Obsidian's: of the headings' sections and the list items with
    /// children around the caret, the innermost not folded yet folds (more), or the outermost
    /// folded unfolds (less).
    fn fold_step(&mut self, more: bool) {
        let text = self.editor_text();
        let around: Vec<usize> = {
            let view = self.view();
            let styles = view.styles();
            let line = styles.line_of(self.editor.focus);
            let ix = kition_note_markdown::index(&text);
            let mut starts: Vec<usize> = ix
                .headings
                .iter()
                .filter(|heading| heading.range.start <= self.editor.focus && self.editor.focus <= heading.section.end)
                .map(|heading| line_around(&text, heading.range.start).0)
                .collect();
            starts.extend(
                (0..=line)
                    .filter(|candidate| kition_note_view::layout::item_children(view.text(), styles, *candidate).is_some_and(|last| last >= line))
                    .filter_map(|candidate| styles.starts.get(candidate).copied()),
            );
            starts.sort_unstable();
            starts.dedup();
            starts
        };
        let folded = |at: &usize| self.editor.folded.contains(at);
        let target = if more { around.iter().rev().find(|at| !folded(at)) } else { around.iter().find(|at| folded(at)) };
        if let Some(at) = target.copied() {
            self.toggle_fold(at);
        }
    }

    /// Toggle highlight (`color` None) and Obsidian 1.14's coloured highlights (`==🔴text==`), as
    /// Obsidian's: in a highlight of that colour (any one, for the plain toggle) the highlight goes;
    /// in another the colour changes; elsewhere the selection (or the word at the caret, or an
    /// empty pair) is highlighted in it.
    fn highlight_color(&mut self, color: Option<&str>) -> Result<(), String> {
        let emoji = match color {
            None => None,
            Some(name) => Some(
                ["red", "orange", "yellow", "green", "blue", "purple"]
                    .iter()
                    .position(|known| *known == name)
                    .and_then(|index| kition_note_view::style::HIGHLIGHT_COLORS.get(index))
                    .map(|(emoji, _)| *emoji)
                    .ok_or_else(|| format!("invalid-params: no highlight colour {name}"))?,
            ),
        };
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (line_start, line_end) = line_around(&text, from);
        let line = text.get(line_start..line_end).unwrap_or("");
        let inside = mark_spans(line, "==").into_iter().map(|span| span.shifted(line_start)).find(|span| span.open.1 <= from && to <= span.close.0);
        let current = |open: usize| {
            kition_note_view::style::HIGHLIGHT_COLORS
                .iter()
                .map(|(mark, _)| *mark)
                .find(|mark| text.get(open..).is_some_and(|rest| rest.starts_with(mark)))
        };
        match (inside, emoji) {
            // The plain toggle in a coloured highlight: the highlight goes, colour and all.
            (Some(span), None) if current(span.open.1).is_some() => {
                let colour = current(span.open.1).map_or(0, str::len);
                let (anchor, focus) = (self.editor.anchor, self.editor.focus);
                let gone = 2 + colour;
                self.grouped(|doc| {
                    doc.replace(span.close.0, span.close.1, "", false)?;
                    doc.replace(span.open.0, span.open.1 + colour, "", false)
                })?;
                self.editor.anchor = anchor - gone;
                self.editor.focus = focus - gone;
                Ok(())
            }
            (_, None) => self.wrap("=="),
            (Some(span), Some(emoji)) => {
                let (anchor, focus) = (self.editor.anchor, self.editor.focus);
                match current(span.open.1) {
                    // The same colour: the highlight goes.
                    Some(mark) if mark == emoji => {
                        let gone = 2 + mark.len();
                        self.grouped(|doc| {
                            doc.replace(span.close.0, span.close.1, "", false)?;
                            doc.replace(span.open.0, span.open.1 + mark.len(), "", false)
                        })?;
                        self.editor.anchor = anchor - gone;
                        self.editor.focus = focus - gone;
                    }
                    // Another colour, or none: this one.
                    other => {
                        let old = other.map_or(0, str::len);
                        self.replace(span.open.1, span.open.1 + old, emoji, false)?;
                        let shift = |at: usize| at + emoji.len() - old;
                        self.editor.anchor = shift(anchor);
                        self.editor.focus = shift(focus);
                    }
                }
                Ok(())
            }
            (None, Some(emoji)) => {
                // Highlighted as the plain toggle does it, then the colour after the opening marks.
                self.grouped(|doc| {
                    doc.wrap("==")?;
                    let text = doc.editor_text();
                    let (from, to) = doc.editor.ordered();
                    let (line_start, line_end) = line_around(&text, from);
                    let line = text.get(line_start..line_end).unwrap_or("");
                    let empty = from == to
                        && text.get(..from).is_some_and(|before| before.ends_with("=="))
                        && text.get(from..).is_some_and(|after| after.starts_with("=="));
                    let open = if empty {
                        Some(from)
                    } else {
                        mark_spans(line, "==")
                            .into_iter()
                            .map(|span| span.shifted(line_start))
                            .find(|span| span.open.1 <= from && to <= span.close.0)
                            .map(|span| span.open.1)
                    };
                    if let Some(open) = open {
                        let (anchor, focus) = (doc.editor.anchor, doc.editor.focus);
                        doc.replace(open, open, emoji, false)?;
                        doc.editor.anchor = anchor + emoji.len();
                        doc.editor.focus = focus + emoji.len();
                    }
                    Ok(())
                })
            }
        }
    }

    /// Clear formatting, as Obsidian's: the Markdown marks inside the selection go (emphasis,
    /// strikethrough, highlight, code and math marks, a link's brackets and parentheses, a
    /// heading's `#`s), each one whose own characters are selected; the text stays selected.
    fn clear_formatting(&mut self) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        if from == to {
            return Ok(());
        }
        let (first, _) = line_around(&text, from);
        let (_, last) = line_around(&text, to);
        let links = kition_note_markdown::index(&text).links;
        let mut tokens: Vec<(usize, usize)> = Vec::new();
        let mut at = first;
        while at <= last {
            let (start, end) = line_around(&text, at);
            let line = text.get(start..end).unwrap_or("");
            for mark in ["**", "*", "~~", "==", "`", "$"] {
                for span in mark_spans(line, mark) {
                    let span = span.shifted(start);
                    tokens.extend([span.open, span.close]);
                }
            }
            // A link's `[`, `](`, and `)`, as the index reads its links: an image or an embed is
            // content, not formatting, and stays; a task's box is no link's bracket.
            for link in links.iter().filter(|link| link.kind == LinkKind::Markdown && start <= link.range.start && link.range.end <= end) {
                let (Some(shown), Some(destination)) = (&link.text_range, &link.destination_range) else { continue };
                tokens.extend([(link.range.start, shown.start), (shown.end, destination.start), (destination.end, link.range.end)]);
            }
            // A heading's marks and the space after them.
            let hashes = line.len() - line.trim_start_matches('#').len();
            if hashes > 0 && hashes <= 6 && line.get(hashes..).is_some_and(|rest| rest.starts_with(' ')) {
                tokens.push((start, start + hashes + 1));
            }
            if end >= text.len() {
                break;
            }
            at = end + 1;
        }
        let mut tokens: Vec<(usize, usize)> = tokens.into_iter().filter(|(a, b)| *a >= from && *b <= to).collect();
        tokens.sort_unstable();
        tokens.dedup();
        // Overlapping tokens (a run of three marks) go once.
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (a, b) in tokens {
            match merged.last_mut() {
                Some(last) if a < last.1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        if merged.is_empty() {
            return Ok(());
        }
        let removed: usize = merged.iter().map(|(a, b)| b - a).sum();
        let removed_before_from: usize = merged.iter().filter(|(_, b)| *b <= from).map(|(a, b)| b - a).sum();
        self.grouped(|doc| {
            for (a, b) in merged.iter().rev() {
                doc.replace(*a, *b, "", false)?;
            }
            Ok(())
        })?;
        self.editor.anchor = from - removed_before_from;
        self.editor.focus = to - removed;
        Ok(())
    }

    /// A format's mark with no selection, as Obsidian does it: inside a span the mark wraps (on
    /// the caret's line), the marks go; touching a word, the word is wrapped; else the marks go in
    /// with the caret between them. The caret keeps its place in the text.
    fn wrap_at_caret(&mut self, text: &str, at: usize, mark: &str) -> Result<(), String> {
        let n = mark.len();
        let (line_start, line_end) = line_around(text, at);
        let line = text.get(line_start..line_end).unwrap_or("");
        let caret = at - line_start;
        let lone = mark.chars().next();
        let around = |offset: usize| {
            (line.get(..offset).and_then(|before| before.chars().next_back()), line.get(offset..).and_then(|after| after.chars().next()))
        };
        // An opening mark: not after a letter or the same mark, before text; a closing one: after
        // text, not before a letter or the same mark.
        let opening = line.get(..caret).and_then(|before| before.rfind(mark)).filter(|open| {
            let (previous, _) = around(*open);
            let (_, next) = around(open + n);
            previous.is_none_or(|c| !c.is_alphanumeric() && Some(c) != lone) && next.is_some_and(|c| !c.is_whitespace())
        });
        let closing = line.get(caret..).and_then(|after| after.find(mark)).map(|close| caret + close).filter(|close| {
            let (previous, _) = around(*close);
            let (_, next) = around(close + n);
            previous.is_some_and(|c| !c.is_whitespace()) && next.is_none_or(|c| !c.is_alphanumeric() && Some(c) != lone)
        });
        if let (Some(open), Some(close)) = (opening, closing)
            && open + n <= caret
            && !line.get(open + n..close).unwrap_or("").contains(mark)
        {
            let (open, close) = (line_start + open, line_start + close);
            self.grouped(|doc| {
                doc.replace(close, close + n, "", false)?;
                doc.replace(open, open + n, "", false)
            })?;
            self.editor.anchor = at - n;
            self.editor.focus = at - n;
            return Ok(());
        }
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let start = line.get(..caret).map_or(caret, |before| before.len() - before.trim_end_matches(word).len());
        let start = caret - start;
        let end = caret + line.get(caret..).map_or(0, |after| after.len() - after.trim_start_matches(word).len());
        if start < end {
            let selected = line.get(start..end).unwrap_or("").to_owned();
            let begin = line_start + start;
            self.replace(begin, line_start + end, &format!("{mark}{selected}{mark}"), false)?;
            // At the word's end the caret goes after the closing mark, elsewhere it stays in the word.
            let caret = if caret == end { at + 2 * n } else { at + n };
            self.editor.anchor = caret;
            self.editor.focus = caret;
            return Ok(());
        }
        self.replace(at, at, &format!("{mark}{mark}"), false)?;
        self.editor.anchor = at + n;
        self.editor.focus = at + n;
        Ok(())
    }

    /// Puts `before` and `after` around the selection; with no selection the caret goes between them.
    fn surround(&mut self, before: &str, after: &str) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let selected = text.get(from..to).unwrap_or("").to_owned();
        self.replace(from, to, &format!("{before}{selected}{after}"), false)?;
        let inner = from + before.len();
        self.editor.anchor = inner;
        self.editor.focus = inner + selected.len();
        Ok(())
    }

    /// Rewrites the prefix of every line the selection touches: `change` answers how many bytes
    /// of the line's start to replace and with what (`None` leaves the line).
    fn prefix_lines(&mut self, change: impl Fn(&str) -> Option<(usize, String)>) -> Result<(), String> {
        self.grouped(|doc| doc.prefix_lines_now(&change))
    }

    fn prefix_lines_now(&mut self, change: &dyn Fn(&str) -> Option<(usize, String)>) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (first, _) = line_around(&text, from);
        let (last, _) = line_around(&text, to);
        let mut starts = vec![first];
        starts.extend(text.get(first..last).unwrap_or("").match_indices('\n').map(|(at, _)| first + at + 1));
        let properties = front_matter_end(&text);
        starts.retain(|start| *start >= properties);
        let (mut anchor, mut focus) = (self.editor.anchor, self.editor.focus);
        for start in starts.into_iter().rev() {
            let current = self.editor_text();
            let (_, end) = line_around(&current, start);
            let line = current.get(start..end).unwrap_or("");
            let Some((used, prefix)) = change(line) else { continue };
            let used = used.min(line.len());
            self.transact(start, start + used, &prefix)?;
            let delta = prefix.len() as isize - used as isize;
            let shift = |at: usize| {
                if at >= start + used {
                    (at as isize + delta) as usize
                } else if at > start {
                    start + prefix.len().min(at - start)
                } else {
                    at
                }
            };
            anchor = shift(anchor);
            focus = shift(focus);
        }
        self.editor.anchor = anchor;
        self.editor.focus = focus;
        Ok(())
    }

    /// Adds `prefix` to every selected line, or takes it away when each already has it.
    fn toggle_prefix(&mut self, prefix: &str) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (first, _) = line_around(&text, from);
        let (_, last_end) = line_around(&text, to);
        let lines = text.get(first.max(front_matter_end(&text)).min(last_end)..last_end).unwrap_or("");
        let all = lines.split('\n').all(|line| line.trim_start().starts_with(prefix.trim_end()) || line.trim().is_empty());
        let prefix = prefix.to_owned();
        self.prefix_lines(move |line| {
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            let rest = line.get(indent..).unwrap_or("");
            if all {
                let had = if rest.starts_with(&prefix) {
                    prefix.len()
                } else if rest.starts_with(prefix.trim_end()) {
                    prefix.trim_end().len()
                } else {
                    return None;
                };
                Some((indent + had, line.get(..indent).unwrap_or("").to_owned()))
            } else {
                // A list marker the line has already is replaced, not doubled.
                let used = continuation(line).map_or(indent, |(used, _)| used);
                Some((used, format!("{}{prefix}", line.get(..indent).unwrap_or(""))))
            }
        })
    }

    /// Wraps the selection in `before` and `after`, the caret at the end inside (as Obsidian's
    /// Add internal link and Add embed).
    fn surround_to_end(&mut self, before: &str, after: &str) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let selected = text.get(from..to).unwrap_or("").to_owned();
        self.replace(from, to, &format!("{before}{selected}{after}"), false)?;
        let inner = from + before.len() + selected.len();
        self.editor.anchor = inner;
        self.editor.focus = inner;
        Ok(())
    }

    /// The lines the selection touches, from the first one's start to the last one's end.
    fn selected_lines(&self, text: &str) -> (usize, usize) {
        let (from, to) = self.editor.ordered();
        let (start, _) = line_around(text, from);
        let (_, end) = line_around(text, to);
        (start, end)
    }

    /// Insert code block and Insert math block, as Obsidian's: on an empty line the fences with
    /// the caret between them; else the lines the selection touches go between fences, the
    /// selection kept.
    fn fence_lines(&mut self, fence: &str) -> Result<(), String> {
        let text = self.editor_text();
        let (start, end) = self.selected_lines(&text);
        let lines = text.get(start..end).unwrap_or("").to_owned();
        if lines.trim().is_empty() {
            self.replace(start, end, &format!("{fence}\n\n{fence}"), false)?;
            let caret = start + fence.len() + 1;
            self.editor.anchor = caret;
            self.editor.focus = caret;
            return Ok(());
        }
        let (anchor, focus) = (self.editor.anchor, self.editor.focus);
        self.replace(start, end, &format!("{fence}\n{lines}\n{fence}"), false)?;
        self.editor.anchor = anchor + fence.len() + 1;
        self.editor.focus = focus + fence.len() + 1;
        Ok(())
    }

    /// Insert horizontal rule and Insert table, as Obsidian's: at the caret, after a blank line
    /// (a line break first on an empty line, two after text), the caret `caret` bytes into it.
    fn insert_at_caret(&mut self, block: &str, caret: usize) -> Result<(), String> {
        let text = self.editor_text();
        let (from, to) = self.editor.ordered();
        let (line_start, _) = line_around(&text, from);
        let lead = if text.get(line_start..from).is_some_and(|before| before.trim().is_empty()) { "\n" } else { "\n\n" };
        self.replace(from, to, &format!("{lead}{block}"), false)?;
        let at = from + lead.len() + caret;
        self.editor.anchor = at;
        self.editor.focus = at;
        Ok(())
    }

    /// Insert callout, as Obsidian's: the lines the selection touches go in a note callout; on an
    /// empty line a new one with its title and contents; its type selected, to be typed over.
    fn insert_callout(&mut self) -> Result<(), String> {
        let text = self.editor_text();
        let (start, end) = self.selected_lines(&text);
        let lines = text.get(start..end).unwrap_or("").to_owned();
        let at = if lines.trim().is_empty() {
            let (from, to) = self.editor.ordered();
            self.replace(from, to, "\n> [!NOTE] Title\n> Contents\n", false)?;
            from + 1
        } else {
            let quoted: Vec<String> = lines.split('\n').map(|line| format!("> {line}")).collect();
            self.replace(start, end, &format!("> [!NOTE]\n{}", quoted.join("\n")), false)?;
            start
        };
        self.editor.anchor = at + 4;
        self.editor.focus = at + 8;
        Ok(())
    }
}

/// What the editor's `openLink` dialog request carries for `link`.
fn open_link_params(link: &kition_note_markdown::Link) -> Value {
    let (heading, block) = match &link.anchor {
        Some(kition_note_markdown::Anchor::Heading(heading)) => (Some(heading.clone()), None),
        Some(kition_note_markdown::Anchor::Block(block)) => (None, Some(block.clone())),
        None => (None, None),
    };
    json!({ "kind": link.kind.name(), "destination": link.destination, "file": link.file, "heading": heading, "blockId": block, "external": link.is_external() })
}

impl NoteDocument {
    /// The pointer moved over (`x`, `y`) with no button held: the heading under it shows its fold
    /// chevron, and a link drawn as a link or a chevron takes the pointing hand.
    fn hover(&mut self, x: f32, y: f32) -> ToolResponse {
        let (heading, pointing, at) = {
            let view = self.view();
            let chevron = view.fold_at(x, y);
            let heading = view.heading_at(y).or_else(|| chevron.map(|at| view.styles().line_of(at)));
            let at = view.hit(x, y);
            let hidden = |byte: usize| view.styles().bytes.get(byte).is_some_and(|value| value & kition_note_view::style::flag::HIDE != 0);
            let link = view.links().iter().any(|link| link.range.start < at && at < link.range.end && hidden(link.range.start));
            (heading, link || chevron.is_some(), at)
        };
        self.view.borrow_mut().set_hover(heading);
        self.editor.cursor = if pointing {
            "pointer"
        } else if self.task_box_line(x, y, at).is_some() {
            "default"
        } else {
            "text"
        };
        self.respond()
    }

    /// The line (from 1) of the task whose box is under (`x`, `y`), where the pointer's byte is
    /// `at`: on its `[`, `x` or `]` (flagged by the view), or in the box's area (the hidden marker
    /// before it hit tests to the line's start, so the box's left half lands there).
    fn task_box_line(&self, x: f32, y: f32, at: usize) -> Option<usize> {
        let view = self.view();
        let is_task = |byte: usize| view.styles().bytes.get(byte).is_some_and(|value| value & kition_note_view::style::flag::TASK != 0);
        let text = self.body();
        let (line_start, line_end) = line_around(text, at);
        let in_box = (line_start..line_end).find(|byte| is_task(*byte) && text.get(*byte..*byte + 1) == Some("[")).is_some_and(|open| {
            let caret = view.caret(open);
            let area = view.theme().body_size * 1.9375;
            x >= caret.x - 2.0 && x < caret.x + area && y >= caret.top - 2.0 && y < caret.top + caret.height + 2.0
        });
        (is_task(at) || (at > 0 && is_task(at - 1)) || in_box).then(|| text.get(..at).map_or(0, |before| before.matches('\n').count()) + 1)
    }

    /// A press on what the note draws besides text (a fold chevron, a link, a task's box).
    fn pointer_widget(&mut self, event: &ToolPointer, at: usize) -> Result<Option<ToolResponse>, String> {
        if event.action == PointerAction::Down {
            let chevron = self.view().fold_at(event.x as f32, event.y as f32);
            if let Some(heading) = chevron {
                self.toggle_fold(heading);
                return Ok(Some(self.respond()));
            }
        }
        if event.action == PointerAction::Down && !event.mods.shift {
            // A click on a link drawn as a link (its syntax hidden), or Cmd+click (Ctrl+click) on
            // one shown as Markdown, asks the editor to open it (the host opens it).
            let request = {
                let view = self.view();
                let hidden = |byte: usize| view.styles().bytes.get(byte).is_some_and(|value| value & kition_note_view::style::flag::HIDE != 0);
                view.links()
                    .iter()
                    .find(|link| {
                        link.range.start <= at && at < link.range.end && (event.mods.command || event.mods.control || hidden(link.range.start))
                    })
                    .map(open_link_params)
            };
            if let Some(params) = request {
                let mut response = self.respond();
                response.requests.push(ToolRequest::Dialog { name: "openLink".into(), params });
                return Ok(Some(response));
            }
            // A click on a task's box checks or unchecks it.
            let task_line = self.task_box_line(event.x as f32, event.y as f32, at);
            if !self.is_read_only()
                && let Some(line) = task_line
            {
                self.stop_typing();
                self.change(at, at, |doc| doc.execute("task.toggle", json!({ "line": line })).map(|_| ()))?;
                return Ok(Some(self.respond()));
            }
        }
        Ok(None)
    }

    /// Drops the text `from..to` at `drop` (outside it): moved there, or copied with `copy`, one
    /// undo step, the dropped text selected.
    fn drop_selection(&mut self, from: usize, to: usize, drop: usize, copy: bool) -> Result<(), String> {
        let text = self.editor_text();
        let moved = text.get(from..to).unwrap_or("").to_owned();
        let at = self.grouped(|doc| {
            if copy {
                doc.replace(drop, drop, &moved, false)?;
                Ok(drop)
            } else if drop > to {
                doc.replace(drop, drop, &moved, false)?;
                doc.replace(from, to, "", false)?;
                Ok(drop - (to - from))
            } else {
                doc.replace(from, to, "", false)?;
                doc.replace(drop, drop, &moved, false)?;
                Ok(drop)
            }
        })?;
        self.editor.anchor = at;
        self.editor.focus = at + moved.len();
        Ok(())
    }

    /// A press, drag, or multi-click on the text.
    fn pointer_text(&mut self, event: &ToolPointer, at: usize) -> Result<ToolResponse, String> {
        // A press soon after a double click, where it was, is a triple click: the line.
        if event.action == PointerAction::Down
            && let Some((when, x, y)) = self.editor.double.take()
            && crate::clock::now_millis() - when < 500
            && (event.x - x).abs() < 4.0
            && (event.y - y).abs() < 4.0
        {
            let text = self.editor_text();
            let (start, end) = line_around(&text, at);
            self.stop_typing();
            // The line without its line break, as Obsidian selects it.
            self.editor.anchor = start;
            self.editor.focus = end;
            self.editor.consumed = true;
            self.editor.goal = None;
            return Ok(self.respond());
        }
        if matches!(event.action, PointerAction::Down) {
            self.editor.pairs.clear();
        }
        // A press in the selection drags it (CodeMirror's drag and drop): the release moves the
        // text there (Alt copies it), selected; a release where it was pressed puts the caret there.
        let (from, to) = self.editor.ordered();
        if event.action == PointerAction::Down && !event.mods.shift && from < to && from <= at && at <= to && !self.is_read_only() {
            self.editor.dragged = Some((from, to, at, None));
            return Ok(self.respond());
        }
        if let Some((from, to, press, _)) = self.editor.dragged {
            match event.action {
                PointerAction::Drag => {
                    self.editor.dragged = Some((from, to, press, (at != press).then_some(at)));
                    return Ok(self.respond());
                }
                PointerAction::Up => {
                    self.editor.dragged = None;
                    let drop = self.view().clamp(at);
                    match drop {
                        drop if drop < from || drop > to => self.drop_selection(from, to, drop, event.mods.alt)?,
                        _ if at == press => self.move_to(press, false),
                        _ => self.move_to(from, false),
                    }
                    self.editor.goal = None;
                    return Ok(self.respond());
                }
                _ => self.editor.dragged = None,
            }
        }
        match event.action {
            PointerAction::Down => self.move_to(at, event.mods.shift),
            PointerAction::Drag => self.editor.focus = at,
            PointerAction::DoubleClick => {
                let text = self.editor_text();
                let (start, end) = word_at(&text, at);
                self.stop_typing();
                self.editor.anchor = start;
                self.editor.focus = end;
                self.editor.double = Some((crate::clock::now_millis(), event.x, event.y));
            }
            PointerAction::Up | PointerAction::Move => {}
        }
        self.leave_front_matter();
        self.editor.goal = None;
        Ok(self.respond())
    }
}

impl KitTools for NoteDocument {
    fn tools(&self) -> Vec<ToolInfo> {
        vec![ToolInfo { id: "text".into(), label: "Text".into(), shortcut: None, icon: "tool-type".into(), group: 0 }]
    }

    fn tool(&self) -> String {
        "text".into()
    }

    fn select_tool(&mut self, id: &str, _zoom: f64) -> Result<ToolResponse, String> {
        if id != "text" {
            return Err(format!("no tool {id}"));
        }
        Ok(self.respond())
    }

    fn pointer(&mut self, event: &ToolPointer) -> Result<ToolResponse, String> {
        if event.action == PointerAction::Move {
            return Ok(self.hover(event.x as f32, event.y as f32));
        }
        let at = self.view().hit(event.x as f32, event.y as f32);
        let engage = event.action == PointerAction::Down;
        if event.action == PointerAction::Down {
            self.editor.consumed = false;
        }
        if event.action == PointerAction::Drag && self.editor.consumed {
            return Ok(self.respond());
        }
        let consumed = self.pointer_widget(event, at)?;
        if let Some(response) = consumed {
            self.editor.consumed = true;
            return Ok(response);
        }
        if engage {
            self.editor.engaged = true;
        }
        self.pointer_text(event, at)
    }

    fn key(&mut self, key: &str, mods: ToolMods, _zoom: f64) -> Result<ToolResponse, String> {
        self.editor.engaged = true;
        match key {
            // Alt+Enter follows the link at the caret, and does nothing elsewhere (Obsidian's
            // Follow link under cursor).
            "Enter" if mods.alt && !mods.command && !mods.shift => {
                let focus = self.editor.focus;
                let request = self.view().links().iter().find(|link| link.range.start <= focus && focus <= link.range.end).map(open_link_params);
                let mut response = self.respond();
                if let Some(params) = request {
                    response.requests.push(ToolRequest::Dialog { name: "openLink".into(), params });
                }
                return Ok(response);
            }
            "Backspace" => self.delete(false, mods)?,
            "Delete" => self.delete(true, mods)?,
            "Enter" if !mods.command && mods.shift => self.shift_enter()?,
            "Enter" if !mods.command => self.enter()?,
            "Tab" => self.tab(mods.shift)?,
            // Alt+Up and Alt+Down move by paragraphs (macOS), as Obsidian does.
            "ArrowUp" | "ArrowDown" if mods.alt && !mods.command && !mods.control => self.paragraph_move(key == "ArrowUp", mods.shift),
            "Escape" => {
                let focus = self.editor.focus;
                self.move_to(focus, false);
            }
            _ => self.navigate(key, mods),
        }
        self.unfold_at_caret();
        Ok(self.respond())
    }

    fn text(&mut self, text: &str, _zoom: f64) -> Result<ToolResponse, String> {
        self.editor.engaged = true;
        self.type_text(text)?;
        Ok(self.respond())
    }

    fn state(&mut self, _x: f64, _y: f64, _mods: ToolMods, _zoom: f64) -> ToolResponse {
        self.respond()
    }
}

#[cfg(test)]
mod tests;
