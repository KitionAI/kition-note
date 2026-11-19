//! `kition-note-view`: the Note kit's editing view, drawn by the engine (Word's model).
//!
//! The note's text is laid out in one column ([`layout`]) with every character in place: the
//! Markdown's structure sets sizes, weights, colors, and boxes ([`style`]), and its syntax draws
//! dimmed. Because each row keeps its source bytes, the [`View`] answers what an editor asks of
//! it exactly: where the caret for a byte is, which byte a point hits, the rectangles a selection
//! covers, and the rows above and below. [`View::frame`] draws any area at any scale.

pub mod chunk;
mod color;
mod edit;
mod highlight;
pub mod layout;
mod light;
mod math;
mod paint;
pub mod style;
pub mod theme;

use std::ops::Range;

use layout::{Cache, Placed, Row, Stop};
use style::Styles;
pub use theme::{Scheme, Theme};

/// Where the caret for a byte is drawn, in document pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub x: f32,
    pub top: f32,
    pub height: f32,
}

/// A rectangle in document pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// A table's grid in document pixels, for the editor's table controls: its columns' edges and its
/// rows' (the header first).
#[derive(Clone, Debug, PartialEq)]
pub struct TableGrid {
    pub index: usize,
    pub columns: Vec<f32>,
    pub rows: Vec<(f32, f32)>,
    /// Each row's source line, from 1, as the table commands count (the header is row 0).
    pub lines: Vec<usize>,
}

/// A fenced code block's box in document pixels, its language, and its code's bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeBox {
    pub top: f32,
    pub bottom: f32,
    pub right: f32,
    pub language: String,
    pub code: Range<usize>,
    /// In a callout: Obsidian renders the block there, with only a copy button under the pointer
    /// (no language label). `left` is its box's left edge.
    pub callout: bool,
    pub left: f32,
}

/// A block the editor draws in place of its source (a Mermaid diagram, an HTML block): its box
/// in document pixels (top, left edge, size as the editor measured it), its kind (`mermaid` or
/// `html`), its source (the key the editor sizes it by), and where its source starts (the
/// editor's edit button puts the caret there).
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramBox {
    pub top: f32,
    pub left: f32,
    pub width: f32,
    pub height: f32,
    pub kind: &'static str,
    pub source: String,
    pub code: usize,
}

/// A block the editor can draw: a fenced code block whose language is `mermaid` (its source the
/// code between the fences), or an HTML block (its source the HTML). Its lines, its bytes, where
/// its source starts, and whether it is drawn now.
#[derive(Clone, Debug)]
struct MermaidBlock {
    kind: &'static str,
    first: usize,
    last: usize,
    range: Range<usize>,
    code: usize,
    source: String,
    drawn: bool,
}

/// The HTML blocks of `text` Obsidian renders: from an HTML block's opening tag to its matching
/// closing tag, across blank lines and the Markdown between (as `<details>` holds its body), or
/// the HTML block alone when it does not close. A comment (`<!-- -->`) stays its source.
fn html_blocks(text: &str, styles: &Styles) -> Vec<MermaidBlock> {
    let line_range = |line: usize| {
        let start = styles.starts.get(line).copied().unwrap_or(0);
        (start, styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1)))
    };
    let html = |line: usize| styles.lines.get(line).is_some_and(|style| style.block == style::Block::Html && style.quote == 0 && style.list == 0);
    let mut out = Vec::new();
    let mut index = 0;
    while index < styles.starts.len() {
        if !html(index) {
            index += 1;
            continue;
        }
        let first = index;
        while html(index + 1) {
            index += 1;
        }
        let (start, _) = line_range(first);
        let opening = text.get(start..).unwrap_or("").trim_start();
        let name: String = opening.strip_prefix('<').unwrap_or("").chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let mut last = index;
        if !name.is_empty() && !opening.starts_with("<!--") {
            // The matching closing tag, counting the same tag opened inside.
            let lower = text.get(start..).unwrap_or("").to_ascii_lowercase();
            let (open, close) = (format!("<{}", name.to_ascii_lowercase()), format!("</{}>", name.to_ascii_lowercase()));
            let mut depth = 0usize;
            let mut at = 0;
            while let Some(found) = lower.get(at..).and_then(|rest| rest.find('<')).map(|offset| at + offset) {
                let rest = lower.get(found..).unwrap_or("");
                if rest.starts_with(&close) {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        last = last.max(styles.line_of(start + found));
                        break;
                    }
                } else if rest.starts_with(&open) && rest.get(open.len()..open.len() + 1).is_some_and(|c| c == ">" || c == " " || c == "\n") {
                    depth += 1;
                }
                at = found + 1;
            }
        } else if opening.starts_with("<!--") {
            index += 1;
            continue;
        }
        let (_, end) = line_range(last);
        out.push(MermaidBlock {
            kind: "html",
            first,
            last,
            range: start..end,
            code: start,
            source: text.get(start..end).unwrap_or("").to_owned(),
            drawn: false,
        });
        index = last + 1;
    }
    out
}

/// The Mermaid code blocks of `text`, by its parsed `styles`. A quoted block's source drops its
/// quote markers, as Obsidian renders a diagram in a callout.
fn mermaid_blocks(text: &str, styles: &Styles) -> Vec<MermaidBlock> {
    let line_text = |line: usize| {
        let start = styles.starts.get(line).copied().unwrap_or(0);
        let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
        (start, end, text.get(start..end).unwrap_or("").trim_end_matches('\r'))
    };
    fn unquoted(content: &str, quote: u8) -> &str {
        let mut rest = content;
        for _ in 0..quote {
            let trimmed = rest.trim_start_matches([' ', '\t']);
            match trimmed.strip_prefix('>') {
                Some(after) => rest = after.strip_prefix(' ').unwrap_or(after),
                None => break,
            }
        }
        rest
    }
    let block = |line: usize| styles.lines.get(line).map(|style| style.block);
    let mut out = Vec::new();
    let mut index = 0;
    while index < styles.starts.len() {
        if block(index) != Some(style::Block::Code { fence: true }) {
            index += 1;
            continue;
        }
        let quote = styles.lines.get(index).map_or(0, |style| style.quote);
        let (start, _, fence) = line_text(index);
        let info = unquoted(fence, quote).trim_start().trim_start_matches(['`', '~']).trim();
        // The code runs to the closing fence: a fence line, or a code line that is only the
        // fence's characters (a quoted block's closing fence is parsed as its code).
        let marker = unquoted(fence, quote).trim_start().chars().next().unwrap_or('`');
        let closes = |line: usize| {
            let content = unquoted(line_text(line).2, quote).trim();
            content.len() >= 3 && content.chars().all(|c| c == marker)
        };
        let mut last = index;
        while block(last + 1) == Some(style::Block::Code { fence: false }) && !closes(last + 1) {
            last += 1;
        }
        let closing = matches!(block(last + 1), Some(style::Block::Code { .. }));
        let end_line = if closing { last + 1 } else { last };
        if info.split_whitespace().next().is_some_and(|language| language.eq_ignore_ascii_case("mermaid")) {
            let source: Vec<&str> = (index + 1..=last).filter(|_| last > index).map(|line| unquoted(line_text(line).2, quote)).collect();
            let code = if last > index { line_text(index + 1).0 } else { line_text(index).1 };
            out.push(MermaidBlock {
                kind: "mermaid",
                first: index,
                last: end_line,
                range: start..line_text(end_line).1,
                code,
                source: source.join("\n"),
                drawn: false,
            });
        }
        index = end_line + 1;
    }
    out
}

/// A picture the note links, decoded (premultiplied), by the name the note writes.
#[derive(Clone)]
pub struct Picture {
    pub pixmap: std::sync::Arc<vello_cpu::Pixmap>,
    pub width: u32,
    pub height: u32,
}

