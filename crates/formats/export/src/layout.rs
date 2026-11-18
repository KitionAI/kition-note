//! The reading view laid out on pages: each block of the note placed in the page column, text
//! wrapped at spaces (and between wide characters), pages broken between lines and table rows.
//! The result is a list of marks per page (text, boxes, rules, links) that the pixels
//! (`raster.rs`) and the PDF (`pdf.rs`) both draw, so a preview and an export never disagree.
//!
//! Coordinates are points from the page's top left.

use crate::doc::{Align, Block, Doc, Inline, Item, Located, Runs, Style};
use crate::metrics::{Face, char_width, is_wide, missing_width, text_width};

/// A page size in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

impl PageSize {
    pub const LETTER: PageSize = PageSize { width: 612.0, height: 792.0 };
    pub const A4: PageSize = PageSize { width: 595.28, height: 841.89 };

    /// `letter` or `a4`.
    pub fn named(name: &str) -> Option<PageSize> {
        match name {
            "letter" => Some(PageSize::LETTER),
            "a4" => Some(PageSize::A4),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub size: PageSize,
    pub margin: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self { size: PageSize::LETTER, margin: 72.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

pub const TEXT: Color = Color(26, 26, 26);
pub const MUTED: Color = Color(93, 91, 84);
pub const ACCENT: Color = Color(86, 69, 212);
pub const BORDER: Color = Color(229, 227, 223);
pub const CODE_FILL: Color = Color(246, 245, 244);
pub const MARK_FILL: Color = Color(255, 240, 179);

/// A highlight's fill: a coloured one's as Obsidian draws it (its colour at 30% over white).
fn mark_fill(color: Option<&str>) -> Color {
    match color {
        Some("red") => Color(248, 193, 200),
        Some("orange") => Color(249, 214, 178),
        Some("yellow") => Color(246, 230, 178),
        Some("green") => Color(181, 234, 202),
        Some("blue") => Color(181, 211, 245),
        Some("purple") => Color(215, 203, 250),
        _ => MARK_FILL,
    }
}
pub const HEAD_FILL: Color = Color(250, 250, 249);

/// One thing drawn on a page.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// Text the page fonts draw; `y` is the baseline.
    Text {
        x: f64,
        y: f64,
        size: f64,
        face: Face,
        color: Color,
        text: String,
    },
    /// A character the page fonts have no glyph for, as an outlined box.
    Missing {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Color,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: f64,
        color: Color,
    },
    /// A link's area (the PDF's link annotation; the pixels draw nothing for it).
    Link {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        uri: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub marks: Vec<Mark>,
}

/// A top-level block's place: its page (from 0), its source line (from 1), and its bounds on that page.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub page: usize,
    pub line: usize,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A block wider than the page column: a code line wrapped, a table squeezed.
#[derive(Clone, Debug, PartialEq)]
pub struct Overflow {
    pub page: usize,
    pub line: usize,
    pub kind: &'static str,
    /// The width it needs, in points.
    pub width: f64,
}

/// What the pages could not draw as it is: missing glyphs, and blocks shown as their source or a box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unsupported {
    /// Characters drawn as boxes (no glyph in the page fonts).
    pub missing_glyphs: usize,
    /// Up to 24 distinct missing characters, in order of first use.
    pub missing_sample: String,
    /// Images and embeds drawn as labelled boxes (the engine reads no other file).
    pub images: usize,
    pub embeds: usize,
    /// Math (display and inline) drawn as its source.
    pub math: usize,
    /// Mermaid diagrams drawn as their source.
    pub mermaid: usize,
    /// Raw HTML blocks drawn as their source.
    pub html: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub size: PageSize,
    pub pages: Vec<Page>,
    pub blocks: Vec<Placed>,
    pub overflow: Vec<Overflow>,
    pub unsupported: Unsupported,
    pub title: Option<String>,
}

const BODY: f64 = 11.0;
const LEADING: f64 = 1.45;
const CODE: f64 = 9.5;
const CODE_LEADING: f64 = 1.35;
const PARAGRAPH_GAP: f64 = 8.0;
const INDENT: f64 = 18.0;
const CELL_PAD: f64 = 5.0;

/// One styled piece of a line: text that does not break inside, in one face and size.
#[derive(Clone, Debug)]
struct Seg {
    text: String,
    face: Face,
    size: f64,
    color: Color,
    /// Raised (superscripts, footnote numbers).
    rise: f64,
    code: bool,
    mark: bool,
    /// The highlight's colour: Obsidian's (its colour at 30% over white).
    mark_fill: Color,
    strike: bool,
    underline: bool,
    uri: Option<String>,
    width: f64,
}

/// A breakable unit: a word (one or more segments) and the space after it.
#[derive(Clone, Debug, Default)]
struct Atom {
    segs: Vec<Seg>,
    space: f64,
    /// A hard break after this atom.
    hard: bool,
}

impl Atom {
    fn width(&self) -> f64 {
        self.segs.iter().map(|seg| seg.width).sum()
    }
}

/// A laid-out line: its atoms and its height.
struct Line {
    segs: Vec<(f64, Seg)>,
    height: f64,
    ascent: f64,
}

struct Writer {
    options: Options,
    pages: Vec<Page>,
    y: f64,
    unsupported: Unsupported,
    sample: Vec<char>,
    overflow: Vec<Overflow>,
    /// Measuring only (a table's column widths): nothing is counted as unsupported.
    measuring: bool,
}

/// The text style of a block: its size, weight, and color.
#[derive(Clone, Copy)]
struct Base {
    size: f64,
    bold: bool,
    color: Color,
}

const PLAIN: Base = Base { size: BODY, bold: false, color: TEXT };

impl Writer {
    fn top(&self) -> f64 {
        self.options.margin
    }

    fn bottom(&self) -> f64 {
        self.options.size.height - self.options.margin
    }

    fn page(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }

    /// Draws on the current page.
    fn mark(&mut self, mark: Mark) {
        if self.pages.is_empty() {
            self.pages.push(Page::default());
        }
        if let Some(page) = self.pages.last_mut() {
            page.marks.push(mark);
        }
    }

    fn new_page(&mut self) {
        self.pages.push(Page::default());
        self.y = self.top();
    }

    /// Room for `height` more on this page, or a new page (unless the page is still empty).
    fn ensure(&mut self, height: f64) {
        if self.y + height > self.bottom() && self.y > self.top() + 0.5 {
            self.new_page();
        }
    }

    fn missing(&mut self, c: char) {
        self.unsupported.missing_glyphs += 1;
        if self.sample.len() < 24 && !self.sample.contains(&c) {
            self.sample.push(c);
        }
    }

    /// The atoms of `runs` in the base style.
    fn atoms(&mut self, runs: &Runs, base: Base) -> Vec<Atom> {
        let mut atoms: Vec<Atom> = vec![Atom::default()];
        for inline in runs {
            match inline {
                Inline::Text(text, style) => self.text_atoms(&mut atoms, text, style, base),
                Inline::Soft => push_space(&mut atoms, text_width(Face::Regular, base.size, " ")),
                Inline::Break => {
                    if let Some(last) = atoms.last_mut() {
                        last.hard = true;
                    }
                    atoms.push(Atom::default());
                }
                Inline::Footnote(_, number) => {
                    let style = Style { sup: true, ..Style::default() };
                    self.text_atoms(&mut atoms, &number.to_string(), &style, Base { color: ACCENT, ..base });
                }
                Inline::Image { alt, destination, embed } => {
                    if !self.measuring {
                        if *embed {
                            self.unsupported.embeds += 1;
                        } else {
                            self.unsupported.images += 1;
                        }
                    }
                    let what = if alt.trim().is_empty() || alt == destination { destination.clone() } else { alt.clone() };
                    let label = if *embed { format!("[Embed: {what}]") } else { format!("[Image: {what}]") };
                    let style = Style { italic: true, ..Style::default() };
                    self.text_atoms(&mut atoms, &label, &style, Base { color: MUTED, ..base });
                }
            }
        }
        atoms.retain(|atom| !atom.segs.is_empty() || atom.hard);
        atoms
    }

    fn text_atoms(&mut self, atoms: &mut Vec<Atom>, text: &str, style: &Style, base: Base) {
        if style.math && !self.measuring {
            self.unsupported.math += 1;
        }
        let mono = style.code;
        let face = Face::of(style.bold || base.bold, style.italic, mono);
        let mut size = if mono { base.size * 0.9 } else { base.size };
        let rise = if style.sup { base.size * 0.35 } else { 0.0 };
        if style.sup {
            size *= 0.7;
        }
        let external = style
            .link
            .as_ref()
            .filter(|uri| !style.wiki && (uri.starts_with("http://") || uri.starts_with("https://") || uri.starts_with("mailto:")))
            .cloned();
        let color = if style.link.is_some() || style.tag { ACCENT } else { base.color };
        let make = |text: String| -> Seg {
            let width = text_width(face, size, &text);
            Seg {
                text,
                face,
                size,
                color,
                rise,
                code: mono,
                mark: style.mark,
                mark_fill: mark_fill(style.mark_color),
                strike: style.strike,
                underline: style.link.is_some(),
                uri: external.clone(),
                width,
            }
        };
        let mut word = String::new();
        for c in text.chars() {
            if c == ' ' || c == '\t' || c == '\n' {
                if !word.is_empty() {
                    push_seg(atoms, make(std::mem::take(&mut word)));
                }
                push_space(atoms, text_width(face, size, " "));
            } else if is_wide(c) {
                if !word.is_empty() {
                    push_seg(atoms, make(std::mem::take(&mut word)));
                }
                // A wide character breaks on either side of it.
                if atoms.last().is_some_and(|atom| !atom.segs.is_empty()) {
                    atoms.push(Atom::default());
                }
                push_seg(atoms, make(c.to_string()));
                atoms.push(Atom::default());
            } else {
                word.push(c);
            }
        }
        if !word.is_empty() {
            push_seg(atoms, make(word));
        }
    }

    /// Breaks atoms into lines no wider than `width`; a word wider than the line breaks between characters.
    fn lines(&self, atoms: Vec<Atom>, width: f64) -> Vec<Line> {
        let mut lines: Vec<Line> = Vec::new();
        let mut current: Vec<(f64, Seg)> = Vec::new();
        let mut x = 0.0;
        let finish = |current: &mut Vec<(f64, Seg)>, lines: &mut Vec<Line>| {
            let size = current.iter().map(|(_, seg)| seg.size + seg.rise).fold(0.0_f64, f64::max);
            let size = if size > 0.0 { size } else { BODY };
            lines.push(Line { segs: std::mem::take(current), height: size * LEADING, ascent: size * 0.78 + (LEADING - 1.0) * size / 2.0 });
        };
        for atom in atoms {
            let w = atom.width();
            if x > 0.0 && x + w > width + 0.01 {
                finish(&mut current, &mut lines);
                x = 0.0;
            }
            if w > width + 0.01 {
                // Break the word between characters.
                for seg in atom.segs {
                    let mut piece = String::new();
                    let mut piece_w = 0.0;
                    for c in seg.text.chars() {
                        let cw = char_width(seg.face, c).unwrap_or_else(|| missing_width(c)) * seg.size;
                        if x + piece_w + cw > width + 0.01 && (x > 0.0 || !piece.is_empty()) {
                            if !piece.is_empty() {
                                current.push((x, Seg { text: std::mem::take(&mut piece), width: piece_w, ..seg.clone() }));
                            }
                            finish(&mut current, &mut lines);
                            x = 0.0;
                            piece_w = 0.0;
                        }
                        piece.push(c);
                        piece_w += cw;
                    }
                    if !piece.is_empty() {
                        current.push((x, Seg { text: piece, width: piece_w, ..seg.clone() }));
                        x += piece_w;
                    }
                }
            } else {
                for seg in atom.segs {
                    let sw = seg.width;
                    current.push((x, seg));
                    x += sw;
                }
            }
            x += atom.space;
            if atom.hard {
                finish(&mut current, &mut lines);
                x = 0.0;
            }
        }
        if !current.is_empty() || lines.is_empty() {
            finish(&mut current, &mut lines);
        }
        lines
    }

    /// Draws one line with its left edge at `left` and its top at the current y, then moves down.
    fn draw_line(&mut self, line: &Line, left: f64, offset: f64) {
        let top = self.y;
        let baseline = top + line.ascent;
        for (x, seg) in &line.segs {
            let x = left + offset + x;
            let y = baseline - seg.rise;
            if seg.code {
                self.mark(Mark::Rect { x: x - 1.0, y: y - seg.size * 0.85, w: seg.width + 2.0, h: seg.size * 1.15, fill: CODE_FILL });
            }
            if seg.mark {
                self.mark(Mark::Rect { x, y: y - seg.size * 0.9, w: seg.width, h: seg.size * 1.2, fill: seg.mark_fill });
            }
            self.draw_text(x, y, seg);
            if seg.strike {
                self.mark(Mark::Line { x1: x, y1: y - seg.size * 0.3, x2: x + seg.width, y2: y - seg.size * 0.3, width: 0.7, color: seg.color });
            }
            if seg.underline {
                self.mark(Mark::Line { x1: x, y1: y + 1.5, x2: x + seg.width, y2: y + 1.5, width: 0.5, color: seg.color });
            }
            if let Some(uri) = &seg.uri {
                self.mark(Mark::Link { x, y: y - seg.size, w: seg.width, h: seg.size * 1.3, uri: uri.clone() });
            }
        }
        self.y = top + line.height;
    }

    /// A segment's text: runs the fonts draw as text marks, missing characters as boxes.
    fn draw_text(&mut self, x: f64, y: f64, seg: &Seg) {
        let mut at = x;
        let mut run = String::new();
        let mut run_x = x;
        for c in seg.text.chars() {
            match char_width(seg.face, c) {
                Some(w) => {
                    if run.is_empty() {
                        run_x = at;
                    }
                    run.push(c);
                    at += w * seg.size;
                }
                None => {
                    if !run.is_empty() {
                        let text = std::mem::take(&mut run);
                        self.mark(Mark::Text { x: run_x, y, size: seg.size, face: seg.face, color: seg.color, text });
                    }
                    self.missing(c);
                    let w = missing_width(c) * seg.size;
                    self.mark(Mark::Missing { x: at + w * 0.08, y: y - seg.size * 0.72, w: w * 0.84, h: seg.size * 0.8, color: seg.color });
                    at += w;
                }
            }
        }
        if !run.is_empty() {
            self.mark(Mark::Text { x: run_x, y, size: seg.size, face: seg.face, color: seg.color, text: run });
        }
    }

    /// Runs as wrapped lines in the column `[left, left + width)`.
    fn paragraph(&mut self, runs: &Runs, base: Base, left: f64, width: f64) {
        let atoms = self.atoms(runs, base);
        let lines = self.lines(atoms, width);
        for line in &lines {
            self.ensure(line.height);
            self.draw_line(line, left, 0.0);
        }
    }

    fn blocks(&mut self, blocks: &[Located], left: f64, width: f64, top_level: bool, placed: &mut Vec<Placed>) {
        for (index, located) in blocks.iter().enumerate() {
            if index > 0 {
                self.y += PARAGRAPH_GAP;
                if self.y > self.bottom() {
                    self.new_page();
                }
            }
            let (page, y) = (self.page(), self.y);
            let kind = self.block(located, left, width);
            if top_level {
                let page_now = self.page();
                let bottom = if page_now == page { self.y } else { self.bottom() };
                placed.push(Placed { page, line: located.line, kind, x: left, y, w: width, h: (bottom - y).max(0.0) });
            }
        }
    }

    /// Draws one block; answers its kind.
    fn block(&mut self, located: &Located, left: f64, width: f64) -> &'static str {
        match &located.block {
            Block::Heading { level, runs, .. } => {
                let size = match level {
                    1 => 22.0,
                    2 => 17.0,
                    3 => 14.0,
                    4 => 12.5,
                    5 => 11.5,
                    _ => 11.0,
                };
                self.y += size * 0.35;
                // A heading stays with the line after it.
                self.ensure(size * LEADING + BODY * LEADING);
                self.paragraph(runs, Base { size, bold: true, color: TEXT }, left, width);
                "heading"
            }
            Block::Paragraph { runs, .. } => {
                self.paragraph(runs, PLAIN, left, width);
                "paragraph"
            }
            Block::Code { info, text } => {
                if info.eq_ignore_ascii_case("mermaid") {
                    self.unsupported.mermaid += 1;
                }
                self.source(text, left, width, located.line);
                if info.eq_ignore_ascii_case("mermaid") { "mermaid" } else { "code" }
            }
            Block::Math(text) => {
                self.unsupported.math += 1;
                self.source(text, left, width, located.line);
                "math"
            }
            Block::Html(text) => {
                self.unsupported.html += 1;
                self.source(text.trim_end(), left, width, located.line);
                "html"
            }
            Block::Rule => {
                self.ensure(12.0);
                let y = self.y + 6.0;
                self.mark(Mark::Line { x1: left, y1: y, x2: left + width, y2: y, width: 0.75, color: BORDER });
                self.y += 12.0;
                "rule"
            }
            Block::Quote { callout, blocks } => {
                let (page, y) = (self.page(), self.y);
                let bar = if callout.is_some() { ACCENT } else { BORDER };
                let inner = left + 12.0;
                if let Some(callout) = callout {
                    let title = if callout.title.is_empty() { capitalised(&callout.kind) } else { callout.title.clone() };
                    let runs = vec![Inline::Text(title, Style { bold: true, ..Style::default() })];
                    self.paragraph(&runs, Base { color: ACCENT, ..PLAIN }, inner, width - 12.0);
                    if !blocks.is_empty() {
                        self.y += 4.0;
                    }
                }
                let mut ignore = Vec::new();
                self.blocks_muted(blocks, inner, width - 12.0, &mut ignore);
                self.bar(page, y, left, bar);
                "quote"
            }
            Block::List { start, items } => {
                self.list(*start, items, left, width);
                "list"
            }
            Block::Table { aligns, rows, head } => {
                self.table(aligns, rows, *head, left, width, located.line);
                "table"
            }
        }
    }

    /// Blocks inside a quote: drawn as usual (the quote's bar marks them).
    fn blocks_muted(&mut self, blocks: &[Located], left: f64, width: f64, placed: &mut Vec<Placed>) {
        self.blocks(blocks, left, width, false, placed);
    }

    /// A quote's bar from (`page`, `y`) to the current position, one piece per page.
    fn bar(&mut self, page: usize, y: f64, x: f64, color: Color) {
        let end_page = self.page();
        for at in page..=end_page {
            let from = if at == page { y } else { self.top() };
            let to = if at == end_page { self.y } else { self.bottom() };
            if let Some(target) = self.pages.get_mut(at) {
                target.marks.push(Mark::Rect { x, y: from, w: 3.0, h: (to - from).max(0.0), fill: color });
            }
        }
    }

    /// Source text (code, math, Mermaid, HTML) in a monospace box, wrapped by characters.
    fn source(&mut self, text: &str, left: f64, width: f64, line: usize) {
        let pad = 6.0;
        let inner = width - 2.0 * pad;
        let height = CODE * CODE_LEADING;
        let text = text.strip_suffix('\n').unwrap_or(text);
        let mut wrapped = false;
        let mut widest: f64 = 0.0;
        let mut rows: Vec<String> = Vec::new();
        for source_line in text.split('\n') {
            let source_line = source_line.strip_suffix('\r').unwrap_or(source_line).replace('\t', "    ");
            let full = text_width(Face::Mono, CODE, &source_line);
            widest = widest.max(full);
            if full <= inner + 0.01 {
                rows.push(source_line);
                continue;
            }
            wrapped = true;
            let mut piece = String::new();
            let mut w = 0.0;
            for c in source_line.chars() {
                let cw = char_width(Face::Mono, c).unwrap_or_else(|| missing_width(c)) * CODE;
                if w + cw > inner + 0.01 && !piece.is_empty() {
                    rows.push(std::mem::take(&mut piece));
                    w = 0.0;
                }
                piece.push(c);
                w += cw;
            }
            rows.push(piece);
        }
        if wrapped {
            self.overflow.push(Overflow { page: self.page(), line, kind: "code", width: widest + 2.0 * pad });
        }
        self.ensure(height + 2.0 * pad);
        let mut box_top = self.y;
        let mut box_page = self.page();
        self.y += pad;
        for row in rows {
            if self.y + height > self.bottom() - pad {
                self.code_fill(box_page, box_top, left, width, self.y + pad);
                self.new_page();
                box_top = self.y;
                box_page = self.page();
                self.y += pad;
            }
            let seg = Seg {
                width: text_width(Face::Mono, CODE, &row),
                text: row,
                face: Face::Mono,
                size: CODE,
                color: TEXT,
                rise: 0.0,
                code: false,
                mark: false,
                mark_fill: MARK_FILL,
                strike: false,
                underline: false,
                uri: None,
            };
            let baseline = self.y + CODE * 0.85;
            self.draw_text(left + pad, baseline, &seg);
            self.y += height;
        }
        self.y += pad;
        self.code_fill(box_page, box_top, left, width, self.y);
    }

    /// A code box's background, put under the text already drawn on its page.
    fn code_fill(&mut self, page: usize, top: f64, left: f64, width: f64, bottom: f64) {
        if let Some(target) = self.pages.get_mut(page) {
            target.marks.insert(0, Mark::Rect { x: left, y: top, w: width, h: (bottom - top).max(0.0), fill: CODE_FILL });
        }
    }

    fn list(&mut self, start: Option<u64>, items: &[Item], left: f64, width: f64) {
        let inner = left + INDENT;
        let mut ignore = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                self.y += 2.0;
            }
            self.ensure(BODY * LEADING);
            let marker_y = self.y + BODY * 0.78 + (LEADING - 1.0) * BODY / 2.0;
            match (item.task, start) {
                (Some(done), _) => {
                    let (x, y, s) = (left + 3.0, marker_y - BODY * 0.72, BODY * 0.8);
                    for (x1, y1, x2, y2) in [(x, y, x + s, y), (x + s, y, x + s, y + s), (x + s, y + s, x, y + s), (x, y + s, x, y)] {
                        self.mark(Mark::Line { x1, y1, x2, y2, width: 0.8, color: MUTED });
                    }
                    if done {
                        let seg = Seg {
                            width: 0.0,
                            text: "x".into(),
                            face: Face::Bold,
                            size: BODY * 0.8,
                            color: ACCENT,
                            rise: 0.0,
                            code: false,
                            mark: false,
                            mark_fill: MARK_FILL,
                            strike: false,
                            underline: false,
                            uri: None,
                        };
                        self.draw_text(x + s * 0.22, marker_y - BODY * 0.06, &seg);
                    }
                }
                (None, Some(first)) => {
                    let number = first.saturating_add(u64::try_from(index).unwrap_or(u64::MAX));
                    let label = format!("{number}.");
                    let w = text_width(Face::Regular, BODY, &label);
                    let seg = Seg {
                        width: w,
                        text: label,
                        face: Face::Regular,
                        size: BODY,
                        color: MUTED,
                        rise: 0.0,
                        code: false,
                        mark: false,
                        mark_fill: MARK_FILL,
                        strike: false,
                        underline: false,
                        uri: None,
                    };
                    self.draw_text(inner - 4.0 - w, marker_y, &seg);
                }
                (None, None) => {
                    let seg = Seg {
                        width: 0.0,
                        text: "\u{2022}".into(),
                        face: Face::Regular,
                        size: BODY,
                        color: MUTED,
                        rise: 0.0,
                        code: false,
                        mark: false,
                        mark_fill: MARK_FILL,
                        strike: false,
                        underline: false,
                        uri: None,
                    };
                    self.draw_text(left + 5.0, marker_y, &seg);
                }
            }
            if item.blocks.is_empty() {
                self.y += BODY * LEADING;
            } else {
                self.blocks(&item.blocks, inner, width - INDENT, false, &mut ignore);
            }
        }
    }

