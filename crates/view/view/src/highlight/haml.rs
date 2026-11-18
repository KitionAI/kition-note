//! HAML, as CodeMirror 5's haml mode colors it, ported token for token: `%tags` tags, `#id`
//! and `.class` attributes, `{…}` and `(…)` attribute hashes and `=`/`-` lines as Ruby, `/`
//! comments (and the lines indented under them), `!!!` doctypes, other text as HTML. Ruby and
//! HTML are colored by their ports, a piece at a time.

use super::Kind;
use super::scan::{self, Out, Stream};
use super::{State, ruby, xml};

/// What reads the next token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tokenize {
    #[default]
    Base,
    /// Ruby to the line's end (a `,` there carries it on).
    Ruby,
    /// Ruby to the closing bracket of an attribute hash.
    Attributes(u8),
}

/// The last styled token: its style (as the mode names it) and its line's indentation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Previous {
    #[default]
    None,
    Tag,
    Attribute,
    CloseAttribute,
    Comment,
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct Haml {
    tokenize: Tokenize,
    previous: Previous,
    previous_indented: usize,
    ruby: ruby::Ruby,
    html: Option<Box<State>>,
}

fn name_byte(byte: u8) -> bool {
    scan::js_word(byte) || byte == b':'
}

/// The kind of the last span `out` holds (a comment, or something else).
fn last_previous(out: &Out) -> Previous {
    match out.last_kind() {
        Some(Kind::Comment) => Previous::Comment,
        Some(_) => Previous::Other,
        None => Previous::None,
    }
}

pub fn line(text: &str, state: &mut Haml, out: &mut Out) {
    let indented = text.bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count();
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    let mut start_of_line = true;
    while !stream.eol() {
        stream.start = stream.pos;
        if stream.eat_space() {
            continue;
        }
        let (style, previous) = token(text, &mut stream, state, out, start_of_line, indented);
        start_of_line = false;
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        if let Some(previous) = previous {
            state.previous = previous;
            state.previous_indented = indented;
        }
        if stream.eol() && state.tokenize == Tokenize::Ruby && !text.trim_end_matches(['\r']).ends_with(',') {
            state.tokenize = Tokenize::Base;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// Colors `range` of the line with a port, giving the kind its last span leaves.
fn piece(text: &str, range: std::ops::Range<usize>, out: &mut Out, color: impl FnOnce(&str, &mut Out)) -> Previous {
    let mut inner = Out::default();
    color(text.get(range.clone()).unwrap_or(""), &mut inner);
    let previous = last_previous(&inner);
    out.extend_shifted(inner, range.start);
    previous
}

/// Where Ruby in an attribute hash reaches the `closer` (outside strings): or the line's end.
fn attribute_end(stream: &Stream, closer: u8) -> Option<usize> {
    let bytes = stream.bytes;
    let mut at = stream.pos;
    let mut quote: Option<u8> = None;
    while at < bytes.len() {
        let here = bytes.get(at).copied().unwrap_or(0);
        match quote {
            Some(open) => {
                if here == b'\\' {
                    at += 1;
                } else if here == open {
                    quote = None;
                }
            }
            None if here == b'"' || here == b'\'' => quote = Some(here),
            None if here == closer && at > stream.pos => return Some(at),
            None => {}
        }
        at += 1;
    }
    None
}

fn token(text: &str, stream: &mut Stream, state: &mut Haml, out: &mut Out, start_of_line: bool, indented: usize) -> (Option<Kind>, Option<Previous>) {
    match state.tokenize {
        Tokenize::Ruby => {
            if stream.take("-#") {
                stream.skip_to_end();
                return (Some(Kind::Comment), Some(Previous::Comment));
            }
            let ruby = &mut state.ruby;
            let previous = piece(text, stream.pos..text.len(), out, |part, out| ruby::line(part, ruby, out));
            stream.skip_to_end();
            return (None, Some(previous).filter(|previous| *previous != Previous::None));
        }
        Tokenize::Attributes(closer) => {
            if stream.peek() == closer {
                stream.next();
                state.tokenize = Tokenize::Base;
                return (None, Some(Previous::CloseAttribute));
            }
            let end = attribute_end(stream, closer).unwrap_or(text.len());
            let ruby = &mut state.ruby;
            let previous = piece(text, stream.pos..end, out, |part, out| ruby::line(part, ruby, out));
            stream.pos = end;
            return (None, Some(previous).filter(|previous| *previous != Previous::None));
        }
        Tokenize::Base => {}
    }
    let here = stream.peek();
    if state.previous == Previous::Comment && indented > state.previous_indented {
        stream.skip_to_end();
        return (Some(Kind::Comment), None);
    }
    if start_of_line {
        if here == b'!' && stream.at("!!") {
            stream.skip_to_end();
            return (Some(Kind::Operator), Some(Previous::Other));
        }
        if here == b'%' {
            let name = (stream.pos + 1..text.len())
                .find(|at| !(name_byte(stream.byte(*at)) || matches!(stream.byte(*at), b'#' | b'.')))
                .unwrap_or(text.len());
            if name > stream.pos + 1 && stream.byte(name) == b'=' {
                stream.pos = name + 1;
                state.tokenize = Tokenize::Ruby;
                return (Some(Kind::Operator), Some(Previous::Tag));
            }
            let tag = (stream.pos + 1..text.len()).find(|at| !name_byte(stream.byte(*at))).unwrap_or(text.len());
            if tag > stream.pos + 1 {
                stream.pos = tag;
                return (Some(Kind::Operator), Some(Previous::Tag));
            }
        }
        if here == b'/' {
            stream.skip_to_end();
            return (Some(Kind::Comment), Some(Previous::Comment));
        }
    }
    if (start_of_line || state.previous == Previous::Tag) && (here == b'#' || here == b'.') {
        stream.eat_while(|byte| scan::js_word(byte) || matches!(byte, b'-' | b'#' | b'.'));
        return (Some(Kind::Property), Some(Previous::Attribute));
    }
    if start_of_line && !stream.at("-->") && (here == b'=' || here == b'-') {
        state.tokenize = Tokenize::Ruby;
        return token(text, stream, state, out, false, indented);
    }
    if matches!(state.previous, Previous::Tag | Previous::CloseAttribute | Previous::Attribute) {
        if here == b'(' {
            state.tokenize = Tokenize::Attributes(b')');
            return token(text, stream, state, out, false, indented);
        }
        if here == b'{' && !stream.at("{%") {
            state.tokenize = Tokenize::Attributes(b'}');
            return token(text, stream, state, out, false, indented);
        }
    }
    // The rest of the line as HTML.
    let html = state.html.get_or_insert_with(Box::default);
    let previous = piece(text, stream.pos..text.len(), out, |part, out| xml::line(part, html, out));
    stream.skip_to_end();
    (None, Some(previous).filter(|previous| *previous != Previous::None))
}
