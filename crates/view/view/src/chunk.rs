//! Parsing a large note in chunks, so an edit parses only its own part. A note is cut at blank
//! lines that are not inside a fenced code block or a metadata block (the front matter, or a
//! `---` block pulldown-cmark reads as one later in the note; `$$` math is inline, in a paragraph
//! a blank line ends); each
//! chunk is parsed on its own (its styles, tables, links, callouts) and memoised by its text, and
//! the chunks' results are joined with their offsets moved. Markdown's blocks end at blank lines
//! outside those constructs, so a chunk parses as it does in the whole note (a reference link whose
//! definition is in another chunk is the exception: it draws as text until the two meet).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use kition_note_markdown::{Callout, Link, Table};

use crate::style::Styles;

/// One chunk parsed: its styles (offsets from the chunk's start) and what the index found in it.
pub struct Parsed {
    pub styles: Styles,
    pub tables: Vec<Table>,
    pub links: Vec<Link>,
    pub callouts: Vec<Callout>,
    pub summary: Summary,
    /// The whole note's chunks (empty for one chunk's parse), for an edit's update.
    pub ranges: Vec<Range<usize>>,
    /// The whole note's link reference labels (empty for one chunk's parse).
    pub labels: std::collections::BTreeMap<String, String>,
    /// The fenced code blocks with a language, not highlighted in `styles` yet (ranges and
    /// languages): see [`Styles::light`].
    pub code: crate::style::CodeBlocks,
}

impl Parsed {
    /// The styles with every code block highlighted, as [`Styles::of_with`] makes them.
    #[cfg(test)]
    pub(crate) fn lit(&self, text: &str) -> Styles {
        let mut styles = self.styles.clone();
        for (range, language) in &self.code {
            let tokens = styles.tokens(text, range, language);
            styles.light(&tokens, 0);
        }
        styles
    }
}

/// The rest of what the index found, in short (for the note's summary, `inspect`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// Each heading's level, plain text, and first byte.
    pub headings: Vec<(u8, String, usize)>,
    pub tags: usize,
    pub block_ids: usize,
    pub tasks_open: usize,
    pub tasks_done: usize,
    pub words: usize,
    pub callouts: usize,
    /// The front matter (the note's first chunk only).
    pub frontmatter: Option<kition_note_markdown::Frontmatter>,
}

/// The note's chunks, as byte ranges that cover it, each ending after a line break (but the last).
pub fn chunks(text: &str) -> Vec<Range<usize>> {
    chunks_closed(text, true).0
}

