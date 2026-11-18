//! `kition-note-markdown`: a source-positioned index of Obsidian-flavoured Markdown.
//!
//! The note's file is its text (`docs/design.md` section 2); structure is never stored. This crate
//! finds it: headings with their paths and sections, links, wikilinks, embeds, tags, block ids,
//! callouts, tables with every cell's span, tasks, code, and the property block. Every range is a
//! **byte range** of the text; [`LineTable`] converts to lines, Unicode scalars and UTF-16.
//!
//! CommonMark and GFM come from pulldown-cmark's offset iterator (wikilinks too); the rest of the
//! Obsidian syntax (tags, block ids, `==highlights==`, `%%comments%%`, bare URLs, callout types)
//! from one byte-driven scan.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag as Cm, TagEnd};

pub mod frontmatter;
pub mod lines;
mod obsidian;
pub mod table;

pub use frontmatter::{Entry, Frontmatter, Style, Typed};
pub use lines::LineTable;
pub use table::{Align, Cell, Row, Table};

/// `text[range]`, or `""` when the range is not on char boundaries inside the text.
pub fn sub(text: &str, range: Range<usize>) -> &str {
    text.get(range).unwrap_or_default()
}

/// A heading of the note's outline (not one inside a quote or a list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    /// Its plain text: inline Markdown removed, trimmed.
    pub text: String,
    /// The heading's lines (two for a Setext heading), with the line ending.
    pub range: Range<usize>,
    /// The `#` run of an ATX heading, or the underline of a Setext heading (without its ending).
    pub marker: Range<usize>,
    pub setext: bool,
    /// The heading and everything under it, up to the next heading of the same or a higher level.
    pub section: Range<usize>,
    /// The plain texts of the headings above it and its own.
    pub path: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// `[text](destination)`.
    Markdown,
    /// `![alt](destination)`.
    Image,
    /// `[text][label]`, `[label][]`, `[label]`: the destination is in a definition.
    Reference,
    /// `<https://...>`.
    Autolink,
    /// `https://...` in running text.
    BareUrl,
    /// `[[note#heading|alias]]`.
    Wikilink,
    /// `![[file|size]]`.
    Embed,
}

impl LinkKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Image => "image",
            Self::Reference => "reference",
            Self::Autolink => "autolink",
            Self::BareUrl => "url",
            Self::Wikilink => "wikilink",
            Self::Embed => "embed",
        }
    }

    /// Wikilinks and embeds.
    pub fn is_wiki(self) -> bool {
        matches!(self, Self::Wikilink | Self::Embed)
    }
}

/// The part of a link after `#`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    Heading(String),
    /// `#^id`.
    Block(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub kind: LinkKind,
    /// The whole construct.
    pub range: Range<usize>,
    /// The destination as written (a wikilink's part before `|`; a Markdown link's URL).
    pub destination: String,
    /// Where the destination is written, when it is in the link itself.
    pub destination_range: Option<Range<usize>>,
    /// The file part of the destination (before `#`), percent-decoded for Markdown links.
    pub file: String,
    pub anchor: Option<Anchor>,
    /// A wikilink's alias (`|alias`), or a Markdown link's text.
    pub text: Option<String>,
    pub text_range: Option<Range<usize>>,
}

impl Link {
    /// Whether the destination is a URL with a scheme (`https:`, `mailto:`), not a file.
    pub fn is_external(&self) -> bool {
        matches!(self.kind, LinkKind::Autolink | LinkKind::BareUrl) || has_scheme(&self.file)
    }
}

