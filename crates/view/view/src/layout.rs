//! The note in one column: each source line shaped in its styled runs and wrapped into rows whose
//! pieces keep their source bytes, so every caret stop, hit, and selection maps to the text exactly.
//! A table is laid out as a whole, as a grid: each of its lines is one row whose cells wrap in
//! their columns, and the caret moves through the cells' text (pipes and padding have no stops).
//!
//! A source line's rows are memoised by everything that decides them (its text, its bytes' flags,
//! its block, the width), so typing lays out only the line it changes; a table is memoised whole.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use kition_note_fonts::{self as fonts, FaceId, Family};
use kition_note_markdown::{Align, Table};
use unicode_segmentation::UnicodeSegmentation;

use crate::style::{Block, LineStyle, Styles, flag};
use crate::theme::{self, Theme};

/// How a run of text is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunStyle {
    pub family: Family,
    pub weight: u16,
    pub size: f32,
    pub color: [u8; 4],
    pub italic: bool,
    pub strike: bool,
    /// A link's underline.
    pub underline: bool,
    /// Raised above the baseline by this much (a footnote's reference).
    pub rise: f32,
    /// A background behind the run (inline code, a highlight, a tag).
    pub background: Option<[u8; 4]>,
}

/// One glyph to draw, at (`x`, `y`) from the row's left and top (`y` is its baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawGlyph {
    pub face: FaceId,
    pub id: u32,
    pub x: f32,
    pub y: f32,
    /// Font size over units per em: the glyph outline's scale.
    pub scale: f32,
    pub style: u16,
}

/// A rectangle drawn behind or over a row's text, relative to the row (x from the content's left,
/// y from the row's top). `outline` draws only its edge, one pixel wide; `check` draws a check
/// mark inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub color: [u8; 4],
    pub outline: bool,
    pub check: bool,
    /// A filled circle in the rectangle.
    pub dot: bool,
    /// Rounded corners (a tag's pill, inline code, a task's box).
    pub radius: f32,
    /// A callout's icon drawn in the rectangle, by its kind (0 none).
    pub icon: u8,
}

/// A place the caret can be: a byte (from the line's start), its x (from the content's left),
/// the top of its text line within the row, and the cell it is in (0 outside tables).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub byte: usize,
    pub x: f32,
    pub dy: f32,
    pub cell: u16,
}

/// One visual row of a source line.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Bytes from the source line's start.
    pub start: usize,
    pub end: usize,
    /// Top within the line's block, and height.
    pub top: f32,
    pub height: f32,
    /// The caret's height (a table cell's text line is shorter than its row).
    pub caret_height: f32,
    /// Caret stops, ascending by byte.
    pub stops: Vec<Stop>,
    pub glyphs: Vec<DrawGlyph>,
    pub behind: Vec<RowRect>,
    pub over: Vec<RowRect>,
    /// A table row: the column edges (x from the content's left) and the row's index in its table
    /// (0 is the header).
    pub table: Option<TableRow>,
    /// A picture drawn in the row (an image-only line away from the caret): its name and its box,
    /// x from the content's left and y from the row's top.
    pub image: Option<(String, RowRect)>,
}

/// Where a table row's cells are.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    /// The table's index in the note.
    pub table: usize,
    /// 0 is the header.
    pub row: usize,
    /// Each column's left edge, then the table's right edge.
    pub columns: Vec<f32>,
}

/// A source line laid out: its rows, the styles its glyphs name, and the space above it.
#[derive(Clone, Debug, PartialEq)]
pub struct LineBox {
    pub rows: Vec<Row>,
    pub styles: Vec<RunStyle>,
    pub height: f32,
    /// Extra space above the first row (headings, a table).
    pub above: f32,
    /// A height guessed from the line's length, not laid out (a large note's line far from what
    /// is shown): `View::realize` lays it out before it is drawn.
    pub estimated: bool,
}

/// A laid-out source line in the note.
#[derive(Clone, Debug)]
pub struct Placed {
    /// Byte range of the line in the text, without its `\n`.
    pub range: Range<usize>,
    /// Top of the line's first row in document pixels.
    pub y: f32,
    pub style: LineStyle,
    pub boxed: Arc<LineBox>,
    /// A heading or a list item with children: `Some(true)` folded (its section or its children
    /// hidden), `Some(false)` open.
    pub fold: Option<bool>,
    /// Space above the line, outside any box (a heading's).
    pub space: f32,
    /// Padding above and below the line inside its block's box (a callout's, a list item's, a
    /// table's), as Obsidian lays them out.
    pub pad_top: f32,
    pub pad_bottom: f32,
}

/// The space above line `index` and its box padding (`boxed_above` is its block's own space):
/// Obsidian's live preview spacing. A heading right after a heading and a blank line has none;
/// a list item has 1.2 px above and below; a callout has 12 px inside its box above its title,
/// 16 px between its title and body, and 28 px below its body; a table has 16 px above and below.
pub fn spacing(styles: &Styles, text: &str, index: usize, boxed_above: f32, hidden: &[bool]) -> (f32, f32, f32) {
    let style = |at: usize| styles.lines.get(at).copied().unwrap_or_default();
    let blank = |at: usize| {
        let start = styles.starts.get(at).copied().unwrap_or(0);
        let end = styles.starts.get(at + 1).map_or(text.len(), |next| next.saturating_sub(1));
        text.get(start..end).is_none_or(|line| line.trim().is_empty())
    };
    let line = style(index);
    // (The first line after front matter with nothing in it is the note's first.)
    let after_empty_front = index > 0
        && style(index - 1).block == Block::Property
        && text.starts_with("---")
        && kition_note_markdown::frontmatter::locate(text)
            .is_some_and(|(body, close)| body.is_empty() && styles.starts.get(index) == Some(&close.end));
    let mut space = if index == 0 || after_empty_front { 0.0 } else { boxed_above };
    // An empty line in a callout (only its `>`) is the 16 px between its blocks, as Obsidian
    // renders a callout's content (paragraphs and lists with 16 px margins).
    if line.callout > 0 && !line.title {
        let start = styles.starts.get(index).copied().unwrap_or(0);
        let end = styles.starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
        if text.get(start..end).is_some_and(|content| content.trim_start_matches(['>', ' ', '\t', '\r']).is_empty()) {
            space -= 8.0;
        }
        // A block right after another kind (a list after a quote, a paragraph after a list),
        // with no empty line between, is 16 px from it too: the rendered blocks' margins.
        let kind = |other: LineStyle| {
            if other.list > 0 {
                1
            } else if Theme::quotes_in_callout(other) > 0 {
                2
            } else {
                0
            }
        };
        if index > 0 {
            let previous = style(index - 1);
            let previous_blank = {
                let start = styles.starts.get(index - 1).copied().unwrap_or(0);
                text.get(start..styles.starts.get(index).map_or(text.len(), |next| next.saturating_sub(1)))
                    .is_none_or(|content| content.trim_start_matches(['>', ' ', '\t', '\r']).is_empty())
            };
            let own = text.get(start..end).is_some_and(|content| !content.trim_start_matches(['>', ' ', '\t', '\r']).is_empty());
            if own
                && !previous_blank
                && !previous.title
                && previous.callout == line.callout
                && previous.outer == line.outer
                && matches!(previous.block, Block::Paragraph)
                && matches!(line.block, Block::Paragraph)
                && kind(previous) != kind(line)
            {
                space += 16.0;
            }
        }
    }
    // A blank line right after a footnote or link reference definition is the definition's
    // (a size smaller, as Obsidian's editor draws it): 3 px less at 16 px text.
    if index > 0 && blank(index) && line.block == Block::Paragraph {
        let previous = styles.starts.get(index - 1).copied().unwrap_or(0);
        if styles.bytes.get(previous).is_some_and(|value| value & flag::FOOTDEF != 0) && !blank(index - 1) {
            space -= 3.0;
        }
    }
    if matches!(line.block, Block::Heading(_)) && index >= 2 && blank(index - 1) && matches!(style(index - 2).block, Block::Heading(_)) {
        space = 0.0;
    }
    let (mut pad_top, mut pad_bottom) = (0.0, 0.0);
    // (A math block's lines after its first take no room, nor the callout's padding.)
    let math_rest = line.block == Block::Math && index > 0 && style(index - 1).block == Block::Math;
    if line.callout > 0 && !math_rest {
        // The line's own callout's lines (a nested callout's own, or an outer one's with the
        // callouts nested in it).
        let outermost = |other: LineStyle| if other.outer > 0 { other.outer } else { other.callout };
        let own = |other: LineStyle| {
            if line.outer > 0 { other.callout == line.callout && other.outer == line.outer } else { outermost(other) == line.callout }
        };
        let first = index == 0 || !own(style(index - 1));
        // The last line of the callout shown (a folded callout's body is hidden).
        // (A math block's lines after its first take no room in a callout, which renders it.)
        let drawn = |at: usize| !(style(at).block == Block::Math && at > 0 && style(at - 1).block == Block::Math);
        let next_shown = (index + 1..styles.starts.len()).find(|at| !hidden.get(*at).copied().unwrap_or(false) && drawn(*at));
        let last = next_shown.is_none_or(|at| !own(style(at)));
        if first {
            pad_top += 12.0;
        }
        // A math block in a callout has no margins (MathJax's container has its own 16 px of
        // padding), so it sits right under the title and 12 px above the box's end, as in
        // Obsidian. Its lines after the first take no room, so its last shown line ends it.
        let math_next = next_shown.is_some_and(|at| style(at).block == Block::Math);
        if line.title && !last {
            pad_bottom += if math_next { 0.0 } else { 16.0 };
        }
        if last {
            pad_bottom += if line.title || line.block == Block::Math { 12.0 } else { 28.0 };
        }
        // A nested callout's last line also ends the callout around it: the nested box's 16 px
        // margin and the outer box's 12 px padding.
        if line.outer > 0 && next_shown.is_none_or(|at| outermost(style(at)) != line.outer) {
            pad_bottom += 28.0;
        }
    }
    // (An item's lazy line, its text going on unindented, is a plain line in Obsidian's editor:
    // no padding.)
    let lazy = line.quote == 0 && continuation(text, styles, index) && {
        let start = styles.starts.get(index).copied().unwrap_or(0);
        !text.get(start..).is_some_and(|rest| rest.starts_with([' ', '\t']))
    };
    if line.list > 0 && !lazy && !matches!(line.block, Block::Code { .. } | Block::Table { .. } | Block::Math) && !blank(index) {
        // An item's continuation line (no marker) has the padding below only, as Obsidian's.
        if !continuation(text, styles, index) {
            pad_top += 1.2;
        }
        pad_bottom += 1.2;
    }
    // A code block's line in a list item keeps the item's 0.075 em below it, at code's 14 px.
    if line.list > 0 && matches!(line.block, Block::Code { .. }) {
        pad_bottom += 1.05;
    }
    // Obsidian's properties block: 8 px padding, its 27 px heading, and 8 px above the rows; rows
    // 3 px apart; then the Add property row (4 px above, 29 px), 8 px padding, and 32 px below.
    // (Front matter with no rows has no block: no room.)
    let rows = || {
        (0..styles.starts.len()).take_while(|at| style(*at).block == Block::Property).any(|at| {
            let start = styles.starts.get(at).copied().unwrap_or(0);
            let end = styles.starts.get(at + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let content = text.get(start..end).unwrap_or("").trim_end_matches('\r');
            !content.starts_with([' ', '\t']) && !content.starts_with("- ") && content.contains(':') && content != "---"
        })
    };
    if line.block == Block::Property && rows() {
        if index == 0 || style(index - 1).block != Block::Property {
            pad_top += 43.0;
        }
        let key_line = {
            let start = styles.starts.get(index).copied().unwrap_or(0);
            let end = styles.starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let content = text.get(start..end).unwrap_or("").trim_end_matches('\r');
            !content.starts_with([' ', '\t']) && !content.starts_with("- ") && content.contains(':') && content != "---"
        };
        if key_line {
            pad_bottom += 3.0;
        }
        if style(index + 1).block != Block::Property {
            pad_bottom += 70.0;
        }
    }
    // (In a callout its 16 px margins fold into the callout's own spacing, as the browser
    // collapses them in Obsidian's rendered callout.)
    if let Block::Table { .. } = line.block
        && line.callout == 0
    {
        let table = |at: usize| matches!(style(at).block, Block::Table { .. });
        if index == 0 || !table(index - 1) {
            pad_top += 16.0;
        }
        if !table(index + 1) {
            // The rows hold the table's borders, as Obsidian's do.
            pad_bottom += 16.0;
        }
    }
    (space, pad_top, pad_bottom)
}

/// A byte's style, its syntax marks (where they show, on the caret's line) as Obsidian draws
/// each: an emphasis's, code's, highlight's, or strikethrough's marks in the text's color and
/// their element's style, a link's address in the link color, math's dollars in math's; brackets,
/// heading, quote, and list marks faint.
fn mark_style(theme: &Theme, line: LineStyle, flags: flag::Flags, byte: u8) -> RunStyle {
    let style = run_style(theme, line, flags);
    if flags & flag::MARK == 0
        || flags & flag::HIDE != 0
        || !matches!(line.block, Block::Paragraph | Block::Heading(_) | Block::Table { delimiter: false, .. })
    {
        return style;
    }
    let element = |bits: flag::Flags| flags & bits != 0;
    match byte {
        b'*' | b'_' if element(flag::STRONG | flag::EMPHASIS) => run_style(theme, line, flags & !flag::MARK),
        b'`' if element(flag::CODE) => run_style(theme, line, flags & !flag::MARK),
        b'~' if element(flag::STRIKE) => run_style(theme, line, flags & !flag::MARK),
        b'$' if element(flag::MATH) => run_style(theme, line, flags & !flag::MARK),
        b'=' if !element(flag::LINK) => run_style(theme, line, flags & !flag::MARK),
        _ if element(flag::LINK) && !b"[]()<>!".contains(&byte) => RunStyle { color: theme.link, underline: true, ..style },
        _ => style,
    }
}

/// Obsidian's heading weights, from `#` to `######`.
const HEADING_WEIGHTS: [u16; 6] = [700, 680, 660, 640, 620, 600];

/// The run style of a byte in a line.
fn run_style(theme: &Theme, line: LineStyle, flags: flag::Flags) -> RunStyle {
    let base = theme.size_of(line.block);
    let code = flags & flag::CODE != 0 || matches!(line.block, Block::Code { .. } | Block::Frontmatter | Block::Math);
    let mut style = RunStyle {
        family: if code { Family::Mono } else { Family::Sans },
        weight: fonts::REGULAR,
        size: if code && !matches!(line.block, Block::Code { .. } | Block::Frontmatter) { base * theme::INLINE_CODE } else { base },
        color: theme.text,
        italic: flags & flag::EMPHASIS != 0,
        strike: flags & flag::STRIKE != 0,
        underline: false,
        rise: 0.0,
        background: None,
    };
    if let Block::Code { .. } = line.block {
        style.size = theme.code_size;
    }
    // Front matter shown as its source is front matter that is not YAML: Obsidian's red.
    if matches!(line.block, Block::Frontmatter) {
        style.size = theme.code_size;
        style.color = theme.invalid;
    }
    // Obsidian's weights: headings 700 to 600 by level, and bold 200 more than the text around it.
    if let Block::Heading(level) = line.block {
        style.weight = HEADING_WEIGHTS.get(usize::from(level.saturating_sub(1))).copied().unwrap_or(fonts::SEMIBOLD);
        style.color = theme.heading;
    }
    // A callout's title is semibold (Obsidian's 600), so its bold text is 800.
    let callout_title = line.title && line.callout > 0 && flags & flag::MARK == 0;
    if callout_title {
        style.weight = fonts::SEMIBOLD;
    }
    if flags & flag::STRONG != 0 {
        style.weight = (style.weight + 200).min(900);
    }
    if flags & flag::LINK != 0 {
        style.color = theme.link;
        style.underline = true;
    }
    if flags & flag::TAG != 0 {
        style.color = theme.link;
        style.background = Some(theme.tag_background);
        style.size = base * theme::TAG;
    }
    if flags & flag::HIGHLIGHT != 0 {
        style.background = Some(theme.highlight);
    }
    if flags & flag::CODE != 0 && !matches!(line.block, Block::Code { .. } | Block::Frontmatter | Block::Property) {
        style.background = Some(theme.code_background);
    }
    // Math source near the caret is tinted; a formula drawn in its place (its dollars hidden)
    // keeps the text color, as Obsidian's.
    if flags & flag::MATH != 0 && flags & flag::HIDE == 0 {
        style.color = theme.math;
    }
    if flags & flag::FOOTDEF != 0 && line.block == Block::Paragraph {
        style.size = base * theme::INLINE_CODE;
    }
    if flags & flag::INLINE_NOTE != 0 {
        style.size *= 0.8;
        style.rise = style.size * 0.35;
        style.underline = false;
    }
    let math_source = flags & flag::MATH != 0 && flags & flag::HIDE == 0;
    if matches!(line.block, Block::Code { fence: false }) || math_source {
        // Math source shows as Obsidian's: italic, in the text's color but for its tokens.
        if math_source {
            style.italic = true;
            style.color = theme.text;
        }
        // A code block's token (or math's), as Obsidian colors it.
        if let Some(kind) = crate::highlight::Kind::of_code((flags & flag::TOKEN) >> flag::COLOR_SHIFT) {
            style.color = match kind {
                crate::highlight::Kind::Keyword if math_source => theme.math,
                crate::highlight::Kind::Keyword => theme.code_keyword,
                crate::highlight::Kind::String => theme.code_literal,
                crate::highlight::Kind::Number => theme.code_number,
                crate::highlight::Kind::Comment => theme.code_comment,
                crate::highlight::Kind::Variable => theme.code_variable,
                crate::highlight::Kind::Operator => theme.code_operator,
                crate::highlight::Kind::Property => theme.code_property,
                crate::highlight::Kind::Punctuation => theme.code_punctuation,
                crate::highlight::Kind::Link => theme.code_link,
            };
        }
    } else if let Some(color) = crate::style::interned_color((flags & flag::COLOR) >> flag::COLOR_SHIFT) {
        // A coloured highlight's colour (alpha 254) is its background; any other, the text's.
        if flags & flag::HIGHLIGHT != 0 && color[3] == 254 {
            style.background = Some(theme.highlight_background(color));
        } else {
            style.color = color;
        }
    }
    if flags & flag::UNDERLINE != 0 {
        style.underline = true;
    }
    // `<sub>` and `<sup>` as the browser sets them: five sixths the size, lowered or raised.
    if flags & flag::SUB != 0 {
        style.size *= 5.0 / 6.0;
        style.rise = -base * 0.2;
    }
    if flags & flag::SUP != 0 {
        style.size *= 5.0 / 6.0;
        style.rise = base * 0.33;
    }
    if flags & flag::MARKED != 0 {
        style.background = Some([255, 255, 0, 255]);
        style.color = [0, 0, 0, 255];
    }
    if flags & flag::FOOTNOTE != 0 {
        style.size *= 0.8;
        style.underline = false;
        if flags & flag::FOOTDEF != 0 {
            // A definition's label: raised 0.3 em, in the secondary text color (Obsidian's
            // `top: -0.3em`).
            style.color = theme.secondary;
            style.rise = style.size * 0.3;
        } else {
            style.color = theme.muted;
            style.rise = style.size * 0.35;
        }
    }
    match flags & flag::SYNTAX {
        flag::KEYWORD => style.color = theme.code_keyword,
        flag::LITERAL => style.color = theme.code_literal,
        flag::COMMENT => style.color = theme.code_comment,
        _ => {}
    }
    // A code block's fences (shown while the caret is in it) are in the text's color, as Obsidian's.
    if (flags & flag::MARK != 0 && line.block != (Block::Code { fence: true }))
        || matches!(line.block, Block::Rule)
        || matches!(line.block, Block::Table { delimiter: true, .. })
    {
        style.color = theme.muted;
        style.italic = false;
        style.strike = false;
        style.underline = false;
        style.background = if flags & flag::CODE != 0 && !matches!(line.block, Block::Code { .. }) { style.background } else { None };
    }
    // A callout's title in its kind's color, but for its inline code (the text's color).
    if callout_title && flags & flag::CODE == 0 {
        style.color = crate::theme::callout_color(theme, line.callout);
    }
    style
}

/// What a piece draws besides its glyphs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    None,
    /// No face is registered: the piece draws as a bar at its x-height.
    Bar,
    /// A task's box (`done`: checked).
    Task {
        done: bool,
    },
    /// A bullet list's dot.
    Bullet,
    /// A callout's icon, by its type (`theme::CALLOUT_TYPES`).
    Callout {
        kind: u8,
    },
    /// The external link icon, `after` the piece's glyphs.
    External {
        after: f32,
    },
    /// A foldable callout's chevron after its title: pointing right when `folded`, down when open.
    Chevron {
        after: f32,
        folded: bool,
    },
}