/// One row of the properties block: a key, its type (`text`, `tags`, `aliases`, `list`,
/// `number`, `checkbox`, `date`, `datetime`), its values, and where its line is.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    pub key: String,
    pub kind: &'static str,
    pub values: Vec<String>,
    /// Top and height of its line, in document pixels.
    pub y: f32,
    pub height: f32,
    /// Where its value starts in the text (bytes).
    pub value_at: usize,
}

/// The front matter as Obsidian's properties block: its box (document pixels) and its rows.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertiesBlock {
    pub top: f32,
    pub bottom: f32,
    /// Where the front matter's body ends (bytes): a new property goes there.
    pub end: usize,
    pub rows: Vec<PropertyRow>,
    pub folded: bool,
}

/// A note laid out for editing.
pub struct View {
    /// The properties block folded to its heading (Obsidian's Toggle fold properties): a view
    /// state, nothing is written.
    properties_folded: bool,
    /// The editor draws the properties block over the view (it asked for it): the view leaves its
    /// room blank. Otherwise the view draws it (a thumbnail, a preview, a host without the editor).
    properties_overlaid: bool,
    /// Text the open comment threads are anchored to (bytes), tinted.
    marks: Vec<std::ops::Range<usize>>,
    theme: Theme,
    text: String,
    styles: Styles,
    placed: Vec<Placed>,
    height: f32,
    cache: Cache,
    pictures: std::collections::HashMap<String, Picture>,
    /// Pictures that could not be read or decoded: not asked for again.
    missing: std::collections::HashSet<String>,
    /// Each image-only line's picture name, by line.
    picture_lines: Vec<(usize, String)>,
    chunks: chunk::ChunkCache,
    /// The note's links and tables, from the last update.
    links: Vec<kition_note_markdown::Link>,
    tables: Vec<kition_note_markdown::Table>,
    /// What the last whole layout was made for (see `relay_caret_lines`).
    laid: Option<Laid>,
    /// The note in short, from the last update (see `summary`).
    summary: chunk::Summary,
    /// The heading line the pointer is over: its fold chevron shows (an open heading's chevron
    /// shows only then, as in Obsidian; a folded one's always).
    hover: Option<usize>,
    /// The note's name drawn above its first line, as Obsidian's inline title (not text of the
    /// note: no caret goes there), and its laid out box.
    title: Option<String>,
    title_box: Option<std::sync::Arc<layout::LineBox>>,
    /// The Mermaid diagrams' sizes as the editor drew them, by source.
    diagrams: std::collections::HashMap<String, (f32, f32)>,
    /// The note's Mermaid code blocks, from the last update.
    mermaid: Vec<MermaidBlock>,
    /// The note's chunks and link reference labels, from the last update (see `update_edit`).
    chunk_ranges: Vec<Range<usize>>,
    labels: std::collections::BTreeMap<String, String>,
    /// The code blocks not highlighted yet (see `light`).
    lights: light::Lights,
}

/// The last whole layout's inputs: the parsed styles before the live preview's hiding, the
/// callouts, the lines with the caret, the folds, and the width.
struct Laid {
    base: Styles,
    callouts: Vec<kition_note_markdown::Callout>,
    active: Range<usize>,
    /// The selection's bytes (what a caret line's inline syntax shows by).
    selection: Range<usize>,
    folded: Vec<usize>,
    width: f32,
}

/// A note longer than this many lines lays out exactly only near its start and its caret, and
/// guesses the rest's heights until they are shown.
const ESTIMATE_FROM_LINES: usize = 2000;

/// How many lines around the caret (and from the start) a large note lays out exactly (a few
/// screens; what is drawn is laid out exactly anyway, see `realize`).
const EXACT_AROUND: usize = 120;

/// Image file extensions the note draws.
const PICTURE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// Where a picture line's picture starts in `line`: past its indent, a quote's marks, and one list
/// item's marker (`-`, `*`, `+`, `1.`, `1)`) with the space after it, as a picture pasted from a
/// document site is often a list item of its own.
pub(crate) fn picture_start(line: &str) -> usize {
    let mut at = layout::quote_marks_end(line);
    at += line.get(at..).map_or(0, |rest| rest.len() - rest.trim_start_matches([' ', '\t']).len());
    let rest = line.get(at..).unwrap_or("");
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker = match rest.as_bytes().first() {
        Some(b'-' | b'*' | b'+') => 1,
        _ if (1..=9).contains(&digits) && matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) => digits + 1,
        _ => return at,
    };
    let after = rest.get(marker..).unwrap_or("");
    let space = after.len() - after.trim_start_matches([' ', '\t']).len();
    if space == 0 {
        return at;
    }
    at + marker + space
}

/// The image-only lines of `text`: a line whose only content is one image link to a picture (in
/// a list item or a quote, after their marks).
fn picture_lines(text: &str, styles: &Styles, links: &[kition_note_markdown::Link]) -> Vec<(usize, String)> {
    links
        .iter()
        .filter(|link| matches!(link.kind, kition_note_markdown::LinkKind::Image | kition_note_markdown::LinkKind::Embed))
        .filter(|link| !link.is_external())
        .filter(|link| link.file.rsplit('.').next().is_some_and(|ext| PICTURE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())))
        .filter_map(|link| {
            let line = styles.line_of(link.range.start);
            let start = styles.starts.get(line).copied()?;
            let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let content = text.get(start..end)?;
            let picture = picture_start(content);
            (text.get(start + picture..end)?.trim_end() == text.get(link.range.clone())?).then(|| (line, link.file.clone()))
        })
        .collect()
}

impl Default for View {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl View {
    pub fn new(theme: Theme) -> Self {
        let mut view = Self {
            properties_folded: false,
            properties_overlaid: false,
            marks: Vec::new(),
            theme,
            text: String::new(),
            styles: Styles::default(),
            placed: Vec::new(),
            height: 0.0,
            cache: Cache::default(),
            pictures: std::collections::HashMap::new(),
            missing: std::collections::HashSet::new(),
            picture_lines: Vec::new(),
            chunks: chunk::ChunkCache::default(),
            links: Vec::new(),
            tables: Vec::new(),
            laid: None,
            summary: chunk::Summary::default(),
            hover: None,
            title: None,
            title_box: None,
            diagrams: std::collections::HashMap::new(),
            mermaid: Vec::new(),
            lights: light::Lights::default(),
            chunk_ranges: Vec::new(),
            labels: std::collections::BTreeMap::new(),
        };
        view.update("", 0..0);
        view
    }

    /// Lays out `text` again (lines that did not change come from the cache). `active` is the
    /// bytes the selection touches: their lines show their Markdown syntax, the others hide it.
    pub fn update(&mut self, text: &str, active: Range<usize>) {
        self.update_folded(text, active, &[]);
    }

    /// As [`View::update`], with the headings at the bytes `folded` folded (their sections hidden).
    pub fn update_folded(&mut self, text: &str, active: Range<usize>, folded: &[usize]) {
        let selection = active.clone();
        self.update_selected(text, active, selection, folded);
    }

