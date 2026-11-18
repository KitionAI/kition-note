//! Slim, as CodeMirror 5's slim mode colors it, ported token for token: tags, `#id` and
//! `.class` attributes, attributes and their values (strings, `true`/`false`/`nil` keywords,
//! Ruby expressions), `-` and `=` lines and `#{…}` as Ruby, `|` text and `<` lines as HTML,
//! `javascript:`-style blocks in their language, `/` comments (and the lines under them),
//! `doctype` lines. Ruby, HTML, and the blocks' languages are colored by their ports, a piece at
//! a time; an attribute's Ruby expression runs to the first space outside brackets and strings
//! (where the mode finds Ruby back at its top level).

use super::Kind;
use super::scan::{self, Out, Stream};
use super::{State, coffee, css, js, ruby, stylus, xml};

/// What reads the next token (the mode's tokenizers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tokenize {
    /// A line's start or a tag's rest (`O`).
    #[default]
    Line,
    /// After a tag: shortcuts, then attributes (`I`, `P`, `j`).
    AfterTag,
    Attributes,
    /// After an attribute's name: its `=` (`Z`), then its value (`_`).
    AttributeEquals,
    AttributeValue,
    /// Inside `( )`, `[ ]`, `{ }` around attributes (`g`, `v`, `E`): its closer.
    Wrapped(u8),
    WrappedEquals(u8),
    WrappedValue(u8),
    /// HTML text (`M`) to the line's end.
    Text,
    /// Ruby to the line's end (`=` after a tag, `S`).
    Ruby,
    /// A `-`/`=` line's Ruby (a line ending in `,` or `\` carries it on).
    RubyLine,
    /// A comment's line (`w`), a submode block's lines (`A`).
    Comment,
    Submode,
    /// The rest of a `doctype` line.
    Doctype,
}

/// A block a line opened, for the lines indented under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Block {
    indented: usize,
    /// What reads its lines.
    tokenize: Tokenize,
    /// What reads lines when a less indented line ends it (the mode's `stack.tokenize`).
    line: Tokenize,
    /// What reads lines when its closer ends it (a wrapper's `stack.line`).
    closed: Tokenize,
}

/// A submode block's language.
#[derive(Clone, Debug)]
enum Submode {
    Js(js::Js),
    Css(css::Css, bool),
    Ruby(ruby::Ruby),
    Stylus(stylus::Stylus),
    Coffee(coffee::Coffee),
    Plain,
}

#[derive(Clone, Debug, Default)]
pub struct Slim {
    tokenize: Tokenize,
    line: Tokenize,
    blocks: Vec<Block>,
    last_submode: bool,
    /// The submode's language and the column its block's text starts at.
    submode: Option<Submode>,
    submode_column: Option<usize>,
    ruby: ruby::Ruby,
    html: Option<Box<State>>,
    /// A `-` line's Ruby ended with `,`: the next line carries it on.
    continued: bool,
    /// The line's indentation.
    indented: usize,
}

const SUBMODES: &[&str] = &[
    "ruby",
    "javascript",
    "css",
    "sass",
    "scss",
    "less",
    "styl",
    "coffee",
    "asciidoc",
    "markdown",
    "textile",
    "creole",
    "wiki",
    "mediawiki",
    "rdoc",
    "builder",
    "nokogiri",
    "erb",
];

fn name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

fn name_char(byte: u8) -> bool {
    name_start(byte) || byte.is_ascii_digit() || byte == b'-'
}

/// Colors `range` with a port.
fn piece(text: &str, range: std::ops::Range<usize>, out: &mut Out, color: impl FnOnce(&str, &mut Out)) {
    let mut inner = Out::default();
    color(text.get(range.clone()).unwrap_or(""), &mut inner);
    out.extend_shifted(inner, range.start);
}

/// Where a Ruby expression from `at` ends: the first space outside brackets and strings.
fn expression_end(bytes: &[u8], at: usize) -> usize {
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut index = at;
    while index < bytes.len() {
        let here = bytes.get(index).copied().unwrap_or(0);
        match quote {
            Some(open) => {
                if here == b'\\' {
                    index += 1;
                } else if here == open {
                    quote = None;
                }
            }
            None => match here {
                b'"' | b'\'' => quote = Some(here),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b' ' | b'\t' if depth == 0 && index > at => return index,
                _ => {}
            },
        }
        index += 1;
    }
    bytes.len()
}

/// Where `#{…}`'s Ruby ends: at the first `}` outside strings.
fn interpolation_end(bytes: &[u8], at: usize) -> usize {
    let mut quote: Option<u8> = None;
    let mut index = at;
    while index < bytes.len() {
        let here = bytes.get(index).copied().unwrap_or(0);
        match quote {
            Some(open) => {
                if here == b'\\' {
                    index += 1;
                } else if here == open {
                    quote = None;
                }
            }
            None if here == b'"' || here == b'\'' => quote = Some(here),
            None if here == b'}' => return index,
            None => {}
        }
        index += 1;
    }
    bytes.len()
}