/// The icon number of the external link icon (`RowRect::icon`).
pub const EXTERNAL_ICON: u8 = 100;
/// The icon numbers of a callout's fold chevrons.
pub const CHEVRON_RIGHT: u8 = 101;
pub const CHEVRON_DOWN: u8 = 102;

/// How far style `index` raises its glyphs.
fn style_rise(styles: &[RunStyle], index: u16) -> f32 {
    styles.get(usize::from(index)).map_or(0.0, |style| style.rise)
}

/// One piece of a line: a grapheme and its glyphs.
struct Piece {
    start: usize,
    end: usize,
    width: f32,
    space: bool,
    style: u16,
    glyphs: Vec<(FaceId, u32, f32, f32, f32)>,
    mark: Mark,
    /// Solid rules drawn with the glyphs (a formula's fraction bars): x0, y0, x1, y1 from the
    /// piece's start and its baseline, y down.
    rules: Vec<[f32; 4]>,
    /// Room before the piece's glyphs that is its box's (inline code's, a tag's, a number's
    /// padding): the caret at the piece is drawn after it, as Obsidian draws it.
    lead: f32,
}

/// A character's width when no face can measure it: CJK and other wide characters a full em,
/// the rest a little over half.
fn estimated_width(c: char, size: f32) -> f32 {
    let wide = matches!(u32::from(c), 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD);
    if wide { size } else { size * 0.55 }
}

fn grapheme_ends(text: &str) -> Vec<usize> {
    text.grapheme_indices(true).map(|(at, part)| at + part.len()).collect()
}

/// Shapes `range` of `text` (no `\n`) with a style per byte into pieces. `flags` is indexed like `text`.
fn pieces(theme: &Theme, line: LineStyle, text: &str, range: Range<usize>, flags: &[flag::Flags], styles: &mut Vec<RunStyle>) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut start = range.start;
    let flag_at = |at: usize| flags.get(at).copied().unwrap_or(0);
    let hidden = |at: usize| flag_at(at) & flag::HIDE != 0;
    let style_at = |at: usize| {
        let style = mark_style(theme, line, flag_at(at), text.as_bytes().get(at).copied().unwrap_or(0));
        // A link whose syntax shows (its `[` drawn) has its text without the underline, as
        // Obsidian draws it with the caret on it.
        let link_text = |value: flag::Flags| value & flag::LINK != 0 && value & flag::MARK == 0;
        if style.underline && link_text(flag_at(at)) {
            let mut before = at;
            while before > 0 && link_text(flag_at(before - 1)) {
                before -= 1;
            }
            let opener = before
                .checked_sub(1)
                .filter(|open| text.as_bytes().get(*open) == Some(&b'[') && flag_at(*open) & (flag::MARK | flag::HIDE) == flag::MARK);
            if opener.is_some() {
                return RunStyle { underline: false, ..style };
            }
        }
        style
    };
    while start < range.end {
        let style = style_at(start);
        let hide = hidden(start);
        let mut end = start;
        for (offset, c) in text.get(start..range.end).unwrap_or("").char_indices() {
            let at = start + offset;
            if style_at(at) != style || hidden(at) != hide {
                break;
            }
            end = at + c.len_utf8();
        }
        if end == start {
            end = start + text.get(start..).and_then(|rest| rest.chars().next()).map_or(1, char::len_utf8);
        }
        let index = styles.iter().position(|known| *known == style).unwrap_or_else(|| {
            styles.push(style);
            styles.len() - 1
        });
        let index = u16::try_from(index).unwrap_or(u16::MAX);
        if hide {
            out.extend(hidden_span(text, start..end, flags, index, style.size));
        } else {
            out.extend(shape_span(text, start..end, style, index, theme));
        }
        start = end;
    }
    out
}

/// Whether a property's value is a date (`2026-10-01`) or a date and time (`2026-10-01T09:30`),
/// as Obsidian types it.
fn is_date_value(value: &str) -> bool {
    let bytes = value.as_bytes();
    let digits = |range: std::ops::Range<usize>| bytes.get(range).is_some_and(|part| part.iter().all(u8::is_ascii_digit));
    let date = bytes.len() >= 10 && digits(0..4) && bytes.get(4) == Some(&b'-') && digits(5..7) && bytes.get(7) == Some(&b'-') && digits(8..10);
    date && (bytes.len() == 10
        || (bytes.get(10) == Some(&b'T') && bytes.len() >= 16 && digits(11..13) && bytes.get(13) == Some(&b':') && digits(14..16)))
}

/// The width of `text` in the text's font at `size`.
pub(crate) fn text_width(text: &str, size: f32) -> f32 {
    fonts::shape(text, Family::Sans, fonts::REGULAR)
        .iter()
        .map(|run| run.glyphs.iter().map(|glyph| glyph.advance).sum::<f32>() * size / run.upem)
        .sum()
}

/// Hidden syntax: zero-width pieces, but a task's `[` draws its box.
fn hidden_span(text: &str, range: Range<usize>, flags: &[flag::Flags], style: u16, size: f32) -> Vec<Piece> {
    let part = text.get(range.clone()).unwrap_or("");
    let mut out = Vec::new();
    let mut piece_start = 0;
    for end in grapheme_ends(part) {
        let at = range.start + piece_start;
        let is_task = flags.get(at).is_some_and(|value| value & flag::TASK != 0) && part.get(piece_start..end) == Some("[");
        let is_bullet = flags.get(at).is_some_and(|value| value & flag::BULLET != 0);
        let mark = if is_task {
            // Any mark but a space checks the box (Obsidian's `[-]`, `[/]`, …).
            let done = text.get(at + 1..at + 2).is_some_and(|c| c != " ");
            Mark::Task { done }
        } else if is_bullet {
            Mark::Bullet
        } else {
            Mark::None
        };
        // Obsidian's marker areas: a task's text 31 px in, a bullet's 23 px (at 16 px text).
        let width = if is_task {
            size * 1.9375
        } else if is_bullet {
            // A `+` marker is wider than `-` (and `*`, which Obsidian draws as `-`): its text
            // starts that much further in (25.95 px against 23.42 px at 16 px text).
            let wider = if part.get(piece_start..end) == Some("+") { (text_width("+", size) - text_width("-", size)).max(0.0) } else { 0.0 };
            size * 1.4375 + wider
        } else {
            0.0
        };
        out.push(Piece { start: at, end: range.start + end, width, space: false, style, glyphs: Vec::new(), mark, rules: Vec::new(), lead: 0.0 });
        piece_start = end;
    }
    out
}

