//! Message sequence charts, as CodeMirror 5's mscgen mode colors them in its three languages
//! (mscgen, xu, msgenny), ported token for token: keywords, options, and arcs keywords, `=` an
//! operator, `[label=…]` attributes, braces brackets, strings, `#`, `//`, and `/* */` comments.

use super::Kind;
use super::scan::{self, Out, Stream};

/// The language's word lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Mscgen,
    Xu,
    Msgenny,
}

#[derive(Clone, Debug, Default)]
pub struct Mscgen {
    comment: bool,
    string: bool,
    /// Inside `[ ]` (the mode keeps it on the language, for every chart; here, per block).
    attributes: bool,
}

const ARCS: &[&str] = &[
    "|||", "...", "---", "--", "<->", "==", "<<=>>", "<=>", "..", "<<>>", "::", "<:>", "->", "=>>", "=>", ">>", ":>", "<-", "<<=", "<=", "<<", "<:",
    "x-", "-x",
];

struct Words {
    keywords: &'static str,
    options: &'static str,
    constants: &'static [&'static str],
    attributes: &'static str,
    arcs: &'static str,
}

fn words(dialect: Dialect) -> Words {
    let mscgen_attributes = "label|idurl|id|url|linecolor|linecolour|textcolor|textcolour|textbgcolor|textbgcolour|arclinecolor|\
arclinecolour|arctextcolor|arctextcolour|arctextbgcolor|arctextbgcolour|arcskip";
    let xu_arcs = "note|abox|rbox|box|alt|else|opt|break|par|seq|strict|neg|critical|ignore|consider|assert|loop|ref|exc";
    let xu_options = "hscale|width|arcgradient|wordwraparcs|wordwrapentities|watermark";
    match dialect {
        Dialect::Mscgen => Words {
            keywords: "msc",
            options: "hscale|width|arcgradient|wordwraparcs",
            constants: &["true", "false", "on", "off"],
            attributes: mscgen_attributes,
            arcs: "note|abox|rbox|box",
        },
        Dialect::Xu => Words {
            keywords: "msc|xu",
            options: xu_options,
            constants: &["true", "false", "on", "off", "auto"],
            attributes: "label|idurl|id|url|linecolor|linecolour|textcolor|textcolour|textbgcolor|textbgcolour|arclinecolor|\
arclinecolour|arctextcolor|arctextcolour|arctextbgcolor|arctextbgcolour|arcskip|title|deactivate|activate|activation",
            arcs: xu_arcs,
        },
        Dialect::Msgenny => {
            Words { keywords: "", options: xu_options, constants: &["true", "false", "on", "off", "auto"], attributes: "", arcs: xu_arcs }
        }
    }
}

/// Takes the first of `list` the position starts with, in any case (no word boundary).
fn take_prefix(stream: &mut Stream, list: &[&str]) -> bool {
    let found = list.iter().find(|item| stream.rest().get(..item.len()).is_some_and(|text| text.eq_ignore_ascii_case(item.as_bytes())));
    if let Some(item) = found {
        stream.pos += item.len();
    }
    found.is_some()
}

/// `^\b(?:…)\b` in any case: a word of `list` at a word's start.
fn take_word(stream: &mut Stream, list: &str) -> bool {
    !list.is_empty() && scan::js_word(stream.peek()) && stream.take_word_of(list, true)
}

pub fn line(text: &str, state: &mut Mscgen, dialect: Dialect, out: &mut Out) {
    let words = words(dialect);
    scan::run(text, out, |stream| token(stream, state, &words));
}

fn token(stream: &mut Stream, state: &mut Mscgen, words: &Words) -> Option<Kind> {
    if stream.take("{") || stream.take("}") {
        return Some(Kind::Punctuation);
    }
    if !state.comment {
        if stream.at("/*") {
            stream.pos += 2;
            stream.eat_while(|byte| byte != b'*' && byte != b'/');
            state.comment = true;
            return Some(Kind::Comment);
        }
        if stream.at("//") || stream.at("#") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
    }
    if state.comment {
        // `[^*/]*\*/`, else the line.
        let run = stream.rest().iter().position(|byte| *byte == b'*' || *byte == b'/');
        match run {
            Some(at) if stream.rest().get(at..at + 2) == Some(b"*/") => {
                stream.pos += at + 2;
                state.comment = false;
            }
            _ => stream.skip_to_end(),
        }
        return Some(Kind::Comment);
    }
    if !state.string && stream.take("\"") {
        // `"(\\"|[^"])*`: to the next quote not after a backslash.
        while let Some(here) = stream.next() {
            if here == b'\\' && stream.peek() == b'"' {
                stream.next();
            } else if here == b'"' {
                stream.back_up(1);
                break;
            }
        }
        state.string = true;
        return Some(Kind::String);
    }
    if state.string {
        match stream.rest().iter().position(|byte| *byte == b'"') {
            Some(at) => {
                stream.pos += at + 1;
                state.string = false;
            }
            None => stream.skip_to_end(),
        }
        return Some(Kind::String);
    }
    if take_word(stream, words.keywords) || take_word(stream, words.options) || take_word(stream, words.arcs) || take_prefix(stream, ARCS) {
        return Some(Kind::Keyword);
    }
    if stream.take("=") {
        return Some(Kind::Operator);
    }
    if take_prefix(stream, words.constants) {
        return Some(Kind::Variable);
    }
    if !state.attributes && !words.attributes.is_empty() && stream.take("[") {
        state.attributes = true;
        return Some(Kind::Punctuation);
    }
    if state.attributes {
        if take_word(stream, words.attributes) {
            return Some(Kind::Property);
        }
        if stream.take("]") {
            state.attributes = false;
            return Some(Kind::Punctuation);
        }
    }
    stream.next();
    None
}
