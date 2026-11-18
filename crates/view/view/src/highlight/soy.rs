//! Closure Templates (Soy), as CodeMirror 5's soy mode colors them, ported token for token with
//! its stack of states: commands keywords (an unmatched end tag an error), template names
//! definitions, `$variables` variable-2 (an unknown one an error), parameters and their types,
//! literals, operators, strings, comments; the text between commands as HTML (or the CSS,
//! JavaScript, or plain text a `kind` asks for), colored by those ports a piece at a time.

use super::Kind;
use super::scan::{self, Out, Stream};
use super::{State, css, js, xml};

/// The tags the mode knows: whether one has no end tag, the state it opens, whether it scopes
/// variables.
fn tag_info(name: &str) -> Option<(bool, Option<&'static str>, bool)> {
    Some(match name {
        "alias" | "delpackage" | "print" | "fallbackmsg" | "elseif" | "else" | "case" | "default" | "ifempty" => (true, None, false),
        "namespace" => (true, Some("namespace-def"), false),
        "@attribute" | "@attribute?" | "@param" | "@param?" | "@inject" | "@inject?" | "@state" => (true, Some("param-def"), false),
        "template" | "deltemplate" => (false, Some("templ-def"), true),
        "extern" => (false, Some("param-def"), false),
        "export" => (false, Some("export"), false),
        "let" => (false, Some("var-def"), false),
        "foreach" | "for" => (false, Some("for-loop"), true),
        "call" | "delcall" => (false, Some("templ-ref"), false),
        "param" => (false, Some("param-ref"), false),
        "const" => (false, Some("const-def"), false),
        "element" => (false, None, true),
        "literal" | "msg" | "select" | "plural" | "if" | "javaimpl" | "jsimpl" | "switch" | "log" | "velog" => (false, None, false),
        _ => return None,
    })
}

#[derive(Clone, Debug)]
struct Context {
    tag: String,
    kind: bool,
    /// The variables to go back to when it closes (a scoping tag's).
    scope: Option<Vec<String>>,
}

/// A language the text between commands is read in.
#[derive(Clone, Debug)]
enum Local {
    Html(Box<State>),
    Css(css::Css),
    Js(js::Js),
    Text,
}

#[derive(Clone, Debug)]
pub struct Soy {
    states: Vec<&'static str>,
    variables: Vec<String>,
    contexts: Vec<Context>,
    quote: Option<u8>,
    lookup: bool,
    tag: Option<String>,
    locals: Vec<Local>,
}

impl Default for Soy {
    fn default() -> Self {
        Soy { states: Vec::new(), variables: vec!["ij".to_string()], contexts: Vec::new(), quote: None, lookup: true, tag: None, locals: Vec::new() }
    }
}

impl Soy {
    fn top(&self) -> Option<&'static str> {
        self.states.last().copied()
    }
    /// Leaves the innermost context (its variables back as they were).
    fn pop_context(&mut self) {
        if let Some(context) = self.contexts.pop()
            && let Some(scope) = context.scope
        {
            self.variables = scope;
        }
    }
}

fn word_end(stream: &Stream, from: usize) -> usize {
    (from..stream.bytes.len()).find(|at| !scan::js_word(stream.byte(*at))).unwrap_or(stream.bytes.len().max(from))
}

/// A `$name`'s style: known, a variable-2; else a variable (in a list's `for`) or an error.
fn variable_style(state: &Soy, name: &str) -> Option<Kind> {
    // The mode's `variable-2 error`: the error's color.
    if state.variables.iter().any(|known| known == name) || !state.lookup { Some(Kind::Variable) } else { Some(Kind::Operator) }
}