fn shape_span(text: &str, range: Range<usize>, style: RunStyle, index: u16, theme: &Theme) -> Vec<Piece> {
    let Some(part) = text.get(range.clone()) else { return Vec::new() };
    // Tabs and carriage returns draw as space and nothing; the shaper sees spaces in their place
    // so clusters stay aligned with the bytes.
    let cleaned: String = part.chars().map(|c| if c == '\t' || c == '\r' { ' ' } else { c }).collect();
    let runs = fonts::shape(&cleaned, style.family, style.weight);
    let unshaped = runs.is_empty();
    let mut clusters: Vec<(usize, FaceId, u32, f32, f32, f32)> = Vec::new();
    for run in &runs {
        let scale = style.size / run.upem;
        for glyph in &run.glyphs {
            clusters.push((glyph.cluster, run.face, glyph.id, glyph.advance * scale, glyph.x_offset * scale, glyph.y_offset * scale));
        }
    }
    let mut out = Vec::new();
    let mut piece_start = 0;
    let mut cursor = 0;
    for end in grapheme_ends(&cleaned) {
        let mut width = 0.0;
        let mut glyphs = Vec::new();
        while let Some((cluster, face, id, advance, dx, dy)) = clusters.get(cursor).copied() {
            if cluster >= end {
                break;
            }
            glyphs.push((face, id, width + dx, dy, style.size));
            width += advance;
            cursor += 1;
        }
        let source = cleaned.get(piece_start..end).unwrap_or("");
        let original = part.get(piece_start..end).unwrap_or("");
        let space = source.chars().all(char::is_whitespace);
        if unshaped {
            width = source.chars().next().map_or(0.0, |c| estimated_width(c, style.size));
        }
        if original == "\t" {
            width = theme.tab_width(style.size);
            glyphs.clear();
        } else if original == "\r" {
            width = 0.0;
            glyphs.clear();
        }
        let mark = if unshaped && !space { Mark::Bar } else { Mark::None };
        out.push(Piece {
            start: range.start + piece_start,
            end: range.start + end,
            width,
            space,
            style: index,
            glyphs,
            mark,
            rules: Vec::new(),
            lead: 0.0,
        });
        piece_start = end;
    }
    out
}

/// Where inline HTML's line breaks (`<br>`, `<br/>`, `<br />`, hidden away from the caret)
/// in `range` of `text` end: a new row starts there, as the browser breaks Obsidian's.
fn hard_breaks(text: &str, flags: &[flag::Flags], range: Range<usize>) -> Vec<usize> {
    let Some(part) = text.get(range.clone()) else { return Vec::new() };
    let lower = part.to_ascii_lowercase();
    let mut out = Vec::new();
    for (at, _) in lower.match_indices("<br") {
        let Some(close) = lower.get(at + 3..).and_then(|rest| rest.find('>')).map(|end| at + 3 + end) else { continue };
        let inside = lower.get(at + 3..close).unwrap_or("x");
        let start = range.start + at;
        let end = range.start + close + 1;
        if matches!(inside, "" | "/" | " /")
            && end < range.end
            && flags.get(start..end).is_some_and(|tag| tag.iter().all(|value| value & flag::HIDE != 0))
        {
            out.push(end);
        }
    }
    out
}

/// Where `text` may break: byte offsets a row may start at.
fn break_points(text: &str) -> Vec<usize> {
    // As the browser breaks (and so Obsidian): not after an address's punctuation (`/ ? & = #`)
    // before a letter or digit, so an address is one word that breaks only where the row ends.
    let solidus_inside = |at: usize| {
        text.get(..at).is_some_and(|before| before.ends_with(['/', '?', '&', '=', '#']))
            && text.get(at..).and_then(|after| after.chars().next()).is_some_and(char::is_alphanumeric)
    };
    unicode_linebreak::linebreaks(text).map(|(at, _)| at).filter(|at| *at < text.len() && !solidus_inside(*at)).collect()
}

/// The pieces in rows no wider than `available`, as index ranges, breaking at `breaks` (bytes)
/// and always at `forced` (bytes).
fn wrap(pieces: &[Piece], available: f32, hang: f32, breaks: &[usize], forced: &[usize]) -> Vec<Range<usize>> {
    let mut rows: Vec<Range<usize>> = Vec::new();
    let mut row_start = 0;
    let mut x = 0.0;
    let mut last_break: Option<usize> = None;
    for (index, piece) in pieces.iter().enumerate() {
        // A hard break (`<br>`) starts a row.
        if index > row_start && forced.binary_search(&piece.start).is_ok() {
            rows.push(row_start..index);
            row_start = index;
            x = 0.0;
            last_break = None;
        }
        if index > row_start && breaks.binary_search(&piece.start).is_ok() {
            last_break = Some(index);
        }
        // Rows after the first hang under the text (a list item's), so they have less room.
        let room = if rows.is_empty() { available } else { (available - hang).max(available * 0.25) };
        if !piece.space && x + piece.width > room && index > row_start {
            // A word wider than a whole row stays on this row and breaks where it ends, as the
            // browser breaks it (an address, a long word); a shorter one moves to the next row.
            let full = (available - hang).max(available * 0.25);
            let word_too_wide = last_break.filter(|at| *at > row_start).is_some_and(|from| {
                let end = (index..pieces.len())
                    .find(|at| *at > index && pieces.get(*at).is_some_and(|next| breaks.binary_search(&next.start).is_ok()))
                    .unwrap_or(pieces.len());
                pieces.get(from..end).map_or(0.0, |word| word.iter().filter(|piece| !piece.space).map(|piece| piece.width).sum::<f32>()) > full
            });
            let at = if word_too_wide { index } else { last_break.filter(|at| *at > row_start).unwrap_or(index) };
            rows.push(row_start..at);
            row_start = at;
            x = pieces.get(at..index).map_or(0.0, |rest| rest.iter().map(|piece| piece.width).sum());
            last_break = None;
        }
        x += piece.width;
    }
    rows.push(row_start..pieces.len());
    rows
}

/// Font metrics at `size`: (ascent, descent).
fn extents(size: f32) -> (f32, f32) {
    let metrics = fonts::metrics(Family::Sans);
    (size * metrics.ascent / metrics.upem, size * metrics.descent / metrics.upem)
}

/// Draws `slice` from (`x`, text line top `dy`) into `row`: glyphs, backgrounds, strikes, bars,
/// task boxes; adds a stop per piece (in `cell`). Answers the x after the last piece.
#[allow(clippy::too_many_arguments)]
fn place(
    row: &mut Row,
    slice: &[Piece],
    styles: &[RunStyle],
    theme: &Theme,
    mut x: f32,
    dy: f32,
    line_height: f32,
    base: f32,
    cell: u16,
    origin: usize,
) -> f32 {
    let (ascent, descent) = extents(base);
    let baseline = dy + (line_height - (ascent + descent)) / 2.0 + ascent;
    let mut struck = false;
    for piece in slice {
        row.stops.push(Stop { byte: piece.start - origin, x: x + piece.lead, dy, cell });
        let style = styles.get(usize::from(piece.style)).copied();
        for (face, id, gx, gy, size) in &piece.glyphs {
            let rise = style_rise(styles, piece.style);
            row.glyphs.push(DrawGlyph {
                face: *face,
                id: *id,
                x: x + gx,
                y: baseline - gy - rise,
                scale: size / fonts::upem(*face),
                style: piece.style,
            });
        }
        if let Some(style) = style {
            let (glyph_top, glyph_bottom) = (baseline - style.size * 0.82, baseline + style.size * 0.24);
            if let Some(color) = style.background
                && piece.width > 0.0
            {
                // A tag is a pill 24 px high, inline code a 21 px box with 4 px corners (at 14 px
                // text), a highlight the text's own height, as Obsidian draws them.
                let (pad_y, radius) = if color == theme.tag_background {
                    (style.size * 0.32, (glyph_bottom - glyph_top) / 2.0 + style.size * 0.32)
                } else if color == theme.code_background {
                    (style.size * 0.22, 4.0)
                } else {
                    (1.0, 0.0)
                };
                row.behind.push(RowRect {
                    x0: x,
                    y0: glyph_top - pad_y,
                    x1: x + piece.width,
                    y1: glyph_bottom + pad_y,
                    color,
                    outline: false,
                    check: false,
                    dot: false,
                    radius,
                    icon: 0,
                });
            }
            match piece.mark {
                Mark::Bar => {
                    let mut color = style.color;
                    color[3] = 150;
                    row.over.push(RowRect {
                        x0: x,
                        y0: baseline - style.size * 0.5,
                        x1: x + piece.width,
                        y1: baseline,
                        color,
                        outline: false,
                        check: false,
                        dot: false,
                        radius: 0.0,
                        icon: 0,
                    });
                }
                Mark::Task { done } => {
                    // A 16 px box 7 px in (at 16 px text), as Obsidian's checkbox.
                    let side = style.size.round();
                    let (x0, y0) = (x + style.size * 0.4375, (baseline - style.size * 0.36 - side / 2.0).round());
                    let rect = |color, outline, check| RowRect {
                        x0,
                        y0,
                        x1: x0 + side,
                        y1: y0 + side,
                        color,
                        outline,
                        check,
                        dot: false,
                        radius: 3.0,
                        icon: 0,
                    };
                    if done {
                        row.over.push(rect(theme.accent, false, false));
                        row.over.push(rect(theme.paper, false, true));
                    } else {
                        row.over.push(rect(theme.muted, true, false));
                    }
                }
                Mark::Bullet => {
                    // Obsidian's dot: 0.3 em, centered 15.5 px in (at 16 px text).
                    let side = style.size * 0.3;
                    let (cx, cy) = (x + style.size * 0.97, baseline - style.size * 0.34);
                    row.over.push(RowRect {
                        x0: cx - side / 2.0,
                        y0: cy - side / 2.0,
                        x1: cx + side / 2.0,
                        y1: cy + side / 2.0,
                        color: theme.muted,
                        outline: false,
                        check: false,
                        dot: true,
                        radius: 0.0,
                        icon: 0,
                    });
                }
                Mark::Callout { kind } => {
                    let side = style.size * 1.05;
                    let (x0, y0) = (x, baseline - style.size * 0.4 - side / 2.0);
                    let color = crate::theme::callout_color(theme, kind);
                    row.over.push(RowRect {
                        x0,
                        y0,
                        x1: x0 + side,
                        y1: y0 + side,
                        color,
                        outline: false,
                        check: false,
                        dot: false,
                        radius: 0.0,
                        icon: crate::theme::callout_icon(kind),
                    });
                }
                Mark::External { after } => {
                    let side = (style.size * 0.75).round();
                    let (x0, y0) = (x + after + style.size * 0.1, baseline - style.size * 0.36 - side / 2.0);
                    row.over.push(RowRect {
                        x0,
                        y0,
                        x1: x0 + side,
                        y1: y0 + side,
                        color: theme.muted,
                        outline: false,
                        check: false,
                        dot: false,
                        radius: 0.0,
                        icon: EXTERNAL_ICON,
                    });
                }
                Mark::Chevron { after, folded } => {
                    let side = (style.size * 0.9).round();
                    let (x0, y0) = (x + after + style.size * 0.2, baseline - style.size * 0.36 - side / 2.0);
                    row.over.push(RowRect {
                        x0,
                        y0,
                        x1: x0 + side,
                        y1: y0 + side,
                        color: style.color,
                        outline: false,
                        check: false,
                        dot: false,
                        radius: 0.0,
                        icon: if folded { CHEVRON_RIGHT } else { CHEVRON_DOWN },
                    });
                }
                Mark::None => {}
            }
            if style.underline && piece.width > 0.0 {
                let y = baseline + (style.size * 0.12).max(1.5);
                let mut color = style.color;
                color[3] = 110;
                row.over.push(RowRect {
                    x0: x,
                    y0: y - 0.5,
                    x1: x + piece.width,
                    y1: y + 0.5,
                    color,
                    outline: false,
                    check: false,
                    dot: false,
                    radius: 0.0,
                    icon: 0,
                });
            }
            // Struck text is one line across its words, but not the spaces before them.
            struck |= style.strike && !piece.space && piece.width > 0.0;
            for [x0, y0, x1, y1] in &piece.rules {
                row.over.push(RowRect {
                    x0: x + x0,
                    y0: baseline + y0,
                    x1: x + x1,
                    y1: baseline + y1,
                    color: style.color,
                    outline: false,
                    check: false,
                    dot: false,
                    radius: 0.0,
                    icon: 0,
                });
            }
            if style.strike && struck && piece.width > 0.0 {
                let y = baseline - style.size * 0.3;
                row.over.push(RowRect {
                    x0: x,
                    y0: y - 0.6,
                    x1: x + piece.width,
                    y1: y + 0.6,
                    color: style.color,
                    outline: false,
                    check: false,
                    dot: false,
                    radius: 0.0,
                    icon: 0,
                });
            }
        }
        x += piece.width;
    }
    x
}

