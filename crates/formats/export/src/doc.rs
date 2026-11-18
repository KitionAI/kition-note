//! The note as reading-view blocks: what the page layout, the pixels, the PDF and the HTML draw.
//!
//! Built from pulldown-cmark's events with the note's Obsidian options. The Obsidian syntax
//! pulldown-cmark does not know is read here: `%%comments%%` are dropped (blanked in place, so
//! source lines stay where they were), `==highlights==`, `#tags`, block ids (` ^id` at the end of
//! a block) and callouts (`> [!kind] Title`). The frontmatter is not drawn.

use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};

/// Obsidian 1.14's highlight colours: the emoji written after a highlight's `==` and the colour's
/// name (its reading view's `data-highlight`).
pub const HIGHLIGHT_COLORS: [(&str, &str); 6] =
    [("🔴", "red"), ("🟠", "orange"), ("🟡", "yellow"), ("🟢", "green"), ("🔵", "blue"), ("🟣", "purple")];

/// How a run of text is drawn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub mark: bool,
    /// A coloured highlight's colour (`==🔴text==`: `red`), as Obsidian 1.14 names it.
    pub mark_color: Option<&'static str>,
    pub math: bool,
    pub tag: bool,
    pub sup: bool,
    /// Raw inline HTML: kept as it is in HTML, drawn as its source on pages.
    pub raw: bool,
    /// A link's destination; `wiki` when it is a wikilink (a note in the workspace).
    pub link: Option<String>,
    pub wiki: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String, Style),
    /// A hard line break.
    Break,
    /// A soft line break: a space on pages.
    Soft,
    /// A footnote reference: its label and its number in the note.
    Footnote(String, usize),
    /// An image or an embed (`![[...]]`): drawn as a labelled box, never read (the engine reads no
    /// other file).
    Image {
        alt: String,
        destination: String,
        embed: bool,
    },
}

pub type Runs = Vec<Inline>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callout {
    pub kind: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub task: Option<bool>,
    pub blocks: Vec<Located>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        runs: Runs,
        id: Option<String>,
    },
    Paragraph {
        runs: Runs,
        id: Option<String>,
    },
    Code {
        info: String,
        text: String,
    },
    Quote {
        callout: Option<Callout>,
        blocks: Vec<Located>,
    },
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Table {
        aligns: Vec<Align>,
        rows: Vec<Vec<Runs>>,
        head: usize,
    },
    Rule,
    /// Display math, drawn as its source (no math typesetting in the engine).
    Math(String),
    /// Raw HTML: passed through in HTML, drawn as its source on pages.
    Html(String),
}

/// A block and the line of the note it starts on (from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    pub line: usize,
    pub block: Block,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub blocks: Vec<Located>,
    /// Footnote definitions in reference order: label, number, content.
    pub footnotes: Vec<(String, usize, Vec<Located>)>,
    /// The first heading's text, for a title.
    pub title: Option<String>,
}

/// One open container while the events are read.
enum Frame {
    Root(Vec<Located>),
    Quote {
        kind: Option<BlockQuoteKind>,
        blocks: Vec<Located>,
        line: usize,
    },
    List {
        start: Option<u64>,
        items: Vec<Item>,
        line: usize,
    },
    Item {
        task: Option<bool>,
        blocks: Vec<Located>,
    },
    Footnote {
        label: String,
        blocks: Vec<Located>,
    },
    /// A paragraph; `implicit` for a tight list item's text, which has no paragraph tags.
    Paragraph {
        runs: Runs,
        line: usize,
        implicit: bool,
    },
    Heading {
        level: u8,
        runs: Runs,
        line: usize,
    },
    Table {
        aligns: Vec<Align>,
        rows: Vec<Vec<Runs>>,
        head: usize,
        line: usize,
    },
    Cell(Runs),
    Code {
        info: String,
        text: String,
        line: usize,
    },
    Html {
        text: String,
        line: usize,
    },
    /// The frontmatter's text, not drawn.
    Metadata,
    /// An image's alt text being read.
    Image {
        alt: String,
        destination: String,
        embed: bool,
        line: usize,
    },
}

/// The 1-based line of a byte offset.
struct Lines(Vec<usize>);

impl Lines {
    fn new(text: &str) -> Self {
        Self(text.match_indices('\n').map(|(at, _)| at).collect())
    }

    fn line(&self, at: usize) -> usize {
        match self.0.binary_search(&at) {
            Ok(index) | Err(index) => index + 1,
        }
    }
}

