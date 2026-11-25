//! What each byte of the note looks like: its inline style, and the block each source line
//! belongs to. One pass over pulldown-cmark's offset iterator (the same options as the note's
//! index), painting flags over byte ranges; bytes inside an element that no text event covers
//! are its Markdown syntax (`**`, `#`, `>`, a link's destination) and draw dimmed.

use std::ops::Range;
use std::sync::Arc;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

/// Inline style flags of one byte.
pub mod flag {
    /// A byte's flags.
    pub type Flags = u32;

    pub const STRONG: Flags = 1;
    pub const EMPHASIS: Flags = 1 << 1;
    pub const STRIKE: Flags = 1 << 2;
    pub const CODE: Flags = 1 << 3;
    pub const LINK: Flags = 1 << 4;
    /// Markdown syntax: drawn dimmed.
    pub const MARK: Flags = 1 << 5;
    /// Covered by a text event (content, not syntax).
    pub const TEXT: Flags = 1 << 6;
    /// Inside an inline element or a block whose uncovered bytes are syntax.
    pub const MARKUP: Flags = 1 << 7;
    pub const MATH: Flags = 1 << 8;
    pub const HIGHLIGHT: Flags = 1 << 9;
    pub const TAG: Flags = 1 << 10;
    /// Hidden in the live preview: Markdown syntax on a line the caret is not on.
    pub const HIDE: Flags = 1 << 11;
    /// A task's `[ ]` or `[x]`, drawn as a box in the live preview.
    pub const TASK: Flags = 1 << 12;
    /// A bullet list's marker, drawn as a dot in the live preview.
    pub const BULLET: Flags = 1 << 13;
    /// Code highlighting, in the two top bits: a keyword, a literal (string or number), a comment.
    pub const SYNTAX: Flags = 0b11 << 14;
    pub const KEYWORD: Flags = 0b01 << 14;
    pub const LITERAL: Flags = 0b10 << 14;
    pub const COMMENT: Flags = 0b11 << 14;
    /// A footnote's reference (`[^1]`) or its definition's label: small, raised, and muted.
    pub const FOOTNOTE: Flags = 1 << 16;
    /// A footnote's definition: its text a size smaller.
    pub const FOOTDEF: Flags = 1 << 17;
    /// A bare web address in text, drawn as a link (without the external link icon).
    pub const BAREURL: Flags = 1 << 18;
    /// A Markdown link to a web address: the external link icon after its text.
    pub const EXTERNAL: Flags = 1 << 19;
    /// Inline HTML's `<u>` and `<ins>`: underlined.
    pub const UNDERLINE: Flags = 1 << 20;
    /// Inline HTML's `<sub>`: smaller and lowered.
    pub const SUB: Flags = 1 << 21;
    /// Inline HTML's `<sup>`: smaller and raised.
    pub const SUP: Flags = 1 << 22;
    /// Inline HTML's `<mark>`: the browser's yellow behind black text.
    pub const MARKED: Flags = 1 << 23;
    /// An inline footnote's text (`^[text]`): small and raised, in the text color.
    pub const INLINE_NOTE: Flags = 1 << 24;
    /// Inline HTML's text color (`<span style="color: …">`, `<font color>`): an index into the
    /// colors [`super::intern_color`] keeps, 0 for none.
    pub const COLOR: Flags = 0x7f << 25;
    pub const COLOR_SHIFT: u32 = 25;
    /// In a fenced code block, its highlighted token's kind (`highlight::Kind::code`) in the
    /// color bits, which inline HTML never sets there.
    pub const TOKEN: Flags = COLOR;
}

/// Obsidian 1.14's highlight colours: the emoji written after a highlight's `==`, and the colour
/// drawn behind its text (Obsidian's at 30% over white; alpha 254 tells it from a text colour).
pub const HIGHLIGHT_COLORS: [(&str, [u8; 4]); 6] = [
    ("🔴", [248, 193, 200, 254]),
    ("🟠", [249, 214, 178, 254]),
    ("🟡", [246, 230, 178, 254]),
    ("🟢", [181, 234, 202, 254]),
    ("🔵", [181, 211, 245, 254]),
    ("🟣", [215, 203, 250, 254]),
];

fn palette() -> &'static std::sync::Mutex<Vec<[u8; 4]>> {
    static PALETTE: std::sync::OnceLock<std::sync::Mutex<Vec<[u8; 4]>>> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// The index (1 to 127) of `color` among the colors inline HTML has used, the same for the same
/// color in every note (so laid out lines stay valid); 0 when all are taken.
pub fn intern_color(color: [u8; 4]) -> u32 {
    let mut colors = palette().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(at) = colors.iter().position(|known| *known == color) {
        return u32::try_from(at + 1).unwrap_or(0);
    }
    if colors.len() >= 127 {
        return 0;
    }
    colors.push(color);
    u32::try_from(colors.len()).unwrap_or(0)
}

/// The color [`intern_color`] gave `index`.
pub fn interned_color(index: u32) -> Option<[u8; 4]> {
    let colors = palette().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    colors.get(usize::try_from(index).ok()?.checked_sub(1)?).copied()
}

/// A CSS color as inline HTML writes it: `#rgb`, `#rrggbb`, `rgb(r, g, b)`, or a common name.
pub fn css_color(value: &str) -> Option<[u8; 4]> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let digit = |at: usize, width: usize| u8::from_str_radix(hex.get(at..at + width)?, 16).ok();
        return match hex.len() {
            3 => Some([digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17, 255]),
            6 => Some([digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, 255]),
            _ => None,
        };
    }
    if let Some(inner) = value.strip_prefix("rgb(").or_else(|| value.strip_prefix("rgba(")).and_then(|rest| rest.strip_suffix(')')) {
        let parts: Vec<u8> = inner
            .split([',', ' '])
            .filter(|part| !part.is_empty())
            .take(3)
            .filter_map(|part| part.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 255.0) as u8))
            .collect();
        return match parts.as_slice() {
            [r, g, b] => Some([*r, *g, *b, 255]),
            _ => None,
        };
    }
    let named: &[(&str, [u8; 3])] = &[
        ("red", [255, 0, 0]),
        ("green", [0, 128, 0]),
        ("blue", [0, 0, 255]),
        ("black", [0, 0, 0]),
        ("white", [255, 255, 255]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
        ("orange", [255, 165, 0]),
        ("yellow", [255, 255, 0]),
        ("purple", [128, 0, 128]),
        ("pink", [255, 192, 203]),
        ("brown", [165, 42, 42]),
        ("cyan", [0, 255, 255]),
        ("magenta", [255, 0, 255]),
        ("teal", [0, 128, 128]),
        ("navy", [0, 0, 128]),
        ("maroon", [128, 0, 0]),
        ("olive", [128, 128, 0]),
        ("lime", [0, 255, 0]),
        ("silver", [192, 192, 192]),
        ("gold", [255, 215, 0]),
        ("crimson", [220, 20, 60]),
        ("darkred", [139, 0, 0]),
        ("darkgreen", [0, 100, 0]),
        ("darkblue", [0, 0, 139]),
        ("darkorange", [255, 140, 0]),
        ("violet", [238, 130, 238]),
        ("indigo", [75, 0, 130]),
        ("tomato", [255, 99, 71]),
        ("coral", [255, 127, 80]),
        ("salmon", [250, 128, 114]),
        ("royalblue", [65, 105, 225]),
        ("steelblue", [70, 130, 180]),
        ("seagreen", [46, 139, 87]),
        ("forestgreen", [34, 139, 34]),
        ("dodgerblue", [30, 144, 255]),
        ("deepskyblue", [0, 191, 255]),
        ("hotpink", [255, 105, 180]),
        ("deeppink", [255, 20, 147]),
        ("chocolate", [210, 105, 30]),
        ("darkgray", [169, 169, 169]),
        ("darkgrey", [169, 169, 169]),
        ("lightgray", [211, 211, 211]),
        ("lightgrey", [211, 211, 211]),
    ];
    named.iter().find(|(name, _)| *name == value).map(|(_, [r, g, b])| [*r, *g, *b, 255])
}

/// A link reference definition's label as CommonMark matches it: case-folded, its runs of
/// white space one space.
pub fn reference_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// The link reference definitions in `text` (`[label]: destination`, a line of its own, not a
/// footnote's `[^label]:`): each label, as [`reference_label`] gives it, and its destination.
/// The first definition of a label counts, as in CommonMark.
pub fn reference_labels(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut found = std::collections::BTreeMap::new();
    for line in text.lines() {
        if let Some((label, _, destination, _)) = definition(line) {
            found
                .entry(reference_label(label))
                .or_insert_with(|| line.get(destination).unwrap_or("").trim_start_matches('<').trim_end_matches('>').to_owned());
        }
    }
    found
}