/// A line's box guessed from its length, without shaping it: its shown characters at an average
/// width (a wide one, CJK, an em), in rows of its block's line height.
pub fn estimate_line(theme: &Theme, line: LineStyle, text: &str, flags: &[flag::Flags], width: f32) -> LineBox {
    let size = theme.size_of(line.block);
    let mut advance = 0.0;
    for (at, c) in text.char_indices() {
        if flags.get(at).is_some_and(|value| value & flag::HIDE != 0) {
            continue;
        }
        advance += if c.is_ascii() {
            size * 0.52
        } else if c as u32 >= 0x1100 {
            size
        } else {
            size * 0.6
        };
    }
    // A quote's lazy line (its text goes on without a `>`) starts at the column's edge, over the
    // quote's bar, as Obsidian draws it.
    let lazy = line.quote > 0 && line.callout == 0 && line.list == 0 && line.block == Block::Paragraph && !text.trim_start().starts_with('>');
    let indent = if lazy { 0.0 } else { theme.indent_of(line) };
    let available = (width - indent - theme.inset_right(line)).max(size * 4.0);
    let rows = (advance / available).ceil().max(1.0);
    let line_height = theme.line_height_of(line.block);
    let height = rows * line_height;
    let row = Row {
        start: 0,
        end: text.len(),
        top: 0.0,
        height,
        caret_height: line_height,
        stops: vec![Stop { byte: 0, x: indent, dy: 0.0, cell: 0 }, Stop { byte: text.len(), x: indent, dy: 0.0, cell: 0 }],
        glyphs: Vec::new(),
        behind: Vec::new(),
        over: Vec::new(),
        table: None,
        image: None,
    };
    LineBox { rows: vec![row], styles: Vec::new(), height, above: theme.space_above(line.block), estimated: true }
}

/// The external link icon after a Markdown link to the web (in its hidden `](`), as Obsidian
/// draws it, at `size`; a bare address has none.
fn external_link_icons(text: &str, flags: &[flag::Flags], pieces: &mut [Piece], size: f32) {
    for index in 0..pieces.len() {
        let Some(piece) = pieces.get(index) else { continue };
        let value = flags.get(piece.start).copied().unwrap_or(0);
        let link_end = value & flag::EXTERNAL != 0
            && value & flag::HIDE != 0
            && piece.width == 0.0
            && text.get(piece.start..).is_some_and(|rest| rest.starts_with("]("));
        // A link whose syntax shows (the caret on it) has the icon after its closing `)`.
        let shown_end = value & flag::EXTERNAL != 0
            && value & (flag::HIDE | flag::MARK) == flag::MARK
            && text.get(piece.start..piece.end) == Some(")")
            && flags.get(piece.end).is_none_or(|next| next & flag::EXTERNAL == 0);
        if let Some(piece) = pieces.get_mut(index)
            && (link_end || shown_end)
        {
            let after = piece.width;
            piece.width += size * 0.9;
            piece.mark = Mark::External { after };
        }
    }
}

/// Inline math away from the caret (its dollars hidden) draws as its formula, in the first piece
/// of its source, at `size`. Answers the height the tallest formula needs.
fn typeset_math(text: &str, flags: &[flag::Flags], pieces: &mut [Piece], size: f32) -> f32 {
    let mut formula_height: f32 = 0.0;
    let mut index = 0;
    while index < pieces.len() {
        let Some(piece) = pieces.get(index) else { break };
        let value = flags.get(piece.start).copied().unwrap_or(0);
        if value & flag::MATH == 0 || value & flag::HIDE == 0 || text.get(piece.start..piece.start + 1) != Some("$") {
            index += 1;
            continue;
        }
        let run_start = piece.start;
        let mut last = index;
        while pieces.get(last + 1).is_some_and(|next| flags.get(next.start).is_some_and(|value| value & flag::MATH != 0)) {
            last += 1;
        }
        let run_end = pieces.get(last).map_or(run_start, |piece| piece.end);
        let source = text.get(run_start..run_end).unwrap_or("").trim_matches('$');
        if let Some(formula) = crate::math::formula(source, false, size)
            && let Some(face) = formula.face
        {
            let upem = fonts::upem(face);
            let glyphs = formula.glyphs.iter().map(|(id, x, y, per_unit)| (face, *id, x + 1.0, -y, per_unit * upem)).collect();
            let rules = formula.rules.iter().map(|[x0, y0, x1, y1]| [x0 + 1.0, *y0, x1 + 1.0, *y1]).collect();
            if let Some(first) = pieces.get_mut(index) {
                first.width = formula.width + 2.0;
                first.glyphs = glyphs;
                first.rules = rules;
            }
            for piece in pieces.iter_mut().skip(index + 1).take(last - index) {
                piece.width = 0.0;
                piece.glyphs.clear();
            }
            formula_height = formula_height.max(formula.ascent + formula.descent + 6.0);
        }
        index = last + 1;
    }
    formula_height
}

