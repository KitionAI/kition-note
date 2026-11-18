//! Pug (Jade), as CodeMirror 5's pug mode colors it, ported token for token: tags, `#id`
//! builtins, `.class` qualifiers, `(attributes)` (their values JavaScript), keywords (`if`,
//! `each`, `mixin`, `include`, …), `+mixin` calls, `= expression` and `- code` lines and
//! `#{…}` interpolation as JavaScript, text strings, `//` comments (and the lines indented under
//! them), `doctype` meta, and the blocks under `script.` and `style.` as JavaScript and CSS, a
//! line starting `<` as HTML. JavaScript is colored by the JavaScript port, a piece at a time.

use super::Kind;
use super::scan::{self, Out, Stream};
use super::{State, css, js, xml};

#[derive(Clone, Debug, Default)]
pub struct Pug {
    javascript_line: bool,
    excludes_colon: bool,
    arguments: bool,
    mixin_call_after: bool,
    interpolating: bool,
    rest_of_line: Option<Option<Kind>>,
    each: bool,
    last_tag: String,
    script_type: String,
    attrs: bool,
    attrs_nest: Vec<u8>,
    in_attribute_name: bool,
    attribute_is_type: bool,
    attr_value: String,
    /// The indentation lines must pass to belong to the block above (none: no block).
    indent_of: Option<usize>,
    indent_token: Option<Kind>,
    inner: Option<Inner>,
    inner_for_line: bool,
    /// The JavaScript the template's code is read with, and the block's own.
    js: js::Js,
    inner_js: js::Js,
    inner_css: css::Css,
}

/// A block's own language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inner {
    JavaScript,
    Css,
    Html,
}

fn indentation(text: &str) -> usize {
    text.bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count()
}

/// Where a JavaScript token starting at `at` ends (spaces, a string, a comment, a number, a
/// name, an operator run, or one byte), as the javascript mode reads one.
fn js_token_end(bytes: &[u8], at: usize) -> usize {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let len = bytes.len();
    let run = |from: usize, ok: &dyn Fn(u8) -> bool| (from..len).find(|end| !ok(byte(*end))).unwrap_or(len.max(from));
    let here = byte(at);
    if at >= len {
        return at;
    }
    if here == b' ' || here == b'\t' {
        return run(at, &|next| next == b' ' || next == b'\t');
    }
    if matches!(here, b'"' | b'\'' | b'`') {
        return scan::string_end(bytes, at, here).unwrap_or(len);
    }
    if here == b'/' && byte(at + 1) == b'/' {
        return len;
    }
    if here == b'/' && byte(at + 1) == b'*' {
        return bytes.get(at + 2..).and_then(|rest| rest.windows(2).position(|pair| pair == b"*/")).map_or(len, |found| at + 2 + found + 2);
    }
    if here.is_ascii_digit() {
        return run(at, &|next| scan::js_word(next) || next == b'.');
    }
    if scan::js_word(here) || here == b'$' || here >= 0x80 {
        return run(at, &|next| scan::js_word(next) || next == b'$' || next >= 0x80);
    }
    if b"+-*&%=<>!?|~^@".contains(&here) {
        return run(at, &|next| b"+-*&%=<>!?|~^@".contains(&next));
    }
    at + 1
}

const JS_KEYWORD_OPERATORS: &[&str] = &["typeof", "new", "void", "delete", "in", "instanceof", "function", "var", "let", "const", "return"];