pub fn line(text: &str, state: &mut Soy, out: &mut Out) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        let mut style = None;
        for _ in 0..10 {
            style = token(text, &mut stream, state, out);
            if stream.pos > stream.start {
                break;
            }
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
            style = None;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// The expression tokenizer (the mode's `v`).
fn expression(stream: &mut Stream, state: &mut Soy) -> Option<Kind> {
    let at = stream.pos;
    if stream.take("[") {
        state.states.push("list-literal");
        state.contexts.push(Context { tag: "list-literal".to_string(), kind: false, scope: Some(state.variables.clone()) });
        state.lookup = false;
        return None;
    }
    for (word, pushed) in [("map", "map-literal"), ("record", "record-literal")] {
        if stream.at(word) && stream.byte(at + word.len()) == b'(' {
            stream.pos += word.len();
            state.states.push(pushed);
            return Some(Kind::Keyword);
        }
    }
    let word = word_end(stream, at);
    if word > at && stream.byte(word) == b'(' {
        stream.pos = word;
        return Some(Kind::Variable);
    }
    if matches!(stream.peek(), b'"' | b'\'') {
        state.quote = Some(stream.peek());
        stream.next();
        state.states.push("string");
        return Some(Kind::String);
    }
    if stream.take("(") {
        state.states.push("open-parentheses");
        return None;
    }
    for literal in ["null", "true", "false"] {
        if stream.at(literal) && !scan::js_word(stream.byte(at + literal.len())) {
            stream.pos += literal.len();
            return None;
        }
    }
    if stream.at("0x") {
        let end = (at + 2..stream.bytes.len()).find(|index| !stream.byte(*index).is_ascii_hexdigit()).unwrap_or(stream.bytes.len());
        if end >= at + 4 {
            stream.pos = end;
            return None;
        }
    }
    if let Some(end) = soy_number(stream) {
        stream.pos = end;
        return None;
    }
    let pair = stream.bytes.get(at..at + 2).unwrap_or_default();
    if matches!(pair, b"==" | b"!=" | b"?:" | b"<=" | b">=") {
        stream.pos += 2;
        return Some(Kind::Operator);
    }
    if b"|+-*/%<>".contains(&stream.peek()) {
        stream.next();
        return Some(Kind::Operator);
    }
    if stream.peek() == b'$' && scan::js_word(stream.byte(at + 1)) {
        let end = word_end(stream, at + 1);
        let name = String::from_utf8_lossy(stream.bytes.get(at + 1..end).unwrap_or_default()).into_owned();
        stream.pos = end;
        return variable_style(state, &name);
    }
    if word > at {
        stream.pos = word;
        let text = stream.current();
        return ["as", "and", "or", "not", "in", "if"].contains(&text).then_some(Kind::Keyword);
    }
    stream.next();
    None
}

/// `-?([0-9]*[.])?[0-9]+(e[0-9]*)?` where the stream stands: where it ends.
fn soy_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let len = stream.bytes.len();
    let digits = |from: usize| (from..len).find(|index| !stream.byte(*index).is_ascii_digit()).unwrap_or(len.max(from));
    let sign = if stream.byte(at) == b'-' { at + 1 } else { at };
    let whole = digits(sign);
    let mantissa = if stream.byte(whole) == b'.' && stream.byte(whole + 1).is_ascii_digit() {
        digits(whole + 1)
    } else if whole > sign {
        whole
    } else {
        return None;
    };
    Some(if stream.byte(mantissa) == b'e' { digits(mantissa + 1) } else { mantissa })
}

/// Text in the innermost local language, to where `stop` (a command, a comment) begins.
fn local(text: &str, stream: &mut Stream, state: &mut Soy, out: &mut Out, stop: &dyn Fn(&Stream, usize) -> bool) -> Option<Kind> {
    // At the line's start, its indentation is a token of its own (the mode eats it first).
    if stream.sol() && stream.eat_while(|byte| byte == b' ' || byte == b'\t') {
        return None;
    }
    let at = stream.pos;
    let end = (at..text.len()).find(|index| stop(stream, *index)).unwrap_or(text.len());
    let end = if end == at { at + 1 } else { end };
    let piece = text.get(at..end).unwrap_or("");
    let mut inner = Out::default();
    if state.locals.is_empty() {
        state.locals.push(Local::Html(Box::default()));
    }
    match state.locals.last_mut() {
        Some(Local::Html(html)) => xml::line(piece, html, &mut inner),
        Some(Local::Css(css)) => css::line(piece, css, false, &mut inner),
        Some(Local::Js(js_state)) => js::line(piece, js_state, js::Flavor::JavaScript, &mut inner),
        _ => {}
    }
    out.extend_shifted(inner, at);
    stream.pos = end;
    None
}

