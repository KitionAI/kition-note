//! HTML and XML, as CodeMirror 5's xml and htmlmixed modes color them: a tag's brackets are
//! brackets, its name a tag, its attributes attributes and their values strings, an entity an
//! atom, a doctype meta; a `<script>`'s body is JavaScript and a `<style>`'s CSS.

use super::scan::{self, Out};
use super::{Kind, State, css, js};

/// Where a line goes on inside a tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tag {
    #[default]
    Outside,
    /// Its name is next (after `<` or `</`).
    Name,
    /// Its attributes (an `=` makes the next word a value).
    Attributes { value: bool },
}

/// A body in another language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inner {
    Script,
    Style,
}

#[derive(Clone, Debug, Default)]
pub struct Xml {
    tag: Tag,
    /// The tag being read (`script`, `style`: what its body is).
    name: String,
    comment: bool,
    inner: Option<Inner>,
    /// An attribute's quoted value left open (its quote): it runs on to the quote.
    string: Option<u8>,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn name_char(byte: u8) -> bool {
    scan::word(byte) || matches!(byte, b'-' | b':' | b'.')
}

pub fn line(text: &str, state: &mut State, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if let Some(quote) = state.xml.string {
            let end = bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == quote)).map_or(bytes.len(), |found| at + found + 1);
            out.push(at..end, Kind::String);
            if end <= bytes.len() && bytes.get(end - 1) == Some(&quote) {
                state.xml.string = None;
            }
            at = end;
            continue;
        }
        if state.xml.comment {
            match text.get(at..).and_then(|rest| rest.find("-->")) {
                Some(found) => {
                    out.push(at..at + found + 3, Kind::Comment);
                    state.xml.comment = false;
                    at += found + 3;
                }
                None => {
                    out.push(at..bytes.len(), Kind::Comment);
                    return;
                }
            }
            continue;
        }
        if let Some(inner) = state.xml.inner {
            let close = if inner == Inner::Script { "</script" } else { "</style" };
            let rest = text.get(at..).unwrap_or("");
            let end = rest.to_ascii_lowercase().find(close).map_or(bytes.len(), |found| at + found);
            let mut part = Out::default();
            let slice = text.get(at..end).unwrap_or("");
            match inner {
                Inner::Script => js::line(slice, &mut state.js, js::Flavor::JavaScript, &mut part),
                Inner::Style => css::line(slice, &mut state.css, false, &mut part),
            }
            out.extend_shifted(part, at);
            at = end;
            if end < bytes.len() {
                state.xml.inner = None;
            }
            continue;
        }
        let here = byte(bytes, at);
        let start = at;
        match state.xml.tag {
            Tag::Outside => {
                if scan::at(bytes, at, "<!--") {
                    state.xml.comment = true;
                    out.push(at..at + 4, Kind::Comment);
                    at += 4;
                } else if scan::at(bytes, at, "<![CDATA[") {
                    at = text.get(at..).and_then(|rest| rest.find("]]>")).map_or(bytes.len(), |found| at + found + 3);
                } else if scan::at(bytes, at, "<!") || scan::at(bytes, at, "<?") {
                    at = text.get(at..).and_then(|rest| rest.find('>')).map_or(bytes.len(), |found| at + found + 1);
                    out.push(start..at, Kind::Comment);
                } else if here == b'<' && (name_char(byte(bytes, at + 1)) || byte(bytes, at + 1) == b'/') {
                    at += if byte(bytes, at + 1) == b'/' { 2 } else { 1 };
                    out.push(start..at, Kind::Punctuation);
                    state.xml.tag = Tag::Name;
                    state.xml.name.clear();
                    if at - start == 2 {
                        state.xml.name.push('/');
                    }
                } else if here == b'&' {
                    let end = text.get(at..).and_then(|rest| rest.find(';')).filter(|found| *found < 12).map_or(at + 1, |found| at + found + 1);
                    at = end;
                } else {
                    at += 1;
                }
            }
            Tag::Name => {
                if name_char(here) {
                    while name_char(byte(bytes, at)) {
                        at += 1;
                    }
                    out.push(start..at, Kind::Operator);
                    state.xml.name.push_str(&text.get(start..at).unwrap_or("").to_ascii_lowercase());
                }
                state.xml.tag = Tag::Attributes { value: false };
            }
            Tag::Attributes { value } => match here {
                b' ' | b'\t' | b'\r' => at += 1,
                b'>' => {
                    at += 1;
                    out.push(start..at, Kind::Punctuation);
                    end_tag(state, false);
                }
                b'/' if byte(bytes, at + 1) == b'>' => {
                    at += 2;
                    out.push(start..at, Kind::Punctuation);
                    end_tag(state, true);
                }
                b'=' => {
                    at += 1;
                    state.xml.tag = Tag::Attributes { value: true };
                }
                b'"' | b'\'' => {
                    // To the quote (no escapes in an attribute); open, it runs on.
                    match bytes.get(at + 1..).and_then(|rest| rest.iter().position(|byte| *byte == here)) {
                        Some(found) => at += found + 2,
                        None => {
                            at = bytes.len();
                            state.xml.string = Some(here);
                        }
                    }
                    out.push(start..at, Kind::String);
                    state.xml.tag = Tag::Attributes { value: false };
                }
                _ if value => {
                    while at < bytes.len() && !matches!(byte(bytes, at), b' ' | b'\t' | b'>' | b'<' | b'=' | b'"' | b'\'') {
                        at += 1;
                    }
                    out.push(start..at, Kind::String);
                    state.xml.tag = Tag::Attributes { value: false };
                }
                _ => {
                    while at < bytes.len() && !matches!(byte(bytes, at), b' ' | b'\t' | b'>' | b'=' | b'"' | b'\'') && !scan::at(bytes, at, "/>") {
                        at += 1;
                    }
                    at = at.max(start + 1);
                    out.push(start..at, Kind::Property);
                }
            },
        }
    }
}

/// A tag's `>`: a `<script>` or `<style>` opens its body.
fn end_tag(state: &mut State, closed: bool) {
    state.xml.tag = Tag::Outside;
    if !closed {
        state.xml.inner = match state.xml.name.as_str() {
            "script" => Some(Inner::Script),
            "style" => Some(Inner::Style),
            _ => None,
        };
    }
}