/// `text` with every `%%comment%%` blanked (each character a space, line breaks kept).
fn blank_comments(text: &str) -> String {
    if !text.contains("%%") {
        return text.to_owned();
    }
    let ix = kition_note_markdown::index(text);
    if ix.comments.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for range in &ix.comments {
        if range.start < at {
            continue;
        }
        out.push_str(text.get(at..range.start).unwrap_or_default());
        for c in text.get(range.clone()).unwrap_or_default().chars() {
            out.push(if c == '\n' || c == '\r' { c } else { ' ' });
        }
        at = range.end;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}

struct Builder {
    stack: Vec<Frame>,
    style: Vec<Style>,
    /// Inside `==...==`, and its colour (`==🔴...==`).
    mark: bool,
    mark_color: Option<&'static str>,
    footnotes: Vec<String>,
    definitions: Vec<(String, Vec<Located>)>,
}

impl Builder {
    fn style(&self) -> Style {
        let mut style = self.style.last().cloned().unwrap_or_default();
        style.mark = self.mark;
        style.mark_color = if self.mark { self.mark_color } else { None };
        style
    }

    fn push_style(&mut self, change: impl FnOnce(&mut Style)) {
        let mut style = self.style.last().cloned().unwrap_or_default();
        change(&mut style);
        self.style.push(style);
    }

    /// Where blocks go now.
    fn blocks(&mut self) -> Option<&mut Vec<Located>> {
        for frame in self.stack.iter_mut().rev() {
            match frame {
                Frame::Root(blocks) | Frame::Quote { blocks, .. } | Frame::Item { blocks, .. } | Frame::Footnote { blocks, .. } => {
                    return Some(blocks);
                }
                _ => {}
            }
        }
        None
    }

    fn add(&mut self, line: usize, block: Block) {
        if let Some(blocks) = self.blocks() {
            blocks.push(Located { line, block });
        }
    }

    /// Closes a tight item's implicit paragraph before a block starts or its item ends.
    fn close_implicit(&mut self) {
        if matches!(self.stack.last(), Some(Frame::Paragraph { implicit: true, .. })) {
            self.close();
        }
    }

    /// The runs inline content goes to: the open paragraph, heading or cell, or a new implicit paragraph.
    fn runs(&mut self, line: usize) -> Option<&mut Runs> {
        let needs_paragraph =
            !matches!(self.stack.last(), Some(Frame::Paragraph { .. } | Frame::Heading { .. } | Frame::Cell(_) | Frame::Image { .. }));
        if needs_paragraph {
            self.stack.push(Frame::Paragraph { runs: Vec::new(), line, implicit: true });
        }
        match self.stack.last_mut() {
            Some(Frame::Paragraph { runs, .. } | Frame::Heading { runs, .. } | Frame::Cell(runs)) => Some(runs),
            _ => None,
        }
    }

    fn inline(&mut self, line: usize, inline: Inline) {
        if let Some(Frame::Image { alt, .. }) = self.stack.last_mut() {
            if let Inline::Text(text, _) = &inline {
                alt.push_str(text);
            }
            return;
        }
        if let Some(runs) = self.runs(line) {
            runs.push(inline);
        }
    }

    /// Text with `==highlights==` and `#tags` read.
    fn text(&mut self, line: usize, text: &str) {
        if matches!(self.stack.last(), Some(Frame::Metadata)) {
            return;
        }
        if let Some(Frame::Code { text: code, .. } | Frame::Html { text: code, .. }) = self.stack.last_mut() {
            code.push_str(text);
            return;
        }
        let base = self.style();
        if base.code || base.link.is_some() {
            self.inline(line, Inline::Text(text.to_owned(), base));
            return;
        }
        let mut rest = text;
        loop {
            let (piece, toggle) = match rest.find("==") {
                Some(at) => (rest.get(..at).unwrap_or_default(), true),
                None => (rest, false),
            };
            self.tagged(line, piece);
            if !toggle {
                break;
            }
            self.mark = !self.mark;
            rest = rest.get(piece.len() + 2..).unwrap_or_default();
            // Obsidian 1.14's coloured highlight: a colour's emoji right after the opening `==`.
            self.mark_color = None;
            if self.mark
                && let Some((emoji, name)) = HIGHLIGHT_COLORS.iter().find(|(emoji, _)| rest.starts_with(*emoji))
            {
                self.mark_color = Some(name);
                rest = rest.get(emoji.len()..).unwrap_or_default();
            }
        }
    }

    /// A piece of text, with `#tag` words styled as tags.
    fn tagged(&mut self, line: usize, piece: &str) {
        if piece.is_empty() {
            return;
        }
        let style = self.style();
        let mut plain = String::new();
        let mut chars = piece.char_indices().peekable();
        let mut previous: Option<char> = None;
        while let Some((at, c)) = chars.next() {
            let starts = c == '#' && previous.is_none_or(char::is_whitespace);
            if starts {
                let name: String =
                    piece.get(at + 1..).unwrap_or_default().chars().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/')).collect();
                if !name.is_empty() && !name.chars().all(|c| c.is_ascii_digit()) {
                    if !plain.is_empty() {
                        self.inline(line, Inline::Text(std::mem::take(&mut plain), style.clone()));
                    }
                    let mut tag = style.clone();
                    tag.tag = true;
                    self.inline(line, Inline::Text(format!("#{name}"), tag));
                    for _ in 0..name.chars().count() {
                        chars.next();
                    }
                    previous = name.chars().last();
                    continue;
                }
            }
            plain.push(c);
            previous = Some(c);
        }
        if !plain.is_empty() {
            self.inline(line, Inline::Text(plain, style));
        }
    }

    fn footnote_number(&mut self, label: &str) -> usize {
        if let Some(at) = self.footnotes.iter().position(|known| known == label) {
            return at + 1;
        }
        self.footnotes.push(label.to_owned());
        self.footnotes.len()
    }

    /// Closes the top frame and puts what it built where it belongs.
    fn close(&mut self) {
        let Some(frame) = self.stack.pop() else { return };
        match frame {
            Frame::Paragraph { mut runs, line, .. } => {
                self.mark = false;
                self.mark_color = None;
                let id = take_block_id(&mut runs);
                if !runs.is_empty() || id.is_some() {
                    self.add(line, Block::Paragraph { runs, id });
                }
            }
            Frame::Heading { level, mut runs, line } => {
                self.mark = false;
                self.mark_color = None;
                let id = take_block_id(&mut runs);
                self.add(line, Block::Heading { level, runs, id });
            }
            Frame::Quote { kind, mut blocks, line } => {
                let callout = callout_of(kind, &mut blocks);
                self.add(line, Block::Quote { callout, blocks });
            }
            Frame::List { start, items, line } => self.add(line, Block::List { start, items }),
            Frame::Item { task, blocks } => {
                if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                    items.push(Item { task, blocks });
                }
            }
            Frame::Footnote { label, blocks } => self.definitions.push((label, blocks)),
            Frame::Table { aligns, rows, head, line } => self.add(line, Block::Table { aligns, rows, head }),
            Frame::Cell(runs) => {
                if let Some(Frame::Table { rows, .. }) = self.stack.last_mut()
                    && let Some(row) = rows.last_mut()
                {
                    row.push(runs);
                }
            }
            Frame::Code { info, text, line } => {
                let block = if info.trim() == "math" { Block::Math(text) } else { Block::Code { info, text } };
                self.add(line, block);
            }
            Frame::Html { text, line } => self.add(line, Block::Html(text)),
            Frame::Image { alt, destination, embed, line } => self.inline(line, Inline::Image { alt, destination, embed }),
            Frame::Metadata | Frame::Root(_) => {}
        }
    }
}