/// As [`chunks`], and whether the text ends outside a fenced code block and a metadata block,
/// with no `---` whose closing line could be past it (a part of a note that ends one open, or may,
/// shapes the note's later chunks too).
/// `starts_note`: the text starts the note, so its front matter (as the index finds it, which
/// takes blank lines that pulldown-cmark's does not) is kept in one chunk.
pub(crate) fn chunks_closed(text: &str, starts_note: bool) -> (Vec<Range<usize>>, bool) {
    let held = if starts_note { kition_note_markdown::frontmatter::locate(text).map_or(0, |(_, close)| close.end) } else { 0 };
    let mut out = Vec::new();
    let mut start = 0;
    let mut at = 0;
    // The open fence: its marker, its length, and the content column of the list item it is in
    // (0 outside lists): a line indented less ends the item, and the fence with it.
    let mut fence: Option<(char, usize, usize)> = None;
    // In a metadata block (`---` … `---` or `...`): the front matter, or one later in the note,
    // which pulldown-cmark reads wherever a block starts with `---`.
    let mut frontmatter = false;
    // Whether a `---` that would open one has its closing line past the text (a part of a note
    // can not tell how the note reads it).
    let mut beyond = false;
    // The open list items' content columns, and whether the last line was paragraph text (a
    // less indented line after it can be its lazy continuation, which keeps the items open).
    let mut items: Vec<usize> = Vec::new();
    let mut paragraph = false;
    // Whether that paragraph is in a quote: a line without the quote's `>` is no paragraph text
    // to it but a lazy line, and an item there starts a list outside the quote.
    let mut quoted = false;
    // A fence open in a quote (its marker and length): its lines are no paragraph text, and a
    // line without the quote's `>` ends it.
    let mut quote_fence: Option<(char, usize)> = None;
    // Whether the last line opened an empty list item: a blank line after it ends it (an item
    // begins with at most one blank line).
    let mut empty_item = false;
    // In a table's rows (from its header on): its lines are no paragraph text, and a line that
    // starts a block (a `---` too) ends it.
    let mut table = false;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    // For each line, the first line from it on that closes a metadata block.
    let mut closing: Vec<Option<usize>> = vec![None; lines.len()];
    let mut next_closing = None;
    for (number, line) in lines.iter().enumerate().rev() {
        if closes_metadata(line) {
            next_closing = Some(number);
        }
        if let Some(slot) = closing.get_mut(number) {
            *slot = next_closing;
        }
    }
    for (number, line) in lines.iter().copied().enumerate() {
        let content = line.trim_end_matches(['\n', '\r']);
        let trimmed = content.trim_start();
        let line_start = at;
        at += line.len();
        if frontmatter {
            if closes_metadata(line) {
                frontmatter = false;
            }
            continue;
        }
        let blank = trimmed.is_empty();
        let indent = columns(content);
        if std::mem::take(&mut empty_item) && blank && fence.is_none() {
            items.pop();
        }
        if let Some((marker, length, inside)) = fence {
            if blank || indent >= inside {
                // (Indented four columns past its item, a fence is code: it closes nothing.)
                let run = trimmed.chars().take_while(|c| *c == marker).count();
                if run >= length && indent - inside.min(indent) < 4 && trimmed.chars().skip(run).all(char::is_whitespace) {
                    fence = None;
                }
                continue;
            }
            // Its list item ended: the line is read as any other.
            fence = None;
        }
        // A block starting with `---` (not a heading's underline) whose next line is text and
        // which a later line closes is a metadata block, as pulldown-cmark reads it.
        let block_start = !paragraph || quoted || !items.is_empty();
        if fence.is_none()
            && block_start
            && content.strip_prefix("---").is_some_and(|rest| rest.bytes().all(|byte| byte.is_ascii_whitespace()))
            && line.ends_with('\n')
            && lines.get(number + 1).is_some_and(|next| !next.trim().is_empty() && !closes_metadata(next))
        {
            if closing.get(number + 1).copied().flatten().is_some() {
                frontmatter = true;
                (paragraph, quoted, quote_fence, empty_item, table) = (false, false, None, false, false);
                items.clear();
                continue;
            }
            beyond = true;
        }
        // A list item, unless after paragraph text it would start a list it cannot interrupt the
        // paragraph with (empty, or numbered from other than 1: the paragraph's text, or a
        // heading's underline).
        let opens_item = item_width(trimmed, indent).is_some()
            && (!paragraph || quoted || items.last().is_some_and(|column| indent < *column) || interrupts(trimmed));
        if !blank {
            // The items the line is not in (indented less, and no lazy continuation).
            let lazy = paragraph && !opens_item && !(starts_block(trimmed) && indent < items.last().copied().unwrap_or(0) + 4);
            if !lazy {
                while items.last().is_some_and(|column| indent < *column) {
                    items.pop();
                }
            }
        }
        // The list items the line opens (`- - text` opens two), and what follows their markers.
        let mut rest = trimmed;
        let mut column = indent;
        if opens_item {
            while let Some((width, marker)) = item_width(rest, column) {
                items.push(column + width);
                column += width;
                rest = rest.get(marker..).unwrap_or("").trim_start_matches([' ', '\t']);
            }
            empty_item = rest.is_empty();
        }
        let marker = rest.chars().next().filter(|c| *c == '`' || *c == '~');
        // (Indented four columns past its item, it is indented code.)
        let container = items.last().copied().unwrap_or(0);
        let opens_at = if rest.len() == trimmed.len() { indent } else { column };
        if let Some(marker) = marker.filter(|_| opens_at < container + 4) {
            let run = rest.chars().take_while(|c| *c == marker).count();
            // (A backtick fence's info string has no backtick: such a line is inline code.)
            if run >= 3 && !(marker == '`' && rest.get(run..).is_some_and(|info| info.contains('`'))) {
                fence = Some((marker, run, container));
                // (A line without the quote's `>` ends the quote, and a fence in it.)
                (paragraph, quoted, quote_fence, table) = (false, false, None, false);
                continue;
            }
        }
        // (A line indented four columns past its item, not after paragraph text, is code.)
        let code = !paragraph && !opens_item && indent >= container + 4;
        // (Nor does a block start four columns past its item: it is the paragraph's text.)
        let lazy = paragraph && !opens_item && !(starts_block(trimmed) && opens_at < container + 4);
        if let Some(inner) = rest.strip_prefix('>').filter(|_| opens_at < container + 4) {
            // A quote's own text (nested quotes and all).
            let inner = inner.trim_start_matches(['>', ' ', '\t']);
            let run = |marker: char| inner.chars().take_while(|c| *c == marker).count();
            if let Some((marker, length)) = quote_fence {
                if run(marker) >= length && inner.chars().skip(run(marker)).all(char::is_whitespace) {
                    quote_fence = None;
                }
                paragraph = false;
            } else if let Some(marker) = inner.chars().next().filter(|c| (*c == '`' || *c == '~') && run(*c) >= 3) {
                quote_fence = Some((marker, run(marker)));
                paragraph = false;
            } else {
                paragraph = !inner.is_empty() && !starts_block(inner);
            }
            quoted = true;
            table = false;
        } else {
            quote_fence = None;
            // (Tables only outside lists and quotes, here.)
            let top = items.is_empty() && !(quoted && lazy);
            if table && (blank || !top || opens_item || (starts_block(rest) && opens_at < container + 4)) {
                table = false;
            }
            // A table's header: a line with pipes starting a paragraph (or with a pipe first, which
            // interrupts one), its delimiter row next with as many columns.
            if !table && top && !blank && (!paragraph || rest.starts_with('|')) && table_head(rest, lines.get(number + 1).copied()) {
                table = true;
            }
        }
        if table {
            (paragraph, quoted) = (false, false);
        } else if rest.starts_with('>') && opens_at < container + 4 {
            // (A quote line: read above.)
        } else {
            // A heading's underline (`-` or `=` only, after paragraph text) ends its paragraph.
            let marks = rest.trim_end();
            let underline = paragraph
                && !quoted
                && !opens_item
                && opens_at < container + 4
                && !marks.is_empty()
                && (marks.chars().all(|c| c == '-') || marks.chars().all(|c| c == '='));
            paragraph = !(blank || rest.is_empty() || (starts_block(rest) && opens_at < container + 4) || code || underline);
            quoted = quoted && lazy && paragraph;
        }
        // A blank line ends a chunk unless an indented line follows: a list item going on past
        // it (its code, quote, or paragraph) is parsed with its item.
        // So does a list item after blank lines: a list goes on over them (a loose list), its blank
        // lines the list's.
        let next_content = lines.get(number + 1..).into_iter().flatten().find(|next| !next.trim().is_empty());
        let continues = next_content.is_some_and(|next| next.starts_with([' ', '\t']) || list_item(next));
        // A run of blank lines ends a chunk at its last: a list goes on over them in the note (they
        // take its style), so the next chunk must not start with one.
        let blank_next = lines.get(number + 1).is_some_and(|next| next.trim().is_empty());
        if blank && line_start > start && !continues && !blank_next && at > held {
            out.push(start..at);
            start = at;
            items.clear();
        }
    }
    if start < text.len() || out.is_empty() {
        out.push(start..text.len());
    }
    (out, fence.is_none() && !frontmatter && !beyond)
}