/// A command, a comment, or a ` //` comment begins here.
fn breaks(stream: &Stream, at: usize) -> bool {
    let byte = stream.byte(at);
    if byte == b'{' || (byte == b'/' && stream.byte(at + 1) == b'*') {
        return true;
    }
    if byte.is_ascii_whitespace() {
        let after = (at..stream.bytes.len()).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(stream.bytes.len());
        return scan::at(stream.bytes, after, "//");
    }
    false
}

fn token(text: &str, stream: &mut Stream, state: &mut Soy, out: &mut Out) -> Option<Kind> {
    match state.top() {
        Some("comment") => {
            match stream.rest().windows(2).position(|pair| pair == b"*/") {
                Some(found) => {
                    stream.pos += found + 2;
                    state.states.pop();
                }
                None => stream.skip_to_end(),
            }
            if state.contexts.last().is_none_or(|context| context.scope.is_none()) {
                let current = stream.current().to_string();
                let mut rest = current.as_str();
                while let Some(found) = rest.find("@param") {
                    let after = rest.get(found + 6..).unwrap_or("");
                    let after = after.strip_prefix('?').unwrap_or(after);
                    let trimmed = after.trim_start();
                    if trimmed.len() < after.len() {
                        let name: String = trimmed.chars().take_while(|c| !c.is_whitespace()).collect();
                        if !name.is_empty() {
                            state.variables.push(name);
                        }
                    }
                    rest = after;
                }
            }
            return Some(Kind::Comment);
        }
        Some("string") => {
            let rest = stream.bytes.get(stream.pos..).unwrap_or_default();
            let found = (0..rest.len()).find(|index| matches!(rest.get(*index), Some(b'"' | b'\'' | b'\\')));
            match found {
                Some(index) if rest.get(index) == Some(&b'\\') && index + 1 < rest.len() => stream.pos += index + 2,
                Some(index) if rest.get(index) != Some(&b'\\') => {
                    let quote = rest.get(index).copied();
                    stream.pos += index + 1;
                    if quote == state.quote {
                        state.quote = None;
                        state.states.pop();
                    }
                }
                _ => stream.skip_to_end(),
            }
            return Some(Kind::String);
        }
        _ => {}
    }
    if state.top() != Some("literal") {
        if stream.take("/*") {
            state.states.push("comment");
            return Some(Kind::Comment);
        }
        let spaces = (stream.pos..text.len()).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(text.len());
        if (stream.sol() || spaces > stream.pos) && scan::at(stream.bytes, spaces, "//") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
    }
    let at = stream.pos;
    match state.top() {
        Some("templ-def") => {
            let dot = usize::from(stream.peek() == b'.');
            let end = word_end(stream, at + dot);
            if end > at + dot {
                stream.pos = end;
                state.states.pop();
                return None;
            }
            stream.next();
            return None;
        }
        Some("templ-ref") => {
            // `(\.?[a-zA-Z_][a-zA-Z_0-9]+)+`
            let mut end = at;
            loop {
                let dot = usize::from(stream.byte(end) == b'.');
                let first = stream.byte(end + dot);
                if !(first.is_ascii_alphabetic() || first == b'_') || !scan::js_word(stream.byte(end + dot + 1)) {
                    break;
                }
                end = word_end(stream, end + dot + 1);
            }
            if end > at {
                stream.pos = end;
                state.states.pop();
                return Some(Kind::Variable);
            }
            if stream.peek() == b'$' && scan::js_word(stream.byte(at + 1)) {
                let end = word_end(stream, at + 1);
                let name = String::from_utf8_lossy(stream.bytes.get(at + 1..end).unwrap_or_default()).into_owned();
                stream.pos = end;
                state.states.pop();
                return variable_style(state, &name);
            }
            stream.next();
            return None;
        }
        Some("namespace-def") => {
            let dot = usize::from(stream.peek() == b'.');
            let end = (at + dot..text.len()).find(|index| !(scan::js_word(stream.byte(*index)) || stream.byte(*index) == b'.')).unwrap_or(text.len());
            if end > at + dot {
                stream.pos = end;
                state.states.pop();
                return Some(Kind::Variable);
            }
            stream.next();
            return None;
        }
        Some("param-def") => {
            if stream.take("*") {
                state.states.pop();
                state.states.push("param-type");
                return Some(Kind::Property);
            }
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.variables.push(stream.current().to_string());
                state.states.pop();
                state.states.push("param-type");
                return None;
            }
            stream.next();
            return None;
        }
        Some("param-ref") => {
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.states.pop();
                return Some(Kind::Property);
            }
            stream.next();
            return None;
        }
        Some("open-parentheses") => {
            if stream.take(")") {
                state.states.pop();
                return None;
            }
            return expression(stream, state);
        }
        Some("param-type") => {
            let peek = stream.peek();
            if b"}]=>,".contains(&peek) {
                state.states.pop();
                return None;
            }
            match peek {
                b'[' => state.states.push("param-type-record"),
                b'(' => state.states.push("param-type-template"),
                b'<' => state.states.push("param-type-parameter"),
                _ => {
                    let end = word_end(stream, at);
                    if end > at || peek == b'?' {
                        stream.pos = end.max(at + 1);
                        return Some(Kind::Property);
                    }
                    stream.next();
                }
            }
            return None;
        }
        Some("param-type-record") => {
            if stream.peek() == b']' {
                state.states.pop();
                return None;
            }
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.states.push("param-type");
                return Some(Kind::Property);
            }
            stream.next();
            return None;
        }
        Some("param-type-parameter") => {
            if stream.take(">") {
                state.states.pop();
                return None;
            }
            if stream.eat(|byte| byte == b'<' || byte == b',').is_some() {
                state.states.push("param-type");
            } else {
                stream.next();
            }
            return None;
        }
        Some("param-type-template") => {
            if let Some(found) = stream.rest().iter().position(|byte| *byte == b'>').filter(|found| *found == 0) {
                stream.pos += found + 1;
                state.states.pop();
                state.states.push("param-type");
                return None;
            }
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.states.push("param-type");
                return None;
            }
            stream.next();
            return None;
        }
        Some("var-def") => {
            if stream.peek() == b'$' && scan::js_word(stream.byte(at + 1)) {
                stream.pos = word_end(stream, at + 1);
                let name = String::from_utf8_lossy(stream.bytes.get(at + 1..stream.pos).unwrap_or_default()).into_owned();
                state.variables.push(name);
                state.states.pop();
                return None;
            }
            stream.next();
            return None;
        }
        Some("for-loop") => {
            if stream.at("in") && !scan::js_word(stream.byte(at + 2)) {
                stream.pos += 2;
                state.states.pop();
                return Some(Kind::Keyword);
            }
            if stream.peek() == b'$' {
                state.states.push("var-def");
            } else {
                stream.next();
            }
            return None;
        }
        Some(literal @ ("record-literal" | "map-literal")) => {
            if stream.take(")") {
                state.states.pop();
                return None;
            }
            if stream.eat(|byte| byte == b'(' || byte == b',').is_some() {
                state.states.push("map-value");
                state.states.push(if literal == "record-literal" { "record-key" } else { "map-value" });
            } else {
                stream.next();
            }
            return None;
        }
        Some("list-literal") => {
            if stream.take("]") {
                state.states.pop();
                state.lookup = true;
                state.pop_context();
                return None;
            }
            if stream.at("for") && !scan::js_word(stream.byte(at + 3)) {
                stream.pos += 3;
                state.lookup = true;
                state.states.push("for-loop");
                return Some(Kind::Keyword);
            }
            return expression(stream, state);
        }
        Some("record-key") => {
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                return Some(Kind::Property);
            }
            if stream.take(":") {
                state.states.pop();
            } else {
                stream.next();
            }
            return None;
        }
        Some("map-value") => {
            if matches!(stream.peek(), b')' | b',') {
                state.states.pop();
                return None;
            }
            if stream.take(":") {
                state.states.pop();
                return None;
            }
            return expression(stream, state);
        }
        Some("import") => {
            if stream.take(";") {
                state.states.pop();
                return None;
            }
            let end = word_end(stream, at);
            if end > at {
                let spaces = (end..text.len()).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(text.len());
                stream.pos = end;
                if spaces > end && scan::at(stream.bytes, spaces, "as") && !scan::js_word(stream.byte(spaces + 2)) {
                    return Some(Kind::Variable);
                }
                return if matches!(stream.current(), "from" | "as") { Some(Kind::Keyword) } else { None };
            }
            if matches!(stream.peek(), b'"' | b'\'') {
                state.quote = Some(stream.peek());
                stream.next();
                state.states.push("string");
                return Some(Kind::String);
            }
            stream.next();
            return None;
        }
        Some("tag") => return tag(stream, state),
        Some("template-call-expression") => {
            let end = (at..text.len())
                .find(|index| !(scan::js_word(stream.byte(*index)) || matches!(stream.byte(*index), b'-' | b'?')))
                .unwrap_or(text.len());
            if end > at && stream.byte(end) == b'=' {
                stream.pos = end;
                return Some(Kind::Property);
            }
            // (The mode's `eat("/>")` never takes the two bytes: `/` is an operator.)
            if stream.take(">") {
                state.states.pop();
                return Some(Kind::Keyword);
            }
            return expression(stream, state);
        }
        Some("literal") => {
            if stream.at("{/literal}") {
                state.states.pop();
                return token(text, stream, state, out);
            }
            return local(text, stream, state, out, &|stream, index| scan::at(stream.bytes, index, "{/literal}"));
        }
        Some("export") => {
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.states.pop();
                match stream.current() {
                    "const" => {
                        state.states.push("const-def");
                        return Some(Kind::Keyword);
                    }
                    "extern" => {
                        state.states.push("param-def");
                        return Some(Kind::Keyword);
                    }
                    _ => {}
                }
            } else {
                stream.next();
            }
            return None;
        }
        Some("const-def") => {
            let end = word_end(stream, at);
            if end > at {
                stream.pos = end;
                state.states.pop();
                return None;
            }
            stream.next();
            return None;
        }
        _ => {}
    }
    if stream.take("{literal}") {
        state.states.push("literal");
        state.contexts.push(Context { tag: "literal".to_string(), kind: false, scope: Some(state.variables.clone()) });
        return Some(Kind::Keyword);
    }
    if let Some(end) = command_name(stream) {
        let name = String::from_utf8_lossy(stream.bytes.get(at + 1..end).unwrap_or_default()).into_owned();
        stream.pos = end;
        return open_command(stream, state, name);
    }
    if stream.take("{") {
        state.tag = Some("print".to_string());
        state.states.push("tag");
        return Some(Kind::Keyword);
    }
    if state.contexts.is_empty() && stream.sol() && stream.at("import") && !scan::js_word(stream.byte(at + 6)) {
        stream.pos += 6;
        state.states.push("import");
        return Some(Kind::Keyword);
    }
    if stream.take("<{") {
        state.states.push("template-call-expression");
        state.states.push("tag");
        return Some(Kind::Keyword);
    }
    if stream.take("</>") {
        return Some(Kind::Keyword);
    }
    local(text, stream, state, out, &breaks)
}