pub fn line(text: &str, state: &mut Slim, out: &mut Out) {
    let indented = text.bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count();
    state.indented = indented;
    state.tokenize = state.line;
    if state.continued {
        state.continued = false;
        state.tokenize = Tokenize::RubyLine;
    } else if !state.last_submode {
        while let Some(block) = state.blocks.last().copied().filter(|block| block.indented > indented) {
            state.blocks.pop();
            state.line = block.line;
            state.tokenize = block.line;
            state.submode = None;
            state.submode_column = None;
        }
    }
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    let mut start_of_line = true;
    while !stream.eol() {
        stream.start = stream.pos;
        if stream.eat_space() {
            continue;
        }
        let style = token(text, &mut stream, state, out, start_of_line, indented);
        start_of_line = false;
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// HTML text from the position to the line's end, `#{…}` in it as Ruby.
fn text_rest(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out) {
    let bytes = stream.bytes;
    let mut at = stream.pos;
    while at < bytes.len() {
        let next = (at..bytes.len()).find(|index| scan::at(bytes, *index, "#{") && (*index == 0 || bytes.get(*index - 1) != Some(&b'\\')));
        let html_end = next.unwrap_or(bytes.len());
        if html_end > at {
            let html = state.html.get_or_insert_with(Box::default);
            piece(text, at..html_end, out, |part, out| xml::line(part, html, out));
        }
        let Some(open) = next else { break };
        let close = interpolation_end(bytes, open + 2);
        let ruby_state = &mut state.ruby;
        piece(text, open + 2..close, out, |part, out| ruby::line(part, ruby_state, out));
        at = (close + 1).min(bytes.len());
    }
    stream.skip_to_end();
}

fn token(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out, start_of_line: bool, indented: usize) -> Option<Kind> {
    let at = stream.pos;
    match state.tokenize {
        Tokenize::Comment => {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        Tokenize::Doctype => {
            stream.skip_to_end();
            return None;
        }
        Tokenize::Text => {
            text_rest(text, stream, state, out);
            return None;
        }
        Tokenize::Ruby | Tokenize::RubyLine => {
            let line_mode = state.tokenize == Tokenize::RubyLine;
            let ruby_state = &mut state.ruby;
            piece(text, at..text.len(), out, |part, out| ruby::line(part, ruby_state, out));
            stream.skip_to_end();
            if line_mode {
                let trimmed = text.trim_end();
                state.continued = trimmed.ends_with(',') || trimmed.ends_with('\\');
            }
            return None;
        }
        Tokenize::Submode => {
            if state.submode_column.is_none()
                && let Some(block) = state.blocks.last_mut()
            {
                // The block's first line sets the column its text starts at.
                block.indented = indented;
            }
            let column = *state.submode_column.get_or_insert(indented);
            let from = column.min(text.len());
            if let Some(submode) = state.submode.as_mut() {
                let range = from..text.len();
                match submode {
                    Submode::Js(js_state) => piece(text, range, out, |part, out| js::line(part, js_state, js::Flavor::JavaScript, out)),
                    Submode::Css(css_state, scss) => {
                        let scss = *scss;
                        piece(text, range, out, |part, out| css::line(part, css_state, scss, out));
                    }
                    Submode::Ruby(ruby_state) => piece(text, range, out, |part, out| ruby::line(part, ruby_state, out)),
                    Submode::Stylus(stylus_state) => piece(text, range, out, |part, out| stylus::line(part, stylus_state, out)),
                    Submode::Coffee(coffee_state) => piece(text, range, out, |part, out| coffee::line(part, coffee_state, out)),
                    Submode::Plain => {}
                }
            }
            state.last_submode = false;
            stream.skip_to_end();
            return None;
        }
        _ => {}
    }
    state.last_submode = false;
    match state.tokenize {
        Tokenize::Line if start_of_line => line_start(text, stream, state, out, indented),
        Tokenize::Line => tag(text, stream, state, out),
        Tokenize::AfterTag => after_tag(text, stream, state, out),
        Tokenize::Attributes => attributes(text, stream, state, out),
        Tokenize::AttributeEquals => {
            if stream.take("==") || stream.take("=") {
                state.tokenize = Tokenize::AttributeValue;
                return None;
            }
            attributes(text, stream, state, out)
        }
        Tokenize::AttributeValue => value(text, stream, state, out, Tokenize::Attributes),
        Tokenize::Wrapped(closer) => wrapped(stream, state, closer),
        Tokenize::WrappedEquals(closer) => {
            if stream.take("==") || stream.take("=") {
                state.tokenize = Tokenize::WrappedValue(closer);
                return None;
            }
            wrapped(stream, state, closer)
        }
        Tokenize::WrappedValue(closer) => value(text, stream, state, out, Tokenize::Wrapped(closer)),
        _ => None,
    }
}

/// A line's first token (the mode's `U`).
fn line_start(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out, indented: usize) -> Option<Kind> {
    let at = stream.pos;
    if stream.peek() == b'<' {
        text_rest(text, stream, state, out);
        return None;
    }
    if matches!(stream.peek(), b'|' | b'\'') {
        stream.next();
        state.blocks.push(Block { indented: at + 1, tokenize: Tokenize::Text, line: state.line, closed: state.line });
        state.line = Tokenize::Text;
        state.tokenize = Tokenize::Text;
        return None;
    }
    if stream.peek() == b'/' {
        stream.skip_to_end();
        state.blocks.push(Block { indented: indented + 1, tokenize: Tokenize::Comment, line: state.line, closed: state.line });
        state.line = Tokenize::Comment;
        return Some(Kind::Comment);
    }
    if stream.take("-") || stream.take("==<") || stream.take("==>") || stream.take("==") || stream.take("=<") || stream.take("=>") || stream.take("=")
    {
        state.tokenize = Tokenize::RubyLine;
        return Some(Kind::Operator);
    }
    if stream.at("doctype") && !scan::js_word(stream.byte(at + 7)) {
        stream.pos += 7;
        state.tokenize = Tokenize::Doctype;
        return Some(Kind::Keyword);
    }
    if let Some(name) = SUBMODES.iter().find(|name| stream.at(name) && stream.byte(at + name.len()) == b':') {
        stream.pos += name.len() + 1;
        state.submode = Some(match *name {
            "javascript" => Submode::Js(js::Js::default()),
            "css" => Submode::Css(css::Css::default(), false),
            "scss" | "less" => Submode::Css(css::Css::default(), true),
            "ruby" => Submode::Ruby(ruby::Ruby::default()),
            "styl" => Submode::Stylus(stylus::Stylus::default()),
            "coffee" => Submode::Coffee(coffee::Coffee::default()),
            _ => Submode::Plain,
        });
        state.submode_column = None;
        state.blocks.push(Block { indented: indented + 1, tokenize: Tokenize::Submode, line: state.line, closed: state.line });
        state.line = Tokenize::Submode;
        state.tokenize = Tokenize::Submode;
        state.last_submode = true;
        return Some(Kind::Keyword);
    }
    tag(text, stream, state, out)
}

/// A tag's name (the mode's `R`).
fn tag(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out) -> Option<Kind> {
    let at = stream.pos;
    if stream.take("*") {
        let end = expression_end(stream.bytes, stream.pos);
        let fresh = &mut ruby::Ruby::default();
        piece(text, stream.pos..end, out, |part, out| ruby::line(part, fresh, out));
        stream.pos = end;
        state.tokenize = Tokenize::AfterTag;
        return None;
    }
    let first = stream.peek();
    if first == b':' || name_start(first) {
        let mut end = at + 1;
        if stream.byte(end) == b':' && name_char(stream.byte(end + 1)) {
            end += 2;
        } else {
            while name_char(stream.byte(end)) {
                end += 1;
            }
        }
        stream.pos = end;
        state.tokenize = Tokenize::AfterTag;
        return Some(Kind::Operator);
    }
    shortcuts(text, stream, state, out)
}

/// After a tag's name: `<`/`>` whitespace marks (the mode's `I`).
fn after_tag(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out) -> Option<Kind> {
    if stream.take("<>") || stream.take("<") || stream.take("><") || stream.take(">") {
        state.tokenize = Tokenize::AfterTag;
        return None;
    }
    shortcuts(text, stream, state, out)
}

/// `#id` and `.class` (the mode's `P`).
fn shortcuts(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out) -> Option<Kind> {
    let at = stream.pos;
    for mark in *b"#." {
        if stream.peek() == mark {
            let dash = usize::from(mark == b'.' && stream.byte(at + 1) == b'-');
            let first = at + 1 + dash;
            let byte = stream.byte(first);
            if byte.is_ascii_alphabetic() || byte == b'_' {
                let mut end = first;
                while stream.byte(end).is_ascii_alphabetic() || stream.byte(end) == b'_' {
                    end += 1;
                }
                while scan::js_word(stream.byte(end)) || stream.byte(end) == b'-' {
                    end += 1;
                }
                stream.pos = end;
                state.tokenize = Tokenize::AfterTag;
                return Some(Kind::Property);
            }
        }
    }
    attributes(text, stream, state, out)
}

/// An attribute's name, before `\s*=`.
fn attribute_name(stream: &Stream, at: usize) -> Option<usize> {
    let first = stream.byte(at);
    if !(first == b':' || name_start(first)) {
        return None;
    }
    let end = (at + 1..stream.bytes.len())
        .find(|index| !(name_char(stream.byte(*index)) || matches!(stream.byte(*index), b':' | b'.')))
        .unwrap_or(stream.bytes.len());
    Some(end)
}

/// Attributes (the mode's `j`).
fn attributes(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out) -> Option<Kind> {
    let at = stream.pos;
    if let Some(closer) = match stream.peek() {
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        b'(' => Some(b')'),
        _ => None,
    } {
        stream.next();
        // A wrapper ended by a less indented line leaves attributes reading (its `stack.tokenize`).
        state.blocks.push(Block {
            indented: state.indented + 1,
            tokenize: Tokenize::Wrapped(closer),
            line: Tokenize::Attributes,
            closed: state.line,
        });
        state.line = Tokenize::Wrapped(closer);
        state.tokenize = Tokenize::Wrapped(closer);
        return None;
    }
    if let Some(end) = attribute_name(stream, at) {
        let equals = (end..stream.bytes.len()).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(stream.bytes.len());
        if stream.byte(equals) == b'=' {
            stream.pos = end;
            state.tokenize = Tokenize::AttributeEquals;
            return Some(Kind::Property);
        }
    }
    if stream.take("*") {
        let end = expression_end(stream.bytes, stream.pos);
        let fresh = &mut ruby::Ruby::default();
        piece(text, stream.pos..end, out, |part, out| ruby::line(part, fresh, out));
        stream.pos = end;
        state.tokenize = Tokenize::Attributes;
        return None;
    }
    // The mode's `V`: `=` Ruby, `/` an empty tag's end, `:` another tag, else text.
    if stream.take("==") || stream.take("=") {
        state.tokenize = Tokenize::Ruby;
        return Some(Kind::Operator);
    }
    if stream.rest() == b"/" {
        stream.next();
        state.tokenize = Tokenize::Line;
        return None;
    }
    if stream.take(":") {
        state.tokenize = Tokenize::Line;
        return Some(Kind::Operator);
    }
    // Text: its lines go on under it, from its column.
    state.blocks.push(Block { indented: at, tokenize: Tokenize::Text, line: state.line, closed: state.line });
    state.line = Tokenize::Text;
    text_rest(text, stream, state, out);
    None
}

/// An attribute's value (the mode's `_` and `E`): a string, a keyword, or Ruby.
fn value(text: &str, stream: &mut Stream, state: &mut Slim, out: &mut Out, after: Tokenize) -> Option<Kind> {
    let at = stream.pos;
    let quote = stream.peek();
    if quote == b'"' || quote == b'\'' {
        // To the quote, `#{…}` in it as Ruby.
        let bytes = stream.bytes;
        let mut index = at + 1;
        let mut escaped = false;
        let mut start = at;
        while index < bytes.len() {
            let here = bytes.get(index).copied().unwrap_or(0);
            if here == quote && !escaped {
                index += 1;
                break;
            }
            if here == b'#' && !escaped && bytes.get(index + 1) == Some(&b'{') {
                out.push(start..index, Kind::String);
                let close = interpolation_end(bytes, index + 2);
                let ruby_state = &mut state.ruby;
                piece(text, index + 2..close, out, |part, out| ruby::line(part, ruby_state, out));
                index = (close + 1).min(bytes.len());
                start = index;
                continue;
            }
            escaped = !escaped && here == b'\\';
            index += 1;
        }
        out.push(start..index, Kind::String);
        stream.pos = index;
        state.tokenize = after;
        return None;
    }
    for word in ["true", "false", "nil"] {
        if quote != b'[' && stream.at(word) && !scan::js_word(stream.byte(at + word.len())) {
            stream.pos += word.len();
            state.tokenize = after;
            return Some(Kind::Keyword);
        }
    }
    let end = expression_end(stream.bytes, at);
    let fresh = &mut ruby::Ruby::default();
    piece(text, at..end, out, |part, out| ruby::line(part, fresh, out));
    stream.pos = end;
    state.tokenize = after;
    None
}

/// Inside `( )`-style attribute brackets (the mode's `g`).
fn wrapped(stream: &mut Stream, state: &mut Slim, closer: u8) -> Option<Kind> {
    if stream.eat(|byte| byte == closer).is_some() {
        if let Some(block) = state.blocks.pop() {
            state.line = block.closed;
        }
        state.tokenize = Tokenize::Attributes;
        return None;
    }
    if let Some(end) = attribute_name(stream, stream.pos) {
        stream.pos = end;
        state.tokenize = Tokenize::WrappedEquals(closer);
        return Some(Kind::Property);
    }
    stream.next();
    None
}