    /// As [`View::update_folded`], with the selection's own bytes: on the lines it touches, a
    /// paragraph's inline syntax shows only where the selection touches it (Obsidian's live
    /// preview; see `Styles::conceal`).
    pub fn update_selected(&mut self, text: &str, active: Range<usize>, selection: Range<usize>, folded: &[usize]) {
        let same = self.laid.as_ref().is_some_and(|laid| laid.folded == folded && laid.width == self.theme.width) && self.text == text;
        // (The note's styles are this text's: a caret a fold hides is on the folded line, below.)
        let active = match same.then(|| layout::folding_line(text, &self.styles, folded, self.styles.line_of(active.start.min(text.len())))).flatten()
        {
            Some(line) => self.styles.starts.get(line).copied().unwrap_or(active.start)..active.end,
            None => active,
        };
        if same && self.relay_caret_lines(&active, &selection) {
            return;
        }
        // An edit inside one self-contained chunk updates only that chunk (see `edit`).
        if !same && self.text != text && self.update_edit(text, &active, &selection, folded) {
            return;
        }
        self.text = text.to_owned();
        // Parsed chunk by chunk: an edit parses only the part it is in.
        let chunk::Parsed { mut styles, tables, links, callouts, summary, ranges, labels, code } = self.chunks.parse(text);
        self.chunk_ranges = ranges;
        self.labels = labels;
        self.summary = summary;
        self.laid = None;
        // A large note lays out exactly only its first lines and those around the caret (and lines
        // laid out before); the rest get guessed heights until they are shown (`realize`).
        let window = (styles.starts.len() > ESTIMATE_FROM_LINES).then(|| {
            let caret = styles.line_of(active.start.min(text.len()));
            caret.saturating_sub(EXACT_AROUND)..caret + EXACT_AROUND
        });
        let window = window.map(|around| if around.start <= EXACT_AROUND { 0..around.end } else { around });
        let first = 0..EXACT_AROUND;
        let window = window.map(|around| if around.start <= first.end { 0..around.end.max(first.end) } else { around });
        // Its code blocks are highlighted there (and where `realize` lays out); a note that is
        // not large has all of them highlighted.
        self.lights.reset(code);
        let bytes_of = |lines: &Range<usize>| {
            styles.starts.get(lines.start).copied().unwrap_or(text.len())..styles.starts.get(lines.end).copied().unwrap_or(text.len())
        };
        let lit: [Range<usize>; 2] = match &window {
            Some(window) => [bytes_of(&first), bytes_of(window)],
            None => [0..text.len(), 0..text.len()],
        };
        for bytes in lit {
            self.lights.light(text, bytes, &mut styles, None);
        }
        let base = styles.clone();
        // A caret a fold hides is on the folded line (Obsidian's fold ends at the section's last
        // character, its line the caret's): that line shows its syntax.
        let active = match layout::folding_line(text, &styles, folded, styles.line_of(active.start.min(text.len()))) {
            Some(line) => styles.starts.get(line).copied().unwrap_or(active.start)..active.end,
            None => active,
        };
        // Callouts first: the hiding tells a callout from a plain quote.
        styles.callouts(text, &callouts, &active);
        styles.conceal(text, &active, &selection);

        // Mermaid blocks away from the caret whose diagram the editor has sized draw as it: their
        // lines keep the diagram's room and show nothing.
        self.mermaid = mermaid_blocks(text, &styles);
        self.mermaid.extend(html_blocks(text, &styles));
        self.diagrams.retain(|source, _| self.mermaid.iter().any(|block| block.source == *source));
        let mut diagrams = Vec::new();
        for block in &mut self.mermaid {
            if block.range.start <= active.end && active.start <= block.range.end {
                continue;
            }
            let Some(&(width, height)) = self.diagrams.get(&block.source) else { continue };
            block.drawn = true;
            styles.set_lines(&block.range, |line| line.block = style::Block::Diagram);
            diagrams.push((block.first, block.last, width, height));
        }
        self.styles = styles;
        self.picture_lines = picture_lines(text, &self.styles, &links);
        let loaded: Vec<(usize, String, (u32, u32))> = self
            .picture_lines
            .iter()
            .filter_map(|(line, name)| self.pictures.get(name).map(|picture| (*line, name.clone(), (picture.width, picture.height))))
            .collect();
        // The inline title takes its own height above the first line (Obsidian: 1.2 line height,
        // half an em below it), and the first line its space as after any line.
        let mut theme = self.theme.clone();
        self.title_box = self.title.as_ref().map(|title| {
            let style = style::LineStyle { block: style::Block::Heading(1), ..style::LineStyle::default() };
            let flags = vec![style::flag::TEXT; title.len()];
            self.cache.line(&self.theme, style, title, &flags, self.theme.content_width())
        });
        if let Some(title) = &self.title_box {
            let first_space = self.styles.lines.first().map_or(0.0, |line| self.theme.space_above(line.block));
            theme.top += title.height + (self.theme.size_of(style::Block::Heading(1)) * 0.5).round() + first_space;
        }
        // Folded callouts (`[!kind]-`) the caret is not in show their title only.
        let collapsed: Vec<Range<usize>> = callouts
            .iter()
            .filter(|callout| callout.fold == Some('-') && !(callout.range.start <= active.end && active.start <= callout.range.end))
            .filter_map(|callout| {
                let body = text.get(callout.range.clone())?.find('\n').map(|at| callout.range.start + at + 1)?;
                (body < callout.range.end).then_some(body..callout.range.end)
            })
            .collect();
        let (placed, height) = layout::lay_out(
            &theme,
            &self.text,
            &self.styles,
            &tables,
            &loaded,
            &diagrams,
            &active,
            folded,
            &collapsed,
            self.properties_folded,
            window.as_ref(),
            self.lights.unlit(),
            &mut self.cache,
        );
        let (placed, height) = if let Some(window) = window.filter(|window| window.start > 0) {
            // The note's first lines are what an opened note shows: exact too.
            self.placed = placed;
            self.height = height;
            self.realize_lines(0..EXACT_AROUND.min(window.start));
            (std::mem::take(&mut self.placed), self.height)
        } else {
            (placed, height)
        };
        self.placed = placed;
        self.height = height;
        self.links = links;
        self.tables = tables;
        self.laid = Some(Laid { base, callouts, active, selection, folded: folded.to_vec(), width: self.theme.width });
    }