/// Lays out one source line in `width`.
fn lay_line(theme: &Theme, line: LineStyle, text: &str, flags: &[flag::Flags], width: f32) -> LineBox {
    let mut styles = Vec::new();
    let mut pieces = pieces(theme, line, text, 0..text.len(), flags, &mut styles);
    // Inline math away from the caret draws as its formula; the line grows to hold a tall one.
    let formula_height = typeset_math(text, flags, &mut pieces, theme.size_of(line.block));
    // Tags and inline code are padded inside their boxes, as Obsidian's (0.65 em and 0.3 em at
    // their size), so the box does not touch the text around it.
    {
        let mut index = 0;
        while index < pieces.len() {
            let background = pieces
                .get(index)
                .and_then(|piece| {
                    (piece.width > 0.0).then(|| styles.get(usize::from(piece.style)).and_then(|style| style.background.map(|bg| (bg, style.size))))
                })
                .flatten();
            let Some((color, size)) = background.filter(|(color, _)| *color == theme.tag_background || *color == theme.code_background) else {
                index += 1;
                continue;
            };
            let mut last = index;
            while let Some(next) = pieces.get(last + 1) {
                let same = next.width == 0.0 || styles.get(usize::from(next.style)).is_some_and(|style| style.background == Some(color));
                if !same {
                    break;
                }
                last += 1;
            }
            while last > index && pieces.get(last).is_some_and(|piece| piece.width == 0.0) {
                last -= 1;
            }
            let pad = if color == theme.tag_background { size * 0.65 } else { size * 0.3 };
            if let Some(first) = pieces.get_mut(index) {
                first.width += pad;
                first.lead += pad;
                for glyph in &mut first.glyphs {
                    glyph.2 += pad;
                }
            }
            if let Some(end) = pieces.get_mut(last) {
                end.width += pad;
            }
            index = last + 1;
        }
    }
    let size = theme.size_of(line.block);
    external_link_icons(text, flags, &mut pieces, size);
    // A list line's leading space indents as Obsidian's does: each whole indent (a tab or four
    // spaces) one list indent wide, the spaces left over as wide as spaces.
    // A quote line's space after its marks indents the same way.
    if (line.block == Block::Paragraph && line.callout == 0 && (line.list > 0 || line.quote > 0)) || line.block == Block::Indented {
        let size = theme.size_of(line.block);
        // An indented code line's first indent only: the rest of its space is its code's.
        let starts = leading_unit_starts(text, line.quote > 0);
        let starts = if line.block == Block::Indented { starts.into_iter().take(1).collect() } else { starts };
        for start in starts {
            let end = start + if text.get(start..start + 1) == Some("\t") { 1 } else { 4 };
            for piece in pieces.iter_mut().filter(|piece| piece.start >= start && piece.end <= end) {
                piece.width = if piece.start == start { theme.tab_width(size) } else { 0.0 };
                piece.glyphs.clear();
            }
        }
    }
    // A nested list item in a callout sits a list indent (2.25 em) further in per level, as the
    // callout's rendered list nests it, whatever space it is written with.
    if line.block == Block::Paragraph && line.callout > 0 && line.list > 1 {
        let marks = quote_marks_end(text);
        let indent = text.get(marks..).map_or(0, |rest| rest.len() - rest.trim_start_matches([' ', '\t']).len());
        if indent > 0 {
            let width = theme.tab_width(theme.size_of(line.block)) * f32::from(line.list - 1);
            for piece in pieces.iter_mut().filter(|piece| piece.start >= marks && piece.end <= marks + indent) {
                piece.width = if piece.start == marks { width } else { 0.0 };
                piece.glyphs.clear();
            }
        }
    }
    // A footnote definition's continuation line (indented under it) indents the same way, at
    // the definition's smaller size (Obsidian's 31.5 px at 14 px text).
    if line.block == Block::Paragraph
        && line.list == 0
        && line.quote == 0
        && flags.first().is_some_and(|value| value & flag::FOOTDEF != 0)
        && text.starts_with([' ', '\t'])
    {
        let size = theme.size_of(line.block) * theme::INLINE_CODE;
        for start in indent_unit_starts(text) {
            let end = start + if text.get(start..start + 1) == Some("\t") { 1 } else { 4 };
            for piece in pieces.iter_mut().filter(|piece| piece.start >= start && piece.end <= end) {
                piece.width = if piece.start == start { theme.tab_width(size) } else { 0.0 };
                piece.glyphs.clear();
            }
        }
    }
    // A numbered item's text starts 30 px in (at 16 px text), as Obsidian's: the space after its
    // number takes the rest.
    if line.list > 0 {
        // Past a callout's or quote's `> ` marks, then the item's leading space.
        let tabs = text.len() - text.trim_start_matches(['\t', '>', ' ']).len();
        let tabs = if line.quote > 0 { tabs } else { text.len() - text.trim_start_matches(['\t', ' ']).len() };
        let body = text.get(tabs..).unwrap_or("");
        let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0 && (body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") "))) {
            // The number starts 0.75 em in and the text follows it (at least 1.875 em in), as
            // Obsidian draws a number wider than `1.` (`10. ` 40 px in); a callout renders its
            // list, its numbers right-aligned in the marker's area.
            let space_at = tabs + digits + 1;
            let marker: f32 = pieces.iter().filter(|piece| piece.start >= tabs && piece.start <= space_at).map(|piece| piece.width).sum();
            let size = theme.size_of(line.block);
            let wanted = size * 1.875;
            let least = if line.callout > 0 { 0.0 } else { size * 0.75 };
            if let Some(first) = pieces.iter_mut().find(|piece| piece.start == tabs)
                && (marker < wanted || least > 0.0)
            {
                let pad = (wanted - marker).max(least);
                first.width += pad;
                first.lead += pad;
                for glyph in &mut first.glyphs {
                    glyph.2 += pad;
                }
            }
        }
    }
    let mut bullet_lead = 0.0;
    // The caret's line shows a bullet item's marker where its dot is (0.75 em in) and its text
    // where it was (1.4375 em in), as Obsidian keeps the item's text in place.
    if line.list > 0 && line.block == Block::Paragraph && line.quote == 0 {
        let tabs = text.len() - text.trim_start_matches(['\t', ' ']).len();
        let body = text.get(tabs..).unwrap_or("");
        let size = theme.size_of(line.block);
        if ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker))
            && let Some(index) = pieces.iter().position(|piece| piece.start == tabs)
            && pieces.get(index).is_some_and(|piece| piece.width > 0.0 && matches!(piece.mark, Mark::None) && piece.end == tabs + 1)
            && pieces.get(index + 1).is_some_and(|piece| piece.start == tabs + 1 && piece.end == tabs + 2)
        {
            // The row starts 0.75 em in (the caret before the marker, the selection from it).
            bullet_lead = size * 0.75;
            let dash = pieces.get(index).map_or(0.0, |dash| dash.width);
            if let Some(space) = pieces.get_mut(index + 1) {
                space.width = (size * 1.4375 - bullet_lead - dash).max(space.width);
            }
        }
    }
    // An item's continuation line (spaces, then text without a marker) is set in by 1 em more
    // than its spaces, as Obsidian pads them (its list indent plus marker space), so the text
    // lines up under the item's.
    if line.list > 0 && line.block == Block::Paragraph {
        // Past the whole indents (a continuation indented by them alone is not padded).
        let units = indent_unit_starts(text);
        let tabs = units.last().map_or(0, |last| last + if text.get(*last..last + 1) == Some("\t") { 1 } else { 4 });
        let body = text.get(tabs..).unwrap_or("");
        let spaces = body.len() - body.trim_start_matches(' ').len();
        let rest = body.get(spaces..).unwrap_or("");
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let marker = ["- ", "* ", "+ "].iter().any(|marker| rest.starts_with(marker) || rest == marker.trim_end())
            || (digits > 0 && rest.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")));
        if spaces > 0
            && !rest.is_empty()
            && !marker
            && let Some(first) = pieces.iter_mut().find(|piece| piece.start == tabs)
        {
            let pad = theme.size_of(line.block);
            first.width += pad;
            for glyph in &mut first.glyphs {
                glyph.2 += pad;
            }
        }
    }
    // A bullet item in a callout is set in as a numbered one (Obsidian renders a callout's lists
    // with one padding: the text 54 px in at 16 px text, the dot 15 px before it). The room goes
    // after the dot, which stays where a list's is.
    if line.callout > 0
        && let Some(bullet) = pieces.iter_mut().find(|piece| matches!(piece.mark, Mark::Bullet))
    {
        bullet.width += theme.size_of(line.block) * (1.875 - 1.4375);
    }
    // A callout's title: its hidden `[!kind]` marker makes room for the kind's icon, a callout
    // without a title shows its kind's name (capitalized, as Obsidian does), and a foldable one
    // (`+` or `-`) a chevron after its title.
    if line.title
        && line.callout > 0
        && let Some(open) = text.find("[!")
        && let Some(close) = text.get(open..).and_then(|rest| rest.find(']')).map(|at| open + at)
        && pieces.iter().any(|piece| piece.start == open && piece.width == 0.0)
    {
        let size = theme.size_of(line.block);
        if let Some(piece) = pieces.iter_mut().find(|piece| piece.start == open) {
            piece.width = size * 1.4;
            piece.mark = Mark::Callout { kind: line.callout };
        }
        let fold = text.get(close + 1..close + 2).filter(|c| *c == "+" || *c == "-");
        let after = close + 1 + fold.map_or(0, str::len);
        if text.get(after..).is_none_or(|rest| rest.trim().is_empty()) {
            let kind = text.get(open + 2..close).unwrap_or("");
            let mut label: String = kind.chars().take(1).flat_map(char::to_uppercase).collect();
            label.push_str(&kind.chars().skip(1).collect::<String>().to_lowercase());
            let title = run_style(theme, line, flag::TEXT);
            let index = styles.iter().position(|known| *known == title).unwrap_or_else(|| {
                styles.push(title);
                styles.len() - 1
            });
            let mut x = 0.0;
            let mut glyphs = Vec::new();
            for run in fonts::shape(&label, title.family, title.weight) {
                let scale = title.size / run.upem;
                for glyph in &run.glyphs {
                    glyphs.push((run.face, glyph.id, x + glyph.x_offset * scale, glyph.y_offset * scale, title.size));
                    x += glyph.advance * scale;
                }
            }
            if let Some(piece) = pieces.iter_mut().find(|piece| piece.start == open + 1) {
                piece.glyphs = glyphs;
                piece.width = x;
                piece.style = u16::try_from(index).unwrap_or(u16::MAX);
            }
        }
        if let Some(fold) = fold
            && let Some(last) = pieces.iter_mut().rev().find(|piece| piece.width > 0.0)
            && matches!(last.mark, Mark::None | Mark::Callout { .. })
        {
            let marked = pieces.iter().any(|piece| piece.start == close + 1 && piece.width == 0.0);
            // Drawn folded away from the caret (the marker hidden), open on the caret's line.
            let folded = fold == "-" && marked;
            if let Some(last) = pieces.iter_mut().rev().find(|piece| piece.width > 0.0 && matches!(piece.mark, Mark::None)) {
                let after = last.width;
                last.width += size * 1.25;
                last.mark = Mark::Chevron { after, folded };
            }
        }
    }
    // A done task (`[x]`) reads as done: its text is secondary and struck through. Other marks
    // check the box and leave the text, as in Obsidian.
    let struck = pieces.iter().any(|piece| {
        matches!(piece.mark, Mark::Task { done: true }) && text.get(piece.start + 1..piece.start + 2).is_some_and(|c| c.eq_ignore_ascii_case("x"))
    });
    // With its marker shown (the caret on it) the whole line is struck through, the marker too,
    // as Obsidian draws a done task's line.
    let shown_done = !struck && line.list > 0 && line.block == Block::Paragraph && {
        let body = text.trim_start_matches([' ', '\t']);
        let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let rest = if ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker)) {
            body.get(2..)
        } else if digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")) {
            body.get(digits + 2..)
        } else {
            None
        };
        rest.is_some_and(|rest| rest.starts_with("[x]") || rest.starts_with("[X]"))
    };
    if struck || shown_done {
        for style in &mut styles {
            if style.color == theme.text {
                style.color = theme.secondary;
                style.strike = true;
            } else if shown_done {
                style.strike = true;
            }
        }
    }
    let breaks = break_points(text);
    let base = theme.size_of(line.block);
    // A properties block's row: 29 px for a key's line (Obsidian's row), nothing for its fences
    // and a key's further lines (a block list's items), as the editor draws the block over them.
    let property_row = line.block == Block::Property && {
        let trimmed = text.trim_end_matches('\r');
        !trimmed.starts_with([' ', '\t']) && !trimmed.starts_with("- ") && trimmed.contains(':') && trimmed != "---"
    };
    // A callout's title is set tight (1.3), as Obsidian's.
    let line_height = if line.block == Block::Property {
        // A number's, a checkbox's, a date's, or a date and time's row is 28 px (its field is
        // an input, not a text line), as Obsidian's.
        let value = text.split_once(':').map_or("", |(_, value)| value.trim());
        let field = value.parse::<f64>().is_ok() || value == "true" || value == "false" || is_date_value(value);
        if !property_row {
            0.0
        } else if field {
            28.0
        } else {
            29.0
        }
    } else if line.title && line.callout > 0 {
        (theme.size_of(line.block) * 1.3).round()
    } else if line.callout > 0 && matches!(line.block, Block::Code { .. }) {
        // A callout's code block is rendered HTML, as in Obsidian: its fences are the box's
        // 12 px padding, its lines the text's 24 px.
        if line.block == (Block::Code { fence: true }) { 12.0 } else { theme.line_height_of(Block::Paragraph) }
    } else if line.block == Block::Paragraph && flags.first().is_some_and(|value| value & flag::FOOTDEF != 0) {
        // A footnote's or link reference's definition: its smaller text at 1.5 (21 px at 16 px).
        base * theme::INLINE_CODE * 1.5
    } else {
        theme.line_height_of(line.block)
    };
    let line_height = line_height.max(formula_height.ceil());
    // A lowered `<sub>` and a raised `<sup>` each take 3 px more of the line, as in the browser.
    let shown = |bit: flag::Flags| flags.iter().any(|value| value & bit != 0 && value & flag::HIDE == 0);
    let line_height = line_height + if shown(flag::SUB) { 3.0 } else { 0.0 } + if shown(flag::SUP) { 3.0 } else { 0.0 };
    // A quote's lazy line (its text goes on without a `>`) starts at the column's edge, over the
    // quote's bar, as Obsidian draws it.
    let lazy = line.quote > 0 && line.callout == 0 && line.list == 0 && line.block == Block::Paragraph && !text.trim_start().starts_with('>');
    let indent = if lazy { 0.0 } else { theme.indent_of(line) };
    let mut out = Vec::new();
    let mut top = 0.0;
    // A callout's text keeps clear of its box's right padding, as Obsidian's.
    let available = (width - indent - theme.inset_right(line)).max(base * 4.0);
    // A list item's wrapped rows start under its text, as Obsidian hangs them: past its leading
    // space, its marker, and its task box.
    let hang = if line.list > 0 && line.block == Block::Paragraph {
        let content = {
            let body = if line.quote > 0 { text.trim_start_matches([' ', '\t', '>']) } else { text.trim_start_matches([' ', '\t']) };
            let mut at = text.len() - body.len();
            let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            if ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker)) {
                at += 2;
            } else if digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")) {
                at += digits + 2;
            }
            let rest = text.get(at..).unwrap_or("");
            let mut chars = rest.chars();
            if chars.next() == Some('[') && chars.next().is_some_and(|c| c != ']') && chars.next() == Some(']') {
                at += rest.char_indices().nth(3).map_or(rest.len(), |(offset, _)| offset) + usize::from(rest.get(3..4) == Some(" "));
            }
            at
        };
        pieces.iter().take_while(|piece| piece.end <= content).map(|piece| piece.width).sum::<f32>() + bullet_lead
    } else if line.block == Block::Paragraph
        && line.list == 0
        && flags.first().is_some_and(|value| value & flag::FOOTDEF != 0)
        && text.starts_with([' ', '\t'])
    {
        // A footnote definition's indented line hangs its wrapped rows under its text (31.5 px
        // at 14 px), as Obsidian's.
        let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
        pieces.iter().take_while(|piece| piece.end <= indent).map(|piece| piece.width).sum::<f32>()
    } else {
        0.0
    };
    // The caret's quote line shows its marks in the quote's padding, its text where it was.
    let quote_marks = if line.quote > 0 && line.callout == 0 {
        let marks = quote_marks_end(text);
        pieces.iter().take_while(|piece| piece.end <= marks).map(|piece| piece.width).sum::<f32>().min(indent)
    } else {
        0.0
    };
    let forced = hard_breaks(text, flags, 0..text.len());
    for range in wrap(&pieces, available, hang, &breaks, &forced) {
        let slice = pieces.get(range).unwrap_or(&[]);
        let start = slice.first().map_or(if out.is_empty() { 0 } else { text.len() }, |piece| piece.start);
        let end = slice.last().map_or(start, |piece| piece.end);
        let mut row = Row {
            start,
            end,
            top,
            height: line_height,
            caret_height: line_height,
            stops: Vec::with_capacity(slice.len() + 1),
            glyphs: Vec::new(),
            behind: Vec::new(),
            over: Vec::new(),
            table: None,
            image: None,
        };
        let left = if out.is_empty() { indent - quote_marks + bullet_lead } else { indent + hang };
        let x = place(&mut row, slice, &styles, theme, left, 0.0, line_height, base, 0, 0);
        row.stops.push(Stop { byte: end, x, dy: 0.0, cell: 0 });
        top += line_height;
        out.push(row);
    }
    // Front matter that is not YAML: its error's line has its first character marked, white on
    // the red (Obsidian's error marker, 16.5 px tall, 5 px down a 24 px line).
    if line.error
        && let Some(first) = text.char_indices().find(|(_, c)| !c.is_whitespace()).map(|(at, c)| at..at + c.len_utf8())
        && let Some(row) = out.first_mut()
    {
        let x = |byte: usize| row.stops.iter().find(|stop| stop.byte >= byte).map_or(0.0, |stop| stop.x);
        let (x0, x1) = (x(first.start), x(first.end));
        let top = ((row.height - 16.5) / 2.0 + 1.0).max(0.0);
        row.behind.push(RowRect {
            x0,
            y0: top,
            x1,
            y1: top + 16.5,
            color: theme.invalid,
            outline: false,
            check: false,
            dot: false,
            radius: 0.0,
            icon: 0,
        });
        let white = RunStyle {
            color: [255, 255, 255, 255],
            ..styles
                .get(usize::from(row.glyphs.iter().find(|glyph| glyph.x >= x0 - 0.5).map_or(0, |glyph| glyph.style)))
                .copied()
                .unwrap_or_else(|| run_style(theme, line, 0))
        };
        let index = styles.iter().position(|known| *known == white).unwrap_or_else(|| {
            styles.push(white);
            styles.len() - 1
        });
        for glyph in row.glyphs.iter_mut().filter(|glyph| glyph.x >= x0 - 0.5 && glyph.x < x1 - 0.5) {
            glyph.style = u16::try_from(index).unwrap_or(0);
        }
    }
    merge_rects(&mut out);
    LineBox { rows: out, styles, height: top, above: theme.space_above(line.block), estimated: false }
}

/// A table row's cells: each cell's content range and its pieces, by source line.
type ShapedRow = (usize, Vec<(Range<usize>, Vec<Piece>)>);

/// A table laid out: each source line's box, by line.
type LaidTable = Arc<Vec<(usize, Arc<LineBox>)>>;

/// Inner padding of a table cell.
const CELL_PAD_X: f32 = 8.0;
const CELL_PAD_Y: f32 = 4.0;