    fn table(&mut self, aligns: &[Align], rows: &[Vec<Runs>], head: usize, left: f64, width: f64, line: usize) {
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return;
        }
        // Each column's natural width (its widest cell on one line) and least width (its widest word).
        let mut natural = vec![0.0_f64; columns];
        let mut least = vec![0.0_f64; columns];
        self.measuring = true;
        for (r, row) in rows.iter().enumerate() {
            let base = Base { bold: r < head, ..PLAIN };
            for (c, cell) in row.iter().enumerate() {
                let atoms = self.atoms(cell, base);
                let total: f64 = atoms.iter().map(|atom| atom.width() + atom.space).sum();
                let word = atoms.iter().map(Atom::width).fold(0.0_f64, f64::max);
                if let Some(slot) = natural.get_mut(c) {
                    *slot = slot.max(total + 2.0 * CELL_PAD);
                }
                if let Some(slot) = least.get_mut(c) {
                    *slot = slot.max(word.min(width / 2.0) + 2.0 * CELL_PAD);
                }
            }
        }
        self.measuring = false;
        let natural_sum: f64 = natural.iter().sum();
        let least_sum: f64 = least.iter().sum();
        let widths: Vec<f64> = if natural_sum <= width {
            natural.clone()
        } else if least_sum <= width {
            let spare = width - least_sum;
            let want: f64 = natural.iter().zip(&least).map(|(n, l)| n - l).sum();
            natural.iter().zip(&least).map(|(n, l)| l + if want > 0.0 { spare * (n - l) / want } else { 0.0 }).collect()
        } else {
            self.overflow.push(Overflow { page: self.page(), line, kind: "table", width: least_sum });
            least.iter().map(|l| l * width / least_sum).collect()
        };
        let table_width: f64 = widths.iter().sum();
        for (r, row) in rows.iter().enumerate() {
            let base = Base { bold: r < head, ..PLAIN };
            let laid: Vec<Vec<Line>> = (0..columns)
                .map(|c| {
                    let cell_width = widths.get(c).copied().unwrap_or(0.0) - 2.0 * CELL_PAD;
                    let atoms = row.get(c).map(|cell| self.atoms(cell, base)).unwrap_or_default();
                    self.lines(atoms, cell_width.max(1.0))
                })
                .collect();
            let height = laid.iter().map(|lines| lines.iter().map(|line| line.height).sum::<f64>()).fold(0.0_f64, f64::max) + 2.0 * CELL_PAD;
            self.ensure(height);
            let top = self.y;
            if r < head {
                self.mark(Mark::Rect { x: left, y: top, w: table_width, h: height, fill: HEAD_FILL });
            }
            let mut x = left;
            for (c, lines) in laid.iter().enumerate() {
                let cell_width = widths.get(c).copied().unwrap_or(0.0);
                self.y = top + CELL_PAD;
                for line in lines {
                    let used = line.segs.last().map(|(x, seg)| x + seg.width).unwrap_or(0.0);
                    let free = (cell_width - 2.0 * CELL_PAD - used).max(0.0);
                    let offset = match aligns.get(c).copied().unwrap_or(Align::None) {
                        Align::Center => free / 2.0,
                        Align::Right => free,
                        Align::None | Align::Left => 0.0,
                    };
                    self.draw_line(line, x + CELL_PAD, offset);
                }
                x += cell_width;
            }
            // The grid: a rule under the row, and the column edges.
            let bottom = top + height;
            if r == 0 {
                self.mark(Mark::Line { x1: left, y1: top, x2: left + table_width, y2: top, width: 0.6, color: BORDER });
            }
            self.mark(Mark::Line {
                x1: left,
                y1: bottom,
                x2: left + table_width,
                y2: bottom,
                width: if r + 1 == head { 0.9 } else { 0.6 },
                color: BORDER,
            });
            let mut edge = left;
            for c in 0..=columns {
                self.mark(Mark::Line { x1: edge, y1: top, x2: edge, y2: bottom, width: 0.6, color: BORDER });
                edge += widths.get(c).copied().unwrap_or(0.0);
            }
            self.y = bottom;
        }
    }

    fn footnotes(&mut self, doc: &Doc, left: f64, width: f64) {
        if doc.footnotes.is_empty() {
            return;
        }
        self.y += 14.0;
        self.ensure(20.0);
        self.mark(Mark::Line { x1: left, y1: self.y, x2: left + width / 3.0, y2: self.y, width: 0.6, color: BORDER });
        self.y += 6.0;
        let mut ignore = Vec::new();
        for (_, number, content) in &doc.footnotes {
            self.ensure(BODY * LEADING);
            let label = format!("{number}.");
            let w = text_width(Face::Regular, 9.0, &label);
            let seg = Seg {
                width: w,
                text: label,
                face: Face::Regular,
                size: 9.0,
                color: MUTED,
                rise: 0.0,
                code: false,
                mark: false,
                mark_fill: MARK_FILL,
                strike: false,
                underline: false,
                uri: None,
            };
            let baseline = self.y + 9.0 * 0.78 + (LEADING - 1.0) * 9.0 / 2.0;
            self.draw_text(left, baseline, &seg);
            self.blocks(content, left + INDENT, width - INDENT, false, &mut ignore);
            self.y += 2.0;
        }
    }
}