fn has_scheme(destination: &str) -> bool {
    let Some((scheme, _)) = destination.split_once(':') else { return false };
    scheme.len() > 1
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub range: Range<usize>,
    /// Without the `#`.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockId {
    /// The `^id` token.
    pub range: Range<usize>,
    /// Without the caret.
    pub id: String,
    /// The block it names, through the end of the id's line (without its ending).
    pub block: Range<usize>,
    /// The id stands on a line of its own after the block (tables, quotes, lists).
    pub own_line: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callout {
    pub kind: String,
    /// `+` or `-` after the type.
    pub fold: Option<char>,
    pub range: Range<usize>,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    /// The list item.
    pub item: Range<usize>,
    /// The `[ ]` marker.
    pub marker: Range<usize>,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
    pub range: Range<usize>,
    pub fenced: bool,
    pub info: String,
    /// A fenced block whose closing fence is missing (it runs to the end of its container).
    pub unclosed: bool,
    /// The opening fence's marker (```` ``` ```` or `~~~`, as long as written).
    pub fence: String,
}

/// The index of one text.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub lines: LineTable,
    pub frontmatter: Option<Frontmatter>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub tags: Vec<Tag>,
    pub block_ids: Vec<BlockId>,
    pub callouts: Vec<Callout>,
    pub tables: Vec<Table>,
    pub tasks: Vec<Task>,
    pub code: Vec<CodeBlock>,
    /// Inline code and math spans (skipped by the Obsidian scan).
    pub inline_code: Vec<Range<usize>>,
    pub highlights: Vec<Range<usize>>,
    /// `%%...%%`.
    pub comments: Vec<Range<usize>>,
    /// `$$` blocks opened and never closed: the opening line.
    pub unclosed_math: Option<Range<usize>>,
    pub footnotes: usize,
    pub words: usize,
}

/// pulldown-cmark's options for Obsidian-flavoured Markdown.
pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_WIKILINKS
}

/// A block-level element seen while indexing, to find the block a `^id` names.
struct Block {
    range: Range<usize>,
    /// Nesting depth (0: top level).
    depth: usize,
}

/// A table being read: its range, alignments, and each row's start.
type OpenTable = (Range<usize>, Vec<Align>, Vec<Range<usize>>);

/// Indexes `text`.
pub fn index(text: &str) -> Index {
    index_observed(text, true, |_, _| {})
}

/// As [`index`], for a part of a note: `front_matter` whether it starts the note (a part later
/// in it that starts with a `---` block has no front matter).
pub fn index_part(text: &str, front_matter: bool) -> Index {
    index_observed(text, front_matter, |_, _| {})
}