/// Lays out a table whole: one line box per source line from its header to its last row (the
/// delimiter row draws nothing and has no stops).
fn lay_table(theme: &Theme, text: &str, styles_of: &Styles, table: &Table, index: usize, width: f32) -> Vec<(usize, LineBox)> {
    // Obsidian's table (measured): text at 1.3 line height in cells padded 4 px, under a 1 px top
    // border, each row its cells and a 1 px border below (rows 29.8 px apart at 16 px text).
    let line_height = theme.body_size * 1.3;
    let base = theme.body_size;
    let mut styles: Vec<RunStyle> = Vec::new();
    // Each row's cells as pieces, and each column's natural width.
    let columns = table.columns().max(1);
    let mut shaped: Vec<ShapedRow> = Vec::new();
    // A table in a callout sits inside its padding (24 px in, 12 px from its right), as
    // Obsidian renders a callout's content.
    let first_style =
        table.rows.first().map(|row| styles_of.lines.get(styles_of.line_of(row.line.start)).copied().unwrap_or_default()).unwrap_or_default();
    let inset = if first_style.callout > 0 { theme.indent_of(first_style) } else { 0.0 };
    let width = width - inset - theme.inset_right(first_style);
    // Each column's widest text (its max-content) and widest word (its min-content), inside
    // its padding, at least Obsidian's 6 ch (its `--table-column-min-width`, the cell's `0`).
    let mut natural = vec![0.0f32; columns];
    let mut minimum = vec![0.0f32; columns];
    for (row_index, row) in table.rows.iter().enumerate() {
        let line = styles_of.line_of(row.line.start);
        let style = styles_of.lines.get(line).copied().unwrap_or_default();
        let header = matches!(style.block, Block::Table { header: true, .. });
        let zero = shape_span("0", 0..1, run_style(theme, style, if header { flag::STRONG } else { 0 }), 0, theme);
        let floor = (6.0 * zero.iter().map(|piece| piece.width).sum::<f32>() - 2.0 * CELL_PAD_X).max(0.0);
        let mut cells = Vec::new();
        for (column, cell) in row.cells.iter().enumerate() {
            let mut cell_pieces = pieces(theme, style, text, cell.content.clone(), &styles_of.bytes, &mut styles);
            // Inline math in a cell draws as its formula, as Obsidian renders it.
            typeset_math(text, &styles_of.bytes, &mut cell_pieces, theme.body_size);
            external_link_icons(text, &styles_of.bytes, &mut cell_pieces, theme.body_size);
            let breaks: Vec<usize> =
                break_points(text.get(cell.content.clone()).unwrap_or("")).into_iter().map(|at| at + cell.content.start).collect();
            let forced = hard_breaks(text, &styles_of.bytes, cell.content.clone());
            // Its widest line (`<br>` breaks a cell's text) and its widest word.
            let (mut natural_width, mut line_width) = (0.0f32, 0.0f32);
            let (mut widest, mut word) = (0.0f32, 0.0f32);
            for piece in &cell_pieces {
                if forced.binary_search(&piece.start).is_ok() {
                    natural_width = natural_width.max(line_width);
                    line_width = 0.0;
                }
                line_width += piece.width;
                if breaks.binary_search(&piece.start).is_ok() || forced.binary_search(&piece.start).is_ok() {
                    widest = widest.max(word);
                    word = 0.0;
                }
                if !piece.space {
                    word += piece.width;
                }
            }
            widest = widest.max(word);
            natural_width = natural_width.max(line_width);
            if let Some(slot) = natural.get_mut(column) {
                *slot = slot.max(natural_width).max(floor);
            }
            if let Some(slot) = minimum.get_mut(column) {
                *slot = slot.max(widest).max(floor);
            }
            cells.push((cell.content.clone(), cell_pieces));
        }
        let _ = row_index;
        shaped.push((line, cells));
    }
    // Column widths as the browser's automatic table layout sets them (and so Obsidian's,
    // whose tables are at most the column wide): natural when they fit, else each column's
    // widest word plus a share of the room left in proportion to how much more its text wants.
    let padding = 2.0 * CELL_PAD_X * columns as f32;
    let total: f32 = natural.iter().sum();
    let least: f32 = minimum.iter().sum();
    let room = width - padding;
    let inner: Vec<f32> = if total <= room {
        natural.clone()
    } else if least >= room || total <= least {
        // Words wider than the column: Obsidian's table scrolls sideways, which the canvas does
        // not, so the columns share the room by their widest words and those words break.
        // A narrow column keeps 2 em, taken from the widest.
        let mut widths: Vec<f32> = minimum.iter().map(|least_width| room * least_width / least.max(1.0)).collect();
        let short: f32 = widths.iter().map(|w| (2.0 * base - w).max(0.0)).sum();
        for w in &mut widths {
            *w = w.max(2.0 * base);
        }
        if let Some(widest) = widths.iter_mut().max_by(|a, b| a.total_cmp(b)) {
            *widest = (*widest - short).max(2.0 * base);
        }
        widths
    } else {
        let share = (room - least) / (total - least);
        natural.iter().zip(&minimum).map(|(most, least)| least + (most - least) * share).collect()
    };
    let mut edges = vec![inset];
    for w in &inner {
        let last = edges.last().copied().unwrap_or(0.0);
        edges.push(last + w + 2.0 * CELL_PAD_X);
    }
    let right = edges.last().copied().unwrap_or(0.0);
    let mut out = Vec::new();
    let x0 = inset;
    for (row_index, (line, cells)) in shaped.into_iter().enumerate() {
        let origin = styles_of.starts.get(line).copied().unwrap_or(0);
        let line_end = styles_of.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
        let mut row = Row {
            start: 0,
            end: line_end - origin,
            top: 0.0,
            height: 0.0,
            caret_height: line_height,
            stops: Vec::new(),
            glyphs: Vec::new(),
            behind: Vec::new(),
            over: Vec::new(),
            table: Some(TableRow { table: index, row: row_index, columns: edges.clone() }),
            image: None,
        };
        let mut lines_in_row = 1usize;
        // The header row holds the table's top border above its cells.
        let top_border = if row_index == 0 { 1.0 } else { 0.0 };
        for (column, (content, cell_pieces)) in cells.iter().enumerate() {
            let column_x = edges.get(column).copied().unwrap_or(0.0);
            let column_width = inner.get(column).copied().unwrap_or(4.0 * base);
            let align = table.align.get(column).copied().unwrap_or(Align::None);
            let cell = u16::try_from(column + 1).unwrap_or(u16::MAX);
            let breaks: Vec<usize> = break_points(text.get(content.clone()).unwrap_or("")).into_iter().map(|at| at + content.start).collect();
            let forced = hard_breaks(text, &styles_of.bytes, content.clone());
            let rows = wrap(cell_pieces, column_width, 0.0, &breaks, &forced);
            lines_in_row = lines_in_row.max(rows.len());
            for (line_index, range) in rows.iter().enumerate() {
                let slice = cell_pieces.get(range.clone()).unwrap_or(&[]);
                let used: f32 = slice.iter().map(|piece| piece.width).sum();
                let offset = match align {
                    Align::Center => (column_width - used).max(0.0) / 2.0,
                    Align::Right => (column_width - used).max(0.0),
                    _ => 0.0,
                };
                let dy = top_border + CELL_PAD_Y + line_index as f32 * line_height;
                let x = place(&mut row, slice, &styles, theme, column_x + CELL_PAD_X + offset, dy, line_height, base, cell, origin);
                // The cell's end, after its last text line.
                if line_index + 1 == rows.len() {
                    let end = slice.last().map_or(content.start, |piece| piece.end);
                    row.stops.push(Stop { byte: end - origin, x, dy, cell });
                }
            }
        }
        let height = top_border + 2.0 * CELL_PAD_Y + lines_in_row as f32 * line_height + 1.0;
        row.height = height;
        // The grid: a header band, the row's bottom rule, the column rules, the outer frame.
        // (Obsidian's header has no background of its own: in a callout its tint shows.)
        if row_index == 0 && first_style.callout == 0 {
            row.behind.insert(
                0,
                RowRect {
                    x0,
                    y0: 0.0,
                    x1: right,
                    y1: height,
                    color: theme.table_header,
                    outline: false,
                    check: false,
                    dot: false,
                    radius: 0.0,
                    icon: 0,
                },
            );
        }
        if row_index == 0 {
            row.behind.push(RowRect {
                x0,
                y0: 0.0,
                x1: right,
                y1: 1.0,
                color: theme.table_border,
                outline: false,
                check: false,
                dot: false,
                radius: 0.0,
                icon: 0,
            });
        }
        row.behind.push(RowRect {
            x0,
            y0: height - 1.0,
            x1: right,
            y1: height,
            color: theme.table_border,
            outline: false,
            check: false,
            dot: false,
            radius: 0.0,
            icon: 0,
        });
        for edge in &edges {
            let x = (edge - 0.5).max(x0).min(right - 1.0);
            row.behind.push(RowRect {
                x0: x,
                y0: 0.0,
                x1: x + 1.0,
                y1: height,
                color: theme.table_border,
                outline: false,
                check: false,
                dot: false,
                radius: 0.0,
                icon: 0,
            });
        }
        row.stops.sort_by_key(|stop| stop.byte);
        out.push((line, LineBox { rows: vec![row], styles: styles.clone(), height, above: 0.0, estimated: false }));
    }
    // The delimiter row: no height and no stops.
    let delimiter_line = styles_of.line_of(table.delimiter.line.start);
    let delimiter_end = styles_of.starts.get(delimiter_line + 1).map_or(text.len(), |next| next.saturating_sub(1));
    let delimiter_start = styles_of.starts.get(delimiter_line).copied().unwrap_or(0);
    let empty = Row {
        start: 0,
        end: delimiter_end - delimiter_start,
        top: 0.0,
        height: 0.0,
        caret_height: line_height,
        stops: Vec::new(),
        glyphs: Vec::new(),
        behind: Vec::new(),
        over: Vec::new(),
        table: None,
        image: None,
    };
    out.push((delimiter_line, LineBox { rows: vec![empty], styles: Vec::new(), height: 0.0, above: 0.0, estimated: false }));
    out.sort_by_key(|(line, _)| *line);
    out
}

/// A table's lines' boxes guessed without shaping its cells: each row one line of text in its
/// padding and border, the delimiter row nothing (`View::realize` lays the table out).
fn estimate_table(theme: &Theme, text: &str, styles: &Styles, table: &Table) -> LaidTable {
    let line_height = theme.body_size * 1.3;
    let line_end = |line: usize| styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
    let mut out: Vec<(usize, Arc<LineBox>)> = Vec::new();
    for (row_index, source) in std::iter::once(&table.delimiter).chain(&table.rows).enumerate() {
        let line = styles.line_of(source.line.start);
        let length = line_end(line) - styles.starts.get(line).copied().unwrap_or(0);
        let height = if row_index == 0 { 0.0 } else { line_height + 9.0 };
        let row = Row {
            start: 0,
            end: length,
            top: 0.0,
            height,
            caret_height: line_height,
            stops: vec![Stop { byte: 0, x: 0.0, dy: 0.0, cell: 0 }, Stop { byte: length, x: 0.0, dy: 0.0, cell: 0 }],
            glyphs: Vec::new(),
            behind: Vec::new(),
            over: Vec::new(),
            table: None,
            image: None,
        };
        out.push((line, Arc::new(LineBox { rows: vec![row], styles: Vec::new(), height, above: 0.0, estimated: true })));
    }
    out.sort_by_key(|(line, _)| *line);
    Arc::new(out)
}

/// Joins adjacent background rectangles of one color into one (a code span's pieces).
fn merge_rects(rows: &mut [Row]) {
    for row in rows {
        let mut merged: Vec<RowRect> = Vec::with_capacity(row.behind.len());
        for rect in row.behind.drain(..) {
            match merged.last_mut() {
                Some(last)
                    if last.color == rect.color && !last.outline && !rect.outline && (last.x1 - rect.x0).abs() < 0.01 && last.y0 == rect.y0 =>
                {
                    last.x1 = rect.x1
                }
                _ => merged.push(rect),
            }
        }
        row.behind = merged;
    }
}

/// Memoised line boxes and tables.
#[derive(Default)]
pub struct Cache {
    boxes: HashMap<u64, Arc<LineBox>>,
    /// Laid tables by their text and look, with the line and the index they were laid out at.
    tables: HashMap<u64, (usize, usize, LaidTable)>,
    /// The keys the layout in progress used (see `Cache::sweep`).
    used_boxes: Vec<u64>,
    used_tables: Vec<u64>,
}

fn key(text: &str, flags: &[flag::Flags], line: LineStyle, width: f32) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    text.hash(&mut hasher);
    flags.hash(&mut hasher);
    line.hash(&mut hasher);
    width.to_bits().hash(&mut hasher);
    hasher.finish()
}

impl Cache {
    /// After a whole layout: keeps what it used, and what the layout before used for an undo
    /// (a fixed cap smaller than a large note's line count emptied the cache on every edit).
    fn sweep(&mut self) {
        if self.boxes.len() > self.used_boxes.len() * 2 + 1024 {
            let keep: std::collections::HashSet<u64> = self.used_boxes.iter().copied().collect();
            self.boxes.retain(|key, _| keep.contains(key));
        }
        if self.tables.len() > self.used_tables.len() * 2 + 64 {
            let keep: std::collections::HashSet<u64> = self.used_tables.iter().copied().collect();
            self.tables.retain(|key, _| keep.contains(key));
        }
        self.used_boxes.clear();
        self.used_tables.clear();
    }

    /// A line's box when it was laid out before (kept as used), else `None`.
    pub fn cached(&mut self, line: LineStyle, text: &str, flags: &[flag::Flags], width: f32) -> Option<Arc<LineBox>> {
        let key = key(text, flags, line, width);
        let found = self.boxes.get(&key).cloned()?;
        self.used_boxes.push(key);
        Some(found)
    }

    pub fn line(&mut self, theme: &Theme, line: LineStyle, text: &str, flags: &[flag::Flags], width: f32) -> Arc<LineBox> {
        let key = key(text, flags, line, width);
        self.used_boxes.push(key);
        if let Some(found) = self.boxes.get(&key) {
            return found.clone();
        }
        let boxed = Arc::new(lay_line(theme, line, text, flags, width));
        self.boxes.insert(key, boxed.clone());
        boxed
    }

    pub(crate) fn table(&mut self, theme: &Theme, text: &str, styles: &Styles, table: &Table, index: usize, width: f32) -> LaidTable {
        self.table_if(theme, text, styles, table, index, width, true).unwrap_or_default()
    }