/// Whether `header` (a line without its indentation) heads a table whose delimiter row is `next`,
/// as pulldown-cmark finds one: as many columns (by the header's unescaped pipes) as the
/// delimiter row has of `-`, `:`, and pipes.
fn table_head(header: &str, next: Option<&str>) -> bool {
    let header = header.trim_end_matches(['\n', '\r']);
    let mut pipes = 0;
    let mut last_pipe = 0;
    let mut escaped = false;
    for (at, byte) in header.bytes().enumerate() {
        if byte == b'|' && !escaped {
            pipes += 1;
            last_pipe = at;
        }
        escaped = byte == b'\\' && !escaped;
    }
    if pipes == 0 {
        return false;
    }
    if header.starts_with('|') {
        pipes -= 1;
    }
    let columns_in_header = if header.get(last_pipe + 1..).is_some_and(|rest| rest.trim().is_empty()) { pipes } else { pipes + 1 };
    let Some(next) = next.map(|line| line.trim_end_matches(['\n', '\r'])) else { return false };
    if columns(next) > 3 {
        return false;
    }
    let delimiter = next.trim_start_matches([' ', '\t']);
    let mut rest = delimiter.as_bytes();
    let mut found_pipe = false;
    if let Some(after) = rest.strip_prefix(b"|") {
        rest = after;
        found_pipe = true;
    }
    let (mut count, mut start_col, mut hyphen, mut hyphen_in_col) = (0, true, false, false);
    for &byte in rest {
        match byte {
            b' ' | b'\t' => {}
            b':' => start_col = false,
            b'-' => {
                start_col = false;
                hyphen = true;
                hyphen_in_col = true;
            }
            b'|' => {
                if !hyphen_in_col {
                    return false;
                }
                start_col = true;
                found_pipe = true;
                count += 1;
                hyphen_in_col = false;
            }
            _ => return false,
        }
    }
    if !start_col {
        count += 1;
    }
    found_pipe && hyphen && count == columns_in_header
}

/// Whether a line closes a metadata block: `---` or `...` and spaces only.
pub(crate) fn closes_metadata(line: &str) -> bool {
    let content = line.trim_end_matches(['\n', '\r']).trim_end_matches(' ');
    content == "---" || content == "..."
}

/// A line's indentation in columns (a tab to the next multiple of four).
fn columns(line: &str) -> usize {
    let mut column = 0;
    for c in line.chars() {
        match c {
            ' ' => column += 1,
            '\t' => column = (column / 4 + 1) * 4,
            _ => break,
        }
    }
    column
}

/// When `rest` (at `column`) starts with a list item's marker, how many columns its marker and the
/// spaces after it take (its content's column from `column`; CommonMark: one to four spaces, one
/// column past the marker when there are more or none), and its marker's length in bytes.
fn item_width(rest: &str, column: usize) -> Option<(usize, usize)> {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let marker = if rest.starts_with(['-', '*', '+']) {
        1
    } else if digits > 0 && digits < 10 && rest.get(digits..).is_some_and(|after| after.starts_with(['.', ')'])) {
        digits + 1
    } else {
        return None;
    };
    let after = rest.get(marker..)?;
    if after.trim().is_empty() {
        return Some((marker + 1, marker));
    }
    if !after.starts_with([' ', '\t']) {
        return None;
    }
    // A thematic break (`- - -`, `* * *`) is no item.
    if marker == 1
        && rest.chars().filter(|c| !c.is_whitespace()).all(|c| rest.starts_with(c))
        && rest.chars().filter(|c| !c.is_whitespace()).count() >= 3
    {
        return None;
    }
    // The spaces after the marker, in columns from where it ends (a tab to the next stop).
    let mut end = column + marker;
    for c in after.chars() {
        match c {
            ' ' => end += 1,
            '\t' => end = (end / 4 + 1) * 4,
            _ => break,
        }
    }
    let spaces = end - column - marker;
    Some((marker + if (1..=4).contains(&spaces) { spaces } else { 1 }, marker))
}