/// Whether `var x <value>` is a whole JavaScript statement (the mode tries it with `Function`):
/// nothing, or `=` and a complete expression.
fn complete_initializer(value: &str) -> bool {
    let value = value.trim_end();
    let value = value.strip_suffix(',').map_or(value, str::trim_end);
    let value = value.strip_prefix('!').unwrap_or(value);
    let value = value.trim();
    if value.is_empty() {
        return true;
    }
    let Some(expression) = value.strip_prefix('=') else { return false };
    if expression.starts_with('=') {
        return false;
    }
    let bytes = expression.as_bytes();
    let mut at = 0;
    let mut depth: Vec<u8> = Vec::new();
    let mut last: Option<&[u8]> = None;
    while at < bytes.len() {
        let end = js_token_end(bytes, at);
        let token = bytes.get(at..end).unwrap_or_default();
        at = end.max(at + 1);
        let first = token.first().copied().unwrap_or(0);
        if first == b' ' || first == b'\t' || (first == b'/' && matches!(token.get(1), Some(b'/' | b'*'))) {
            continue;
        }
        if matches!(first, b'"' | b'\'' | b'`') && (token.len() < 2 || token.last() != Some(&first)) {
            return false;
        }
        match first {
            b'(' | b'[' | b'{' => depth.push(first),
            b')' | b']' | b'}' => {
                let open = match first {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                if depth.pop() != Some(open) {
                    return false;
                }
            }
            _ => {}
        }
        last = Some(token);
    }
    let Some(last) = last else { return false };
    let first = last.first().copied().unwrap_or(0);
    let operator = b"+-*&%=<>!?|~^@.,:;(".contains(&first) && !(last == b"++" || last == b"--");
    let keyword = std::str::from_utf8(last).is_ok_and(|word| JS_KEYWORD_OPERATORS.contains(&word));
    depth.is_empty() && !operator && !keyword && first != b'['
}

/// What the javascript mode would call a token: a string, a variable (a name), a keyword.
fn js_style_counts(token: &[u8]) -> bool {
    let first = token.first().copied().unwrap_or(0);
    if matches!(first, b'"' | b'\'') {
        return true;
    }
    if scan::js_word(first) && !first.is_ascii_digit() || first == b'$' {
        let word = std::str::from_utf8(token).unwrap_or("");
        return !["true", "false", "null", "undefined", "NaN", "Infinity"].contains(&word);
    }
    false
}

/// Colors `range` of the line as JavaScript with `state`.
fn javascript(text: &str, range: std::ops::Range<usize>, state: &mut js::Js, out: &mut Out) {
    let Some(piece) = text.get(range.clone()) else { return };
    let mut inner = Out::default();
    js::line(piece, state, js::Flavor::JavaScript, &mut inner);
    out.extend_shifted(inner, range.start);
}

pub fn line(text: &str, state: &mut Pug, out: &mut Out) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        let style = token(text, &mut stream, state, out);
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// The mode's `h`: a line under a block goes to the block's language.
fn block_line(text: &str, stream: &mut Stream, state: &mut Pug, out: &mut Out, force: bool) -> Option<Option<Kind>> {
    let deeper = state.indent_of.is_some_and(|indent_of| indentation(text) > indent_of);
    if deeper || (state.inner_for_line && !stream.sol()) || force {
        let at = stream.pos;
        match state.inner {
            Some(Inner::JavaScript) => {
                javascript(text, at..text.len(), &mut state.inner_js, out);
                stream.skip_to_end();
                return Some(None);
            }
            Some(Inner::Css) => {
                let mut inner = Out::default();
                css::line(text.get(at..).unwrap_or(""), &mut state.inner_css, false, &mut inner);
                out.extend_shifted(inner, at);
                stream.skip_to_end();
                return Some(None);
            }
            Some(Inner::Html) => {
                let mut inner = Out::default();
                let mut html = State::default();
                xml::line(text.get(at..).unwrap_or(""), &mut html, &mut inner);
                out.extend_shifted(inner, at);
                stream.skip_to_end();
                return Some(None);
            }
            None => {
                stream.skip_to_end();
                // A block with no language of its own colors its lines as its token, or (with
                // none) not at all: the mode reads on with nothing left.
                return state.indent_token.map(Some);
            }
        }
    }
    if stream.sol() {
        state.indent_of = None;
        state.indent_token = None;
        state.inner = None;
    }
    None
}

/// The mode's `l`: a block whose lines (indented under this one) are in `language`.
fn open_block(text: &str, state: &mut Pug, language: Option<Inner>) {
    state.indent_of = Some(indentation(text));
    state.inner = language;
    match language {
        Some(Inner::JavaScript) => state.inner_js = js::Js::default(),
        Some(Inner::Css) => state.inner_css = css::Css::default(),
        _ => {}
    }
    if language.is_none() {
        state.indent_token = Some(Kind::String);
    }
}

fn word_at(stream: &Stream, word: &str) -> bool {
    stream.at(word) && !scan::js_word(stream.byte(stream.pos + word.len()))
}

fn token(text: &str, stream: &mut Stream, state: &mut Pug, out: &mut Out) -> Option<Kind> {
    if let Some(style) = block_line(text, stream, state, out, false) {
        return style;
    }
    if stream.sol() {
        state.rest_of_line = None;
    }
    if let Some(style) = state.rest_of_line.take() {
        stream.skip_to_end();
        return style;
    }
    if state.interpolating {
        // JavaScript to the `}` that closes the `#{`.
        let bytes = stream.bytes;
        let mut at = stream.pos;
        let mut nesting = 0usize;
        while at < bytes.len() {
            match bytes.get(at) {
                Some(b'}') if nesting == 0 => break,
                Some(b'}') => nesting -= 1,
                Some(b'{') => nesting += 1,
                _ => {}
            }
            at = js_token_end(bytes, at).max(at + 1);
        }
        if at > stream.pos {
            javascript(text, stream.pos..at, &mut state.js, out);
            stream.pos = at;
            return None;
        }
        stream.next();
        state.interpolating = false;
        return Some(Kind::Punctuation);
    }
    if state.each {
        if stream.at(" in") && !scan::js_word(stream.byte(stream.pos + 3)) {
            stream.pos += 3;
            state.javascript_line = true;
            state.each = false;
            return Some(Kind::Keyword);
        }
        if stream.sol() || stream.eol() {
            state.each = false;
        } else {
            stream.next();
            while (!stream.at(" in") || scan::js_word(stream.byte(stream.pos + 3))) && stream.next().is_some() {}
            return Some(Kind::Variable);
        }
    }
    if state.attrs {
        return attribute(text, stream, state, out);
    }
    if stream.sol() {
        state.javascript_line = false;
        state.excludes_colon = false;
    }
    if state.javascript_line {
        let mut end = text.len();
        if state.excludes_colon {
            // To a `:` where a token starts.
            let mut at = stream.pos;
            while at < text.len() {
                if stream.byte(at) == b':' {
                    break;
                }
                at = js_token_end(stream.bytes, at).max(at + 1);
            }
            end = at;
        }
        if end > stream.pos {
            javascript(text, stream.pos..end, &mut state.js, out);
            stream.pos = end;
            if stream.eol() {
                state.javascript_line = false;
            }
            return None;
        }
        state.javascript_line = false;
        state.excludes_colon = false;
    }
    if state.arguments {
        if stream.peek() == b'(' {
            // JavaScript to the `)` that closes the `(`.
            let bytes = stream.bytes;
            let mut at = stream.pos;
            let mut depth = 0usize;
            while at < bytes.len() {
                match bytes.get(at) {
                    Some(b'(') => depth += 1,
                    Some(b')') => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                at = js_token_end(bytes, at).max(at + 1);
            }
            javascript(text, stream.pos..at, &mut state.js, out);
            stream.pos = at;
            if at < bytes.len() {
                state.arguments = false;
            }
            return None;
        }
        state.arguments = false;
    }
    if state.mixin_call_after {
        state.mixin_call_after = false;
        if !named_arguments(stream) {
            state.arguments = true;
        }
        return None;
    }
    base(text, stream, state, out)
}

/// `^\( *[-\w]+ *=`: a mixin's arguments named, as attributes.
fn named_arguments(stream: &Stream) -> bool {
    let rest = stream.rest();
    if rest.first() != Some(&b'(') {
        return false;
    }
    let spaces = |from: usize| (from..rest.len()).find(|at| rest.get(*at) != Some(&b' ')).unwrap_or(rest.len());
    let name = spaces(1);
    let end = (name..rest.len()).find(|at| !rest.get(*at).is_some_and(|byte| scan::js_word(*byte) || *byte == b'-')).unwrap_or(rest.len());
    end > name && rest.get(spaces(end)) == Some(&b'=')
}

fn attribute(text: &str, stream: &mut Stream, state: &mut Pug, out: &mut Out) -> Option<Kind> {
    let peek = stream.peek();
    let closer = match peek {
        b'{' => Some(b'}'),
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        _ => None,
    };
    if let Some(closer) = closer {
        state.attrs_nest.push(closer);
    }
    if state.attrs_nest.last() == Some(&peek) {
        state.attrs_nest.pop();
    } else if stream.take(")") {
        state.attrs = false;
        return Some(Kind::Punctuation);
    }
    if state.in_attribute_name && stream.eat_while(|byte| !b"=,)!".contains(&byte)) {
        if matches!(stream.peek(), b'=' | b'!') {
            state.in_attribute_name = false;
            state.js = js::Js::default();
            state.attribute_is_type = state.last_tag == "script" && stream.current().trim().eq_ignore_ascii_case("type");
        }
        return Some(Kind::Property);
    }
    // The value, a JavaScript token at a time, to the token that starts the next attribute.
    let bytes = stream.bytes;
    let start = stream.pos;
    let mut at = start;
    let mut nest = state.attrs_nest.clone();
    let mut value = state.attr_value.clone();
    let mut restart = false;
    // In a name, one token (the next may be the name); in a value, all of it.
    let single = state.in_attribute_name;
    while at < bytes.len() && !(single && at > start) {
        let here = bytes.get(at).copied().unwrap_or(0);
        if at > start {
            let closer = match here {
                b'{' => Some(b'}'),
                b'(' => Some(b')'),
                b'[' => Some(b']'),
                _ => None,
            };
            if let Some(closer) = closer {
                nest.push(closer);
            }
            if nest.last() == Some(&here) {
                nest.pop();
            } else if here == b')' {
                break;
            }
        }
        let end = js_token_end(bytes, at).max(at + 1);
        let piece = bytes.get(at..end).unwrap_or_default();
        if state.attribute_is_type && matches!(piece.first(), Some(b'"' | b'\'')) {
            state.script_type = std::str::from_utf8(piece).unwrap_or("").to_string();
        }
        if nest.is_empty() && js_style_counts(piece) && complete_initializer(&value) {
            restart = true;
            break;
        }
        value.push_str(std::str::from_utf8(piece).unwrap_or(""));
        at = end;
    }
    state.attrs_nest = nest;
    if at > start {
        javascript(text, start..at, &mut state.js, out);
    }
    stream.pos = at;
    if restart {
        state.in_attribute_name = true;
        state.attr_value.clear();
    } else {
        state.attr_value = value;
    }
    None
}

fn base(text: &str, stream: &mut Stream, state: &mut Pug, out: &mut Out) -> Option<Kind> {
    if word_at(stream, "yield") {
        stream.pos += 5;
        return Some(Kind::Keyword);
    }
    if stream.at("doctype") {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if stream.take("#{") {
        state.interpolating = true;
        return Some(Kind::Punctuation);
    }
    for (word, line, colon) in [("case", true, false), ("when", true, true), ("default", false, false)] {
        if word_at(stream, word) {
            stream.pos += word.len();
            state.javascript_line = line;
            state.excludes_colon = colon;
            return Some(Kind::Keyword);
        }
    }
    for (word, rest) in [("extends", Kind::String), ("extend", Kind::String), ("append", Kind::Variable), ("prepend", Kind::Variable)] {
        if word_at(stream, word) {
            stream.pos += word.len();
            state.rest_of_line = Some(Some(rest));
            return Some(Kind::Keyword);
        }
    }
    if word_at(stream, "block") {
        stream.pos += 5;
        // `block\b *(?:(prepend|append)\b)?`
        stream.pos = (stream.pos..text.len()).find(|at| stream.byte(*at) != b' ').unwrap_or(text.len());
        for word in ["prepend", "append"] {
            if word_at(stream, word) {
                stream.pos += word.len();
                break;
            }
        }
        state.rest_of_line = Some(Some(Kind::Variable));
        return Some(Kind::Keyword);
    }
    if word_at(stream, "include") {
        stream.pos += 7;
        state.rest_of_line = Some(Some(Kind::String));
        return Some(Kind::Keyword);
    }
    if word_at(stream, "mixin") {
        stream.pos += 5;
        state.javascript_line = true;
        return Some(Kind::Keyword);
    }
    if stream.peek() == b'+' {
        let end = (stream.pos + 1..text.len()).find(|at| !(scan::js_word(stream.byte(*at)) || stream.byte(*at) == b'-')).unwrap_or(text.len());
        if end > stream.pos + 1 {
            stream.pos = end;
            if !named_arguments(stream) {
                state.arguments = true;
            }
            return Some(Kind::Variable);
        }
        if stream.at("+#{") {
            stream.next();
            state.mixin_call_after = true;
            stream.take("#{");
            state.interpolating = true;
            return Some(Kind::Punctuation);
        }
    }
    for word in ["if", "unless", "else if", "else"] {
        if word_at(stream, word) {
            stream.pos += word.len();
            state.javascript_line = true;
            return Some(Kind::Keyword);
        }
    }
    {
        let dash =
            if stream.peek() == b'-' { (stream.pos + 1..text.len()).find(|at| stream.byte(*at) != b' ').unwrap_or(text.len()) } else { stream.pos };
        for word in ["each", "for"] {
            if scan::at(stream.bytes, dash, word) && !scan::js_word(stream.byte(dash + word.len())) {
                stream.pos = dash + word.len();
                state.each = true;
                return Some(Kind::Keyword);
            }
        }
    }
    if word_at(stream, "while") {
        stream.pos += 5;
        state.javascript_line = true;
        return Some(Kind::Keyword);
    }
    // A tag: `\w(?:[-:\w]*\w)?` and an optional `/`.
    if scan::js_word(stream.peek()) {
        let mut end = stream.pos + 1;
        let run = (end..text.len()).find(|at| !(scan::js_word(stream.byte(*at)) || matches!(stream.byte(*at), b'-' | b':'))).unwrap_or(text.len());
        if let Some(last) = (end..run).rev().find(|at| scan::js_word(stream.byte(*at))) {
            end = last + 1;
        }
        state.last_tag = text.get(stream.pos..end).unwrap_or("").to_ascii_lowercase();
        if state.last_tag == "script" {
            state.script_type = "application/javascript".to_string();
        }
        stream.pos = end;
        stream.take("/");
        return Some(Kind::Operator);
    }
    if stream.peek() == b':' {
        let end = (stream.pos + 1..text.len()).find(|at| !(scan::js_word(stream.byte(*at)) || stream.byte(*at) == b'-')).unwrap_or(text.len());
        if end > stream.pos + 1 {
            let name = text.get(stream.pos + 1..end).unwrap_or("");
            stream.pos = end;
            let language = match name {
                "javascript" | "js" => Some(Inner::JavaScript),
                "css" => Some(Inner::Css),
                _ => None,
            };
            open_block(text, state, language);
            return None;
        }
    }
    if stream.take("!=") || stream.take("=") || stream.take("-") {
        state.javascript_line = true;
        return Some(Kind::Punctuation);
    }
    for (mark, kind) in [(b'#', Kind::Property), (b'.', Kind::String)] {
        if stream.peek() == mark {
            let end = (stream.pos + 1..text.len()).find(|at| !(scan::js_word(stream.byte(*at)) || stream.byte(*at) == b'-')).unwrap_or(text.len());
            if end > stream.pos + 1 {
                stream.pos = end;
                return Some(kind);
            }
        }
    }
    if stream.take("(") {
        state.attrs = true;
        state.attrs_nest.clear();
        state.in_attribute_name = true;
        state.attr_value.clear();
        state.attribute_is_type = false;
        return Some(Kind::Punctuation);
    }
    if word_at(stream, "&attributes") {
        stream.pos += 11;
        state.arguments = true;
        return Some(Kind::Keyword);
    }
    if stream.sol() && stream.eat_space() {
        return None;
    }
    // `(?:\| ?| )([^\n]+)`: text, at least a byte after the `|` or the space.
    if (stream.peek() == b'|' || stream.peek() == b' ') && stream.pos + 1 < text.len() {
        stream.skip_to_end();
        return Some(Kind::String);
    }
    if stream.peek() == b'<' {
        open_block(text, state, Some(Inner::Html));
        state.inner_for_line = true;
        return block_line(text, stream, state, out, true).flatten();
    }
    let spaces = (stream.pos..text.len()).find(|at| stream.byte(*at) != b' ').unwrap_or(text.len());
    if scan::at(stream.bytes, spaces, "//") {
        stream.skip_to_end();
        state.indent_of = Some(indentation(text));
        state.indent_token = Some(Kind::Comment);
        return Some(Kind::Comment);
    }
    if stream.take(":") {
        stream.eat_while(|byte| byte == b' ');
        return None;
    }
    if stream.take(".") {
        let language = if state.last_tag == "script" && state.script_type.to_ascii_lowercase().contains("javascript") {
            Some(Inner::JavaScript)
        } else if state.last_tag == "style" {
            Some(Inner::Css)
        } else {
            None
        };
        open_block(text, state, language);
        return None;
    }
    stream.next();
    None
}