/// `\{([/@\\]?\w+\??)(?=$|[\s}]|\/[/*])` where the stream stands: where the name ends.
fn command_name(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    if stream.byte(at) != b'{' {
        return None;
    }
    let first = at + 1 + usize::from(matches!(stream.byte(at + 1), b'/' | b'@' | b'\\'));
    let mut end = word_end(stream, first);
    if end == first {
        return None;
    }
    if stream.byte(end) == b'?' {
        end += 1;
    }
    let next = stream.byte(end);
    let follows =
        end == stream.bytes.len() || next.is_ascii_whitespace() || next == b'}' || (next == b'/' && matches!(stream.byte(end + 1), b'/' | b'*'));
    follows.then_some(end)
}

fn open_command(_stream: &mut Stream, state: &mut Soy, name: String) -> Option<Kind> {
    let closing = name.starts_with('/');
    let bare = name.strip_prefix('/').unwrap_or(&name).to_string();
    let known_as_written = tag_info(&name).is_some();
    state.tag = Some(name.clone());
    state.states.push("tag");
    let mut error = false;
    match tag_info(&bare) {
        Some((no_end_tag, opened, scopes)) => {
            if !closing && let Some(opened) = opened {
                state.states.push(opened);
            }
            if no_end_tag || (!known_as_written && closing) {
                if closing {
                    let extern_in_export = bare == "extern" && state.contexts.last().is_some_and(|context| context.tag == "export");
                    match state.contexts.last() {
                        Some(context) if context.tag == bare || extern_in_export => {
                            if context.kind {
                                state.locals.pop();
                            }
                            state.pop_context();
                        }
                        _ => error = true,
                    }
                }
            } else {
                state.contexts.push(Context { tag: name, kind: false, scope: scopes.then(|| state.variables.clone()) });
            }
        }
        None => error = closing,
    }
    // An end tag with no open tag: the mode's `error keyword`, in the error's color.
    if error { Some(Kind::Operator) } else { Some(Kind::Keyword) }
}