/// As [`index`], showing `observe` each of the parse's events (and its range) first: a reader of
/// the same events needs no parse of its own.
pub fn index_observed<'a>(text: &'a str, front_matter: bool, mut observe: impl FnMut(&Event<'a>, Range<usize>)) -> Index {
    let lines = LineTable::new(text);
    let frontmatter = if front_matter { frontmatter::scan(text, |at| lines.line_of(at)) } else { None };
    let mut ix = Index { frontmatter, lines, ..Index::default() };
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut heading: Option<(Heading, usize)> = None;
    let mut table: Option<OpenTable> = None;
    let mut depth = 0usize;
    let mut blocks: Vec<Block> = Vec::new();
    let mut items: Vec<Range<usize>> = Vec::new();
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        observe(&event, range.clone());
        match event {
            Event::Start(Cm::Heading { level, .. }) => {
                let level = level as u8;
                let setext = !sub(text, range.clone()).trim_start().starts_with('#');
                heading = Some((
                    Heading {
                        level,
                        text: String::new(),
                        range: range.clone(),
                        marker: marker(text, &range, setext),
                        setext,
                        section: range.clone(),
                        path: Vec::new(),
                    },
                    depth,
                ));
                blocks.push(Block { range: range.clone(), depth });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((mut found, at_depth)) = heading.take() {
                    found.text = found.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    // A block id at the end of a heading is not part of its text.
                    if let Some((before, id)) = found.text.rsplit_once(" ^")
                        && !id.is_empty()
                        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                    {
                        found.text = before.to_owned();
                    }
                    if at_depth == 0 {
                        while stack.last().is_some_and(|(level, _)| *level >= found.level) {
                            stack.pop();
                        }
                        stack.push((found.level, found.text.clone()));
                        found.path = stack.iter().map(|(_, text)| text.clone()).collect();
                        ix.headings.push(found);
                    }
                }
            }
            Event::Text(part) if heading.is_some() => {
                if let Some((found, _)) = heading.as_mut() {
                    found.text.push_str(&part);
                }
            }
            // A line break inside an underlined heading reads as a space.
            Event::SoftBreak | Event::HardBreak if heading.is_some() => {
                if let Some((found, _)) = heading.as_mut() {
                    found.text.push(' ');
                }
            }
            Event::Code(part) => {
                if let Some((found, _)) = heading.as_mut() {
                    found.text.push_str(&part);
                }
                ix.inline_code.push(range);
            }
            Event::InlineMath(_) | Event::DisplayMath(_) => ix.inline_code.push(range),
            Event::Start(Cm::CodeBlock(kind)) => {
                let (fenced, info) = match &kind {
                    CodeBlockKind::Fenced(info) => (true, info.to_string()),
                    CodeBlockKind::Indented => (false, String::new()),
                };
                ix.code.push(code_block(text, range.clone(), fenced, info));
                blocks.push(Block { range, depth });
            }
            Event::Start(Cm::Paragraph) => blocks.push(Block { range, depth }),
            Event::Start(Cm::Table(aligns)) => {
                table = Some((range.clone(), aligns.iter().map(|align| Align::from_cmark(*align)).collect(), Vec::new()));
                blocks.push(Block { range, depth });
            }
            Event::Start(Cm::TableHead | Cm::TableRow) => {
                if let Some((_, _, rows)) = table.as_mut() {
                    rows.push(range);
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some((range, aligns, rows)) = table.take()
                    && let Some(found) = table::build(text, &ix.lines, range, aligns, &rows)
                {
                    ix.tables.push(found);
                }
            }
            Event::Start(Cm::BlockQuote(_)) => {
                if let Some(callout) = callout(text, range.clone()) {
                    ix.callouts.push(callout);
                }
                blocks.push(Block { range, depth });
                depth += 1;
            }
            Event::Start(Cm::List(_)) => {
                blocks.push(Block { range, depth });
                depth += 1;
            }
            Event::Start(Cm::Item) => {
                items.push(range.clone());
                blocks.push(Block { range, depth });
                depth += 1;
            }
            Event::Start(Cm::FootnoteDefinition(_)) => {
                ix.footnotes += 1;
                depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::FootnoteDefinition) => depth = depth.saturating_sub(1),
            Event::TaskListMarker(done) => {
                if let Some(item) = items.iter().rev().find(|item| item.start <= range.start && range.end <= item.end) {
                    ix.tasks.push(Task { item: item.clone(), marker: range, done });
                }
            }
            Event::Start(Cm::Link { link_type, dest_url, .. }) => ix.links.push(link(text, range, link_type, &dest_url, false)),
            Event::Start(Cm::Image { link_type, dest_url, .. }) => ix.links.push(link(text, range, link_type, &dest_url, true)),
            _ => {}
        }
    }
    finish_link_texts(text, &mut ix.links);
    ix.inline_code.sort_by_key(|range| range.start);
    sections(text, &mut ix.headings);
    obsidian::scan(text, &mut ix);
    block_ranges(text, &mut ix, &blocks);
    ix.unclosed_math = unclosed_math(text, &ix);
    ix.words = words(text, ix.frontmatter.as_ref().map_or(0, |found| found.range.end));
    ix
}

/// The `#` run of an ATX heading, or a Setext heading's underline.
fn marker(text: &str, range: &Range<usize>, setext: bool) -> Range<usize> {
    let source = sub(text, range.clone());
    if setext {
        let trimmed = source.trim_end_matches(['\n', '\r']);
        let start = trimmed.rfind('\n').map_or(0, |at| at + 1);
        let line = sub(trimmed, start..trimmed.len());
        let lead = line.len() - line.trim_start().len();
        let len = line.trim().len();
        return range.start + start + lead..range.start + start + lead + len;
    }
    let lead = source.len() - source.trim_start().len();
    let hashes = sub(source, lead..source.len()).bytes().take_while(|byte| *byte == b'#').count();
    range.start + lead..range.start + lead + hashes
}

/// Sets each heading's section: up to the next heading of the same or a higher level.
fn sections(text: &str, headings: &mut [Heading]) {
    let starts: Vec<(usize, u8)> = headings.iter().map(|heading| (heading.range.start, heading.level)).collect();
    for (index, heading) in headings.iter_mut().enumerate() {
        let end = starts.iter().skip(index + 1).find(|(_, level)| *level <= heading.level).map_or(text.len(), |(start, _)| *start);
        heading.section = heading.range.start..end.max(heading.range.end);
    }
}

