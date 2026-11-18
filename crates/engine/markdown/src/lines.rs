//! Line starts of a text and conversions between its byte offsets and the two position units
//! callers count in (rule M18): Unicode scalar values and UTF-16 code units.

use std::ops::Range;

/// The lines of one text. Lines end after `\n`; a `\r` belongs to its line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineTable {
    /// Byte offset of each line's start; the first is 0. A text ending in `\n` has an empty last
    /// line after it.
    starts: Vec<usize>,
    /// Scalar values before each line.
    scalars: Vec<usize>,
    /// UTF-16 code units before each line.
    units: Vec<usize>,
    len: usize,
}

impl LineTable {
    pub fn new(text: &str) -> Self {
        let mut table = Self { starts: vec![0], scalars: vec![0], units: vec![0], len: text.len() };
        let (mut scalars, mut units) = (0usize, 0usize);
        for (at, c) in text.char_indices() {
            scalars += 1;
            units += c.len_utf16();
            if c == '\n' {
                table.starts.push(at + 1);
                table.scalars.push(scalars);
                table.units.push(units);
            }
        }
        table
    }

    /// How many lines the text has (an empty text has one, empty).
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// The 0-based line holding byte `at` (the last line for `at` past the end).
    pub fn line_of(&self, at: usize) -> usize {
        match self.starts.binary_search(&at) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        }
    }

    /// The bytes of 0-based `line`, with its line ending.
    pub fn line(&self, line: usize) -> Option<Range<usize>> {
        let start = *self.starts.get(line)?;
        let end = self.starts.get(line + 1).copied().unwrap_or(self.len);
        Some(start..end)
    }

    /// The bytes of 0-based `line` without its `\n` (a `\r` before it is kept).
    pub fn content(&self, text: &str, line: usize) -> Option<Range<usize>> {
        let range = self.line(line)?;
        let end = if text.as_bytes().get(range.end.wrapping_sub(1)) == Some(&b'\n') && range.end > range.start { range.end - 1 } else { range.end };
        Some(range.start..end)
    }

    /// Byte offset `at` (on a char boundary) as a scalar index.
    pub fn scalar(&self, text: &str, at: usize) -> usize {
        let line = self.line_of(at);
        let start = self.starts.get(line).copied().unwrap_or(0);
        self.scalars.get(line).copied().unwrap_or(0) + text.get(start..at).map_or(0, |part| part.chars().count())
    }

    /// Byte offset `at` (on a char boundary) as a UTF-16 index.
    pub fn utf16(&self, text: &str, at: usize) -> usize {
        let line = self.line_of(at);
        let start = self.starts.get(line).copied().unwrap_or(0);
        self.units.get(line).copied().unwrap_or(0) + text.get(start..at).map_or(0, |part| part.encode_utf16().count())
    }

    /// The byte offset of scalar index `index`, or `None` past the end.
    pub fn byte_of_scalar(&self, text: &str, index: usize) -> Option<usize> {
        let line = match self.scalars.binary_search(&index) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        };
        let start = *self.starts.get(line)?;
        let left = index.checked_sub(*self.scalars.get(line)?)?;
        let rest = text.get(start..)?;
        match rest.char_indices().nth(left) {
            Some((offset, _)) => Some(start + offset),
            None if rest.chars().count() == left => Some(text.len()),
            None => None,
        }
    }

    /// The byte offset of UTF-16 index `index`; `None` past the end or inside a character.
    pub fn byte_of_utf16(&self, text: &str, index: usize) -> Option<usize> {
        let line = match self.units.binary_search(&index) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        };
        let start = *self.starts.get(line)?;
        let mut units = *self.units.get(line)?;
        for (offset, c) in text.get(start..)?.char_indices() {
            if units == index {
                return Some(start + offset);
            }
            units += c.len_utf16();
            if units > index {
                return None;
            }
        }
        if units == index { Some(text.len()) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_units() {
        let text = "a\u{1F600}\nb\u{e9}\r\n\nend";
        let table = LineTable::new(text);
        assert_eq!(table.count(), 4);
        assert_eq!(table.line_of(0), 0);
        assert_eq!(table.line_of(6), 1);
        assert_eq!(table.line(1), Some(6..11));
        assert_eq!(table.content(text, 1), Some(6..10));
        assert_eq!(table.line(3), Some(12..15));
        let b = 6;
        assert_eq!(table.scalar(text, b), 3);
        assert_eq!(table.utf16(text, b), 4);
        assert_eq!(table.byte_of_scalar(text, 3), Some(b));
        assert_eq!(table.byte_of_utf16(text, 4), Some(b));
        assert_eq!(table.byte_of_utf16(text, 2), None, "inside the surrogate pair");
        assert_eq!(table.byte_of_scalar(text, text.chars().count()), Some(text.len()));
        assert_eq!(table.byte_of_scalar(text, text.chars().count() + 1), None);
        assert_eq!(table.byte_of_utf16(text, text.encode_utf16().count()), Some(text.len()));
    }

    #[test]
    fn an_empty_text_has_one_line() {
        let table = LineTable::new("");
        assert_eq!(table.count(), 1);
        assert_eq!(table.line(0), Some(0..0));
        assert_eq!(table.byte_of_scalar("", 0), Some(0));
    }
}
