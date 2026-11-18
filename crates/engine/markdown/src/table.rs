//! GFM tables with every cell's source span: the segment between two pipes (padding included)
//! and its trimmed content. A row is one line; the delimiter row is kept apart.

use std::ops::Range;

use crate::lines::LineTable;
use crate::sub;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

impl Align {
    pub fn from_cmark(align: pulldown_cmark::Alignment) -> Self {
        match align {
            pulldown_cmark::Alignment::None => Self::None,
            pulldown_cmark::Alignment::Left => Self::Left,
            pulldown_cmark::Alignment::Center => Self::Center,
            pulldown_cmark::Alignment::Right => Self::Right,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Between the pipes (or a line edge and a pipe), padding included.
    pub segment: Range<usize>,
    /// The segment without its surrounding white space.
    pub content: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The whole line, with its line ending.
    pub line: Range<usize>,
    /// From the row's first character (after a quote's `> `) to the line's end, without the ending.
    pub body: Range<usize>,
    pub leading_pipe: bool,
    pub trailing_pipe: bool,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub range: Range<usize>,
    pub align: Vec<Align>,
    /// Row 0 is the header; the delimiter row is not among them.
    pub rows: Vec<Row>,
    pub delimiter: Row,
}

impl Table {
    pub fn columns(&self) -> usize {
        self.rows.first().map_or(0, |header| header.cells.len())
    }

    /// Every row and the delimiter row, in source order.
    pub fn lines(&self) -> Vec<&Row> {
        let mut all: Vec<&Row> = self.rows.iter().collect();
        all.insert(1.min(all.len()), &self.delimiter);
        all
    }

    /// Whether the cells are padded to aligned columns (any segment with more than one space on a
    /// side).
    pub fn padded(&self, text: &str) -> bool {
        self.lines().iter().flat_map(|row| row.cells.iter()).any(|cell| {
            let raw = sub(text, cell.segment.clone());
            raw.len() - raw.trim_end().len() > 1 || raw.len() - raw.trim_start().len() > 1
        })
    }
}

/// The cells of one row body (`start..end` of the text, without the line ending). Escaped pipes
/// do not split.
pub fn row(text: &str, line: Range<usize>, start: usize, end: usize) -> Row {
    let body = sub(text, start..end).trim_end_matches('\r');
    let end = start + body.len();
    let content_end = start + body.trim_end().len();
    let lead_ws = body.len() - body.trim_start().len();
    let bytes = body.as_bytes();
    let mut pipes = Vec::new();
    let mut at = 0;
    // GFM: only an escaped pipe does not split, also inside a code span.
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 1,
            b'|' => pipes.push(start + at),
            _ => {}
        }
        at += 1;
    }
    let leading = pipes.first() == Some(&(start + lead_ws));
    let trailing = pipes.last().is_some_and(|pipe| pipe + 1 == content_end) && !(leading && pipes.len() == 1);
    let inner = pipes.get(usize::from(leading)..pipes.len().saturating_sub(usize::from(trailing))).unwrap_or_default();
    let mut cells = Vec::new();
    let mut prev = if leading { start + lead_ws + 1 } else { start };
    for pipe in inner {
        cells.push(cell(text, prev..*pipe));
        prev = pipe + 1;
    }
    let last_end = if trailing { pipes.last().copied().unwrap_or(content_end) } else { end };
    cells.push(cell(text, prev..last_end.max(prev)));
    Row { line, body: start..end, leading_pipe: leading, trailing_pipe: trailing, cells }
}

fn cell(text: &str, segment: Range<usize>) -> Cell {
    let raw = sub(text, segment.clone());
    let lead = raw.len() - raw.trim_start().len();
    let content_len = raw.trim().len();
    Cell { content: segment.start + lead..segment.start + lead + content_len, segment }
}

/// The table pulldown-cmark found at `range`, its rows starting at `starts` (header first).
pub fn build(text: &str, lines: &LineTable, range: Range<usize>, align: Vec<Align>, starts: &[Range<usize>]) -> Option<Table> {
    let header_start = starts.first()?.start;
    let header_line = lines.line_of(header_start);
    let prefix = header_start - lines.line(header_line)?.start;
    let mut rows = Vec::new();
    for start in starts {
        let line = lines.line_of(start.start);
        let content = lines.content(text, line)?;
        rows.push(row(text, lines.line(line)?, start.start, content.end));
    }
    let delimiter_line = header_line + 1;
    let delimiter_range = lines.line(delimiter_line)?;
    let delimiter_content = lines.content(text, delimiter_line)?;
    let delimiter = row(text, delimiter_range.clone(), (delimiter_range.start + prefix).min(delimiter_content.end), delimiter_content.end);
    Some(Table { range, align, rows, delimiter })
}
