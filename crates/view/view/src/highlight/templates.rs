//! Grammars and templates, as their CodeMirror 5 modes color them, each ported token for token:
//! PEG.js grammars and Smarty templates (see each function).

use super::Kind;
use super::js;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Templates {
    /// PEG.js: an open string (its quote), comment, or character class; the open braces of an
    /// action and the JavaScript it is read with.
    string: Option<u8>,
    comment: bool,
    class: bool,
    braced: usize,
    js: js::Js,
    /// Smarty: inside a tag (how deep), in a tag's string, in a `{* *}` comment; the last token's
    /// kind.
    depth: usize,
    tag: bool,
    tag_string: Option<u8>,
    tag_comment: bool,
    last: Option<&'static str>,
}

/// PEG.js, as CodeMirror 5's pegjs mode colors it: rule names variables, strings properties,
/// `[classes]` and brackets, `//` and `/* */` comments, and `{ actions }` as JavaScript.
pub fn pegjs(text: &str, state: &mut Templates, out: &mut Out) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    let mut actions: Vec<std::ops::Range<usize>> = Vec::new();
    while !stream.eol() {
        stream.start = stream.pos;
        let style = pegjs_token(&mut stream, state, &mut actions);
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
    // The actions' JavaScript, read as the javascript mode read it: only the tokens it was given
    // (the mode reads strings, comments, and `[` classes itself), one after another.
    let mut joined = String::new();
    let mut starts = Vec::new();
    for range in &actions {
        starts.push((joined.len(), range.start));
        joined.push_str(text.get(range.clone()).unwrap_or(""));
    }
    let mut inner = Out::default();
    js::line(&joined, &mut state.js, js::Flavor::JavaScript, &mut inner);
    for (range, kind) in inner.finish() {
        // Each span back where its text stands (split where the line's pieces part).
        for (index, (from, at)) in starts.iter().enumerate() {
            let until = starts.get(index + 1).map_or(joined.len(), |(next, _)| *next);
            let start = range.start.max(*from);
            let end = range.end.min(until);
            if start < end {
                out.push(start - from + at..end - from + at, kind);
            }
        }
    }
}