/// A link reference definition line's label, and the byte ranges of its label (inside the
/// brackets), its destination, and its title (with its quotes) when it has one.
type Definition<'a> = (&'a str, Range<usize>, Range<usize>, Option<Range<usize>>);

/// The link reference definition `line` is, if it is one.
fn definition(line: &str) -> Option<Definition<'_>> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 || !line[indent..].starts_with('[') || line[indent..].starts_with("[^") {
        return None;
    }
    let close = line[indent + 1..].find("]:")? + indent + 1;
    let label = &line[indent + 1..close];
    if label.trim().is_empty() || label.contains(['[', ']']) {
        return None;
    }
    let after = close + 2;
    let rest = &line[after..];
    let destination_start = after + (rest.len() - rest.trim_start().len());
    let destination_end = line[destination_start..].find(char::is_whitespace).map_or(line.len(), |end| destination_start + end);
    if destination_end == destination_start {
        return None;
    }
    let tail = &line[destination_end..];
    let title_start = destination_end + (tail.len() - tail.trim_start().len());
    let title = line[title_start..].trim_end();
    let quoted = title.len() >= 2
        && matches!((title.chars().next(), title.chars().last()), (Some('"'), Some('"')) | (Some('\''), Some('\'')) | (Some('('), Some(')')));
    Some((label, indent + 1..close, destination_start..destination_end, quoted.then(|| title_start..title_start + title.len())))
}

/// The block a source line belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Block {
    #[default]
    Paragraph,
    /// An ATX or setext heading, 1 to 6.
    Heading(u8),
    /// A fenced or indented code block's line (`fence`: the ``` lines themselves).
    Code { fence: bool },
    /// A `---` front matter block's line.
    Frontmatter,
    /// A front matter line drawn as a property (the caret is outside the front matter): its key
    /// in a column, then its value.
    Property,
    /// A thematic break.
    Rule,
    /// A table's row (`header`, `delimiter`: the `|---|` line).
    Table { header: bool, delimiter: bool },
    /// A `$$` math block's line.
    Math,
    /// An indented code block's line: in Obsidian's live preview a text line, its first indent
    /// (a tab or four spaces) an indent and the rest inline code.
    Indented,
    /// An HTML block's line.
    Html,
    /// A Mermaid code block's line drawn as its diagram (the caret is outside the block): the
    /// editor draws the diagram over the room its first line keeps; the lines draw nothing.
    Diagram,
}

/// The look of every source line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineStyle {
    pub block: Block,
    /// Block quote nesting.
    pub quote: u8,
    /// List nesting.
    pub list: u8,
    /// A callout's line: 0 none, else its type (`theme::CALLOUT_TYPES`, from 1).
    pub callout: u8,
    /// A callout's title line.
    pub title: bool,
    /// The callout around a nested callout's line (its type, as `callout`), 0 for none.
    pub outer: u8,
    /// Front matter that is not YAML: the line its error is on (its first character marked).
    pub error: bool,
}

/// Fenced code blocks: their byte ranges and languages.
pub(crate) type CodeBlocks = Vec<(Range<usize>, Arc<str>)>;

/// A code block's highlighted tokens: byte ranges and their color bits.
pub(crate) type Tokens = Vec<(Range<usize>, flag::Flags)>;

/// The note's styles.
#[derive(Clone, Debug, Default)]
pub struct Styles {
    /// Inline flags per byte.
    pub bytes: Vec<flag::Flags>,
    /// One per source line (split on `\n`).
    pub lines: Vec<LineStyle>,
    /// Byte offset of each source line's start.
    pub starts: Vec<usize>,
}

fn paint(bytes: &mut [flag::Flags], range: &Range<usize>, value: flag::Flags) {
    if let Some(slice) = bytes.get_mut(range.clone()) {
        for byte in slice {
            *byte |= value;
        }
    }
}