    /// A table's lines as laid out before, else (`lay`) laid out now, else `None`.
    #[allow(clippy::too_many_arguments)]
    fn table_if(&mut self, theme: &Theme, text: &str, styles: &Styles, table: &Table, index: usize, width: f32, lay: bool) -> Option<LaidTable> {
        let range = table.range.clone();
        // The table's lines (relative), their flags, the line styles, and where it starts in lines.
        let first = styles.line_of(range.start);
        let mut hasher = rustc_hash::FxHasher::default();
        text.get(range.clone()).hash(&mut hasher);
        styles.bytes.get(range.clone()).hash(&mut hasher);
        styles.lines.get(first..first + table.rows.len() + 1).hash(&mut hasher);
        width.to_bits().hash(&mut hasher);
        let key = hasher.finish();
        // Not keyed by where the table is: a line added above it (Enter) moves every table below,
        // and laying out thousands again took long. A moved table is its boxes renumbered.
        if let Some((at, number, laid)) = self.tables.get(&key) {
            self.used_tables.push(key);
            if *at == first && *number == index {
                return Some(laid.clone());
            }
            let (at, number) = (*at, *number);
            let moved: Vec<(usize, Arc<LineBox>)> = laid
                .iter()
                .map(|(line, boxed)| {
                    let boxed = if number == index {
                        boxed.clone()
                    } else {
                        let mut renumbered = LineBox::clone(boxed);
                        for row in &mut renumbered.rows {
                            if let Some(table) = row.table.as_mut() {
                                table.table = index;
                            }
                        }
                        Arc::new(renumbered)
                    };
                    (line - at + first, boxed)
                })
                .collect();
            let moved = Arc::new(moved);
            self.tables.insert(key, (first, index, moved.clone()));
            return Some(moved);
        }
        if !lay {
            return None;
        }
        self.used_tables.push(key);
        let laid: Vec<(usize, Arc<LineBox>)> =
            lay_table(theme, text, styles, table, index, width).into_iter().map(|(line, boxed)| (line, Arc::new(boxed))).collect();
        let laid = Arc::new(laid);
        self.tables.insert(key, (first, index, laid.clone()));
        Some(laid)
    }
}

/// The `$$` math blocks no active line is in, as line boxes by line: the block's first line holds
/// its formula centered with 8 px above and below, its other lines nothing. A block whose
/// formula does not lay out is left to its source.
fn math_blocks(theme: &Theme, text: &str, styles: &Styles, active: &Range<usize>) -> (HashMap<usize, Arc<LineBox>>, HashMap<usize, Preview>) {
    let mut out = HashMap::new();
    let mut previews = HashMap::new();
    let count = styles.starts.len();
    let mut index = 0;
    while index < count {
        if styles.lines.get(index).is_none_or(|line| line.block != Block::Math) {
            index += 1;
            continue;
        }
        let first = index;
        while styles.lines.get(index + 1).is_some_and(|line| line.block == Block::Math) {
            index += 1;
        }
        let last = index;
        index += 1;
        let start = styles.starts.get(first).copied().unwrap_or(0);
        let end = styles.starts.get(last + 1).map_or(text.len(), |next| next.saturating_sub(1));
        let editing = start <= active.end && active.start <= end;
        let style = styles.lines.get(first).copied().unwrap_or_default();
        let raw = text.get(start..end).unwrap_or("");
        // In a callout the block is rendered without its quote marks; in a plain quote MathJax gets
        // the source between the dollars as written, its `>` marks too (Obsidian's quirk).
        let unquoted: String;
        let source = if style.callout > 0 {
            unquoted = raw.lines().map(|line| line.get(quote_marks_end(line)..).unwrap_or("")).collect::<Vec<_>>().join("\n");
            unquoted.trim().trim_start_matches("$$").trim_end_matches("$$")
        } else if style.quote > 0 {
            match (raw.find("$$"), raw.rfind("$$")) {
                (Some(open), Some(close)) if close > open + 1 => raw.get(open + 2..close).unwrap_or(""),
                _ => raw,
            }
        } else {
            raw.trim().trim_start_matches("$$").trim_end_matches("$$")
        };
        // At the text's size in 16 px of padding above and below, as Obsidian's MathJax sets it.
        let Some(formula) = crate::math::formula(source, true, theme.body_size) else { continue };
        let Some(face) = formula.face else { continue };
        // The formula sits on a text line's strut (MathJax's container is a 24 px line of 16 px
        // text, SF's 0.952 em ascent and 0.241 em descent with half the leading each side), so a
        // short one keeps the line's height, as in Obsidian (56 px in all at 16 px text).
        let pad = 16.0;
        let line_height = theme.line_height_of(Block::Paragraph);
        let strut_above = (line_height + (0.952 - 0.241) * theme.body_size) / 2.0;
        let above = formula.ascent.max(strut_above);
        // A block indented in a list item keeps its indent as a line of its own above it (25 px),
        // as Obsidian's live preview does.
        let indented = style.list > 0 && style.quote == 0 && raw.starts_with([' ', '\t']);
        // In a plain quote the block sits between its marks' line and the closing `>`'s, a text
        // line each (104 px in all for a short formula in Obsidian).
        let plain_quote = style.quote > 0 && style.callout == 0;
        let lead = if indented {
            line_height + 1.0
        } else if plain_quote {
            line_height
        } else {
            0.0
        };
        let trail = if plain_quote { line_height } else { 0.0 };
        let height = lead + above + formula.descent.max(line_height - strut_above) + 2.0 * pad + trail;
        // Centered in the callout's or quote's text area.
        let area = if style.callout > 0 || style.quote > 0 { theme.indent_of(style) } else { 0.0 };
        let room = theme.content_width() - area - theme.inset_right(style);
        let left = area + ((room - formula.width) / 2.0).max(0.0);
        let baseline = lead + pad + above;
        let length = styles.starts.get(first + 1).map_or(end, |next| next.saturating_sub(1)) - start;
        // With the caret in it, the block shows its source and, below it, the formula (Obsidian's
        // live preview of math being written).
        if editing {
            let glyphs = formula
                .glyphs
                .iter()
                .map(|(id, x, y, per_unit)| DrawGlyph { face, id: *id, x: left + x, y: baseline + y, scale: *per_unit, style: 0 })
                .collect();
            let rules = formula.rules.iter().map(|[x0, y0, x1, y1]| [left + x0, baseline + y0, left + x1, baseline + y1]).collect();
            previews.insert(last, Preview { glyphs, rules, height });
            continue;
        }
        let style = RunStyle {
            family: Family::Sans,
            weight: fonts::REGULAR,
            size: theme.body_size,
            color: theme.text,
            italic: false,
            strike: false,
            underline: false,
            rise: 0.0,
            background: None,
        };
        let row = Row {
            start: 0,
            end: length,
            top: 0.0,
            height,
            caret_height: height,
            stops: vec![Stop { byte: 0, x: left, dy: 0.0, cell: 0 }, Stop { byte: length, x: left + formula.width, dy: 0.0, cell: 0 }],
            glyphs: formula
                .glyphs
                .iter()
                .map(|(id, x, y, per_unit)| DrawGlyph { face, id: *id, x: left + x, y: baseline + y, scale: *per_unit, style: 0 })
                .collect(),
            behind: Vec::new(),
            over: formula
                .rules
                .iter()
                .map(|[x0, y0, x1, y1]| RowRect {
                    x0: left + x0,
                    y0: baseline + y0,
                    x1: left + x1,
                    y1: baseline + y1,
                    color: theme.text,
                    outline: false,
                    check: false,
                    dot: false,
                    radius: 0.0,
                    icon: 0,
                })
                .collect(),
            table: None,
            image: None,
        };
        out.insert(first, Arc::new(LineBox { rows: vec![row], styles: vec![style], height, above: 0.0, estimated: false }));
        for line in first + 1..=last {
            let line_start = styles.starts.get(line).copied().unwrap_or(0);
            let line_end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let empty = Row {
                start: 0,
                end: line_end - line_start,
                top: 0.0,
                height: 0.0,
                caret_height: 0.0,
                stops: Vec::new(),
                glyphs: Vec::new(),
                behind: Vec::new(),
                over: Vec::new(),
                table: None,
                image: None,
            };
            out.insert(line, Arc::new(LineBox { rows: vec![empty], styles: Vec::new(), height: 0.0, above: 0.0, estimated: false }));
        }
    }
    (out, previews)
}

/// A math block's formula drawn below its source while the caret is in it: its glyphs and rules
/// from the preview's top (the formula's own style), and its height.
struct Preview {
    glyphs: Vec<DrawGlyph>,
    rules: Vec<[f32; 4]>,
    height: f32,
}

/// `boxed` (a math block's last line) with `preview` drawn below its text, in its own style.
fn with_preview(theme: &Theme, boxed: &LineBox, preview: &Preview) -> LineBox {
    let mut boxed = boxed.clone();
    let style = u16::try_from(boxed.styles.len()).unwrap_or(u16::MAX);
    boxed.styles.push(RunStyle {
        family: Family::Sans,
        weight: fonts::REGULAR,
        size: theme.body_size,
        color: theme.text,
        italic: false,
        strike: false,
        underline: false,
        rise: 0.0,
        background: None,
    });
    if let Some(row) = boxed.rows.last_mut() {
        let below = row.height;
        row.glyphs.extend(preview.glyphs.iter().map(|glyph| DrawGlyph { y: glyph.y + below, style, ..*glyph }));
        row.over.extend(preview.rules.iter().map(|[x0, y0, x1, y1]| RowRect {
            x0: *x0,
            y0: y0 + below,
            x1: *x1,
            y1: y1 + below,
            color: theme.text,
            outline: false,
            check: false,
            dot: false,
            radius: 0.0,
            icon: 0,
        }));
    }
    boxed
}

/// Mermaid blocks drawn as diagrams (first line, last line, the diagram's size): the first line
/// keeps the diagram's room (the editor draws it there), the others none.
fn diagram_blocks(text: &str, styles: &Styles, diagrams: &[(usize, usize, f32, f32)]) -> HashMap<usize, Arc<LineBox>> {
    let mut out = HashMap::new();
    let length_of = |line: usize| {
        let start = styles.starts.get(line).copied().unwrap_or(0);
        styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1)).saturating_sub(start)
    };
    for &(first, last, width, height) in diagrams {
        for line in first..=last {
            let length = length_of(line);
            let (row_height, stops) = if line == first {
                (height, vec![Stop { byte: 0, x: 0.0, dy: 0.0, cell: 0 }, Stop { byte: length, x: width, dy: 0.0, cell: 0 }])
            } else {
                (0.0, Vec::new())
            };
            let row = Row {
                start: 0,
                end: length,
                top: 0.0,
                height: row_height,
                caret_height: row_height,
                stops,
                glyphs: Vec::new(),
                behind: Vec::new(),
                over: Vec::new(),
                table: None,
                image: None,
            };
            out.insert(line, Arc::new(LineBox { rows: vec![row], styles: Vec::new(), height: row_height, above: 0.0, estimated: false }));
        }
    }
    out
}