    /// When only the lines with the caret changed (the text, folds, and width did not), lays out
    /// just the lines that show or hide their syntax and moves the lines below: a large note
    /// stays quick to click in. Lines whose look depends on others (the front matter, tables,
    /// callouts, pictures, folded sections) take the whole layout. Answers whether it was enough.
    fn relay_caret_lines(&mut self, active: &Range<usize>, selection: &Range<usize>) -> bool {
        let Some(laid) = self.laid.as_ref() else { return false };
        if laid.active == *active && laid.selection == *selection {
            return true;
        }
        let text_len = self.text.len();
        let lines_of = |styles: &Styles, range: &Range<usize>| -> Range<usize> {
            if range.start > text_len {
                return 0..0;
            }
            styles.line_of(range.start)..styles.line_of(range.end.min(text_len)) + 1
        };
        let mut affected: Vec<usize> = lines_of(&laid.base, &laid.active).chain(lines_of(&laid.base, active)).collect();
        // A code block's fences show while the caret is anywhere in it: the whole block.
        let code = |line: usize| laid.base.lines.get(line).is_some_and(|style| matches!(style.block, style::Block::Code { .. }));
        let mut blocks = Vec::new();
        for &line in &affected {
            if code(line) {
                let mut first = line;
                while first > 0 && code(first - 1) {
                    first -= 1;
                }
                let mut last = line;
                while code(last + 1) {
                    last += 1;
                }
                blocks.extend(first..=last);
            }
        }
        affected.extend(blocks);
        affected.sort_unstable();
        affected.dedup();
        let in_callout =
            |start: usize| laid.callouts.iter().any(|callout| callout.range.start <= start && start < callout.range.end.max(callout.range.start + 1));
        for &line in &affected {
            let style = laid.base.lines.get(line).copied().unwrap_or_default();
            let start = laid.base.starts.get(line).copied().unwrap_or(0);
            let simple = !matches!(style.block, style::Block::Frontmatter | style::Block::Property | style::Block::Table { .. } | style::Block::Math)
                && !in_callout(start)
                && !self.picture_lines.iter().any(|(picture, _)| *picture == line)
                && !self.mermaid.iter().any(|block| block.first <= line && line <= block.last)
                && self.placed.get(line).is_some_and(|placed| placed.boxed.height > 0.0);
            if !simple {
                return false;
            }
        }
        // Their code blocks highlighted first.
        let bytes: Vec<Range<usize>> = affected
            .iter()
            .map(|&line| {
                laid.base.starts.get(line).copied().unwrap_or(0)..laid.base.starts.get(line + 1).map_or(text_len, |next| next.saturating_sub(1))
            })
            .collect();
        for range in bytes {
            self.lights.light(&self.text, range, &mut self.styles, self.laid.as_mut().map(|laid| &mut laid.base));
        }
        let Some(laid) = self.laid.as_ref() else { return false };
        let width = self.theme.content_width();
        for &line in &affected {
            let start = laid.base.starts.get(line).copied().unwrap_or(0);
            let end = laid.base.starts.get(line + 1).map_or(text_len, |next| next.saturating_sub(1));
            if let (Some(target), Some(source)) = (self.styles.bytes.get_mut(start..end), laid.base.bytes.get(start..end)) {
                target.copy_from_slice(source);
            }
            if let (Some(target), Some(source)) = (self.styles.lines.get_mut(line), laid.base.lines.get(line)) {
                *target = *source;
            }
            self.styles.conceal_line(&self.text, line, active, selection);
            let style = self.styles.lines.get(line).copied().unwrap_or_default();
            let content = self.text.get(start..end).unwrap_or("");
            let flags = self.styles.bytes.get(start..end).unwrap_or(&[]);
            let boxed = self.cache.line(&self.theme, style, content, flags, width);
            if let Some(placed) = self.placed.get_mut(line) {
                placed.boxed = boxed;
                placed.style = style;
            }
        }
        // The lines from the first changed one down, placed again (heights can change).
        if let Some(&first) = affected.first() {
            let mut y = self.placed.get(first).map_or(self.theme.top, |placed| placed.y);
            for (offset, placed) in self.placed.iter_mut().skip(first).enumerate() {
                if offset > 0 {
                    y += placed.space + placed.pad_top;
                }
                placed.y = y;
                y += placed.boxed.height + placed.pad_bottom;
            }
            self.height = (y + self.theme.bottom).max(self.theme.min_height);
        }
        if let Some(laid) = self.laid.as_mut() {
            laid.active = active.clone();
            laid.selection = selection.clone();
        }
        true
    }

    /// Fits the document to `width` document pixels (the visible width): the readable column stays
    /// centered in it. The next `update` lays out again. Answers whether it changed.
    pub fn set_width(&mut self, width: f32) -> bool {
        let width = width.clamp(320.0, 8192.0).round();
        if (width - self.theme.width).abs() < 1.0 {
            return false;
        }
        self.theme.width = width;
        self.laid = None;
        // The readable column, centered; narrower windows keep a side margin.
        self.theme.margin = ((width - Theme::COLUMN) / 2.0).max(Theme::SIDE.min(width * 0.06)).round();
        true
    }

    /// Paints with `scheme`'s palette (the host's light or dark color scheme): the lengths stay,
    /// so nothing moves. Laid out lines hold their colors, so the next `update` lays out again.
    /// Answers whether it changed.
    pub fn set_scheme(&mut self, scheme: theme::Scheme) -> bool {
        if self.theme.scheme == scheme {
            return false;
        }
        self.theme = self.theme.with_scheme(scheme);
        self.cache = Cache::default();
        self.laid = None;
        true
    }

    /// Fits the room below the note to `height` document pixels (the visible height): half of it,
    /// as Obsidian's editor leaves (its content's bottom padding is half the view), and no least
    /// height, so a short note does not scroll. The next `update` lays out again. Answers whether
    /// it changed.
    pub fn set_visible_height(&mut self, height: f32) -> bool {
        let bottom = (height.clamp(0.0, 16384.0) / 2.0).round();
        if (bottom - self.theme.bottom).abs() < 1.0 && self.theme.min_height == 0.0 {
            return false;
        }
        // The room below the note changes its height only: the lines stay where they are.
        let last = self.placed.last().map_or(self.theme.top, |line| line.y + line.boxed.height + line.pad_bottom);
        self.theme.bottom = bottom;
        self.theme.min_height = 0.0;
        self.height = (last + self.theme.bottom).max(self.theme.min_height);
        true
    }

    /// Whether byte `at` is in what a fold hides (a heading's section, a list item's children).
    pub fn is_hidden(&self, at: usize) -> bool {
        self.placed
            .get(self.styles.line_of(self.clamp(at)))
            .is_some_and(|line| line.boxed.height <= 0.0 && line.fold.is_none() && !matches!(line.style.block, style::Block::Table { .. }))
    }

    /// The heading line at point (`x`, `y`) when the point is in the left margin beside it (its fold
    /// chevron): the heading's first byte.
    pub fn fold_at(&self, x: f32, y: f32) -> Option<usize> {
        let line = self.line_at(y)?;
        if line.fold.is_none() || y < line.y || y >= line.y + line.boxed.height {
            return None;
        }
        let margin = self.theme.margin;
        let (from, to) = if line.style.list > 0 {
            let center = margin + self.fold_x(line);
            (center - 7.0, center + 5.0)
        } else {
            (margin - 30.0, margin - 2.0)
        };
        (x >= from && x <= to).then_some(line.range.start)
    }

    /// Where a foldable line's chevron is centred, x from the content's left: a heading's in the
    /// margin, a list item's just before its marker (Obsidian's `.cm-fold-indicator`, 10 px wide).
    pub fn fold_x(&self, line: &Placed) -> f32 {
        if line.style.list == 0 {
            return -16.0;
        }
        let indent = self.text.get(line.range.clone()).map_or(0, |content| content.len() - content.trim_start_matches([' ', '\t']).len());
        let marker = line.boxed.rows.first().and_then(|row| row.stops.iter().find(|stop| stop.byte >= indent)).map_or(0.0, |stop| stop.x);
        marker - 5.0
    }

    /// The note's links, as the last update found them.
    pub fn links(&self) -> &[kition_note_markdown::Link] {
        &self.links
    }