fn tag(stream: &mut Stream, state: &mut Soy) -> Option<Kind> {
    let current_tag = state.tag.clone().unwrap_or_default();
    let closing = current_tag.starts_with('/');
    let bare = current_tag.strip_prefix('/').unwrap_or(&current_tag).to_string();
    if stream.take("/}") || stream.take("}") {
        let self_closing = stream.current() == "/}";
        if self_closing && !closing {
            state.pop_context();
        }
        if current_tag == "/template" || current_tag == "/deltemplate" {
            state.variables = vec!["ij".to_string()];
        }
        state.states.pop();
        return Some(Kind::Keyword);
    }
    let at = stream.pos;
    let end =
        (at..stream.bytes.len()).find(|index| !(scan::js_word(stream.byte(*index)) || stream.byte(*index) == b'?')).unwrap_or(stream.bytes.len());
    if end > at && stream.byte(end) == b'=' {
        stream.pos = end;
        let is_kind = stream.current() == "kind" && state.contexts.last().is_some_and(|context| context.tag == bare);
        if is_kind && stream.byte(end + 1) == b'"' {
            let value_end = (end + 2..stream.bytes.len()).find(|index| stream.byte(*index) == b'"');
            if let Some(value_end) = value_end.filter(|value_end| *value_end > end + 2) {
                let kind = String::from_utf8_lossy(stream.bytes.get(end + 2..value_end).unwrap_or_default()).into_owned();
                if let Some(context) = state.contexts.last_mut() {
                    context.kind = true;
                }
                state.locals.push(match kind.as_str() {
                    "css" => Local::Css(css::Css::default()),
                    "js" => Local::Js(js::Js::default()),
                    "attributes" | "text" | "uri" | "trusted_resource_uri" => Local::Text,
                    _ => Local::Html(Box::default()),
                });
            }
        }
        return Some(Kind::Property);
    }
    expression(stream, state)
}