/// Whether a line (without its indentation) starts a block that ends a paragraph: a heading, a
/// quote, a fence, a list item, or a thematic break.
fn starts_block(rest: &str) -> bool {
    // (Most lines start with a letter: none of these.)
    let Some(first) = rest.bytes().next() else { return false };
    if !matches!(first, b'#' | b'>' | b'`' | b'~' | b'$' | b'-' | b'*' | b'_' | b'+' | b'0'..=b'9') {
        return false;
    }
    let heading = rest.trim_start_matches('#');
    let hashes = rest.len() - heading.len();
    let rule = matches!(first, b'-' | b'*' | b'_')
        && rest.bytes().all(|byte| byte == first || byte == b' ' || byte == b'\t')
        && rest.bytes().filter(|byte| *byte == first).count() >= 3;
    (hashes > 0 && hashes <= 6 && (heading.is_empty() || heading.starts_with([' ', '\t'])))
        || rest.starts_with('>')
        || (rest.starts_with("```") && !rest.trim_start_matches('`').contains('`'))
        || rest.starts_with("~~~")
        || rule
        || interrupts(rest)
}

/// Whether a list item (`rest`, without its indentation) can interrupt a paragraph: it has
/// content, and a numbered one starts from 1.
fn interrupts(rest: &str) -> bool {
    item_width(rest, 0).is_some_and(|(_, marker)| {
        rest.get(marker..).is_some_and(|after| !after.trim().is_empty()) && (marker == 1 || rest.starts_with("1.") || rest.starts_with("1)"))
    })
}

/// Whether a line starts a list item (`-`, `*`, `+`, or a number and `.` or `)`, then a space or
/// the line's end).
fn list_item(line: &str) -> bool {
    let line = line.trim_end_matches(['\n', '\r']);
    let after = |rest: &str| rest.is_empty() || rest.starts_with([' ', '\t']);
    if let Some(rest) = line.strip_prefix(['-', '*', '+']) {
        return after(rest);
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && digits < 10 && line.get(digits..).and_then(|rest| rest.strip_prefix(['.', ')'])).is_some_and(after)
}

fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}

fn shift_row(row: &mut kition_note_markdown::Row, by: usize) {
    row.line = shift(&row.line, by);
    row.body = shift(&row.body, by);
    for cell in &mut row.cells {
        cell.segment = shift(&cell.segment, by);
        cell.content = shift(&cell.content, by);
    }
}

/// A chunk's key in the cache: its text, whether it starts the note, and the note's link
/// reference labels when it may use one.
fn chunk_key(text: &str, first: bool, labels: &std::collections::BTreeMap<String, String>) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    text.hash(&mut hasher);
    first.hash(&mut hasher);
    if !labels.is_empty() && text.contains(']') {
        labels.hash(&mut hasher);
    }
    hasher.finish()
}

fn back(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start.saturating_sub(by)..range.end.saturating_sub(by)
}

/// A note with more chunks than this parses whole when most are not parsed yet (see
/// `ChunkCache::parse`).
const COLD_WHOLE_FROM_CHUNKS: usize = 64;

/// An index in short, for the note's summary (`first`: it starts the note, its front matter counts).
fn summary_of(index: &kition_note_markdown::Index, first: bool) -> Summary {
    Summary {
        headings: index.headings.iter().map(|heading| (heading.level, heading.text.clone(), heading.range.start)).collect(),
        tags: index.tags.len(),
        block_ids: index.block_ids.len(),
        tasks_open: index.tasks.iter().filter(|task| !task.done).count(),
        tasks_done: index.tasks.iter().filter(|task| task.done).count(),
        words: index.words,
        callouts: index.callouts.len(),
        frontmatter: if first { index.frontmatter.clone() } else { None },
    }
}

/// The chunk cache: parsed chunks by their text (and whether they start the note).
#[derive(Default)]
pub struct ChunkCache {
    parsed: HashMap<u64, Arc<Parsed>>,
}

impl ChunkCache {
    /// One chunk parsed (memoised by its text, whether it starts the note, and the labels).
    pub(crate) fn chunk(&mut self, text: &str, first: bool, labels: &std::collections::BTreeMap<String, String>, used: &mut Vec<u64>) -> Arc<Parsed> {
        let key = chunk_key(text, first, labels);
        used.push(key);
        if let Some(found) = self.parsed.get(&key) {
            return found.clone();
        }
        let (styles, code, index) = Styles::unlit_indexed(text, labels, first);
        let summary = summary_of(&index, first);
        let parsed = Arc::new(Parsed {
            styles,
            tables: index.tables,
            links: index.links,
            callouts: index.callouts,
            summary,
            ranges: Vec::new(),
            labels: std::collections::BTreeMap::new(),
            code,
        });
        self.parsed.insert(key, parsed.clone());
        parsed
    }