fn level(heading: HeadingLevel) -> u8 {
    match heading {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

impl Styles {
    /// Callouts (`> [!tip] Title`): their lines are tinted by kind; the title is strong, and away
    /// from the caret the `[!kind]` marker is hidden.
    pub fn callouts(&mut self, text: &str, callouts: &[kition_note_markdown::Callout], active: &Range<usize>) {
        // The callouts being written: a callout nested in one is written with it.
        let mut written: Vec<Range<usize>> = Vec::new();
        for callout in callouts {
            let first = self.line_of(callout.range.start);
            let start = self.starts.get(first).copied().unwrap_or(0);
            let end = self.starts.get(first + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let line = text.get(start..end).unwrap_or("");
            let Some(open) = line.find("[!") else { continue };
            let Some(close) = line.get(open..).and_then(|rest| rest.find(']')).map(|at| open + at + 1) else { continue };
            // With the caret in it, a callout is the quote it is written as, its `[!kind]` faint
            // (as Obsidian shows the callout being written).
            let last_line = self.line_of(callout.range.end.saturating_sub(1).max(callout.range.start));
            let last_end = self.starts.get(last_line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            // A callout in a plain quote (`> > [!tip]` under a quote that is not one) is the
            // quote's text, as Obsidian shows it: its `[!kind]` faint.
            let quoted = self.lines.get(first).is_some_and(|style| style.quote > 1)
                && !callouts.iter().any(|outer| outer.range.start < callout.range.start && callout.range.end <= outer.range.end);
            let editing = quoted
                || (start <= active.end && active.start <= last_end)
                || written.iter().any(|outer| outer.start <= callout.range.start && callout.range.end <= outer.end);
            if editing {
                written.push(callout.range.clone());
                for value in self.bytes.get_mut(start + open..start + close).into_iter().flatten() {
                    *value = (*value & !(flag::MARK | flag::LINK | flag::SYNTAX)) | flag::TEXT | flag::COMMENT;
                }
                continue;
            }
            let color = crate::theme::callout_type(&callout.kind);
            // A callout nested in another keeps the outer one's type for its box.
            self.set_lines(&callout.range, |line| {
                if line.callout > 0 {
                    line.outer = line.callout;
                }
                line.callout = color;
            });
            let mut marker_end = close;
            if matches!(line.get(close..close + 1), Some("+" | "-")) {
                marker_end += 1;
            }
            if line.get(marker_end..marker_end + 1) == Some(" ") {
                marker_end += 1;
            }
            if let Some(style) = self.lines.get_mut(first) {
                style.title = true;
            }
            paint(&mut self.bytes, &(start + open..start + marker_end), flag::HIDE);
        }
    }

    /// The live preview: on every line outside `active` (the lines the selection touches), the
    /// Markdown syntax that only shapes the text is hidden: heading marks, emphasis and code marks,
    /// link brackets and destinations, quote marks. List markers, task boxes, and block syntax
    /// (fences, rules, front matter, tables) stay; a task's marker is hidden and drawn as a box.
    /// On a paragraph or heading line `active` touches, as Obsidian shows it, the heading's and
    /// quote's marks show, a list item's marker only when `selection` touches it, and an inline
    /// element's syntax (emphasis, code, a link, a highlight, math, an HTML tag pair) only when
    /// `selection` touches that element.
    pub fn conceal(&mut self, text: &str, active: &Range<usize>, selection: &Range<usize>) {
        self.properties(text);
        for index in 0..self.starts.len() {
            self.conceal_line(text, index, active, selection);
        }
    }

    /// The live preview's hiding on line `index` (see `conceal`). The front matter's properties
    /// are the whole block's: see `conceal`.
    pub fn conceal_line(&mut self, text: &str, index: usize, active: &Range<usize>, selection: &Range<usize>) {
        {
            let Some(start) = self.starts.get(index).copied() else { return };
            let end = self.starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
            // A list in a quote that is not a callout is the quote's text, as Obsidian lays it out
            // (with the caret on it or not): no list rows or indent.
            if let Some(line) = self.lines.get_mut(index)
                && line.quote > 0
                && line.callout == 0
                && line.list > 0
                && text.get(start..end).is_some_and(|content| {
                    // A list in the quote (its marker after the quote marks), not a quote in a list.
                    let rest = content.trim_start_matches([' ', '\t', '>']);
                    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                    content.trim_start().starts_with('>')
                        && content.len() - content.trim_start().len() < 2
                        && (["- ", "* ", "+ "].iter().any(|marker| rest.starts_with(marker))
                            || (digits > 0 && rest.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") "))))
                })
            {
                line.list = 0;
                // Its marker is text too, not dimmed syntax.
                let content = text.get(start..end).unwrap_or("");
                let rest = content.trim_start_matches([' ', '\t', '>']);
                let marker_at = start + content.len() - rest.len();
                let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                let length = if ["- ", "* ", "+ "].iter().any(|marker| rest.starts_with(marker)) {
                    1
                } else if digits > 0 && rest.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")) {
                    digits + 1
                } else {
                    0
                };
                for value in self.bytes.get_mut(marker_at..marker_at + length).into_iter().flatten() {
                    *value = (*value & !flag::MARK) | flag::TEXT;
                }
            }
            let line = self.lines.get(index).copied().unwrap_or_default();
            let caret_line = start <= active.end && end >= active.start;
            // A line the caret is on shows its Markdown, but a paragraph's or heading's only where
            // the selection touches it (Obsidian's live preview), not in a callout or a definition.
            // (A footnote definition too: its label shows only with the caret in it, its text's
            // marks by element, as Obsidian shows them.)
            let by_element = matches!(line.block, Block::Paragraph | Block::Heading(_) | Block::Table { delimiter: false, .. }) && line.callout == 0;
            if caret_line && !by_element {
                return;
            }
            let elements = if caret_line { inline_elements(text, self, index) } else { Vec::new() };
            let shows = |at: usize| caret_line && touched(&elements, at, selection);
            // An inline HTML element the caret is in shows as its source: its text without the
            // element's styling (Obsidian's HTML widget gives way to the tags and the text).
            if caret_line {
                for pair in html_pairs(text, &elements) {
                    let touched = pair.start <= selection.end && selection.start <= pair.end;
                    let inner = text.get(pair.clone()).and_then(|source| Some(pair.start + source.find('>')? + 1..pair.start + source.rfind('<')?));
                    if let Some(inner) = inner.filter(|_| touched) {
                        for value in self.bytes.get_mut(inner).into_iter().flatten() {
                            *value &= !(flag::CODE | flag::UNDERLINE | flag::SUB | flag::SUP | flag::MARKED | flag::COLOR);
                        }
                    }
                }
            }
            // A code block's fences draw as its box's top and bottom padding, but show while the
            // caret is anywhere in the block, as Obsidian shows them.
            if line.block == (Block::Code { fence: true }) {
                let code = |at: usize| self.lines.get(at).is_some_and(|other| matches!(other.block, Block::Code { .. }));
                let mut first = index;
                while first > 0 && code(first - 1) {
                    first -= 1;
                }
                let mut last = index;
                while code(last + 1) {
                    last += 1;
                }
                let from = self.starts.get(first).copied().unwrap_or(start);
                let to = self.starts.get(last + 1).map_or(text.len(), |next| next.saturating_sub(1));
                if !(from <= active.end && active.start <= to) {
                    paint(&mut self.bytes, &(start..end), flag::HIDE);
                }
                return;
            }
            if line.block == Block::Rule {
                paint(&mut self.bytes, &(start..end), flag::HIDE);
                return;
            }
            // A code block in a list item: the item's indent (its fence's) is not the code's, as
            // Obsidian shows it.
            if line.block == (Block::Code { fence: false }) && line.list > 0 {
                let fence = (0..index).rev().find(|at| self.lines.get(*at).is_some_and(|other| other.block == (Block::Code { fence: true })));
                let indent = fence.and_then(|at| {
                    let from = *self.starts.get(at)?;
                    let to = self.starts.get(at + 1).map_or(text.len(), |next| next.saturating_sub(1));
                    let fence_line = text.get(from..to)?;
                    Some(fence_line.len() - fence_line.trim_start_matches([' ', '\t']).len())
                });
                let content = text.get(start..end).unwrap_or("");
                let leading = content.len() - content.trim_start_matches([' ', '\t']).len();
                let hide = indent.unwrap_or(0).min(leading);
                if hide > 0 {
                    paint(&mut self.bytes, &(start..start + hide), flag::HIDE);
                }
            }
            // A code block's line in a callout: its quote marks hide (Obsidian renders the
            // callout's code block inside its box).
            if line.block == (Block::Code { fence: false }) && line.callout > 0 {
                let content = text.get(start..end).unwrap_or("");
                let mut at = 0;
                while let Some(rest) = content.get(at..) {
                    let spaces = rest.len() - rest.trim_start_matches(' ').len();
                    if spaces > 3 || rest.get(spaces..spaces + 1) != Some(">") {
                        break;
                    }
                    at += spaces + 1 + usize::from(rest.get(spaces + 1..spaces + 2) == Some(" "));
                }
                if at > 0 {
                    paint(&mut self.bytes, &(start..start + at), flag::HIDE);
                }
            }
            // A quote in a list item: the item's indent before its `>` hides (the text is set in
            // by the item's indent, see `Theme::indent_of`).
            if line.quote > 0 && line.list > 0 && line.callout == 0 {
                let content = text.get(start..end).unwrap_or("");
                let leading = content.len() - content.trim_start_matches([' ', '\t']).len();
                if leading >= 2 && content.get(leading..leading + 1) == Some(">") {
                    paint(&mut self.bytes, &(start..start + leading), flag::HIDE);
                }
            }
            if matches!(
                line.block,
                Block::Code { .. }
                    | Block::Indented
                    | Block::Frontmatter
                    | Block::Property
                    | Block::Table { delimiter: true, .. }
                    | Block::Math
                    | Block::Html
            ) {
                return;
            }
            // Inline math's dollars hide, and the view draws the formula (as Obsidian does).
            let mut at = start;
            while at < end {
                if self.bytes.get(at).is_some_and(|value| value & flag::MATH != 0) {
                    let run_end = (at..end).find(|byte| self.bytes.get(*byte).is_none_or(|value| value & flag::MATH == 0)).unwrap_or(end);
                    let run = text.get(at..run_end).unwrap_or("");
                    let open = run.len() - run.trim_start_matches('$').len();
                    let close = run.len() - run.trim_end_matches('$').len();
                    let shown = caret_line && at <= selection.end && selection.start <= run_end;
                    if open > 0 && close > 0 && open + close < run.len() && !shown {
                        paint(&mut self.bytes, &(at..at + open), flag::HIDE);
                        paint(&mut self.bytes, &(run_end - close..run_end), flag::HIDE);
                    }
                    at = run_end;
                } else {
                    at += 1;
                }
            }
            let content = text.get(start..end).unwrap_or("");
            let plain_quote = line.quote > 0 && line.callout == 0;
            // The quote marks and the list marker at the line's start.
            let mut keep_until = start;
            let mut rest = content;
            let mut at = start;
            loop {
                let trimmed = rest.trim_start_matches([' ', '\t']);
                let skipped = rest.len() - trimmed.len();
                if let Some(after) = trimmed.strip_prefix('>') {
                    let mark = at + skipped;
                    let space = usize::from(after.starts_with(' '));
                    // The caret's line shows its quote marks (in the quote's padding, see layout).
                    if !caret_line {
                        paint(&mut self.bytes, &(mark..mark + 1 + space), flag::HIDE);
                    }
                    at = mark + 1 + space;
                    rest = after.get(space..).unwrap_or("");
                    continue;
                }
                let marker = ["- ", "* ", "+ "].iter().find(|marker| trimmed.starts_with(**marker)).map(|marker| marker.len()).or_else(|| {
                    let digits = trimmed.len() - trimmed.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                    let after = trimmed.get(digits..).unwrap_or("");
                    (digits > 0 && (after.starts_with(". ") || after.starts_with(") "))).then_some(digits + 2)
                });
                if let Some(length) = marker {
                    keep_until = at + skipped + length;
                    let bullet = ["- ", "* ", "+ "].iter().any(|marker| trimmed.starts_with(*marker));
                    // A task: `[` and `]` around one mark (a space, `x`, or any other, as
                    // Obsidian reads `[-]` or `[/]`), then a space or the line's end.
                    let task = trimmed.get(length..).is_some_and(|after| {
                        let mut chars = after.chars();
                        chars.next() == Some('[')
                            && chars.next().is_some_and(|mark| mark != ']' && mark.is_ascii())
                            && chars.next() == Some(']')
                            && chars.next().is_none_or(|next| next == ' ')
                    });
                    let mark = at + skipped;
                    // A list in a quote that is not a callout keeps its markers, as Obsidian shows it.
                    if plain_quote {
                        break;
                    }
                    // The caret's line shows its marker (and a task's box) only when the selection
                    // touches them: from the line's start to the item's text.
                    let marker_end = keep_until + if task { 3 + usize::from(text.get(mark + length + 3..mark + length + 4) == Some(" ")) } else { 0 };
                    if caret_line && selection.start < marker_end {
                        break;
                    }
                    if task {
                        // The box's `[` draws it (hidden, as a mark the parser did not read is text).
                        paint(&mut self.bytes, &(mark + length..mark + length + 3), flag::TASK);
                        paint(&mut self.bytes, &(mark + length..mark + length + 1), flag::HIDE);
                        // A task's box stands for its marker, and the space after its `]` goes too
                        // (the box's area ends where Obsidian's text starts).
                        paint(&mut self.bytes, &(mark..mark + length), flag::HIDE);
                        // The box's inside (` ` or `x`) and its `]`; its `[` draws the box.
                        paint(&mut self.bytes, &(mark + length + 1..mark + length + 3), flag::HIDE);
                        let after_box = mark + length + 3;
                        if text.get(after_box..after_box + 1) == Some(" ") {
                            paint(&mut self.bytes, &(after_box..after_box + 1), flag::HIDE);
                        }
                    } else if bullet {
                        // The dot stands for the marker and its space.
                        paint(&mut self.bytes, &(mark..mark + 1), flag::HIDE | flag::BULLET);
                        paint(&mut self.bytes, &(mark + 1..mark + length), flag::HIDE);
                    }
                }
                break;
            }
            for byte in keep_until.max(start)..end {
                let shown = shows(byte);
                if let Some(value) = self.bytes.get_mut(byte)
                    && *value & flag::MARK != 0
                    && !shown
                {
                    *value |= flag::HIDE;
                }
            }
            // A heading's marks and the space after them (the caret's heading shows them).
            if let Block::Heading(_) = line.block
                && !caret_line
            {
                let hashes = content.len() - content.trim_start_matches('#').len();
                if hashes > 0 && content.get(hashes..).is_some_and(|after| after.starts_with(' ')) {
                    paint(&mut self.bytes, &(start..start + hashes + 1), flag::HIDE);
                }
            }
        }
    }

    /// The source line holding byte `at`.
    pub fn line_of(&self, at: usize) -> usize {
        self.starts.partition_point(|start| *start <= at).saturating_sub(1)
    }

    fn lines_of(&self, range: &Range<usize>) -> Range<usize> {
        let first = self.line_of(range.start);
        let last = self.line_of(range.end.saturating_sub(1).max(range.start));
        first..last + 1
    }

    pub(crate) fn set_lines(&mut self, range: &Range<usize>, mut change: impl FnMut(&mut LineStyle)) {
        let lines = self.lines_of(range);
        if let Some(slice) = self.lines.get_mut(lines) {
            for line in slice {
                change(line);
            }
        }
    }

    /// Reads `text`.
    pub fn of(text: &str) -> Self {
        Self::of_with(text, &std::collections::BTreeMap::new())
    }

    /// Reads `text`, whose `[text][label]` and `[label]` links name the definitions `labels` of
    /// the whole note (a chunk is parsed alone, its definitions can be in another).
    pub fn of_with(text: &str, labels: &std::collections::BTreeMap<String, String>) -> Self {
        let (mut styles, fences) = Self::unlit(text, labels);
        for (range, language) in fences {
            styles.highlight(text, &range, &language);
        }
        styles
    }

    /// As [`Styles::of_with`], its fenced code blocks not highlighted: the blocks (their ranges
    /// and languages), for [`Styles::light`] when they are shown (a large note's code took
    /// most of its parse).
    pub(crate) fn unlit(text: &str, labels: &std::collections::BTreeMap<String, String>) -> (Self, CodeBlocks) {
        let (styles, fences, _) = Self::read(text, labels, None);
        (styles, fences)
    }

    /// As [`Styles::unlit`], and the text's index (`kition_note_markdown::index`), from the same
    /// parse when the labels cannot change it (none, or no `]` for a reference to name one).
    /// `first`: the text starts the note (its front matter is the note's).
    pub(crate) fn unlit_indexed(
        text: &str,
        labels: &std::collections::BTreeMap<String, String>,
        first: bool,
    ) -> (Self, CodeBlocks, kition_note_markdown::Index) {
        let (styles, fences, index) = Self::read(text, labels, Some(first));
        (styles, fences, index.unwrap_or_else(|| kition_note_markdown::index_part(text, first)))
    }

    fn read(
        text: &str,
        labels: &std::collections::BTreeMap<String, String>,
        // The text's index too, and whether the text starts the note.
        indexed: Option<bool>,
    ) -> (Self, CodeBlocks, Option<kition_note_markdown::Index>) {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        let mut styles = Self { bytes: vec![0; text.len()], lines: vec![LineStyle::default(); starts.len()], starts };
        let mut table_rows = 0usize;
        let mut fences: Vec<(Range<usize>, String)> = Vec::new();
        let mut maths: Vec<Range<usize>> = Vec::new();
        // The quotes open around an event: whether each is a callout (`> [!kind]`). In a plain
        // quote Obsidian's live preview shows block syntax as text (a heading's marks, a code
        // block's fences as inline code); a callout renders its content.
        let mut quotes: Vec<bool> = Vec::new();
        let known = |link: pulldown_cmark::BrokenLink<'_>| {
            labels
                .get(&reference_label(&link.reference))
                .map(|destination| (pulldown_cmark::CowStr::from(destination.clone()), pulldown_cmark::CowStr::from("")))
        };
        // Metadata blocks later in the note (pulldown-cmark reads a `---` block anywhere): Obsidian
        // reads only the note's first as front matter; these are styled as what they are below.
        let mut later: Vec<Range<usize>> = Vec::new();
        let mut on_event = |event: Event<'_>, range: Range<usize>| {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Heading { .. } if !quotes.is_empty() && !quotes.contains(&true) => {
                        paint(&mut styles.bytes, &range, flag::TEXT);
                    }
                    Tag::CodeBlock(_) if !quotes.is_empty() && !quotes.contains(&true) => {
                        // Each line past its quote marks (`>` and the space after it) is a line of
                        // inline code, its fences too.
                        for line in styles.lines_of(&range) {
                            let from = styles.starts.get(line).copied().unwrap_or(0).max(range.start);
                            let to = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1)).min(range.end);
                            let content = text.get(from..to).unwrap_or("").trim_end_matches(['\n', '\r']);
                            let mut at = 0;
                            while let Some(rest) = content.get(at..) {
                                let spaces = rest.len() - rest.trim_start_matches(' ').len();
                                if spaces > 3 || rest.get(spaces..spaces + 1) != Some(">") {
                                    break;
                                }
                                at += spaces + 1 + usize::from(rest.get(spaces + 1..spaces + 2) == Some(" "));
                            }
                            if content.len() > at {
                                paint(&mut styles.bytes, &(from + at..from + content.len()), flag::CODE | flag::TEXT);
                            }
                        }
                    }
                    // An ATX heading indented by spaces is a text line in Obsidian's live preview
                    // (its quirk): its `#` marks and the space after them hide away from the
                    // caret, its text is the text's.
                    Tag::Heading { .. }
                        if text.get(..range.start).is_some_and(|before| {
                            let indent = &before[before.rfind('\n').map_or(0, |at| at + 1)..];
                            !indent.is_empty() && indent.chars().all(|c| c == ' ' || c == '\t')
                        }) && text.get(range.clone()).is_some_and(|source| source.starts_with('#')) =>
                    {
                        let source = text.get(range.clone()).unwrap_or("").trim_end_matches(['\n', '\r']);
                        let hashes = source.len() - source.trim_start_matches('#').len();
                        let space = usize::from(source.get(hashes..hashes + 1) == Some(" "));
                        paint(&mut styles.bytes, &range, flag::TEXT);
                        paint(&mut styles.bytes, &(range.start..range.start + hashes + space), flag::MARK);
                        // Closing hashes (` ##` at its end) too.
                        let trimmed = source.trim_end();
                        let closing = trimmed.len() - trimmed.trim_end_matches('#').len();
                        if closing > 0 && trimmed.len() - closing > hashes && trimmed[..trimmed.len() - closing].ends_with(' ') {
                            paint(&mut styles.bytes, &(range.start + trimmed.len() - closing - 1..range.start + trimmed.len()), flag::MARK);
                        }
                    }
                    Tag::Heading { level: heading, .. } => {
                        let heading = level(heading);
                        styles.set_lines(&range, |line| line.block = Block::Heading(heading));
                        // Its weight is the heading's; `STRONG` is for its bold text.
                        paint(&mut styles.bytes, &range, flag::MARKUP);
                        // A setext heading's underline (`===` or `---`) is a line of its own,
                        // shown faint at the text's size, as Obsidian shows it.
                        let source = text.get(range.clone()).unwrap_or("").trim_end_matches(['\n', '\r']);
                        if let Some(at) = source.rfind('\n') {
                            let underline = source.get(at + 1..).unwrap_or("").trim();
                            if !underline.is_empty() && (underline.chars().all(|c| c == '=') || underline.chars().all(|c| c == '-')) {
                                let from = range.start + at + 1;
                                let to = range.start + source.len();
                                let underline_line = styles.line_of(from);
                                if let Some(line) = styles.lines.get_mut(underline_line) {
                                    line.block = Block::Paragraph;
                                }
                                for value in styles.bytes.get_mut(from..to).into_iter().flatten() {
                                    *value = flag::TEXT | flag::COMMENT;
                                }
                            }
                        }
                    }
                    Tag::CodeBlock(kind) => {
                        if let CodeBlockKind::Fenced(language) = &kind {
                            fences.push((range.clone(), language.to_string()));
                        }
                        let fenced = matches!(kind, CodeBlockKind::Fenced(_));
                        styles.set_lines(&range, |line| line.block = if fenced { Block::Code { fence: false } } else { Block::Indented });
                        if !fenced {
                            // Past each line's first indent, its code is inline code.
                            for line in styles.lines_of(&range) {
                                let from = styles.starts.get(line).copied().unwrap_or(0);
                                let to = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
                                let content = text.get(from..to).unwrap_or("").trim_end_matches('\r');
                                let indent = if content.starts_with('\t') { 1 } else { content.len() - content.trim_start_matches(' ').len() }.min(4);
                                if content.len() > indent {
                                    paint(&mut styles.bytes, &(from + indent..from + content.len()), flag::CODE);
                                }
                            }
                        }
                        if fenced {
                            let lines = styles.lines_of(&range);
                            let (first, last) = (lines.start, lines.end.saturating_sub(1));
                            if let Some(line) = styles.lines.get_mut(first) {
                                line.block = Block::Code { fence: true };
                            }
                            // A closed fence's last line is its closing ```.
                            let closing = text.get(range.clone()).is_some_and(|block| {
                                let trimmed = block.trim_end_matches(['\n', '\r']);
                                // Past a quote's marks (a callout's block).
                                let last_line = trimmed.rsplit('\n').next().unwrap_or("").trim_start_matches(['>', ' ', '\t']);
                                last > first && (last_line.starts_with("```") || last_line.starts_with("~~~"))
                            });
                            if closing && let Some(line) = styles.lines.get_mut(last) {
                                line.block = Block::Code { fence: true };
                            }
                        }
                        if fenced {
                            paint(&mut styles.bytes, &range, flag::CODE);
                        }
                    }
                    Tag::BlockQuote(_) => {
                        let first = text.get(range.clone()).and_then(|quote| quote.lines().next()).unwrap_or("");
                        quotes.push(first.trim_start().trim_start_matches('>').trim_start().starts_with("[!"));
                        styles.set_lines(&range, |line| line.quote = line.quote.saturating_add(1));
                        paint(&mut styles.bytes, &range, flag::MARKUP);
                    }
                    Tag::Item => {
                        // Not over blank lines it ends with: parsed at the text's end (a chunk
                        // alone), an item runs on into them, which in the whole note it does not.
                        let item = text.get(range.clone()).unwrap_or("");
                        let content = item.trim_end_matches(['\n', '\r', ' ', '\t']).len();
                        let end = item.get(content..).and_then(|rest| rest.find('\n')).map_or(range.end, |found| range.start + content + found + 1);
                        paint(&mut styles.bytes, &(range.start..end.min(range.end)), flag::MARKUP);
                    }
                    Tag::List(_) => styles.set_lines(&list_range(text, &range), |line| line.list = line.list.saturating_add(1)),
                    Tag::Table(_) => table_rows = 0,
                    Tag::TableHead => {
                        styles.set_lines(&range, |line| line.block = Block::Table { header: true, delimiter: false });
                        paint(&mut styles.bytes, &range, flag::MARKUP | flag::STRONG);
                        // The delimiter row is the line after the header.
                        let next = styles.line_of(range.start) + 1;
                        if let Some(line) = styles.lines.get_mut(next) {
                            line.block = Block::Table { header: false, delimiter: true };
                        }
                        if let Some(span) = styles.starts.get(next).copied().zip(styles.starts.get(next + 1).copied().or(Some(text.len()))) {
                            paint(&mut styles.bytes, &(span.0..span.1), flag::MARKUP);
                        }
                    }
                    Tag::TableRow => {
                        table_rows += 1;
                        styles.set_lines(&range, |line| line.block = Block::Table { header: false, delimiter: false });
                        paint(&mut styles.bytes, &range, flag::MARKUP);
                    }
                    Tag::Emphasis => paint(&mut styles.bytes, &range, flag::MARKUP | flag::EMPHASIS),
                    Tag::Strong => paint(&mut styles.bytes, &range, flag::MARKUP | flag::STRONG),
                    Tag::Strikethrough => paint(&mut styles.bytes, &range, flag::MARKUP | flag::STRIKE),
                    Tag::Link { link_type, dest_url, .. } => {
                        let external = !matches!(link_type, pulldown_cmark::LinkType::WikiLink { .. })
                            && ["https://", "http://", "mailto:"].iter().any(|scheme| dest_url.starts_with(scheme));
                        paint(&mut styles.bytes, &range, flag::MARKUP | flag::LINK | if external { flag::EXTERNAL } else { 0 });
                    }
                    Tag::Image { .. } => paint(&mut styles.bytes, &range, flag::MARKUP | flag::LINK),
                    Tag::FootnoteDefinition(_) => {
                        // `[^label]: text`: the label's marks hide away from the caret (the space
                        // after them stays, as in Obsidian), its name is a small label.
                        paint(&mut styles.bytes, &range, flag::FOOTDEF);
                        let source = text.get(range.clone()).unwrap_or("");
                        if let Some(close) = source.find("]:").filter(|_| source.starts_with("[^")) {
                            paint(&mut styles.bytes, &(range.start..range.start + 2), flag::MARK);
                            paint(&mut styles.bytes, &(range.start + 2..range.start + close), flag::FOOTNOTE);
                            paint(&mut styles.bytes, &(range.start + close..range.start + close + 2), flag::MARK);
                        }
                    }
                    Tag::MetadataBlock(_) if range.start > 0 || indexed == Some(false) => later.push(range.clone()),
                    Tag::MetadataBlock(_) => {
                        styles.set_lines(&range, |line| line.block = Block::Frontmatter);
                        paint(&mut styles.bytes, &range, flag::CODE);
                    }
                    Tag::HtmlBlock => styles.set_lines(&range, |line| line.block = Block::Html),
                    _ => {}
                },
                Event::End(TagEnd::Table) => {}
                Event::End(TagEnd::BlockQuote(_)) => {
                    quotes.pop();
                }
                Event::Text(_) => paint(&mut styles.bytes, &range, flag::TEXT),
                Event::Code(_) => {
                    paint(&mut styles.bytes, &range, flag::MARKUP | flag::CODE);
                    // The text between the backticks is content.
                    if let Some(span) = text.get(range.clone()) {
                        let ticks = span.len() - span.trim_start_matches('`').len();
                        let inner = range.start + ticks..range.end.saturating_sub(ticks).max(range.start + ticks);
                        paint(&mut styles.bytes, &inner, flag::TEXT);
                    }
                }
                Event::InlineMath(_) | Event::DisplayMath(_) => {
                    paint(&mut styles.bytes, &range, flag::MATH | flag::TEXT);
                    maths.push(range.clone());
                    if matches!(event, Event::DisplayMath(_)) {
                        styles.set_lines(&range, |line| line.block = Block::Math);
                    }
                }
                Event::Rule => {
                    styles.set_lines(&range, |line| line.block = Block::Rule);
                    paint(&mut styles.bytes, &range, flag::MARKUP);
                }
                Event::TaskListMarker(_) => paint(&mut styles.bytes, &range, flag::MARKUP | flag::TASK),
                Event::Html(_) | Event::InlineHtml(_) => paint(&mut styles.bytes, &range, flag::MARK),
                // A footnote reference stays visible: it has no text of its own.
                Event::FootnoteReference(_) => paint(&mut styles.bytes, &range, flag::FOOTNOTE),
                _ => {}
            }
        };
        // The index's parse is the same as this one's when the labels name nothing in it.
        let index = if let Some(first) = indexed.filter(|_| labels.is_empty() || !text.contains(']')) {
            Some(kition_note_markdown::index_observed(text, first, |event, range| on_event(event.clone(), range)))
        } else {
            for (event, range) in Parser::new_with_broken_link_callback(text, kition_note_markdown::options(), Some(known)).into_offset_iter() {
                on_event(event, range);
            }
            None
        };
        let _ = table_rows;
        styles.syntax(text);
        for range in maths {
            styles.highlight_math(text, &range);
        }
        // A later `---` block is a rule, its lines as their own Markdown, and its closing `---` a
        // rule (Obsidian's editor; a closing `...` is text).
        for block in later {
            let lines = styles.lines_of(&block);
            let (first, last) = (lines.start, lines.end.saturating_sub(1));
            let line_text = |line: usize| {
                let from = styles.starts.get(line).copied().unwrap_or(0);
                let to = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
                (from, to)
            };
            let (from, _) = line_text(first + 1);
            let (closing, closing_end) = line_text(last);
            let closed = last > first && text.get(closing..closing_end).is_some_and(|line| line.trim_end() == "---");
            let content_end = if closed { closing } else { closing_end.min(text.len()) };
            let (inner, _, _) = Self::read(text.get(from..content_end).unwrap_or(""), labels, None);
            for (offset, value) in inner.bytes.iter().enumerate() {
                if let Some(slot) = styles.bytes.get_mut(from + offset) {
                    *slot = *value;
                }
            }
            for (offset, style) in inner
                .lines
                .iter()
                .enumerate()
                .take(inner.starts.len().saturating_sub(usize::from(content_end > from && text.get(content_end - 1..content_end) == Some("\n"))))
            {
                if let Some(slot) = styles.lines.get_mut(first + 1 + offset) {
                    *slot = *style;
                }
            }
            for line in std::iter::once(first).chain(closed.then_some(last)) {
                let (start, end) = line_text(line);
                if let Some(slot) = styles.lines.get_mut(line) {
                    *slot = LineStyle { block: Block::Rule, ..LineStyle::default() };
                }
                for value in styles.bytes.get_mut(start..end).into_iter().flatten() {
                    *value = flag::MARKUP;
                }
            }
        }
        // (Math is never in a code block: highlighting the blocks after it is the same.)
        let fences = fences
            .into_iter()
            .filter(|(_, language)| crate::highlight::grammar(language).is_some())
            .map(|(range, language)| (range, Arc::from(language)))
            .collect();
        (styles, fences, index)
    }

    /// Highlights math source (its `$` delimiters too), line by line, as Obsidian colors it where
    /// it shows (the caret's): the token in each byte's color bits, as a code block's.
    fn highlight_math(&mut self, text: &str, range: &Range<usize>) {
        let mut start = range.start;
        while start < range.end {
            let end = text.get(start..range.end).and_then(|rest| rest.find('\n')).map_or(range.end, |found| start + found);
            let line = text.get(start..end).unwrap_or("");
            for (span, kind) in crate::highlight::tex::spans(line) {
                let value = kind.code() << flag::COLOR_SHIFT;
                if let Some(slice) = self.bytes.get_mut(start + span.start..start + span.end) {
                    for byte in slice {
                        *byte = (*byte & !flag::TOKEN) | value;
                    }
                }
            }
            start = end + 1;
        }
    }

    /// The front matter as properties, always (Obsidian's live preview keeps the caret out of it):
    /// every line a [`Block::Property`] with its text hidden; the editor draws Obsidian's properties
    /// block over them (`code` of `property.view`), one row a key, edited in it.
    fn properties(&mut self, text: &str) {
        // Front matter with nothing between its fences takes no room (Obsidian shows nothing).
        if let Some((body, close)) = kition_note_markdown::frontmatter::locate(text)
            && body.is_empty()
        {
            let lines = self.lines_of(&(0..close.end.max(1)));
            for index in lines {
                if let Some(line) = self.lines.get_mut(index) {
                    line.block = Block::Property;
                }
            }
            paint(&mut self.bytes, &(0..close.end.min(text.len())), flag::HIDE);
            return;
        }
        let count = self.lines.iter().take_while(|line| line.block == Block::Frontmatter).count();
        if count < 2 {
            return;
        }
        // Front matter that is not YAML stays its source, as Obsidian shows it (in red, see
        // `layout::run_style`).
        if text.starts_with("---")
            && let Some(error) = kition_note_markdown::frontmatter::scan(text, |at| text.get(..at).map_or(0, |before| before.matches('\n').count()))
                .and_then(|front| front.invalid)
        {
            // Its line (from 1, the opening fence's after it), as Obsidian marks it.
            if let Some(line) = self.lines.get_mut(error.line.saturating_sub(1)).filter(|line| line.block == Block::Frontmatter) {
                line.error = true;
            }
            return;
        }
        let end_of = |starts: &[usize], index: usize| starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
        for index in 0..count {
            let Some(start) = self.starts.get(index).copied() else { continue };
            let end = end_of(&self.starts, index);
            if let Some(line) = self.lines.get_mut(index) {
                line.block = Block::Property;
            }
            paint(&mut self.bytes, &(start..end), flag::HIDE);
        }
    }

    /// Highlights a fenced code block's lines between its fences, as Obsidian colors its language.
    fn highlight(&mut self, text: &str, range: &Range<usize>, language: &str) {
        let tokens = self.tokens(text, range, language);
        self.light(&tokens, 0);
    }

    /// A code block's tokens (see [`Styles::tokens`]), `by` bytes on, in its bytes: the token kind
    /// in the color bits, the code syntax bits cleared.
    pub(crate) fn light(&mut self, tokens: &[(Range<usize>, flag::Flags)], by: usize) {
        for (span, value) in tokens {
            if let Some(slice) = self.bytes.get_mut(span.start + by..span.end + by) {
                for byte in slice {
                    *byte = (*byte & !(flag::SYNTAX | flag::TOKEN)) | value;
                }
            }
        }
    }

    /// A fenced code block's highlighted tokens: byte ranges and their color bits, in its lines
    /// between its fences, as Obsidian colors its language.
    pub(crate) fn tokens(&self, text: &str, range: &Range<usize>, language: &str) -> Tokens {
        let mut out = Vec::new();
        let Some(grammar) = crate::highlight::grammar(language) else { return out };
        let lines = self.lines_of(range);
        let mut state = crate::highlight::State::default();
        // Not the opening fence, nor the closing one (an unclosed block runs to the note's end).
        for line in lines.start + 1..lines.end {
            let Some(start) = self.starts.get(line).copied() else { continue };
            let end = self.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let content = text.get(start..end).unwrap_or("");
            let fence = content.trim_start();
            if line + 1 == lines.end && (fence.starts_with("```") || fence.starts_with("~~~")) {
                break;
            }
            for (span, kind) in crate::highlight::line(grammar, content, &mut state) {
                out.push((start + span.start..start + span.end, kind.code() << flag::COLOR_SHIFT));
            }
        }
        out
    }

    /// Bytes in markup that no text event covers are syntax; Obsidian's `==highlight==` and `#tags`
    /// (which CommonMark does not know) get their own flags.
    fn syntax(&mut self, text: &str) {
        // Each pass below runs only when its syntax's character is in the text at all (a large
        // note's many small chunks mostly have none of them; their searches were most of a parse).
        let has = |byte: u8| text.as_bytes().contains(&byte);
        let (angle, caret, percent, equals, colon, bracket) = (has(b'<'), has(b'^'), has(b'%'), has(b'='), has(b':'), has(b'['));
        for (byte, value) in self.bytes.iter_mut().enumerate() {
            if *value & flag::MARKUP != 0 && *value & flag::TEXT == 0 && text.as_bytes().get(byte).is_some_and(|b| !b.is_ascii_whitespace()) {
                *value |= flag::MARK;
            }
        }
        // A backslash escape's backslash is syntax (hidden away from the caret).
        let raw = text.as_bytes();
        for (at, byte) in raw.iter().enumerate() {
            if *byte == b'\\'
                && raw.get(at + 1).is_some_and(u8::is_ascii_punctuation)
                && self.bytes.get(at).is_some_and(|value| value & (flag::TEXT | flag::CODE | flag::MATH) == 0)
                && self.bytes.get(at + 1).is_some_and(|value| value & flag::TEXT != 0)
            {
                paint(&mut self.bytes, &(at..at + 1), flag::MARK);
            }
        }
        // A footnote reference `[^label]` in text, whether or not its definition is in the same
        // chunk (the parser sees only a chunk): small and raised. A definition's label (a line's
        // `[^label]:`) is the definition's.
        for (at, _) in text.match_indices("[^").filter(|_| caret) {
            let plain = self.bytes.get(at).is_some_and(|value| value & (flag::CODE | flag::MATH | flag::FOOTDEF) == 0);
            let Some(close) = text.get(at + 2..).and_then(|rest| rest.find(']')).map(|end| at + 2 + end) else { continue };
            let label = text.get(at + 2..close).unwrap_or("");
            let definition = text.get(close + 1..close + 2) == Some(":") && (at == 0 || raw.get(at - 1) == Some(&b'\n'));
            if plain && !definition && !label.is_empty() && !label.contains(char::is_whitespace) {
                paint(&mut self.bytes, &(at..close + 1), flag::FOOTNOTE);
            }
        }
        // A `%%comment%%` in text is drawn faint, its marks too (as Obsidian's live preview).
        let mut from = 0;
        while let Some(open) = text.get(from..).filter(|_| percent).and_then(|rest| rest.find("%%")).map(|at| from + at) {
            let Some(close) = text.get(open + 2..).and_then(|rest| rest.find("%%")).map(|at| open + 2 + at) else { break };
            let plain = self.bytes.get(open..close + 2).is_some_and(|span| span.iter().all(|value| value & (flag::CODE | flag::MATH) == 0));
            if plain {
                paint(&mut self.bytes, &(open..close + 2), flag::COMMENT);
            }
            from = close + 2;
        }
        // A link reference definition (`[label]: destination "title"`) reads as Obsidian shows
        // it, like a footnote's: the line a size smaller, the label small, its brackets and
        // colon hidden away from the caret, the destination a link.
        let mut line_start = 0;
        for line in text.split_inclusive('\n').filter(|_| bracket && colon) {
            let content = line.trim_end_matches(['\n', '\r']);
            let plain = self.bytes.get(line_start).is_some_and(|value| value & (flag::CODE | flag::MATH | flag::FOOTDEF) == 0);
            if plain && let Some((_, label, destination, title)) = definition(content) {
                let at = |range: &Range<usize>| line_start + range.start..line_start + range.end;
                paint(&mut self.bytes, &(line_start..line_start + content.len()), flag::FOOTDEF);
                paint(&mut self.bytes, &(line_start + label.start - 1..line_start + label.start), flag::MARK);
                paint(&mut self.bytes, &at(&label), flag::FOOTNOTE | flag::TEXT);
                paint(&mut self.bytes, &(line_start + label.end..line_start + label.end + 2), flag::MARK);
                paint(&mut self.bytes, &at(&destination), flag::LINK | flag::TEXT);
                if let Some(title) = title {
                    paint(&mut self.bytes, &at(&title), flag::TEXT);
                }
            }
            line_start += line.len();
        }
        // An inline footnote (`^[text]`), as Obsidian shows it: small and raised, its marks faint.
        for (at, _) in text.match_indices("^[").filter(|_| caret) {
            let plain = self.bytes.get(at).is_some_and(|value| value & (flag::CODE | flag::MATH) == 0);
            let Some(close) = text.get(at + 2..).and_then(|rest| rest.find([']', '\n'])).map(|end| at + 2 + end) else { continue };
            if !plain || raw.get(close) != Some(&b']') || close == at + 2 {
                continue;
            }
            // Its marks are the note's; inside, its text keeps its own styling (`*italic*`), as
            // Obsidian renders it, but is no link.
            for at in [at, at + 1, close] {
                if let Some(value) = self.bytes.get_mut(at) {
                    *value &= !(flag::MARK | flag::LINK | flag::MARKUP);
                }
            }
            for value in self.bytes.get_mut(at + 2..close).into_iter().flatten() {
                *value &= !flag::LINK;
            }
            paint(&mut self.bytes, &(at..at + 2), flag::FOOTNOTE | flag::TEXT);
            paint(&mut self.bytes, &(at + 2..close), flag::INLINE_NOTE);
            paint(&mut self.bytes, &(close..close + 1), flag::FOOTNOTE | flag::TEXT);
        }
        // `<kbd>key</kbd>`: the key drawn as inline code, its tags hidden (they are HTML).
        for (at, _) in text.match_indices("<kbd>").filter(|_| angle) {
            let inner = at + "<kbd>".len();
            if let Some(close) = text.get(inner..).and_then(|rest| rest.find("</kbd>")).map(|end| inner + end)
                && !text.get(inner..close).unwrap_or("").contains('\n')
            {
                paint(&mut self.bytes, &(inner..close), flag::CODE);
            }
        }
        // Inline HTML that styles its text, as Obsidian renders it: the content between an
        // opening tag the parser read as HTML and its closing tag on the same line.
        const STYLING_TAGS: &[(&str, flag::Flags)] = &[
            ("u", flag::UNDERLINE),
            ("ins", flag::UNDERLINE),
            ("sub", flag::SUB),
            ("sup", flag::SUP),
            ("mark", flag::MARKED),
            ("b", flag::STRONG),
            ("strong", flag::STRONG),
            ("i", flag::EMPHASIS),
            ("em", flag::EMPHASIS),
            ("s", flag::STRIKE),
            ("del", flag::STRIKE),
            ("strike", flag::STRIKE),
        ];
        for (name, style) in STYLING_TAGS.iter().filter(|_| angle) {
            let (open, close) = (format!("<{name}"), format!("</{name}>"));
            for (at, _) in text.match_indices(open.as_str()) {
                let html = self.bytes.get(at).is_some_and(|value| value & flag::MARK != 0 && value & flag::CODE == 0);
                let Some(tag_end) = text.get(at..).and_then(|rest| rest.find('>')).map(|end| at + end + 1) else { continue };
                let attributes = text.get(at + open.len()..tag_end - 1).unwrap_or("");
                if !html || !(attributes.is_empty() || attributes.starts_with(' ')) || attributes.contains(['\n', '<']) {
                    continue;
                }
                let Some(end) = text.get(tag_end..).and_then(|rest| rest.find(close.as_str())).map(|end| tag_end + end) else { continue };
                if !text.get(tag_end..end).unwrap_or("").contains('\n') {
                    paint(&mut self.bytes, &(tag_end..end), *style);
                }
            }
        }
        // Inline HTML's text color: `<span style="color: …">` and `<font color="…">`.
        for (name, attribute) in [("span", "style"), ("font", "color")].into_iter().filter(|_| angle) {
            let (open, close) = (format!("<{name} "), format!("</{name}>"));
            for (at, _) in text.match_indices(open.as_str()) {
                let html = self.bytes.get(at).is_some_and(|value| value & flag::MARK != 0 && value & flag::CODE == 0);
                let Some(tag_end) = text.get(at..).and_then(|rest| rest.find('>')).map(|end| at + end + 1) else { continue };
                let tag = text.get(at..tag_end).unwrap_or("");
                let Some(end) = text.get(tag_end..).and_then(|rest| rest.find(close.as_str())).map(|end| tag_end + end) else { continue };
                if !html || tag.contains('\n') || text.get(tag_end..end).unwrap_or("").contains('\n') {
                    continue;
                }
                let lower = tag.to_ascii_lowercase();
                let value = lower.find(&format!("{attribute}=")).and_then(|found| {
                    let rest = tag.get(found + attribute.len() + 1..)?;
                    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
                    let inner = rest.get(1..)?;
                    inner.get(..inner.find(quote)?)
                });
                let color = match (name, value) {
                    ("span", Some(style)) => style.split(';').find_map(|rule| {
                        let (property, value) = rule.split_once(':')?;
                        (property.trim().eq_ignore_ascii_case("color")).then(|| css_color(value)).flatten()
                    }),
                    ("font", Some(value)) => css_color(value),
                    _ => None,
                };
                let index = color.map_or(0, intern_color);
                if index > 0 {
                    for value in self.bytes.get_mut(tag_end..end).into_iter().flatten() {
                        *value = (*value & !flag::COLOR) | (index << flag::COLOR_SHIFT);
                    }
                }
            }
        }
        // Bare web addresses in text are links (and `www.` ones, as Obsidian's GFM autolinks).
        let www = text.contains("www.");
        for scheme in [("https://", colon), ("http://", colon), ("www.", www)].into_iter().filter(|(_, any)| *any).map(|(scheme, _)| scheme) {
            for (at, _) in text.match_indices(scheme) {
                let plain = |byte: usize| {
                    self.bytes.get(byte).is_some_and(|value| value & flag::TEXT != 0 && value & (flag::CODE | flag::MATH | flag::LINK) == 0)
                };
                let starts_word = at == 0 || raw.get(at - 1).is_some_and(|b| b.is_ascii_whitespace() || matches!(b, b'(' | b'<'));
                if !plain(at) || !starts_word {
                    continue;
                }
                let rest = text.get(at..).unwrap_or("");
                let mut end = at + rest.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(rest.len());
                while end > at + scheme.len()
                    && raw.get(end - 1).is_some_and(|b| matches!(b, b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b'"' | b'\''))
                {
                    end -= 1;
                }
                // (`www.` needs a dot after it: `www.example.com`.)
                let host = text.get(at + scheme.len()..end).unwrap_or("");
                if end > at + scheme.len() && (scheme != "www." || host.contains('.')) {
                    paint(&mut self.bytes, &(at..end), flag::LINK | flag::BAREURL);
                }
            }
        }
        // `==text==` inside text.
        let mut from = 0;
        while let Some(open) = text.get(from..).filter(|_| equals).and_then(|rest| rest.find("==")).map(|at| from + at) {
            let Some(close) = text.get(open + 2..).and_then(|rest| rest.find("==")).map(|at| open + 2 + at) else { break };
            let inside = text.get(open + 2..close).unwrap_or("");
            let plain = self.bytes.get(open..close + 2).is_some_and(|span| span.iter().all(|value| value & (flag::CODE | flag::MATH) == 0));
            if !inside.is_empty() && !inside.contains('\n') && plain {
                // The marks are the highlight's too: shown (the caret in it), they are on its yellow,
                // as Obsidian draws them.
                paint(&mut self.bytes, &(open..open + 2), flag::MARK | flag::HIGHLIGHT);
                paint(&mut self.bytes, &(open + 2..close), flag::HIGHLIGHT);
                paint(&mut self.bytes, &(close..close + 2), flag::MARK | flag::HIGHLIGHT);
                // Obsidian 1.14's coloured highlight: a colour's emoji after the opening marks,
                // syntax like them, and the highlight in that colour.
                if let Some((emoji, color)) = HIGHLIGHT_COLORS.iter().find(|(emoji, _)| inside.starts_with(*emoji)) {
                    paint(&mut self.bytes, &(open + 2..open + 2 + emoji.len()), flag::MARK);
                    let index = intern_color(*color);
                    for value in self.bytes.get_mut(open..close + 2).into_iter().flatten() {
                        if *value & flag::COLOR == 0 {
                            *value |= index << flag::COLOR_SHIFT;
                        }
                    }
                }
                from = close + 2;
            } else {
                from = open + 2;
            }
        }
        // `#tag` at a word start, outside code.
        let bytes = text.as_bytes();
        for (at, _) in text.match_indices('#') {
            let starts_word = at == 0 || bytes.get(at - 1).is_some_and(|b| b.is_ascii_whitespace());
            let next = text.get(at + 1..).and_then(|rest| rest.chars().next());
            let in_code = self.bytes.get(at).is_some_and(|value| value & (flag::CODE | flag::MATH) != 0);
            let heading = self.lines.get(self.line_of(at)).is_some_and(|line| matches!(line.block, Block::Heading(_)))
                && self.starts.get(self.line_of(at)) == Some(&at);
            if starts_word && !in_code && !heading && next.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '/' || c == '-') {
                let end = text.get(at + 1..).map_or(at + 1, |rest| {
                    at + 1 + rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '/' || c == '-')).unwrap_or(rest.len())
                });
                paint(&mut self.bytes, &(at..end), flag::TAG);
            }
        }
    }
}

