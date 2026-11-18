//! `kition-note-edit`: the note's minimal-span edits (`docs/design.md` section 3).
//!
//! Every function reads the text and its index ([`kition_note_markdown::Index`]) and answers the
//! smallest list of [`Edit`]s that does the job, never the new text: the engine applies them as
//! one KDM transaction, so concurrent edits elsewhere survive and an unchanged byte is never
//! rewritten. Ranges are byte ranges of the text. Reformatting a whole construct is always its own
//! function (`table::format`), never a side effect.

use std::fmt;
use std::ops::Range;

use kition_note_markdown::{Heading, Index, sub};

pub mod block;
pub mod find;
pub mod links;
pub mod props;
pub mod section;
pub mod table;

/// Replace `range` (bytes) with `insert`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub insert: String,
}

impl Edit {
    pub fn new(range: Range<usize>, insert: impl Into<String>) -> Self {
        Self { range, insert: insert.into() }
    }

    pub fn insert(at: usize, insert: impl Into<String>) -> Self {
        Self { range: at..at, insert: insert.into() }
    }

    pub fn delete(range: Range<usize>) -> Self {
        Self { range, insert: String::new() }
    }
}

/// Why an edit was refused: the A18 reasons of the design (3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    InvalidParams,
    NotFound,
    Ambiguous,
    OutOfRange,
    Stale,
    Refused,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidParams => "invalid-params",
            Self::NotFound => "not-found",
            Self::Ambiguous => "ambiguous",
            Self::OutOfRange => "out-of-range",
            Self::Stale => "stale",
            Self::Refused => "refused",
        }
    }
}

/// A refused edit: its reason, a check code when one describes it, and one English sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError {
    pub reason: Reason,
    pub code: Option<&'static str>,
    pub message: String,
}

impl EditError {
    pub fn new(reason: Reason, message: impl Into<String>) -> Self {
        Self { reason, code: None, message: message.into() }
    }

    pub fn coded(reason: Reason, code: &'static str, message: impl Into<String>) -> Self {
        Self { reason, code: Some(code), message: message.into() }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            Some(code) => write!(f, "{}: {code}: {}", self.reason.as_str(), self.message),
            None => write!(f, "{}: {}", self.reason.as_str(), self.message),
        }
    }
}

impl std::error::Error for EditError {}

pub type Result<T> = std::result::Result<T, EditError>;

pub(crate) fn invalid(message: impl Into<String>) -> EditError {
    EditError::new(Reason::InvalidParams, message)
}

pub(crate) fn not_found(message: impl Into<String>) -> EditError {
    EditError::new(Reason::NotFound, message)
}

/// `text` with `edits` applied (sorted by start, not overlapping).
pub fn apply(text: &str, edits: &[Edit]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        out.push_str(sub(text, at..edit.range.start.max(at)));
        out.push_str(&edit.insert);
        at = edit.range.end.max(at);
    }
    out.push_str(sub(text, at..text.len()));
    out
}

/// Sorts `edits` by start and checks that they do not overlap.
pub fn normalize(mut edits: Vec<Edit>) -> Result<Vec<Edit>> {
    edits.retain(|edit| !(edit.range.is_empty() && edit.insert.is_empty()));
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    for pair in edits.windows(2) {
        if let [a, b] = pair
            && a.range.end > b.range.start
        {
            return Err(EditError::new(Reason::Refused, "the change would overlap itself"));
        }
    }
    Ok(edits)
}

/// One edit turning `range` into `new`, shrunk to the part that differs (none when equal).
pub fn minimal(text: &str, range: Range<usize>, new: &str) -> Vec<Edit> {
    let old = sub(text, range.clone());
    if old == new {
        return Vec::new();
    }
    let prefix: usize = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum();
    let (old_rest, new_rest) = (sub(old, prefix..old.len()), sub(new, prefix..new.len()));
    let suffix: usize = old_rest.chars().rev().zip(new_rest.chars().rev()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum();
    let suffix = suffix.min(old_rest.len()).min(new_rest.len());
    vec![Edit::new(range.start + prefix..range.end - suffix, sub(new_rest, 0..new_rest.len() - suffix))]
}

/// The line ending a construct is written with: `\r\n` when its line has one (only a note with
/// mixed endings keeps `\r` in its text), `\n` otherwise.
pub fn eol_of(text: &str, line: Range<usize>) -> &'static str {
    if sub(text, line).ends_with("\r\n") { "\r\n" } else { "\n" }
}