fn code_block(text: &str, range: Range<usize>, fenced: bool, info: String) -> CodeBlock {
    let source = sub(text, range.clone());
    let first = source.lines().next().unwrap_or_default().trim_start();
    let fence_char = first.chars().next().filter(|c| *c == '`' || *c == '~');
    let fence: String = fence_char.map(|c| first.chars().take_while(|x| *x == c).collect()).unwrap_or_default();
    let unclosed = fenced && !fence.is_empty() && {
        let body: Vec<&str> = source.lines().collect();
        body.len() < 2
            || !body.last().is_some_and(|last| {
                let last = last.trim();
                fence_char.is_some_and(|c| last.chars().all(|x| x == c)) && last.len() >= fence.len()
            })
    };
    CodeBlock { range, fenced, info, unclosed, fence }
}

fn callout(text: &str, range: Range<usize>) -> Option<Callout> {
    let first = sub(text, range.clone()).lines().next()?;
    let body = first.trim_start().strip_prefix('>')?.trim_start();
    let rest = body.strip_prefix("[!")?;
    let end = rest.find(']')?;
    let kind = sub(rest, 0..end).to_ascii_lowercase();
    let after = sub(rest, end + 1..rest.len());
    let fold = after.chars().next().filter(|c| *c == '+' || *c == '-');
    let title = after.get(fold.map_or(0, char::len_utf8)..).unwrap_or_default().trim().to_owned();
    Some(Callout { kind, fold, range, title })
}

/// The part of a destination before `#` and the anchor after it.
fn split_anchor(destination: &str) -> (String, Option<Anchor>) {
    match destination.split_once('#') {
        None => (destination.to_owned(), None),
        Some((file, anchor)) => {
            let anchor = match anchor.strip_prefix('^') {
                Some(block) => Anchor::Block(block.to_owned()),
                None => Anchor::Heading(anchor.to_owned()),
            };
            (file.to_owned(), Some(anchor))
        }
    }
}

/// `%XX` escapes decoded (invalid ones kept as written).
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = |offset: usize| bytes.get(at + offset).and_then(|digit| char::from(*digit).to_digit(16));
        match (byte, hex(1), hex(2)) {
            (b'%', Some(high), Some(low)) => {
                out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                at += 3;
            }
            _ => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

fn link(text: &str, range: Range<usize>, link_type: LinkType, url: &str, image: bool) -> Link {
    let source = sub(text, range.clone());
    match link_type {
        LinkType::WikiLink { .. } => {
            let open = if source.starts_with('!') { 3 } else { 2 };
            let inner_start = range.start + open;
            let inner_end = range.end.saturating_sub(2).max(inner_start);
            let inner = sub(text, inner_start..inner_end);
            let (destination, alias) = match inner.split_once('|') {
                Some((destination, alias)) => (destination, Some((alias, inner_start + destination.len() + 1))),
                None => (inner, None),
            };
            let (file, anchor) = split_anchor(destination);
            Link {
                kind: if image { LinkKind::Embed } else { LinkKind::Wikilink },
                range,
                destination: destination.to_owned(),
                destination_range: Some(inner_start..inner_start + destination.len()),
                file: file.trim().to_owned(),
                anchor,
                text: alias.map(|(alias, _)| alias.to_owned()),
                text_range: alias.map(|(alias, at)| at..at + alias.len()),
            }
        }
        LinkType::Autolink | LinkType::Email => {
            let inner = range.start + 1..range.end.saturating_sub(1).max(range.start + 1);
            Link {
                kind: LinkKind::Autolink,
                destination: url.to_owned(),
                destination_range: Some(inner),
                file: url.to_owned(),
                anchor: None,
                text: None,
                text_range: None,
                range,
            }
        }
        LinkType::Inline => {
            let destination_range = inline_destination(text, range.clone());
            let destination = destination_range
                .clone()
                .map_or_else(|| url.to_owned(), |span| sub(text, span).trim_start_matches('<').trim_end_matches('>').to_owned());
            let (file, anchor) = split_anchor(url);
            Link {
                kind: if image { LinkKind::Image } else { LinkKind::Markdown },
                range,
                destination,
                destination_range,
                file: percent_decode(&file),
                anchor,
                text: None,
                text_range: None,
            }
        }
        _ => {
            let (file, anchor) = split_anchor(url);
            Link {
                kind: LinkKind::Reference,
                range,
                destination: url.to_owned(),
                destination_range: None,
                file: percent_decode(&file),
                anchor,
                text: None,
                text_range: None,
            }
        }
    }
}

/// Where an inline link's destination is written: after the `](` that closes its text.
fn inline_destination(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let source = sub(text, range.clone());
    let bytes = source.as_bytes();
    let mut at = usize::from(bytes.first() == Some(&b'!')) + 1;
    let mut depth = 1usize;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        at += 1;
    }
    if bytes.get(at + 1) != Some(&b'(') {
        return None;
    }
    at += 2;
    while bytes.get(at).is_some_and(|byte| byte.is_ascii_whitespace()) {
        at += 1;
    }
    let start = at;
    if bytes.get(at) == Some(&b'<') {
        // Up to the first `>` not escaped by a backslash.
        while let Some(&byte) = bytes.get(at) {
            at += 1;
            match byte {
                b'\\' => at += 1,
                b'>' => break,
                _ => {}
            }
        }
        return Some(range.start + start..range.start + at.min(bytes.len()));
    }
    let mut parens = 0usize;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 1,
            b'(' => parens += 1,
            b')' if parens == 0 => break,
            b')' => parens -= 1,
            byte if byte.is_ascii_whitespace() => break,
            _ => {}
        }
        at += 1;
    }
    Some(range.start + start..range.start + at.min(bytes.len()))
}