    /// The chunks of a note parsed whole, cut from its parse into the cache, as each would parse
    /// alone (so the next edits find them).
    fn fill(
        &mut self,
        text: &str,
        ranges: &[Range<usize>],
        labels: &std::collections::BTreeMap<String, String>,
        styles: &Styles,
        index: &kition_note_markdown::Index,
        code: &[(Range<usize>, Arc<str>)],
    ) {
        // Each list is in the note's order: a chunk's part of it is found by its bounds.
        fn within<'a, T>(items: &'a [T], range: &Range<usize>, at: impl Fn(&T) -> usize) -> &'a [T] {
            let from = items.partition_point(|item| at(item) < range.start);
            let to = items.partition_point(|item| at(item) < range.end);
            items.get(from..to.max(from)).unwrap_or(&[])
        }
        for (number, range) in ranges.iter().enumerate() {
            let Some(part) = text.get(range.clone()) else { continue };
            let first = range.start == 0;
            let key = chunk_key(part, first, labels);
            if self.parsed.contains_key(&key) {
                continue;
            }
            // Its lines, and the empty one after its line break (the next chunk's first line,
            // which a chunk parsed alone has as an empty paragraph).
            let first_line = styles.line_of(range.start);
            let last = number + 1 == ranges.len();
            let end_line = if last { styles.lines.len() } else { styles.line_of(range.end) };
            let mut lines: Vec<crate::style::LineStyle> = styles.lines.get(first_line..end_line).unwrap_or(&[]).to_vec();
            let mut starts: Vec<usize> = styles.starts.get(first_line..end_line).unwrap_or(&[]).iter().map(|start| start - range.start).collect();
            if !last && part.ends_with('\n') {
                lines.push(crate::style::LineStyle::default());
                starts.push(part.len());
            }
            let part_styles = Styles { bytes: styles.bytes.get(range.clone()).unwrap_or(&[]).to_vec(), lines, starts };
            let tables = within(&index.tables, range, |table| table.range.start)
                .iter()
                .cloned()
                .map(|mut table| {
                    table.range = back(&table.range, range.start);
                    for row in table.rows.iter_mut().chain(std::iter::once(&mut table.delimiter)) {
                        row.line = back(&row.line, range.start);
                        row.body = back(&row.body, range.start);
                        for cell in &mut row.cells {
                            cell.segment = back(&cell.segment, range.start);
                            cell.content = back(&cell.content, range.start);
                        }
                    }
                    table
                })
                .collect();
            let links = within(&index.links, range, |link| link.range.start)
                .iter()
                .cloned()
                .map(|mut link| {
                    link.range = back(&link.range, range.start);
                    link.destination_range = link.destination_range.as_ref().map(|inner| back(inner, range.start));
                    link.text_range = link.text_range.as_ref().map(|inner| back(inner, range.start));
                    link
                })
                .collect();
            let callouts: Vec<Callout> = within(&index.callouts, range, |callout| callout.range.start)
                .iter()
                .cloned()
                .map(|mut callout| {
                    callout.range = back(&callout.range, range.start);
                    callout
                })
                .collect();
            let frontmatter = if first { index.frontmatter.clone() } else { None };
            let summary = Summary {
                headings: within(&index.headings, range, |heading| heading.range.start)
                    .iter()
                    .map(|heading| (heading.level, heading.text.clone(), heading.range.start - range.start))
                    .collect(),
                tags: within(&index.tags, range, |tag| tag.range.start).len(),
                block_ids: within(&index.block_ids, range, |id| id.range.start).len(),
                tasks_open: within(&index.tasks, range, |task| task.marker.start).iter().filter(|task| !task.done).count(),
                tasks_done: within(&index.tasks, range, |task| task.marker.start).iter().filter(|task| task.done).count(),
                words: kition_note_markdown::words(part, frontmatter.as_ref().map_or(0, |found| found.range.end)),
                callouts: callouts.len(),
                frontmatter,
            };
            self.parsed.insert(
                key,
                Arc::new(Parsed {
                    styles: part_styles,
                    tables,
                    links,
                    callouts,
                    summary,
                    ranges: Vec::new(),
                    labels: std::collections::BTreeMap::new(),
                    code: within(code, range, |(block, _)| block.start)
                        .iter()
                        .map(|(block, language)| (back(block, range.start), language.clone()))
                        .collect(),
                }),
            );
        }
    }

    /// Consecutive chunks (`ranges` in `text`) parsed and joined as one chunk's parse would hold
    /// them (offsets from the first's start; the last keeps its empty line after a line break), as
    /// the whole parse joins them.
    pub(crate) fn joined(
        &mut self,
        text: &str,
        ranges: &[Range<usize>],
        labels: &std::collections::BTreeMap<String, String>,
        used: &mut Vec<u64>,
    ) -> Parsed {
        let base = ranges.first().map_or(0, |range| range.start);
        let mut styles = Styles { bytes: Vec::new(), lines: Vec::new(), starts: Vec::new() };
        let (mut tables, mut links, mut callouts) = (Vec::new(), Vec::new(), Vec::new());
        let mut code = Vec::new();
        let mut summary = Summary::default();
        for (number, range) in ranges.iter().enumerate() {
            let part = text.get(range.clone()).unwrap_or("");
            let parsed = self.chunk(part, range.start == 0, labels, used);
            let by = range.start - base;
            styles.bytes.extend_from_slice(&parsed.styles.bytes);
            let last = number + 1 == ranges.len();
            let take = if !last && part.ends_with('\n') { parsed.styles.lines.len().saturating_sub(1) } else { parsed.styles.lines.len() };
            for index in 0..take {
                if let (Some(line), Some(start)) = (parsed.styles.lines.get(index), parsed.styles.starts.get(index)) {
                    styles.lines.push(*line);
                    styles.starts.push(start + by);
                }
            }
            tables.extend(parsed.tables.iter().cloned().map(|mut table| {
                table.range = shift(&table.range, by);
                for row in &mut table.rows {
                    shift_row(row, by);
                }
                shift_row(&mut table.delimiter, by);
                table
            }));
            links.extend(parsed.links.iter().cloned().map(|mut link| {
                link.range = shift(&link.range, by);
                link.destination_range = link.destination_range.as_ref().map(|range| shift(range, by));
                link.text_range = link.text_range.as_ref().map(|range| shift(range, by));
                link
            }));
            callouts.extend(parsed.callouts.iter().cloned().map(|mut callout| {
                callout.range = shift(&callout.range, by);
                callout
            }));
            code.extend(parsed.code.iter().map(|(block, language)| (shift(block, by), language.clone())));
            let part_summary = &parsed.summary;
            summary.headings.extend(part_summary.headings.iter().map(|(level, text, at)| (*level, text.clone(), at + by)));
            summary.tags += part_summary.tags;
            summary.block_ids += part_summary.block_ids;
            summary.tasks_open += part_summary.tasks_open;
            summary.tasks_done += part_summary.tasks_done;
            summary.words += part_summary.words;
        }
        summary.callouts = callouts.len();
        Parsed { styles, tables, links, callouts, summary, ranges: Vec::new(), labels: std::collections::BTreeMap::new(), code }
    }

    /// The whole note parsed, from its chunks.
    pub fn parse(&mut self, text: &str) -> Parsed {
        let ranges = chunks(text);
        let kept = ranges.clone();
        // Link reference definitions count across chunks (a chunk is parsed alone).
        let labels = crate::style::reference_labels(text);
        // Nothing parsed yet (a note just opened): the note in one pass, which parses as its
        // chunks do and costs less than thousands of small ones; edits fill the chunks in.
        // Not with link reference definitions: the whole note resolves a reference across
        // chunks (its link, a heading's text), which a chunk parsed alone does not.
        if ranges.len() > COLD_WHOLE_FROM_CHUNKS && self.parsed.len() < ranges.len() / 4 && labels.is_empty() {
            let (mut styles, code, index) = Styles::unlit_indexed(text, &labels, true);
            let summary = summary_of(&index, true);
            if styles.starts.is_empty() {
                styles.starts.push(0);
                styles.lines.push(Default::default());
            }
            self.fill(text, &ranges, &labels, &styles, &index, &code);
            return Parsed { styles, tables: index.tables, links: index.links, callouts: index.callouts, summary, ranges: kept, labels, code };
        }
        let mut used = Vec::with_capacity(ranges.len());
        let mut styles = Styles { bytes: Vec::with_capacity(text.len()), lines: Vec::new(), starts: Vec::new() };
        let (mut tables, mut links, mut callouts) = (Vec::new(), Vec::new(), Vec::new());
        let mut code = Vec::new();
        let mut summary = Summary::default();
        let count = ranges.len();
        for (number, range) in ranges.into_iter().enumerate() {
            let part = text.get(range.clone()).unwrap_or("");
            let parsed = self.chunk(part, range.start == 0, &labels, &mut used);
            let by = range.start;
            styles.bytes.extend_from_slice(&parsed.styles.bytes);
            // A chunk ending in a line break has an empty last line that is the next chunk's first.
            let last = number + 1 == count;
            let take = if !last && part.ends_with('\n') { parsed.styles.lines.len().saturating_sub(1) } else { parsed.styles.lines.len() };
            for index in 0..take {
                if let (Some(line), Some(start)) = (parsed.styles.lines.get(index), parsed.styles.starts.get(index)) {
                    styles.lines.push(*line);
                    styles.starts.push(start + by);
                }
            }
            tables.extend(parsed.tables.iter().cloned().map(|mut table| {
                table.range = shift(&table.range, by);
                for row in &mut table.rows {
                    shift_row(row, by);
                }
                shift_row(&mut table.delimiter, by);
                table
            }));
            links.extend(parsed.links.iter().cloned().map(|mut link| {
                link.range = shift(&link.range, by);
                link.destination_range = link.destination_range.as_ref().map(|range| shift(range, by));
                link.text_range = link.text_range.as_ref().map(|range| shift(range, by));
                link
            }));
            callouts.extend(parsed.callouts.iter().cloned().map(|mut callout| {
                callout.range = shift(&callout.range, by);
                callout
            }));
            code.extend(parsed.code.iter().map(|(block, language)| (shift(block, by), language.clone())));
            let part_summary = &parsed.summary;
            summary.headings.extend(part_summary.headings.iter().map(|(level, text, at)| (*level, text.clone(), at + by)));
            summary.tags += part_summary.tags;
            summary.block_ids += part_summary.block_ids;
            summary.tasks_open += part_summary.tasks_open;
            summary.tasks_done += part_summary.tasks_done;
            summary.words += part_summary.words;
            if summary.frontmatter.is_none() {
                summary.frontmatter = part_summary.frontmatter.clone();
            }
        }
        if styles.starts.is_empty() {
            styles.starts.push(0);
            styles.lines.push(Default::default());
        }
        summary.callouts = callouts.len();
        // Only this note's chunks are kept (a fixed cap smaller than a large note's chunk count
        // emptied the cache on every edit), and those of the edit before for an undo.
        if self.parsed.len() > used.len() * 2 + 64 {
            let keep: std::collections::HashSet<u64> = used.into_iter().collect();
            self.parsed.retain(|key, _| keep.contains(key));
        }
        Parsed { styles, tables, links, callouts, summary, ranges: kept, labels, code }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_cover_the_note_and_keep_fences_whole() {
        let text = "---\ntitle: a\n\nb: c\n---\n# A\n\npara\n\n```\ncode\n\nmore\n```\n\n$$\nx\n\ny\n$$\nend";
        let ranges = chunks(text);
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(text.len()));
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
        let has = |needle: &str| ranges.iter().any(|range| text[range.clone()].contains(needle));
        assert!(has("```\ncode\n\nmore\n```"), "{ranges:?}");
        assert!(has("title: a\n\nb: c"));
        // A `---` block later in the note, which pulldown-cmark reads as metadata, stays whole too.
        let later = "para\n\n---\nkey: a\n\nmore: b\n---\nend";
        assert!(chunks(later).iter().any(|range| later[range.clone()].contains("---\nkey: a\n\nmore: b\n---")), "{:?}", chunks(later));
        // `$$` math is inline: a blank line ends its paragraph, and the chunk with it.
        let whole = Styles::of(text);
        let parsed = ChunkCache::default().parse(text);
        assert_eq!(parsed.lit(text).bytes, whole.bytes);
    }

    #[test]
    fn a_later_chunk_starting_with_a_rule_is_no_front_matter() {
        let text = "para\n\n---\ntext\n---\nmore\n\n+++\nx\n+++\n";
        let whole = Styles::of(text);
        let parsed = ChunkCache::default().parse(text);
        assert_eq!(parsed.styles.lines, whole.lines);
        assert_eq!(parsed.lit(text).bytes, whole.bytes);
    }

    #[test]
    fn chunked_styles_equal_the_whole_note_s() {
        let text = "# Title\n\nSome **bold** and [a link](x.md).\n\n| A | B |\n|---|---|\n| 1 | [[n]] |\n\n> [!tip] Hi\n> body\n\n```rust\nfn x() {}\n\n}\n```\n\n- a\n- b\n";
        let whole = Styles::of(text);
        let parsed = ChunkCache::default().parse(text);
        assert_eq!(parsed.styles.starts, whole.starts);
        assert_eq!(parsed.styles.lines, whole.lines);
        assert_eq!(parsed.lit(text).bytes, whole.bytes);
        let index = kition_note_markdown::index(text);
        assert_eq!(parsed.tables, index.tables);
        assert_eq!(
            parsed.links.iter().map(|link| link.range.clone()).collect::<Vec<_>>(),
            index.links.iter().map(|link| link.range.clone()).collect::<Vec<_>>()
        );
        assert_eq!(parsed.callouts.len(), index.callouts.len());
        let headings: Vec<(u8, String, usize)> =
            index.headings.iter().map(|heading| (heading.level, heading.text.clone(), heading.range.start)).collect();
        assert_eq!(parsed.summary.headings, headings);
        assert_eq!(parsed.summary.words, index.words);
        assert_eq!(parsed.summary.tags, index.tags.len());
        assert_eq!(
            (parsed.summary.tasks_open, parsed.summary.tasks_done),
            (index.tasks.iter().filter(|task| !task.done).count(), index.tasks.iter().filter(|task| task.done).count())
        );
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;

    /// Notes of list items, code fences in and out of them, lazy lines, and blank lines: their
    /// chunks parse as the whole note does.
    #[test]
    fn notes_of_lists_and_fences_chunk_as_they_parse_whole() {
        const PIECES: &[&str] = &[
            "- item",
            "  - nested",
            "1. one",
            "2) two",
            "- ```js",
            "  ```css",
            "    ```",
            "```",
            "~~~",
            "  ~~~",
            "code();",
            "  .a { }",
            "\tx = 1",
            "lazy text",
            "  indented",
            "> quote",
            "> ```",
            "# Heading",
            "---",
            "* * *",
            "$$",
            "$$ y $$",
            "title: a",
            "...",
            "",
            "",
            "",
            "-",
            "10. ten",
            "-\ttab",
            "x^2",
            "   ```py",
            "```\"`",
            "para",
            "    four",
            "#tag https://kition.ai",
            "[a](b.md)",
            "| a | b |",
            "| - | - |",
        ];
        // Eight seeds; `FUZZ_SEED` runs another one and `FUZZ_ROUNDS` more notes, by hand.
        let seeds: Vec<u64> = match std::env::var("FUZZ_SEED").ok().and_then(|seed| seed.parse().ok()) {
            Some(seed) => vec![seed],
            None => vec![0x5eed_1157, 1, 2, 3, 4, 5, 6, 7],
        };
        let rounds: usize = std::env::var("FUZZ_ROUNDS").ok().and_then(|rounds| rounds.parse().ok()).unwrap_or(5000);
        // Whether a note's chunks parse as the note does.
        let agrees = |text: &str| {
            let whole = Styles::of(text);
            let parsed = ChunkCache::default().parse(text);
            let index = kition_note_markdown::index(text);
            let ranges = |links: &[Link]| links.iter().map(|link| (link.range.clone(), link.text_range.clone())).collect::<Vec<_>>();
            parsed.styles.lines == whole.lines
                && parsed.lit(text).bytes == whole.bytes
                && ranges(&parsed.links) == ranges(&index.links)
                && parsed.summary.words == index.words
                && parsed.summary.tags == index.tags.len()
                && parsed.summary.headings.len() == index.headings.len()
                && parsed.tables.len() == index.tables.len()
        };
        for start in seeds {
            // (Spread, so that small seeds start apart; never zero, which xorshift keeps.)
            let mut seed = start.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
            let mut next = |bound: usize| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed % bound as u64) as usize
            };
            for round in 0..rounds {
                let mut lines: Vec<&str> = (0..next(30) + 1).map(|_| PIECES[next(PIECES.len())]).collect();
                let end = if next(2) == 0 { "\n" } else { "" };
                if agrees(&(lines.join("\n") + end)) {
                    continue;
                }
                // The fewest lines that still disagree, for the message.
                let mut at = 0;
                while at < lines.len() {
                    let mut fewer = lines.clone();
                    fewer.remove(at);
                    if !agrees(&(fewer.join("\n") + end)) {
                        lines = fewer;
                    } else {
                        at += 1;
                    }
                }
                let text = lines.join("\n") + end;
                panic!("seed {start}, round {round}: {text:?} cut as {:?} parses otherwise than whole", chunks(&text));
            }
        }
    }
}