/// A list's range without the lines it ends on that hold only quote marks or spaces: in a quote,
/// pulldown-cmark's list takes in a trailing `>` line and the next line's `> `, which are no
/// list's (Obsidian renders the paragraph after the list outside it).
fn list_range(text: &str, range: &Range<usize>) -> Range<usize> {
    let mut end = range.end;
    while end > range.start {
        let body = text.get(range.start..end).unwrap_or("");
        let last_start = body.trim_end_matches('\n').rfind('\n').map_or(range.start, |found| range.start + found + 1);
        let tail = text.get(last_start..end).unwrap_or("");
        if last_start <= range.start || !tail.trim_matches(|c: char| c == '>' || c.is_whitespace()).is_empty() {
            break;
        }
        end = last_start;
    }
    range.start..end
}

/// Whether the syntax byte `at` shows on the caret's line: its innermost inline element touches
/// `selection` (at either end), or it is in none (an escape's backslash shows, as Obsidian's).
fn touched(elements: &[Range<usize>], at: usize, selection: &Range<usize>) -> bool {
    // The outermost element holding the byte: Obsidian shows every mark inside an element the
    // caret is in (a bold's stars in a highlight, a highlight's in a bold around it).
    let outermost = elements.iter().filter(|element| element.start <= at && at < element.end).max_by_key(|element| element.end - element.start);
    outermost.is_none_or(|element| element.start <= selection.end && selection.start <= element.end)
}