/// A picture's line: the picture scaled into the column (never above its natural size, at most
/// `theme.image_height` high), with stops at the line's start and end.
fn image_line(theme: &Theme, name: &str, size: (u32, u32), length: usize, width: f32) -> LineBox {
    let (natural_w, natural_h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let scale = (width / natural_w).min(theme.image_height / natural_h).min(1.0);
    let (w, h) = (natural_w * scale, natural_h * scale);
    let pad = 6.0;
    let height = h + 2.0 * pad;
    let rect = RowRect { x0: 0.0, y0: pad, x1: w, y1: pad + h, color: [0, 0, 0, 0], outline: false, check: false, dot: false, radius: 0.0, icon: 0 };
    let caret = theme.line_height_of(Block::Paragraph).min(height);
    let row = Row {
        start: 0,
        end: length,
        top: 0.0,
        height,
        caret_height: caret,
        stops: vec![Stop { byte: 0, x: 0.0, dy: height - caret, cell: 0 }, Stop { byte: length, x: w, dy: height - caret, cell: 0 }],
        glyphs: Vec::new(),
        behind: Vec::new(),
        over: Vec::new(),
        table: None,
        image: Some((name.to_owned(), rect)),
    };
    LineBox { rows: vec![row], styles: Vec::new(), height, above: 0.0, estimated: false }
}

/// A picture in a list item or a quote: the line laid out as text (its bullet, number, or quote
/// bar, its link hidden), with the picture where its text starts, scaled into the rest of the
/// column as `image_line` scales it.
fn image_after_marks(theme: &Theme, laid: &LineBox, name: &str, size: (u32, u32), picture: usize, length: usize, width: f32) -> LineBox {
    let Some(first) = laid.rows.first() else {
        return image_line(theme, name, size, length, width);
    };
    let x = first.stops.iter().find(|stop| stop.byte >= picture).map_or(0.0, |stop| stop.x);
    let (natural_w, natural_h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let scale = ((width - x).max(1.0) / natural_w).min(theme.image_height / natural_h).min(1.0);
    let (w, h) = (natural_w * scale, natural_h * scale);
    let pad = 6.0;
    let height = (h + 2.0 * pad).max(first.height);
    let caret = theme.line_height_of(Block::Paragraph).min(height);
    // What spans the row (a quote's bar) spans the taller row.
    let stretch = |rects: &[RowRect]| -> Vec<RowRect> {
        rects.iter().map(|rect| if rect.y0 <= 0.0 && rect.y1 >= first.height { RowRect { y1: height, ..*rect } } else { *rect }).collect()
    };
    let rect =
        RowRect { x0: x, y0: pad, x1: x + w, y1: pad + h, color: [0, 0, 0, 0], outline: false, check: false, dot: false, radius: 0.0, icon: 0 };
    let row = Row {
        start: 0,
        end: length,
        top: 0.0,
        height,
        caret_height: caret,
        stops: vec![Stop { byte: 0, x, dy: height - caret, cell: 0 }, Stop { byte: length, x: x + w, dy: height - caret, cell: 0 }],
        glyphs: first.glyphs.clone(),
        behind: stretch(&first.behind),
        over: stretch(&first.over),
        table: None,
        image: Some((name.to_owned(), rect)),
    };
    LineBox { rows: vec![row], styles: laid.styles.clone(), height, above: laid.above, estimated: false }
}

/// Where a quote line's marks end: past each `>` and the one space after it.
pub(crate) fn quote_marks_end(text: &str) -> usize {
    let mut at = 0;
    while let Some(rest) = text.get(at..) {
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        if spaces > 3 || rest.get(spaces..spaces + 1) != Some(">") {
            break;
        }
        at += spaces + 1 + usize::from(rest.get(spaces + 1..spaces + 2) == Some(" "));
    }
    at
}

/// Where each whole indent of a line's leading space starts: past a quote's marks (each `>`
/// and the one space after it) when `quoted`, as Obsidian indents a quote's text.
pub(crate) fn leading_unit_starts(text: &str, quoted: bool) -> Vec<usize> {
    let at = if quoted { quote_marks_end(text) } else { 0 };
    indent_unit_starts(text.get(at..).unwrap_or("")).into_iter().map(|start| start + at).collect()
}

/// Where each whole indent (a tab, or four spaces) of `text`'s leading space starts, as
/// Obsidian groups a list line's indent (its `cm-indent` spans, 2.25 em each).
pub(crate) fn indent_unit_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut spaces = 0;
    for (at, c) in text.char_indices() {
        match c {
            '\t' if spaces == 0 => starts.push(at),
            ' ' => {
                spaces += 1;
                if spaces == 4 {
                    starts.push(at - 3);
                    spaces = 0;
                }
            }
            _ => break,
        }
    }
    starts
}

/// The line of a fold in `folded` (heading or list item bytes) that hides line `index`, if one does.
pub(crate) fn folding_line(text: &str, styles: &Styles, folded: &[usize], index: usize) -> Option<usize> {
    let level = |line: usize| match styles.lines.get(line).map(|style| style.block) {
        Some(Block::Heading(level)) => Some(level),
        _ => None,
    };
    folded.iter().map(|at| styles.line_of(*at)).filter(|line| *line < index).find(|line| {
        if let Some(own) = level(*line) {
            (line + 1..=index).all(|next| level(next).is_none_or(|other| other > own))
        } else {
            item_children(text, styles, *line).is_some_and(|last| index <= last)
        }
    })
}

/// Whether line `index` of a list goes on its item's text without a marker of its own (a lazy or
/// indented continuation).
fn continuation(text: &str, styles: &Styles, index: usize) -> bool {
    let start = styles.starts.get(index).copied().unwrap_or(0);
    let end = styles.starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
    let body = text.get(start..end).unwrap_or("").trim_start_matches([' ', '\t', '>']);
    let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker = ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker) || body == marker.trim_end())
        || (digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ") || after == "." || after == ")"));
    !marker
}

/// A line's leading space in columns (a tab is four), or `None` for a blank line.
fn indent_columns(text: &str, styles: &Styles, line: usize) -> Option<usize> {
    let start = styles.starts.get(line).copied()?;
    let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
    let content = text.get(start..end)?.trim_end_matches('\r');
    if content.trim().is_empty() {
        return None;
    }
    Some(content.chars().take_while(|c| *c == ' ' || *c == '\t').map(|c| if c == '\t' { 4 } else { 1 }).sum())
}

/// When line `line` is a list item with children (the lines after it indented deeper, blank lines
/// between them included), the last of them.
pub fn item_children(text: &str, styles: &Styles, line: usize) -> Option<usize> {
    let style = styles.lines.get(line)?;
    if style.list == 0 || style.block != Block::Paragraph {
        return None;
    }
    let start = styles.starts.get(line).copied()?;
    let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
    let body = text.get(start..end)?.trim_start_matches([' ', '\t']);
    let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker = ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker))
        || (digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")));
    if !marker {
        return None;
    }
    let own = indent_columns(text, styles, line)?;
    let mut last = None;
    for next in line + 1..styles.starts.len() {
        match indent_columns(text, styles, next) {
            None => {}
            Some(deeper) if deeper > own && styles.lines.get(next).is_some_and(|style| style.list > 0 || style.block != Block::Paragraph) => {
                last = Some(next)
            }
            Some(_) => break,
        }
    }
    last
}

/// Lays out the whole note: one [`Placed`] per source line, and the note's height. `images` are
/// the image-only lines' pictures that are loaded (line, name, natural size); `diagrams` the
/// Mermaid blocks drawn as diagrams (first line, last line, size); `active` the bytes
/// whose lines show their Markdown (an image line there shows its source).
#[allow(clippy::too_many_arguments)]
pub fn lay_out(
    theme: &Theme,
    text: &str,
    styles: &Styles,
    tables: &[Table],
    images: &[(usize, String, (u32, u32))],
    diagrams: &[(usize, usize, f32, f32)],
    active: &Range<usize>,
    folded: &[usize],
    collapsed: &[Range<usize>],
    properties_folded: bool,
    window: Option<&Range<usize>>,
    unlit: &[(Range<usize>, std::sync::Arc<str>)],
    cache: &mut Cache,
) -> (Vec<Placed>, f32) {
    let mut placed = Vec::with_capacity(styles.starts.len());
    // The lines a folded heading hides: from the next line to before the next heading of the same
    // or a higher level.
    let level = |line: usize| match styles.lines.get(line).map(|style| style.block) {
        Some(Block::Heading(level)) => Some(level),
        _ => None,
    };
    let mut hidden = vec![false; styles.starts.len()];
    // A folded callout's body (`[!kind]-`, the caret elsewhere) is hidden.
    for range in collapsed {
        for line in styles.line_of(range.start)..=styles.line_of(range.end.saturating_sub(1).max(range.start)) {
            if let Some(slot) = hidden.get_mut(line) {
                *slot = true;
            }
        }
    }
    let mut folds = vec![false; styles.starts.len()];
    for at in folded {
        let line = styles.line_of(*at);
        if let Some(own) = level(line) {
            if let Some(slot) = folds.get_mut(line) {
                *slot = true;
            }
            for next in line + 1..styles.starts.len() {
                if level(next).is_some_and(|other| other <= own) {
                    break;
                }
                if let Some(slot) = hidden.get_mut(next) {
                    *slot = true;
                }
            }
        } else if let Some(last) = item_children(text, styles, line) {
            // A folded list item hides its children (the lines under it indented deeper).
            if let Some(slot) = folds.get_mut(line) {
                *slot = true;
            }
            for next in line + 1..=last {
                if let Some(slot) = hidden.get_mut(next) {
                    *slot = true;
                }
            }
        }
    }
    let mut y = theme.top;
    // A table's lines, laid out as one, by their first line.
    let mut table_lines: HashMap<usize, Arc<LineBox>> = HashMap::new();
    for (index, table) in tables.iter().enumerate() {
        // A table outside the window not laid out before gets guessed rows, as a line does.
        let first = styles.line_of(table.range.start);
        let last = styles.line_of(table.range.end.saturating_sub(1).max(table.range.start));
        let near = window.is_none_or(|window| window.start <= last && first < window.end);
        let laid = cache
            .table_if(theme, text, styles, table, index, theme.content_width(), near)
            .unwrap_or_else(|| estimate_table(theme, text, styles, table));
        for (line, boxed) in laid.iter() {
            table_lines.insert(*line, boxed.clone());
        }
    }
    // `$$` math blocks away from the caret draw as their formula, centered, on their first line.
    let (mut math_blocks, previews) = math_blocks(theme, text, styles, active);
    // Mermaid blocks away from the caret keep their diagram's room on their first line.
    math_blocks.extend(diagram_blocks(text, styles, diagrams));
    // The code blocks not highlighted yet, passed in order.
    let mut dark = unlit.iter().map(|(block, _)| block).peekable();
    for (index, start) in styles.starts.iter().enumerate() {
        let end = styles.starts.get(index + 1).map_or(text.len(), |next| next.saturating_sub(1));
        let line = styles.lines.get(index).copied().unwrap_or_default();
        while dark.next_if(|block| block.end <= *start).is_some() {}
        let unlit_line = dark.peek().is_some_and(|block| block.start <= end && *start < block.end);
        if let Some(boxed) = math_blocks.get(&index) {
            let (space, pad_top, pad_bottom) = spacing(styles, text, index, boxed.above, &hidden);
            y += space + pad_top;
            placed.push(Placed { range: *start..end, y, style: line, boxed: boxed.clone(), fold: None, space, pad_top, pad_bottom });
            y += boxed.height + pad_bottom;
            continue;
        }
        // The properties block folded to its heading: its first line keeps the heading's room
        // (8 px padding, the 27 px heading, 16 px to the box's end, then the block's 32 px below,
        // Obsidian's 51.2 px box), its other lines none.
        if properties_folded && line.block == Block::Property {
            let first = index == 0 || styles.lines.get(index - 1).is_none_or(|before| before.block != Block::Property);
            let (pad_top, pad_bottom) = if first { (43.0, 40.2) } else { (0.0, 0.0) };
            let row = Row {
                start: 0,
                end: end - start,
                top: 0.0,
                height: 0.0,
                caret_height: 0.0,
                stops: Vec::new(),
                glyphs: Vec::new(),
                behind: Vec::new(),
                over: Vec::new(),
                table: None,
                image: None,
            };
            y += pad_top;
            placed.push(Placed {
                range: *start..end,
                y,
                style: line,
                boxed: Arc::new(LineBox { rows: vec![row], styles: Vec::new(), height: 0.0, above: 0.0, estimated: false }),
                fold: None,
                space: 0.0,
                pad_top,
                pad_bottom,
            });
            y += pad_bottom;
            continue;
        }
        let picture = images.iter().find(|(line, ..)| *line == index).filter(|_| !(*start <= active.end && end >= active.start));
        // A heading folds, and so does a list item with children (as in Obsidian).
        let fold = (level(index).is_some() || (line.list > 0 && item_children(text, styles, index).is_some()))
            .then(|| folds.get(index).copied().unwrap_or(false));
        if hidden.get(index).copied().unwrap_or(false) {
            let row = Row {
                start: 0,
                end: end - start,
                top: 0.0,
                height: 0.0,
                caret_height: 0.0,
                stops: Vec::new(),
                glyphs: Vec::new(),
                behind: Vec::new(),
                over: Vec::new(),
                table: None,
                image: None,
            };
            placed.push(Placed {
                range: *start..end,
                y,
                style: line,
                boxed: Arc::new(LineBox { rows: vec![row], styles: Vec::new(), height: 0.0, above: 0.0, estimated: false }),
                fold: None,
                space: 0.0,
                pad_top: 0.0,
                pad_bottom: 0.0,
            });
            continue;
        }
        let boxed = match (table_lines.get(&index), picture) {
            (Some(boxed), _) => boxed.clone(),
            (None, Some((_, name, size))) => {
                let content = text.get(*start..end).unwrap_or("");
                let picture = crate::picture_start(content);
                if content.get(..picture).is_some_and(|marks| marks.trim().is_empty()) {
                    Arc::new(image_line(theme, name, *size, end - start, theme.content_width()))
                } else {
                    let flags = styles.bytes.get(*start..end).unwrap_or(&[]);
                    let laid = cache.line(theme, line, content, flags, theme.content_width());
                    Arc::new(image_after_marks(theme, &laid, name, *size, picture, end - start, theme.content_width()))
                }
            }
            (None, None) => {
                let content = text.get(*start..end).unwrap_or("");
                let flags = styles.bytes.get(*start..end).unwrap_or(&[]);
                // Outside the window (a large note's lines far from what is shown), a line not laid
                // out before gets a guessed height; `View::realize` lays it out before it is drawn.
                // A line of a code block not highlighted yet is guessed: its flags lack the tokens.
                if window.is_none_or(|window| window.contains(&index)) && !unlit_line {
                    cache.line(theme, line, content, flags, theme.content_width())
                } else if unlit_line {
                    Arc::new(estimate_line(theme, line, content, flags, theme.content_width()))
                } else {
                    cache
                        .cached(line, content, flags, theme.content_width())
                        .unwrap_or_else(|| Arc::new(estimate_line(theme, line, content, flags, theme.content_width())))
                }
            }
        };
        let (space, pad_top, mut pad_bottom) = spacing(styles, text, index, boxed.above, &hidden);
        // A math block being written: its formula below its last line, in that line's padding.
        let boxed = match previews.get(&index) {
            Some(preview) => {
                pad_bottom += preview.height;
                Arc::new(with_preview(theme, &boxed, preview))
            }
            None => boxed,
        };
        y += space + pad_top;
        placed.push(Placed { range: *start..end, y, style: line, boxed: boxed.clone(), fold, space, pad_top, pad_bottom });
        y += boxed.height + pad_bottom;
    }
    cache.sweep();
    (placed, (y + theme.bottom).max(theme.min_height))
}