#[cfg(test)]
mod fill_tests {
    use super::*;

    #[test]
    fn chunks_cut_from_a_whole_parse_equal_chunks_parsed_alone() {
        let mut text = String::from("---\ntitle: A\n---\n# A\n");
        for i in 0..120 {
            text.push_str(&format!(
                "\n## Section {i}\n\nText with **bold**, a #tag{i}, a [link](x{i}.md) and [[Wiki]]. ^b{i}\n\n- [ ] task {i}\n- [x] done\n\n| A | B |\n| --- | --- |\n| {i} | c |\n\n> [!note] Callout {i}\n> body\n\n```js\nconst x = {i}\n```\n"
            ));
        }
        let mut whole = ChunkCache::default();
        let parsed = whole.parse(&text);
        assert!(parsed.ranges.len() > COLD_WHOLE_FROM_CHUNKS, "the note is parsed whole: {}", parsed.ranges.len());
        let labels = crate::style::reference_labels(&text);
        for range in &parsed.ranges {
            let part = &text[range.clone()];
            let key = chunk_key(part, range.start == 0, &labels);
            let cut = whole.parsed.get(&key).cloned().expect("every chunk is cut into the cache");
            let mut alone = ChunkCache::default();
            let fresh = alone.chunk(part, range.start == 0, &labels, &mut Vec::new());
            let keep = |lines: usize| lines.saturating_sub(usize::from(part.ends_with('\n') && range.end < text.len()));
            assert_eq!(cut.styles.bytes, fresh.styles.bytes, "bytes of {range:?}");
            assert_eq!(cut.code, fresh.code, "code of {range:?}");
            let (a, b) = (keep(cut.styles.lines.len()), keep(fresh.styles.lines.len()));
            assert_eq!(cut.styles.lines[..a], fresh.styles.lines[..b], "lines of {range:?}");
            assert_eq!(cut.styles.starts, fresh.styles.starts, "starts of {range:?}");
            assert_eq!(format!("{:?}", cut.tables), format!("{:?}", fresh.tables), "tables of {range:?}");
            assert_eq!(format!("{:?}", cut.links), format!("{:?}", fresh.links), "links of {range:?}");
            assert_eq!(format!("{:?}", cut.callouts), format!("{:?}", fresh.callouts), "callouts of {range:?}");
            assert_eq!(cut.summary, fresh.summary, "summary of {range:?}");
        }
    }
}