/// The inline HTML elements among `elements` (an opening tag to its closing one, as
/// [`inline_elements`] pairs them).
fn html_pairs<'a>(text: &'a str, elements: &'a [Range<usize>]) -> impl Iterator<Item = Range<usize>> + 'a {
    elements
        .iter()
        .filter(move |element| {
            text.get((*element).clone())
                .is_some_and(|source| source.starts_with('<') && source.ends_with('>') && source.contains("</") && !source.starts_with("</"))
        })
        .cloned()
}

/// The inline elements of the paragraph holding line `index` (its lines up to blank ones):
/// emphasis, strikethrough, links and images, code, math, footnote references, HTML tags (a
/// styling tag pair as one element too), and highlights, by their byte ranges in `text`.
fn inline_elements(text: &str, styles: &Styles, index: usize) -> Vec<Range<usize>> {
    const MOST: usize = 40;
    let line_range = |line: usize| {
        let start = styles.starts.get(line).copied().unwrap_or(text.len());
        let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
        start..end
    };
    let blank =
        |line: usize| text.get(line_range(line)).is_none_or(|content| content.trim_matches(|c: char| c == '>' || c.is_whitespace()).is_empty());
    let mut first = index;
    while first > 0 && index - first < MOST && !blank(first - 1) {
        first -= 1;
    }
    let mut last = index;
    while last + 1 < styles.starts.len() && last - index < MOST && !blank(last + 1) {
        last += 1;
    }
    let from = line_range(first).start;
    let to = line_range(last).end;
    let Some(slice) = text.get(from..to) else { return Vec::new() };
    let mut elements = Vec::new();
    let mut tags: Vec<(String, Range<usize>)> = Vec::new();
    for (event, range) in Parser::new_ext(slice, kition_note_markdown::options()).into_offset_iter() {
        let range = range.start + from..range.end + from;
        match event {
            Event::Start(
                Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. } | Tag::Superscript | Tag::Subscript,
            )
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::FootnoteReference(_) => elements.push(range),
            Event::InlineHtml(html) => {
                let tag = html.trim_start_matches('<');
                let closing = tag.starts_with('/');
                let name: String =
                    tag.trim_start_matches('/').chars().take_while(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
                if closing {
                    if let Some(open) = tags.iter().rposition(|(open, _)| *open == name) {
                        let (_, opened) = tags.remove(open);
                        elements.push(opened.start..range.end);
                    }
                } else {
                    tags.push((name, range.clone()));
                }
                elements.push(range);
            }
            _ => {}
        }
    }
    // A footnote definition's label (`[^1]:` at a line's start) is one element.
    for line in first..=last {
        let range = line_range(line);
        let content = text.get(range.clone()).unwrap_or("");
        if content.starts_with("[^")
            && let Some(close) = content.find("]:")
        {
            elements.push(range.start..range.start + close + 2);
        }
    }
    // A highlight carries its flag, its `==` on each side too.
    let mut at = from;
    while at < to {
        if styles.bytes.get(at).is_some_and(|value| value & flag::HIGHLIGHT != 0) {
            let run_end = (at..to).find(|byte| styles.bytes.get(*byte).is_none_or(|value| value & flag::HIGHLIGHT == 0)).unwrap_or(to);
            elements.push(at..run_end);
            at = run_end;
        } else {
            at += 1;
        }
    }
    elements
}