    /// The note's tables, as the last update found them.
    pub fn index_tables(&self) -> &[kition_note_markdown::Table] {
        &self.tables
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn width(&self) -> u32 {
        self.theme.width.ceil() as u32
    }

    pub fn height(&self) -> u32 {
        self.height.ceil() as u32
    }

    /// The laid-out lines, top to bottom.
    pub fn lines(&self) -> &[Placed] {
        &self.placed
    }

    /// The note's styles (blocks per line, flags per byte).
    /// Sets the inline title (the note's name), drawn above its first line; answers whether it
    /// changed. The next `update` lays out again.
    pub fn set_title(&mut self, title: Option<String>) -> bool {
        if self.title == title {
            return false;
        }
        self.title = title;
        self.laid = None;
        true
    }

    /// The inline title as a line to draw, at the column's top.
    pub fn title_line(&self) -> Option<layout::Placed> {
        let boxed = self.title_box.clone()?;
        Some(layout::Placed {
            range: 0..0,
            y: self.theme.top,
            style: style::LineStyle::default(),
            boxed,
            fold: None,
            space: 0.0,
            pad_top: 0.0,
            pad_bottom: 0.0,
        })
    }

    /// Sets the heading line the pointer is over; answers whether it changed.
    /// Folds the properties block to its heading, or opens it; the next update lays it out.
    pub fn set_properties_folded(&mut self, folded: bool) {
        if self.properties_folded != folded {
            self.properties_folded = folded;
            self.laid = None;
        }
    }

    /// Whether the properties block is folded to its heading.
    pub fn properties_folded(&self) -> bool {
        self.properties_folded
    }

    /// Hands the properties block's drawing to the editor (it draws it over the view); answers
    /// whether that changed (the view's pixels did).
    pub fn set_properties_overlaid(&mut self, overlaid: bool) -> bool {
        let changed = self.properties_overlaid != overlaid;
        self.properties_overlaid = overlaid;
        changed
    }

    /// Whether the editor draws the properties block.
    pub fn properties_overlaid(&self) -> bool {
        self.properties_overlaid
    }

    /// Sets the text the open comment threads are anchored to (byte ranges); answers whether it
    /// changed.
    pub fn set_marks(&mut self, marks: Vec<std::ops::Range<usize>>) -> bool {
        let changed = self.marks != marks;
        self.marks = marks;
        changed
    }

    /// The text the open comment threads are anchored to (byte ranges).
    pub fn marks(&self) -> &[std::ops::Range<usize>] {
        &self.marks
    }

    /// The front matter as the properties block, `None` while it is edited as its source or holds
    /// nothing (Obsidian shows nothing then).
    pub fn properties_block(&self) -> Option<PropertiesBlock> {
        use kition_note_markdown::Typed;
        let text = self.text();
        let lines = self.lines();
        let first = lines.first().filter(|line| line.style.block == style::Block::Property)?;
        let frontmatter = self.summary().frontmatter.clone()?;
        if frontmatter.entries.is_empty() && text.get(frontmatter.body.clone()).is_none_or(|body| body.trim().is_empty()) {
            return None;
        }
        let row_of = |at: usize| lines.get(self.styles().line_of(at));
        let rows = frontmatter
            .entries
            .iter()
            .filter_map(|entry| {
                let line = row_of(entry.lines.start)?;
                let (kind, values) = match (entry.key.as_str(), entry.typed.as_ref()) {
                    ("tags" | "tag", Some(Typed::List(items))) => ("tags", items.clone()),
                    ("tags" | "tag", Some(Typed::String(item))) => ("tags", vec![item.clone()]),
                    ("aliases" | "alias", Some(Typed::List(items))) => ("aliases", items.clone()),
                    ("aliases" | "alias", Some(Typed::String(item))) => ("aliases", vec![item.clone()]),
                    // Obsidian's cssclasses is a list property even with one value.
                    ("cssclasses" | "cssclass", Some(Typed::String(item))) => ("list", vec![item.clone()]),
                    (_, Some(Typed::List(items))) => ("list", items.clone()),
                    (_, Some(Typed::Boolean(on))) => ("checkbox", vec![on.to_string()]),
                    (_, Some(Typed::Number(number))) => ("number", vec![number.clone()]),
                    (_, Some(Typed::Date(date))) => ("date", vec![date.clone()]),
                    (_, Some(Typed::DateTime(date))) => ("datetime", vec![date.clone()]),
                    (_, Some(Typed::String(value))) => ("text", vec![value.clone()]),
                    _ => ("text", vec![text.get(entry.value.clone()).unwrap_or("").trim().to_owned()]),
                };
                Some(PropertyRow { key: entry.key.clone(), kind, values, y: line.y, height: line.boxed.height, value_at: entry.value.start })
            })
            .collect();
        let last = lines.iter().take_while(|line| line.style.block == style::Block::Property).last();
        Some(PropertiesBlock {
            top: first.y - first.pad_top,
            bottom: last.map_or(first.y, |line| line.y + line.boxed.height + line.pad_bottom),
            end: frontmatter.body.end.saturating_sub(1),
            rows,
            folded: self.properties_folded,
        })
    }

    pub fn set_hover(&mut self, line: Option<usize>) -> bool {
        let changed = self.hover != line;
        self.hover = line;
        changed
    }

    /// The heading line the pointer is over.
    pub fn hover(&self) -> Option<usize> {
        self.hover
    }

    /// The heading line at document height `y` (its chevron's row), if `y` is on one.
    pub fn heading_at(&self, y: f32) -> Option<usize> {
        let index = self.placed.partition_point(|line| line.y + line.boxed.height < y);
        self.placed.get(index).filter(|line| line.fold.is_some() && line.y <= y).map(|_| index)
    }

    /// The note in short, from its chunks' indexes (each memoised by its text): headings,
    /// counts, the front matter. Quick after an edit, where indexing the whole note is not.
    pub fn summary(&self) -> &chunk::Summary {
        &self.summary
    }

    pub fn styles(&self) -> &Styles {
        &self.styles
    }

    /// The byte `at` moved onto a character boundary of the text, within it.
    pub fn clamp(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while at > 0 && !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// The line and row a byte is on. At a wrap, the byte that ends one row and starts the next
    /// is on the next.
    /// The folded line whose fold hides byte `at`'s line (the lines between are hidden).
    fn folded_over(&self, at: usize) -> Option<&Placed> {
        let index = self.styles.line_of(self.clamp(at));
        let line = self.placed.get(index)?;
        if line.boxed.height > 0.0 {
            return None;
        }
        let shown = self.placed.get(..index)?.iter().rev().find(|before| before.boxed.height > 0.0)?;
        (shown.fold == Some(true)).then_some(shown)
    }

    fn row_of(&self, at: usize) -> Option<(&Placed, &Row)> {
        let at = self.clamp(at);
        let line = self.placed.get(self.styles.line_of(at))?;
        let local = at.saturating_sub(line.range.start).min(line.range.len());
        let rows = &line.boxed.rows;
        let row = rows.iter().find(|row| local < row.end).or_else(|| rows.last())?;
        Some((line, row))
    }

    /// The stop for `local` in `row`: the last one at or before it.
    fn stop_of(row: &Row, local: usize) -> Option<&Stop> {
        let index = row.stops.partition_point(|stop| stop.byte <= local);
        row.stops.get(index.saturating_sub(1)).or_else(|| row.stops.first())
    }

    /// Whether the caret can be at byte `at` (a table's pipes and padding have no place).
    pub fn has_stop(&self, at: usize) -> bool {
        match self.row_of(at) {
            Some((line, row)) => {
                let local = self.clamp(at).saturating_sub(line.range.start);
                // A table's delimiter row takes no room: no place either.
                row.height > 0.0 && (row.table.is_none() || row.stops.iter().any(|stop| stop.byte == local))
            }
            None => true,
        }
    }

    /// `at` moved to the nearest place the caret can be, going forward or back from it.
    pub fn snap(&self, at: usize, forward: bool) -> usize {
        let mut at = self.clamp(at);
        for _ in 0..self.text.len().min(4096) {
            if self.has_stop(at) {
                return at;
            }
            // A line with no room (folded away, a table's delimiter): past it at once.
            if let Some(line) = self.placed.get(self.styles.line_of(at))
                && line.boxed.height <= 0.0
            {
                at = if forward { (line.range.end + 1).min(self.text.len()) } else { line.range.start.saturating_sub(1) };
                at = self.clamp(at);
                if (forward && at >= self.text.len()) || (!forward && at == 0) {
                    return at;
                }
                continue;
            }
            let next = if forward {
                self.text.get(at..).and_then(|rest| rest.chars().next()).map(|c| at + c.len_utf8())
            } else {
                self.text.get(..at).and_then(|before| before.chars().next_back()).map(|c| at - c.len_utf8())
            };
            match next {
                Some(next) => at = next,
                None => return at,
            }
        }
        at
    }

    /// The caret for byte `at`.
    pub fn caret(&self, at: usize) -> Caret {
        // A byte a fold hides: the caret is drawn at the folded line's end, as Obsidian draws it
        // (its fold is one range up to the section's last character, the caret allowed there).
        if let Some(folded) = self.folded_over(at) {
            let row = folded.boxed.rows.last();
            let x = row.and_then(|row| row.stops.last()).map_or(0.0, |stop| stop.x);
            let (top, height) = row.map_or((0.0, folded.boxed.height), |row| (row.top, row.caret_height));
            return Caret { x: self.theme.margin + x, top: folded.y + top, height };
        }
        self.caret_in_line(at)
    }

    /// Where the caret is at byte `at` in its own line (a hidden line's: no height).
    fn caret_in_line(&self, at: usize) -> Caret {
        match self.row_of(at) {
            Some((line, row)) => {
                let local = self.clamp(at).saturating_sub(line.range.start);
                let stop = Self::stop_of(row, local).copied().unwrap_or(Stop { byte: 0, x: 0.0, dy: 0.0, cell: 0 });
                Caret { x: self.theme.margin + stop.x, top: line.y + row.top + stop.dy, height: row.caret_height }
            }
            None => Caret { x: self.theme.margin, top: self.theme.top, height: self.theme.line_height_of(style::Block::Paragraph) },
        }
    }

    /// The line whose rows span `y` (the first or last outside them); a line with no height is passed over.
    fn line_at(&self, y: f32) -> Option<&Placed> {
        let index = self.placed.partition_point(|line| line.y + line.boxed.height <= y);
        let found = self.placed.get(index.min(self.placed.len().saturating_sub(1)))?;
        if found.boxed.height > 0.0 {
            return Some(found);
        }
        self.placed
            .get(index..)
            .and_then(|rest| rest.iter().find(|line| line.boxed.height > 0.0))
            .or_else(|| self.placed.get(..index).and_then(|before| before.iter().rev().find(|line| line.boxed.height > 0.0)))
    }

    /// The byte nearest the point (`x`, `y`).
    pub fn hit(&self, x: f32, y: f32) -> usize {
        let Some(line) = self.line_at(y) else { return 0 };
        let rows = &line.boxed.rows;
        let local_y = y - line.y;
        let Some(row) = rows.iter().find(|row| local_y < row.top + row.height).or_else(|| rows.last()) else { return line.range.start };
        let local_x = x - self.theme.margin;
        let in_row = local_y - row.top;
        let wraps = rows.last().is_some_and(|final_row| final_row.start != row.start);
        let last = row.stops.len().saturating_sub(1);
        // In a table, the column the point is in, so a click beside a short cell's text finds that cell.
        let column = row
            .table
            .as_ref()
            .and_then(|table| table.columns.windows(2).position(|edge| matches!(edge, [left, right] if local_x >= *left && local_x < *right)))
            .map(|column| column + 1);
        let mut best = row.stops.first().map_or(0, |stop| stop.byte);
        let mut distance = f32::MAX;
        for (index, stop) in row.stops.iter().enumerate() {
            // A wrapped row's end is the next row's start: a click there stays on this row.
            if index == last && wraps && index > 0 && row.table.is_none() {
                continue;
            }
            if let Some(column) = column
                && usize::from(stop.cell) != column
            {
                continue;
            }
            let vertical = if in_row < stop.dy {
                stop.dy - in_row
            } else if in_row > stop.dy + row.caret_height {
                in_row - stop.dy - row.caret_height
            } else {
                0.0
            };
            let gap = vertical * 1000.0 + (stop.x - local_x).abs();
            if gap < distance {
                distance = gap;
                best = stop.byte;
            }
        }
        line.range.start + best
    }

    /// The rectangles a selection from `a` to `b` covers: each text line's part, to past the line's
    /// end where the selection goes on beyond it.
    pub fn selection(&self, a: usize, b: usize) -> Vec<Rect> {
        let (from, to) = (self.clamp(a.min(b)), self.clamp(a.max(b)));
        if from == to {
            return Vec::new();
        }
        let first = self.styles.line_of(from);
        let last = self.styles.line_of(to);
        let mut out = Vec::new();
        for line in self.placed.get(first..=last).unwrap_or(&[]) {
            for row in &line.boxed.rows {
                let (row_from, row_to) = (line.range.start + row.start, line.range.start + row.end);
                if row_to < from || row_from > to || (row_to == from && row.end > row.start) || row.height <= 0.0 {
                    continue;
                }
                // The stops inside the selection, by text line (a cell's wrapped lines, or the row).
                let mut groups: Vec<(u16, f32, f32, f32)> = Vec::new();
                for stop in &row.stops {
                    let byte = line.range.start + stop.byte;
                    if byte < from || byte > to {
                        continue;
                    }
                    match groups.last_mut() {
                        Some(group) if group.0 == stop.cell && (group.1 - stop.dy).abs() < 0.5 => group.3 = stop.x,
                        _ => groups.push((stop.cell, stop.dy, stop.x, stop.x)),
                    }
                }
                let past = to > row_to && row.table.is_none();
                let count = groups.len();
                for (index, (_, dy, x0, x1)) in groups.into_iter().enumerate() {
                    let x1 = if past && index + 1 == count { x1 + self.theme.body_size * 0.35 } else { x1 };
                    if x1 > x0 {
                        let top = line.y + row.top + dy;
                        out.push(Rect { x0: self.theme.margin + x0, y0: top, x1: self.theme.margin + x1, y1: top + row.caret_height });
                    }
                }
            }
        }
        out
    }

    /// The first and last byte of the text line `at` is on (Home and End): a row, or a table cell.
    pub fn row_edges(&self, at: usize) -> (usize, usize) {
        let Some((line, row)) = self.row_of(at) else { return (0, 0) };
        let local = self.clamp(at).saturating_sub(line.range.start);
        if row.table.is_some() {
            let cell = Self::stop_of(row, local).map_or(0, |stop| stop.cell);
            let mut stops = row.stops.iter().filter(|stop| stop.cell == cell);
            let first = stops.next().map_or(local, |stop| stop.byte);
            let last = row.stops.iter().rev().find(|stop| stop.cell == cell).map_or(local, |stop| stop.byte);
            return (line.range.start + first, line.range.start + last);
        }
        let is_last = line.boxed.rows.last().is_some_and(|final_row| final_row.start == row.start);
        // A wrapped row ends before its trailing space, so End stays on the row; a line's last
        // row ends before a `\r` a mixed-ending note keeps at its end.
        let mut end = line.range.start + row.end;
        if is_last && end > line.range.start && self.text.as_bytes().get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        if !is_last {
            let tail = self.text.get(line.range.start + row.start..end).unwrap_or("");
            end -= tail.len() - tail.trim_end().len();
        }
        (line.range.start + row.start, end)
    }

    /// The byte `rows` text lines above (negative) or below `at`, nearest to `x` (the goal
    /// column); the text's start or end past the first or last line.
    pub fn vertical(&self, at: usize, x: f32, rows: i32) -> usize {
        let mut at_now = at;
        let bottom = self.placed.last().map_or(0.0, |line| line.y + line.boxed.height);
        for _ in 0..rows.unsigned_abs() {
            let here = self.caret_in_line(at_now);
            // Within a wrapped line, the next row; past its first or last row, the next shown
            // line's (over the padding and space between lines).
            let index = self.styles.line_of(at_now);
            let Some(line) = self.placed.get(index) else { return at_now };
            // The visual row the caret is in (a table cell's caret sits inside its row's padding).
            let row = line.boxed.rows.iter().rposition(|row| line.y + row.top <= here.top + 0.5).unwrap_or(0);
            let middle = |line: &layout::Placed, row: &layout::Row| line.y + row.top + row.height / 2.0;
            // A wrapped table cell has several text lines in one row: the next one first.
            let inside = line.boxed.rows.get(row).and_then(|current| {
                // Past the cell's padding (and the header's top border) is out of the row.
                let (top, bottom) = (line.y + current.top + 4.5, line.y + current.top + current.height - 4.5);
                let y = if rows < 0 { here.top - 1.0 } else { here.top + here.height + 1.0 };
                (y > top && y < bottom).then_some(y)
            });
            let y = if let Some(y) = inside {
                y
            } else if rows < 0 {
                if let Some(above) = row.checked_sub(1).and_then(|above| line.boxed.rows.get(above)) {
                    middle(line, above)
                } else {
                    match self.placed.get(..index).and_then(|above| above.iter().rev().find(|line| line.boxed.height > 0.0)) {
                        Some(previous) => previous.boxed.rows.last().map_or(previous.y, |last| middle(previous, last)),
                        None => return 0,
                    }
                }
            } else if let Some(below) = line.boxed.rows.get(row + 1) {
                middle(line, below)
            } else {
                match self.placed.get(index + 1..).and_then(|below| below.iter().find(|line| line.boxed.height > 0.0)) {
                    Some(next) => next.boxed.rows.first().map_or(next.y, |first| middle(next, first)),
                    None => return self.text.len(),
                }
            };
            if y > bottom {
                return self.text.len();
            }
            // A callout the caret goes into shows as the quote it is written as (Obsidian's and
            // ours): the place is found on that quote's geometry, its text 14 px a level in.
            let x = match self.line_at(y).filter(|line| line.style.callout > 0) {
                Some(line) => x + self.theme.indent_of(line.style) - 14.0 * f32::from(line.style.quote),
                None => x,
            };
            at_now = self.past_quote_marks(self.hit(x, y), x);
        }
        at_now
    }

    /// Where a vertical move to `x` lands on `at`'s line when `at` is before its quote marks (each
    /// `>` and the space after it, hidden while the caret is elsewhere): as Obsidian, on the line
    /// as it shows with the caret on it, its marks drawn in the quote's padding before its text.
    /// Left of them is the line's start (`‸> abc`); among them, the nearest place between them.
    fn past_quote_marks(&self, at: usize, x: f32) -> usize {
        let index = self.styles.line_of(at);
        let Some(line) = self.placed.get(index) else { return at };
        if line.style.quote == 0 {
            return at;
        }
        let content = self.text.get(line.range.clone()).unwrap_or("");
        let marks = layout::quote_marks_end(content);
        // The text's start and the marks' widths, laid in the padding before it.
        let size = self.theme.size_of(line.style.block);
        let text_x = self.theme.margin + self.theme.indent_of(line.style);
        // (A goal left of the text lands among the marks even where the hidden marks put the
        // nearest place at the text's start.)
        if at > line.range.start + marks || (at == line.range.start + marks && x >= text_x - 0.5) {
            return at;
        }
        let widths: Vec<f32> = content.get(..marks).unwrap_or("").chars().map(|c| layout::text_width(c.encode_utf8(&mut [0; 4]), size)).collect();
        let total: f32 = widths.iter().sum();
        let mut edge = text_x - total.min(text_x - self.theme.margin);
        let mut best = (0, (x - edge).abs());
        for (offset, width) in widths.iter().enumerate() {
            edge += width;
            let distance = (x - edge).abs();
            if distance < best.1 {
                best = (offset + 1, distance);
            }
        }
        (line.range.start + best.0).min(line.range.end)
    }

    /// The pictures the note's image-only lines name that are neither loaded nor known missing.
    pub fn wanted_pictures(&self) -> Vec<String> {
        let mut wanted: Vec<String> = self
            .picture_lines
            .iter()
            .map(|(_, name)| name.clone())
            .filter(|name| !self.pictures.contains_key(name) && !self.missing.contains(name))
            .collect();
        wanted.sort();
        wanted.dedup();
        wanted
    }

    /// The pictures drawn now: an image-only line's picture that is loaded.
    pub fn drawn_pictures(&self) -> Vec<String> {
        let drawn: std::collections::HashSet<&str> =
            self.placed.iter().flat_map(|line| line.boxed.rows.iter()).filter_map(|row| row.image.as_ref().map(|(name, _)| name.as_str())).collect();
        let mut names: Vec<String> = drawn.into_iter().map(str::to_owned).collect();
        names.sort();
        names
    }

    /// A picture's pixels (straight RGBA8, `width` × `height`), by the name the note writes; the
    /// next `update` draws it. An empty `rgba` records the picture as missing.
    pub fn set_picture(&mut self, name: &str, width: u32, height: u32, rgba: &[u8]) {
        self.laid = None;
        let pixmap = (width > 0
            && height > 0
            && width <= u32::from(u16::MAX)
            && height <= u32::from(u16::MAX)
            && rgba.len() == width as usize * height as usize * 4)
            .then(|| {
                let data: Vec<vello_cpu::color::PremulRgba8> = rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|px| {
                        let alpha = u16::from(px[3]);
                        let premultiply = |channel: u8| ((u16::from(channel) * alpha + 127) / 255) as u8;
                        vello_cpu::color::PremulRgba8 { r: premultiply(px[0]), g: premultiply(px[1]), b: premultiply(px[2]), a: px[3] }
                    })
                    .collect();
                vello_cpu::Pixmap::from_parts(data, width as u16, height as u16)
            });
        match pixmap {
            Some(pixmap) => {
                self.missing.remove(name);
                self.pictures.insert(name.to_owned(), Picture { pixmap: std::sync::Arc::new(pixmap), width, height });
            }
            None => {
                self.missing.insert(name.to_owned());
            }
        }
    }

    /// Lays out exactly the lines with guessed heights that overlap document heights `top` to
    /// `bottom` (with a screen's margin), and moves the lines below them: what is drawn is never
    /// a guess. Answers whether anything changed.
    pub fn realize(&mut self, top: f32, bottom: f32) -> bool {
        let margin = (bottom - top).max(400.0);
        let first = self.placed.partition_point(|line| line.y + line.boxed.height < top - margin);
        let last = self.placed.partition_point(|line| line.y <= bottom + margin);
        self.realize_lines(first..last.max(first))
    }

    /// As [`View::realize`], the lines between `top` and `bottom` only (no margin around them).
    pub fn realize_between(&mut self, top: f32, bottom: f32) -> bool {
        let first = self.placed.partition_point(|line| line.y + line.boxed.height < top);
        let last = self.placed.partition_point(|line| line.y <= bottom);
        self.realize_lines(first..last.max(first))
    }

    /// Lays out exactly the lines in `lines` that have guessed heights, and places the lines from
    /// the first of them down again (as the whole layout places them, so the same note lays out
    /// to the same pixels either way).
    fn realize_lines(&mut self, lines: Range<usize>) -> bool {
        let width = self.theme.content_width();
        let mut first_changed: Option<usize> = None;
        let end = lines.end.min(self.placed.len());
        for index in lines.start..end {
            let Some(line) = self.placed.get(index) else { break };
            if !line.boxed.estimated {
                continue;
            }
            // Its code block highlighted first.
            let range = line.range.clone();
            self.lights.light(&self.text, range, &mut self.styles, self.laid.as_mut().map(|laid| &mut laid.base));
            let Some(line) = self.placed.get(index) else { break };
            // A guessed table's line: the whole table laid out (its columns fit all its rows).
            let start = line.range.start;
            if matches!(line.style.block, style::Block::Table { .. })
                && let Some(number) = self.tables.iter().position(|table| table.range.contains(&start))
                && let Some(table) = self.tables.get(number)
            {
                let laid = self.cache.table(&self.theme, &self.text, &self.styles, table, number, width);
                for (at, boxed) in laid.iter() {
                    if let Some(line) = self.placed.get_mut(*at) {
                        line.boxed = boxed.clone();
                    }
                    first_changed = Some(first_changed.map_or(*at, |first| first.min(*at)));
                }
                continue;
            }
            let content = self.text.get(line.range.clone()).unwrap_or("");
            let flags = self.styles.bytes.get(line.range.clone()).unwrap_or(&[]);
            let boxed = self.cache.line(&self.theme, line.style, content, flags, width);
            if let Some(line) = self.placed.get_mut(index) {
                line.boxed = boxed;
            }
            first_changed.get_or_insert(index);
        }
        let Some(first) = first_changed else { return false };
        let mut y = match first.checked_sub(1).and_then(|line| self.placed.get(line)) {
            Some(previous) => previous.y + previous.boxed.height + previous.pad_bottom,
            None => self.placed.first().map_or(self.theme.top, |line| line.y - line.space - line.pad_top),
        };
        for line in self.placed.iter_mut().skip(first) {
            y += line.space + line.pad_top;
            line.y = y;
            y += line.boxed.height + line.pad_bottom;
        }
        self.height = (y + self.theme.bottom).max(self.theme.min_height);
        true
    }

    /// A Mermaid diagram's size as the editor drew it (document pixels), by its source; the next
    /// `update` keeps its room in place of its code block. Answers whether it changed.
    pub fn set_diagram(&mut self, source: &str, width: f32, height: f32) -> bool {
        let size = (width.max(0.0).ceil(), height.max(0.0).ceil());
        if self.diagrams.get(source) == Some(&size) || !self.mermaid.iter().any(|block| block.source == source) {
            return false;
        }
        self.diagrams.insert(source.to_owned(), size);
        self.laid = None;
        true
    }

    /// The blocks away from the caret the editor has not sized: each one's kind and source.
    pub fn wanted_diagrams(&self) -> Vec<(&'static str, String)> {
        let mut out: Vec<(&'static str, String)> = Vec::new();
        for block in &self.mermaid {
            let active = self.laid.as_ref().is_some_and(|laid| block.range.start <= laid.active.end && laid.active.start <= block.range.end);
            if !active && !self.diagrams.contains_key(&block.source) && !out.iter().any(|(_, source)| *source == block.source) {
                out.push((block.kind, block.source.clone()));
            }
        }
        out
    }

    /// The Mermaid diagrams drawn between document heights `top` and `bottom`.
    pub fn diagrams_in(&self, top: f32, bottom: f32) -> Vec<DiagramBox> {
        self.mermaid
            .iter()
            .filter(|block| block.drawn)
            .filter_map(|block| {
                let line = self.placed.get(block.first)?;
                let (width, height) = self.diagrams.get(&block.source).copied()?;
                (line.y <= bottom && line.y + line.boxed.height >= top).then(|| DiagramBox {
                    top: line.y,
                    left: self.theme.margin + self.theme.indent_of(line.style),
                    width,
                    height,
                    kind: block.kind,
                    source: block.source.clone(),
                    code: block.code,
                })
            })
            .collect()
    }

    pub(crate) fn picture(&self, name: &str) -> Option<&Picture> {
        self.pictures.get(name)
    }

    /// The note's tables as grids, for the editor's table controls.
    pub fn tables(&self) -> Vec<TableGrid> {
        self.tables_in(f32::NEG_INFINITY, f32::INFINITY)
    }

    /// The grids of the tables drawn between document heights `top` and `bottom`, each whole
    /// (the controls of the tables in view: a large note has thousands).
    pub fn tables_in(&self, top: f32, bottom: f32) -> Vec<TableGrid> {
        let first = self.placed.partition_point(|line| line.y + line.boxed.height < top);
        let mut wanted: Vec<usize> = Vec::new();
        for line in self.placed.iter().skip(first).take_while(|line| line.y <= bottom) {
            for row in &line.boxed.rows {
                if let Some(table) = &row.table
                    && wanted.last() != Some(&table.table)
                {
                    wanted.push(table.table);
                }
            }
        }
        wanted.dedup();
        wanted
            .into_iter()
            .filter_map(|index| {
                let source = self.tables.get(index)?;
                let lines =
                    self.styles.line_of(source.range.start)..self.styles.line_of(source.range.end.saturating_sub(1).max(source.range.start)) + 1;
                let mut grid: Option<TableGrid> = None;
                for number in lines {
                    let Some(line) = self.placed.get(number) else { continue };
                    for row in &line.boxed.rows {
                        let Some(table) = row.table.as_ref().filter(|table| table.table == index) else { continue };
                        let span = (line.y + row.top, line.y + row.top + row.height);
                        let grid = grid.get_or_insert_with(|| TableGrid {
                            index,
                            columns: table.columns.iter().map(|x| self.theme.margin + x).collect(),
                            rows: Vec::new(),
                            lines: Vec::new(),
                        });
                        grid.rows.push(span);
                        grid.lines.push(number + 1);
                    }
                }
                grid
            })
            .collect()
    }

    /// The fenced code blocks drawn between document heights `top` and `bottom`: each block's
    /// box (top, bottom, and the column's right edge), its language (the opening fence's info),
    /// and the byte range of its code (between its fences).
    pub fn code_blocks_in(&self, top: f32, bottom: f32) -> Vec<CodeBox> {
        let mut out: Vec<CodeBox> = Vec::new();
        let first = self.placed.partition_point(|line| line.y + line.boxed.height < top);
        // Back to the block's opening fence, so a block cut by the top comes whole.
        let mut index = first;
        while index > 0 && self.placed.get(index - 1).is_some_and(|line| matches!(line.style.block, style::Block::Code { .. })) {
            index -= 1;
        }
        while let Some(line) = self.placed.get(index) {
            if line.y > bottom {
                break;
            }
            if line.style.block != (style::Block::Code { fence: true }) {
                index += 1;
                continue;
            }
            let opening = line;
            let mut last = index;
            while self.placed.get(last + 1).is_some_and(|next| next.style.block == (style::Block::Code { fence: false })) {
                last += 1;
            }
            let closing = self.placed.get(last + 1).filter(|next| next.style.block == (style::Block::Code { fence: true }));
            let end_line = if closing.is_some() { last + 1 } else { last };
            let Some(end) = self.placed.get(end_line) else { break };
            // Past a callout's quote marks.
            let fence = self.text.get(opening.range.clone()).unwrap_or("").trim_start_matches(['>', ' ', '\t']);
            let language = fence.trim_start_matches(['`', '~']).split_whitespace().next().unwrap_or("").to_owned();
            // A block the caret is in shows its fences, not its label (as Obsidian).
            let editing = self.laid.as_ref().is_some_and(|laid| opening.range.start <= laid.active.end && laid.active.start <= end.range.end);
            if editing {
                index = end_line + 1;
                continue;
            }
            let code_start = self.placed.get(index + 1).map_or(opening.range.end, |line| line.range.start);
            let code_end = if last > index { self.placed.get(last).map_or(code_start, |line| line.range.end) } else { code_start };
            out.push(CodeBox {
                top: opening.y,
                bottom: end.y + end.boxed.height,
                right: self.theme.width - self.theme.margin - self.theme.inset_right(opening.style),
                language,
                code: code_start..code_end.max(code_start),
                callout: opening.style.callout > 0,
                left: self.theme.margin + if opening.style.callout > 0 { 24.0 } else { 0.0 },
            });
            index = end_line + 1;
        }
        out
    }

    /// Straight-alpha RGBA of the area at (`x`, `y`) shown at `scale`, `width` × `height` large.
    /// Outside the document is transparent.
    pub fn frame(&self, x: f64, y: f64, scale: f64, width: u16, height: u16) -> Vec<u8> {
        paint::frame(self, x, y, scale, width, height)
    }
}

#[cfg(test)]
mod fuzz;
#[cfg(test)]
mod tests;