/// Whether the line before byte `at` (a line start) is blank or there is none.
fn blank_before(text: &str, at: usize) -> bool {
    let before = sub(text, 0..at);
    if before.is_empty() {
        return true;
    }
    let without = before.strip_suffix('\n').unwrap_or(before);
    let line = sub(without, without.rfind('\n').map_or(0, |offset| offset + 1)..without.len());
    line.trim().is_empty()
}

/// Whether the line at byte `at` is blank or there is none.
fn blank_at(text: &str, at: usize) -> bool {
    let rest = sub(text, at..text.len());
    rest.is_empty() || rest.split('\n').next().is_some_and(|line| line.trim().is_empty())
}

/// Inserts `markdown` as a block at the line start `at`, adding blank lines only where Markdown
/// needs them to keep it a block of its own.
pub fn insert_block(text: &str, at: usize, markdown: &str) -> Edit {
    let body = markdown.trim_matches(['\n', '\r']);
    let mut insert = String::new();
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    if at > 0 && !sub(text, 0..at).ends_with('\n') {
        insert.push('\n');
        insert.push('\n');
    } else if !blank_before(text, at) {
        insert.push('\n');
    }
    insert.push_str(body);
    insert.push('\n');
    if !blank_at(text, at) {
        insert.push('\n');
    }
    Edit::insert(at, insert)
}

/// How a command names a heading: by its path from the top or by its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadingRef {
    /// Each element is a heading's plain text, from the top.
    Path(Vec<String>),
    /// The heading's line, from 1.
    Line(usize),
}

/// The heading `at` names. A path matches from the top; when no heading has that full path, a
/// unique heading whose path ends with it is taken. Two matches are `ambiguous`, with their lines.
pub fn find_heading<'a>(ix: &'a Index, at: &HeadingRef) -> Result<&'a Heading> {
    match at {
        HeadingRef::Line(line) => ix
            .headings
            .iter()
            .find(|heading| ix.lines.line_of(heading.range.start) + 1 == *line)
            .ok_or_else(|| not_found(format!("line {line} is not a heading"))),
        HeadingRef::Path(path) => {
            if path.is_empty() {
                return Err(invalid("the heading path is empty"));
            }
            let full: Vec<&Heading> = ix.headings.iter().filter(|heading| heading.path == *path).collect();
            let found = if full.is_empty() { ix.headings.iter().filter(|heading| heading.path.ends_with(path)).collect() } else { full };
            match found.as_slice() {
                [] => Err(not_found(format!("no heading has the path {}", path.join(" / ")))),
                [one] => Ok(one),
                many => Err(EditError::new(
                    Reason::Ambiguous,
                    format!(
                        "{} headings match {}; name one by line: {}",
                        many.len(),
                        path.join(" / "),
                        many.iter().map(|heading| (ix.lines.line_of(heading.range.start) + 1).to_string()).collect::<Vec<_>>().join(", ")
                    ),
                )),
            }
        }
    }
}

/// The section's end without its trailing blank lines.
pub fn content_end(text: &str, range: Range<usize>) -> usize {
    let source = sub(text, range.clone());
    let trimmed = source.trim_end_matches(['\n', '\r', ' ', '\t']);
    if trimmed.len() == source.len() {
        return range.end;
    }
    // Keep the last content line's ending.
    let end = range.start + trimmed.len();
    let rest = sub(text, end..range.end);
    let newline = rest.find('\n').map_or(0, |offset| offset + 1);
    end + newline
}

#[cfg(test)]
mod tests;