/// A Markdown link's text: between its first `[` and the `]` before the destination.
fn finish_link_texts(text: &str, links: &mut [Link]) {
    for link in links.iter_mut().filter(|link| matches!(link.kind, LinkKind::Markdown | LinkKind::Image)) {
        let Some(destination) = link.destination_range.clone() else { continue };
        let open = link.range.start + if sub(text, link.range.clone()).starts_with('!') { 2 } else { 1 };
        let source = sub(text, open..destination.start);
        if let Some(close) = source.rfind("](") {
            let span = open..open + close;
            link.text = Some(sub(text, span.clone()).to_owned());
            link.text_range = Some(span);
        }
    }
}

/// The block each `^id` names: the innermost paragraph, heading or list item holding it, or, for
/// an id alone on its line, the block just before it.
fn block_ranges(text: &str, ix: &mut Index, blocks: &[Block]) {
    for found in &mut ix.block_ids {
        let line = ix.lines.line_of(found.range.start);
        let line_end = ix.lines.content(text, line).map_or(found.range.end, |content| content.end);
        let line_start = ix.lines.line(line).map_or(found.range.start, |range| range.start);
        let own_line = sub(text, line_start..found.range.start).trim().is_empty();
        found.own_line = own_line;
        let holder = if own_line {
            // The block before it, or the one it continues (a table takes the line as a row).
            blocks
                .iter()
                .filter(|block| block.depth == 0 && block.range.start < line_start)
                .max_by_key(|block| block.range.start)
                .map(|block| block.range.start)
        } else {
            blocks
                .iter()
                .filter(|block| block.range.start <= found.range.start && found.range.end <= block.range.end)
                .max_by_key(|block| block.range.start)
                .map(|block| block.range.start)
        };
        found.block = holder.unwrap_or(line_start)..line_end;
    }
}

/// The opening line of a `$$` block that is never closed (outside code).
fn unclosed_math(text: &str, ix: &Index) -> Option<Range<usize>> {
    let mut open: Option<Range<usize>> = None;
    for line in 0..ix.lines.count() {
        let Some(range) = ix.lines.content(text, line) else { continue };
        let content = sub(text, range.clone()).trim();
        // A line opens or closes a block when it has an odd number of `$$`: `$$E=mc^2$$ and text`
        // is math opened and closed on its line. Only those are looked up in the code blocks
        // (sorted by where they start).
        if content.matches("$$").count().is_multiple_of(2) {
            continue;
        }
        let before = ix.code.partition_point(|code| code.range.start <= range.start);
        if before > 0 && ix.code.get(before - 1).is_some_and(|code| range.start < code.range.end) {
            continue;
        }
        open = if open.is_some() { None } else { Some(range) };
    }
    open
}

/// Words outside the property block (from byte `from`): runs of letters and digits, each CJK
/// character one word.
pub fn words(text: &str, from: usize) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for c in sub(text, from..text.len()).chars() {
        let cjk = matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF);
        if cjk {
            count += 1;
            in_word = false;
        } else if c.is_alphanumeric() {
            if !in_word {
                count += 1;
            }
            in_word = true;
        } else if !(in_word && matches!(c, '\'' | '\u{2019}' | '-' | '_')) {
            in_word = false;
        }
    }
    count
}

#[cfg(test)]
mod tests;