/// A trailing ` ^id` of a block's last text, taken off.
fn take_block_id(runs: &mut Runs) -> Option<String> {
    let Some(Inline::Text(text, style)) = runs.last_mut() else { return None };
    if style.code {
        return None;
    }
    let trimmed = text.trim_end();
    let caret = trimmed.rfind('^')?;
    let id = trimmed.get(caret + 1..)?;
    let before = trimmed.get(..caret)?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || !(before.is_empty() || before.ends_with(char::is_whitespace)) {
        return None;
    }
    let id = id.to_owned();
    let kept = before.trim_end().to_owned();
    if kept.is_empty() {
        runs.pop();
    } else {
        *text = kept;
    }
    Some(id)
}

/// A quote's callout: a GFM alert kind, or Obsidian's `[!kind] Title` opening its first paragraph.
fn callout_of(kind: Option<BlockQuoteKind>, blocks: &mut [Located]) -> Option<Callout> {
    if let Some(kind) = kind {
        let name = match kind {
            BlockQuoteKind::Note => "note",
            BlockQuoteKind::Tip => "tip",
            BlockQuoteKind::Important => "important",
            BlockQuoteKind::Warning => "warning",
            BlockQuoteKind::Caution => "caution",
        };
        return Some(Callout { kind: name.into(), title: String::new() });
    }
    let Some(Located { block: Block::Paragraph { runs, .. }, .. }) = blocks.first_mut() else { return None };
    // The marker may arrive as several text events (`[`, `!info`, `]`).
    let mut head = String::new();
    let mut used = 0;
    for inline in runs.iter() {
        match inline {
            Inline::Text(text, style) if !style.code => {
                head.push_str(text);
                used += 1;
                if head.contains(']') {
                    break;
                }
            }
            _ => break,
        }
    }
    let rest = head.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let kind = rest.get(..close)?.trim().to_ascii_lowercase();
    if kind.is_empty() || !kind.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return None;
    }
    let after = rest.get(close + 1..)?.trim_start_matches(['+', '-']);
    // The title is the rest of the first line; a line break ends it.
    let mut title = after.trim().to_owned();
    runs.drain(..used);
    while let Some(Inline::Text(text, _)) = runs.first() {
        title.push_str(text);
        runs.remove(0);
    }
    if matches!(runs.first(), Some(Inline::Break | Inline::Soft)) {
        runs.remove(0);
    }
    Some(Callout { kind, title: title.trim().to_owned() })
}