fn push_seg(atoms: &mut Vec<Atom>, seg: Seg) {
    if atoms.last().is_none_or(|atom| atom.space > 0.0 || atom.hard) {
        atoms.push(Atom::default());
    }
    if let Some(atom) = atoms.last_mut() {
        atom.segs.push(seg);
    }
}

fn push_space(atoms: &mut [Atom], width: f64) {
    if let Some(atom) = atoms.last_mut() {
        atom.space += width;
    }
}

/// Joins text marks one space apart on one baseline in one face, size and color into one run.
fn merge_text(marks: &mut Vec<Mark>) {
    let mut out: Vec<Mark> = Vec::with_capacity(marks.len());
    for mark in marks.drain(..) {
        if let (
            Some(Mark::Text { x, y, size, face, color, text }),
            Mark::Text { x: next_x, y: next_y, size: next_size, face: next_face, color: next_color, text: next_text },
        ) = (out.last_mut(), &mark)
            && (*y - next_y).abs() < 0.001
            && *face == *next_face
            && (*size - next_size).abs() < 0.001
            && *color == *next_color
        {
            let joined = format!("{text} ");
            if (*x + text_width(*face, *size, &joined) - next_x).abs() < 0.01 {
                *text = joined + next_text;
                continue;
            }
        }
        out.push(mark);
    }
    *marks = out;
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Lays out a note's blocks on pages.
pub fn layout(doc: &Doc, options: Options) -> Layout {
    let mut writer = Writer {
        options,
        pages: vec![Page::default()],
        y: options.margin,
        unsupported: Unsupported::default(),
        sample: Vec::new(),
        overflow: Vec::new(),
        measuring: false,
    };
    let left = options.margin;
    let width = (options.size.width - 2.0 * options.margin).max(72.0);
    let mut placed = Vec::new();
    writer.blocks(&doc.blocks, left, width, true, &mut placed);
    writer.footnotes(doc, left, width);
    for page in &mut writer.pages {
        merge_text(&mut page.marks);
    }
    let mut unsupported = writer.unsupported;
    unsupported.missing_sample = writer.sample.iter().collect();
    Layout { size: options.size, pages: writer.pages, blocks: placed, overflow: writer.overflow, unsupported, title: doc.title.clone() }
}