fn pegjs_token(stream: &mut Stream, state: &mut Templates, actions: &mut Vec<std::ops::Range<usize>>) -> Option<Kind> {
    if state.string.is_none() && !state.comment && matches!(stream.peek(), b'"' | b'\'') {
        state.string = Some(stream.peek());
        stream.next();
    }
    if state.string.is_none() && !state.comment && stream.take("/*") {
        state.comment = true;
    }
    if let Some(quote) = state.string {
        while state.string.is_some() && !stream.eol() {
            if stream.peek() == quote {
                stream.next();
                state.string = None;
            } else if stream.peek() == b'\\' {
                stream.next();
                stream.next();
            } else {
                stream.next();
                stream.eat_while(|byte| !b"\\\"'".contains(&byte));
            }
        }
        return Some(Kind::Property);
    }
    if state.comment {
        while state.comment && !stream.eol() {
            if stream.take("*/") {
                state.comment = false;
            } else {
                stream.next();
                stream.eat_while(|byte| byte != b'*');
            }
        }
        return Some(Kind::Comment);
    }
    if state.class {
        while state.class && !stream.eol() {
            if stream.eat_while(|byte| byte != b']' && byte != b'\\') {
                continue;
            }
            if stream.peek() == b'\\' && stream.pos + 1 < stream.bytes.len() {
                stream.pos += 2;
                continue;
            }
            state.class = false;
        }
        return None;
    }
    if stream.peek() == b'[' {
        stream.next();
        state.class = true;
        return Some(Kind::Punctuation);
    }
    if stream.take("//") {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if state.braced > 0 || stream.peek() == b'{' {
        // JavaScript to the next string, comment, or `[` (which the mode reads itself) or to
        // where the action's braces close, counting the braces it reads as punctuation.
        let bytes = stream.bytes;
        let mut at = stream.pos;
        let mut braced = state.braced;
        while at < bytes.len() {
            let here = bytes.get(at).copied().unwrap_or(0);
            if at > stream.pos && (matches!(here, b'"' | b'\'' | b'[') || scan::at(bytes, at, "/*") || scan::at(bytes, at, "//")) {
                break;
            }
            if here == b'{' {
                braced += 1;
            } else if here == b'}' {
                braced = braced.saturating_sub(1);
                if braced == 0 {
                    at += 1;
                    break;
                }
            }
            at += 1;
        }
        state.braced = braced;
        actions.push(stream.pos..at);
        stream.pos = at;
        return None;
    }
    if stream.peek().is_ascii_alphabetic() || stream.peek() == b'_' {
        stream.eat_while(scan::js_word);
        return Some(Kind::Variable);
    }
    if b"[]()".contains(&stream.peek()) {
        stream.next();
        return Some(Kind::Punctuation);
    }
    if !stream.eat_space() {
        stream.next();
    }
    None
}

/// Smarty templates, as CodeMirror 5's smarty mode colors them (version 2, `{` and `}`): the
/// text plain, tags and their names, `$variables` variable-2, `|modifiers` qualifiers,
/// attributes, operators, numbers, strings, `{* *}` comments.
pub fn smarty(text: &str, state: &mut Templates, out: &mut Out) {
    scan::run(text, out, |stream| smarty_token(stream, state));
}

/// A token's style; it sets the kind the next token reads (`state.last`, the mode's `p`, which
/// stays when a token does not set it).
fn smarty_token(stream: &mut Stream, state: &mut Templates) -> Option<Kind> {
    if state.tag_comment {
        while !stream.eol() {
            if stream.take("*}") {
                state.tag_comment = false;
                break;
            }
            stream.next();
        }
        return Some(Kind::Comment);
    }
    if let Some(quote) = state.tag_string {
        let mut previous = 0;
        while !stream.eol() {
            let here = stream.peek();
            if stream.next() == Some(quote) && previous != b'\\' {
                state.tag_string = None;
                break;
            }
            previous = here;
        }
        return Some(Kind::String);
    }
    if !state.tag {
        // The text to the next `{` (the base mode, none, colors it plain), or a tag there.
        if stream.take("{") {
            if stream.eat(|byte| byte == b'*').is_some() {
                state.tag_comment = true;
                while !stream.eol() {
                    if stream.take("*}") {
                        state.tag_comment = false;
                        break;
                    }
                    stream.next();
                }
                return Some(Kind::Comment);
            }
            state.depth += 1;
            state.tag = true;
            state.last = Some("startTag");
            return Some(Kind::Operator);
        }
        match stream.rest().iter().position(|byte| *byte == b'{') {
            Some(at) => stream.pos += at,
            None => stream.skip_to_end(),
        }
        return None;
    }
    if stream.take("}") {
        state.tag = false;
        state.last = None;
        return Some(Kind::Operator);
    }
    if stream.take("{") {
        state.depth += 1;
        state.last = Some("startTag");
        return Some(Kind::Operator);
    }
    let ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let here = stream.next()?;
    match here {
        b'$' => {
            stream.eat_while(ident);
            return mark(state, "variable", Some(Kind::Variable));
        }
        b'|' => return mark(state, "pipe", Some(Kind::Operator)),
        b'.' => return mark(state, "property", Some(Kind::Operator)),
        b'"' | b'\'' => {
            state.tag_string = Some(here);
            state.last = Some("string");
            return Some(Kind::String);
        }
        _ => {}
    }
    let operator = |byte: u8| b"+-*&%=<>!?".contains(&byte);
    if operator(here) {
        stream.eat_while(operator);
        return mark(state, "operator", Some(Kind::Operator));
    }
    if here == b'[' || here == b']' {
        return mark(state, "bracket", Some(Kind::Punctuation));
    }
    if here == b'(' || here == b')' {
        return mark(state, "operator", Some(Kind::Punctuation));
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| byte.is_ascii_digit());
        return mark(state, "number", Some(Kind::Number));
    }
    let last = state.last;
    if last == Some("variable") {
        if here == b'@' {
            stream.eat_while(ident);
            state.last = Some("property");
            return Some(Kind::Property);
        }
    } else if last == Some("pipe") {
        stream.eat_while(ident);
        state.last = Some("modifier");
        return Some(Kind::String);
    } else if last == Some("whitespace") {
        stream.eat_while(ident);
        state.last = Some("modifier");
        return Some(Kind::Property);
    }
    if last == Some("property") {
        stream.eat_while(ident);
        state.last = None;
        return Some(Kind::Property);
    }
    if here.is_ascii_whitespace() {
        state.last = Some("whitespace");
        return None;
    }
    let start = if here == b'/' { stream.pos } else { stream.pos - 1 };
    stream.eat_while(ident);
    let word = stream.bytes.get(start..stream.pos).and_then(|word| std::str::from_utf8(word).ok()).unwrap_or("");
    if ["debug", "extends", "function", "include", "literal"].contains(&word) {
        state.last = Some("keyword");
        return Some(Kind::Keyword);
    }
    state.last = Some("tag");
    Some(Kind::Operator)
}

/// Sets the kind the next token reads, and gives the style.
fn mark(state: &mut Templates, last: &'static str, style: Option<Kind>) -> Option<Kind> {
    state.last = Some(last);
    style
}