fn align(alignment: &Alignment) -> Align {
    match alignment {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

/// Reads a note's text into blocks.
pub fn read(text: &str) -> Doc {
    let source = blank_comments(text);
    let lines = Lines::new(&source);
    let mut b = Builder {
        stack: vec![Frame::Root(Vec::new())],
        style: Vec::new(),
        mark: false,
        mark_color: None,
        footnotes: Vec::new(),
        definitions: Vec::new(),
    };
    for (event, range) in Parser::new_ext(&source, kition_note_markdown::options()).into_offset_iter() {
        let line = lines.line(range.start);
        match event {
            Event::Start(tag) => {
                let block_level = matches!(
                    tag,
                    Tag::Paragraph
                        | Tag::Heading { .. }
                        | Tag::BlockQuote(_)
                        | Tag::CodeBlock(_)
                        | Tag::HtmlBlock
                        | Tag::List(_)
                        | Tag::Table(_)
                        | Tag::FootnoteDefinition(_)
                        | Tag::MetadataBlock(_)
                        | Tag::DefinitionList
                );
                if block_level {
                    b.close_implicit();
                }
                match tag {
                    Tag::Paragraph | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                        b.stack.push(Frame::Paragraph { runs: Vec::new(), line, implicit: false })
                    }
                    Tag::Heading { level, .. } => b.stack.push(Frame::Heading { level: level as u8, runs: Vec::new(), line }),
                    Tag::BlockQuote(kind) => b.stack.push(Frame::Quote { kind, blocks: Vec::new(), line }),
                    Tag::CodeBlock(kind) => {
                        let info = match kind {
                            CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or_default().to_owned(),
                            CodeBlockKind::Indented => String::new(),
                        };
                        b.stack.push(Frame::Code { info, text: String::new(), line });
                    }
                    Tag::HtmlBlock => b.stack.push(Frame::Html { text: String::new(), line }),
                    Tag::List(start) => b.stack.push(Frame::List { start, items: Vec::new(), line }),
                    Tag::Item => b.stack.push(Frame::Item { task: None, blocks: Vec::new() }),
                    Tag::FootnoteDefinition(label) => b.stack.push(Frame::Footnote { label: label.to_string(), blocks: Vec::new() }),
                    Tag::Table(aligns) => b.stack.push(Frame::Table { aligns: aligns.iter().map(align).collect(), rows: Vec::new(), head: 0, line }),
                    Tag::TableHead | Tag::TableRow => {
                        let head = matches!(tag, Tag::TableHead);
                        if let Some(Frame::Table { rows, head: heads, .. }) = b.stack.last_mut() {
                            rows.push(Vec::new());
                            if head {
                                *heads += 1;
                            }
                        }
                    }
                    Tag::TableCell => b.stack.push(Frame::Cell(Vec::new())),
                    Tag::Emphasis => b.push_style(|style| style.italic = true),
                    Tag::Strong => b.push_style(|style| style.bold = true),
                    Tag::Strikethrough => b.push_style(|style| style.strike = true),
                    Tag::Superscript => b.push_style(|style| style.sup = true),
                    Tag::Subscript => b.push_style(|_| {}),
                    Tag::Link { link_type, dest_url, .. } => {
                        let wiki = matches!(link_type, LinkType::WikiLink { .. });
                        b.push_style(|style| {
                            style.link = Some(dest_url.to_string());
                            style.wiki = wiki;
                        });
                    }
                    Tag::Image { link_type, dest_url, .. } => {
                        let embed = matches!(link_type, LinkType::WikiLink { .. });
                        b.stack.push(Frame::Image { alt: String::new(), destination: dest_url.to_string(), embed, line });
                    }
                    Tag::MetadataBlock(_) => b.stack.push(Frame::Metadata),
                    Tag::DefinitionList => {}
                }
            }
            Event::End(end) => match end {
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Superscript | TagEnd::Subscript | TagEnd::Link => {
                    b.style.pop();
                }
                TagEnd::TableHead | TagEnd::TableRow | TagEnd::DefinitionList => {}
                TagEnd::Item | TagEnd::BlockQuote(_) | TagEnd::FootnoteDefinition => {
                    b.close_implicit();
                    b.close();
                }
                _ => b.close(),
            },
            Event::Text(text) => b.text(line, &text),
            Event::Code(code) => {
                let mut style = b.style();
                style.code = true;
                b.inline(line, Inline::Text(code.to_string(), style));
            }
            Event::InlineMath(math) => {
                let mut style = b.style();
                style.math = true;
                style.code = true;
                b.inline(line, Inline::Text(math.to_string(), style));
            }
            Event::DisplayMath(math) => {
                if matches!(b.stack.last(), Some(Frame::Paragraph { runs, .. }) if runs.is_empty()) {
                    // A paragraph holding only display math is the math block.
                    b.stack.pop();
                    b.add(line, Block::Math(math.trim().to_owned()));
                    b.stack.push(Frame::Paragraph { runs: Vec::new(), line, implicit: true });
                } else {
                    let mut style = b.style();
                    style.math = true;
                    style.code = true;
                    b.inline(line, Inline::Text(math.to_string(), style));
                }
            }
            Event::Html(html) => {
                if let Some(Frame::Html { text, .. }) = b.stack.last_mut() {
                    text.push_str(&html);
                }
            }
            Event::InlineHtml(html) => {
                let style = Style { code: true, raw: true, ..b.style() };
                b.inline(line, Inline::Text(html.to_string(), style));
            }
            Event::FootnoteReference(label) => {
                let number = b.footnote_number(&label);
                b.inline(line, Inline::Footnote(label.to_string(), number));
            }
            Event::SoftBreak => b.inline(line, Inline::Soft),
            Event::HardBreak => b.inline(line, Inline::Break),
            Event::Rule => {
                b.close_implicit();
                b.add(line, Block::Rule);
            }
            Event::TaskListMarker(done) => {
                for frame in b.stack.iter_mut().rev() {
                    if let Frame::Item { task, .. } = frame {
                        *task = Some(done);
                        break;
                    }
                }
            }
        }
    }
    while b.stack.len() > 1 {
        b.close_implicit();
        b.close();
    }
    let blocks = match b.stack.pop() {
        Some(Frame::Root(blocks)) => blocks,
        _ => Vec::new(),
    };
    let mut footnotes: Vec<(String, usize, Vec<Located>)> = Vec::new();
    let mut definitions = b.definitions;
    for (number, label) in b.footnotes.iter().enumerate() {
        if let Some(at) = definitions.iter().position(|(known, _)| known == label) {
            let (label, content) = definitions.swap_remove(at);
            footnotes.push((label, number + 1, content));
        }
    }
    let title = blocks.iter().find_map(|located| match &located.block {
        Block::Heading { runs, .. } => Some(plain_text(runs)),
        _ => None,
    });
    Doc { blocks, footnotes, title }
}

/// The text of runs, without styling.
pub fn plain_text(runs: &Runs) -> String {
    let mut out = String::new();
    for inline in runs {
        match inline {
            Inline::Text(text, _) => out.push_str(text),
            Inline::Break | Inline::Soft => out.push(' '),
            Inline::Footnote(_, number) => out.push_str(&number.to_string()),
            Inline::Image { alt, .. } => out.push_str(alt),
        }
    }
    out
}
