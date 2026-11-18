//! Julia, as CodeMirror 5's julia mode colors it, ported token for token: keywords, a function's
//! (or type's) name a definition, `::Type` annotations builtins, `:symbol`s builtins, `@macro`s
//! meta, numbers, strings (`"`, `"""`, prefixed ones like `r"…"`), char literals, `'` after a
//! value a transpose operator, `#` and nested `#= =#` comments. Anything else (`=` among them) is
//! the mode's error.

use super::Kind;
use super::scan::{Out, Stream};

const KEYWORDS: &[&str] = &[
    "if",
    "else",
    "elseif",
    "while",
    "for",
    "begin",
    "let",
    "end",
    "do",
    "try",
    "catch",
    "finally",
    "return",
    "break",
    "continue",
    "global",
    "local",
    "const",
    "export",
    "import",
    "importall",
    "using",
    "function",
    "where",
    "macro",
    "module",
    "baremodule",
    "struct",
    "type",
    "mutable",
    "immutable",
    "quote",
    "typealias",
    "abstract",
    "primitive",
    "bitstype",
];
const BUILTINS: &[&str] = &["true", "false", "nothing", "NaN", "Inf"];
const OPENERS: &[&str] = &[
    "begin",
    "function",
    "type",
    "struct",
    "immutable",
    "let",
    "macro",
    "for",
    "while",
    "quote",
    "if",
    "else",
    "elseif",
    "try",
    "finally",
    "catch",
    "do",
];
const CLOSERS: &[&str] = &["end", "else", "elseif", "catch", "finally"];
/// Operators as UTF-8 characters (`×`, `∈`, `≤`, …), and `÷` and `⊻` (which may take `=`).
const UNICODE_OPERATORS: &[char] = &['×', '∈', '∉', '∋', '∌', '∘', '√', '∛', '∩', '∪', '≠', '≤', '≥', '⊆', '⊈', '⊊', '⋅'];

/// What the reader reads next.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Reading {
    #[default]
    Code,
    /// A string, to its closing delimiter.
    String(&'static str),
    /// A type annotation after `::`.
    Type,
    Comment,
    Character,
}

#[derive(Clone, Debug, Default)]
pub struct Julia {
    reading: Reading,
    scopes: Vec<String>,
    last: String,
    leaving: bool,
    definition: bool,
    arrays: usize,
    generators: usize,
    comments: usize,
    parameters: usize,
}

fn identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

fn identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// The keyword (one of `list`) starting at the position, as a whole word.
fn word_at<'a>(stream: &Stream, list: &[&'a str]) -> Option<&'a str> {
    let end = (stream.pos..)
        .find(|index| !stream.bytes.get(*index).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_'))
        .unwrap_or(stream.pos);
    let word = std::str::from_utf8(stream.bytes.get(stream.pos..end)?).ok()?;
    list.iter().find(|known| **known == word).copied()
}

/// The length of an operator at `at` (the mode's operator list), if one is there.
fn operator_at(bytes: &[u8], at: usize, symbols: bool) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let (a, b, c) = (byte(at), byte(at + 1), byte(at + 2));
    if b"<>".contains(&a) && b == b':' {
        return Some(2);
    }
    if b"<>=".contains(&a) && b == b'=' {
        return Some(2);
    }
    if a == b'<' && b == b'<' {
        return Some(if c == b'=' { 3 } else { 2 });
    }
    if a == b'>' && b == b'>' {
        let mut end = 2;
        if c == b'>' {
            end += 1;
        }
        if byte(at + end) == b'=' {
            end += 1;
        }
        return Some(end);
    }
    if a == b'=' && b == b'>' {
        return Some(2);
    }
    if a == b'-' && b == b'>' {
        return Some(2);
    }
    if a == b'-' && b == b'-' && c == b'>' {
        return Some(3);
    }
    if a == b'<' && b == b'-' && c == b'-' {
        return Some(if matches!(byte(at + 3), b'-' | b'>') { 4 } else { 3 });
    }
    if a == b'/' && b == b'/' {
        return Some(2);
    }
    if symbols && a == b'.' && b == b'.' {
        return Some(if c == b'.' { 3 } else { 2 });
    }
    let single: &[u8] = if symbols { b".\\%*+-<>!/^|&" } else { b"\\%*+-<>!/^|&" };
    if single.contains(&a) {
        return Some(if b == b'=' { 2 } else { 1 });
    }
    if b"?$~:".contains(&a) {
        return Some(1);
    }
    let text = std::str::from_utf8(bytes.get(at..)?).ok()?;
    let first = text.chars().next()?;
    if !symbols && (first == '÷' || first == '⊻') {
        return Some(first.len_utf8() + usize::from(text[first.len_utf8()..].starts_with('=')));
    }
    if !symbols && UNICODE_OPERATORS.contains(&first) {
        return Some(first.len_utf8());
    }
    // `in` and `isa` as words, not called.
    if !symbols {
        for word in ["in", "isa"] {
            let end = at + word.len();
            let before = at.checked_sub(1).map(byte).is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            if text.starts_with(word) && !before && !(byte(end).is_ascii_alphanumeric() || byte(end) == b'_') {
                let called = byte(end) == b'(' || (byte(end) != 0 && byte(end + 1) == b'(');
                if !called {
                    return Some(word.len());
                }
            }
        }
    }
    None
}

pub fn line(text: &str, state: &mut Julia, out: &mut Out) {
    super::scan::run(text, out, |stream| {
        let style = match state.reading {
            Reading::Code => code(stream, state),
            Reading::String(end) => string(stream, state, end),
            Reading::Type => annotation(stream, state),
            Reading::Comment => comment(stream, state),
            Reading::Character => character(stream, state),
        };
        if style.is_some() && stream.pos > stream.start {
            state.last = stream.current().to_string();
        }
        style
    });
}

fn code(stream: &mut Stream, state: &mut Julia) -> Option<Kind> {
    let bytes = stream.bytes;
    if stream.at("#=") {
        state.reading = Reading::Comment;
        return comment(stream, state);
    }
    let leaving = state.leaving && !stream.sol();
    state.leaving = false;
    if leaving && stream.eat_while(|byte| byte == b'\'') {
        return Some(Kind::Operator);
    }
    let dots = (stream.pos..).take_while(|index| bytes.get(*index) == Some(&b'.')).count();
    if dots >= 4 {
        stream.pos += dots;
        return Some(Kind::Operator);
    }
    if dots > 0 {
        stream.pos += dots;
        return Some(Kind::Operator);
    }
    if stream.eat_space() {
        return None;
    }
    let here = stream.peek();
    if here == b'#' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here == b'[' {
        state.scopes.push("[".to_string());
        state.arrays += 1;
    }
    if here == b'(' {
        state.scopes.push("(".to_string());
        state.generators += 1;
    }
    if state.arrays > 0 && here == b']' {
        while !state.scopes.is_empty() && state.scopes.last().is_some_and(|scope| scope != "[") {
            state.scopes.pop();
        }
        state.scopes.pop();
        state.arrays -= 1;
        state.leaving = true;
    }
    if state.generators > 0 && here == b')' {
        while !state.scopes.is_empty() && state.scopes.last().is_some_and(|scope| scope != "(") {
            state.scopes.pop();
        }
        state.scopes.pop();
        state.generators -= 1;
        state.leaving = true;
    }
    if state.arrays > 0 {
        if state.last == "end" && stream.take(":") {
            return Some(Kind::Operator);
        }
        if stream.take("end") {
            return Some(Kind::Number);
        }
    }
    if let Some(opener) = word_at(stream, OPENERS) {
        state.scopes.push(opener.to_string());
    }
    if word_at(stream, CLOSERS).is_some() {
        state.scopes.pop();
    }
    if stream.at("::") && !matches!(stream.look(2), b':' | b'$') {
        state.reading = Reading::Type;
        return annotation(stream, state);
    }
    if !leaving && here == b':' {
        let next = stream.look(1);
        if identifier_start(next) {
            stream.pos += 2;
            stream.eat_while(identifier);
            stream.eat_while(|byte| byte == b'!');
            return Some(Kind::Property);
        }
        if let Some(length) = operator_at(bytes, stream.pos + 1, true) {
            stream.pos += 1 + length;
            return Some(Kind::Property);
        }
    }
    if let Some(length) = operator_at(bytes, stream.pos, false) {
        stream.pos += length;
        return Some(Kind::Operator);
    }
    if here.is_ascii_digit() || (here == b'.' && stream.look(1).is_ascii_digit()) {
        number(stream);
        if stream.at("im") && !stream.bytes.get(stream.pos + 2).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_') {
            stream.pos += 2;
        }
        state.leaving = true;
        return Some(Kind::Number);
    }
    if stream.take("'") {
        state.reading = Reading::Character;
        return character(stream, state);
    }
    // A string: "`", or a prefix (`r`, `raw`, …) and `"` or `"""`.
    let prefix =
        (stream.pos..).take_while(|index| bytes.get(*index).is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_' || *byte >= 0x80)).count();
    if here == b'`' {
        stream.pos += 1;
        state.reading = Reading::String("`");
        return string(stream, state, "`");
    }
    if bytes.get(stream.pos + prefix) == Some(&b'"') {
        stream.pos += prefix + 1;
        let end = if stream.take("\"\"") { "\"\"\"" } else { "\"" };
        state.reading = Reading::String(end);
        return string(stream, state, end);
    }
    if here == b'@' {
        if identifier_start(stream.look(1)) {
            stream.pos += 2;
            stream.eat_while(identifier);
            stream.eat_while(|byte| byte == b'!');
            return Some(Kind::Comment);
        }
        if let Some(length) = operator_at(bytes, stream.pos + 1, true) {
            stream.pos += 1 + length;
            return Some(Kind::Comment);
        }
    }
    if b";,()[]{}".contains(&here) {
        stream.pos += 1;
        return None;
    }
    if let Some(keyword) = word_at(stream, KEYWORDS) {
        stream.pos += keyword.len();
        return Some(Kind::Keyword);
    }
    if let Some(builtin) = word_at(stream, BUILTINS) {
        stream.pos += builtin.len();
        return Some(Kind::Property);
    }
    let defining = state.definition || matches!(state.last.as_str(), "function" | "macro" | "type" | "struct" | "immutable");
    if identifier_start(here) {
        stream.eat_while(identifier);
        stream.eat_while(|byte| byte == b'!');
        if defining {
            if stream.peek() == b'.' {
                state.definition = true;
                return Some(Kind::Variable);
            }
            state.definition = false;
            // A definition is the last token too (its style has no color).
            state.last = stream.current().to_string();
            return None;
        }
        state.leaving = true;
        return Some(Kind::Variable);
    }
    // The mode's error: one character.
    stream.pos += 1;
    while (0x80..0xC0).contains(&stream.peek()) {
        stream.pos += 1;
    }
    Some(Kind::Operator)
}

/// A number's end (hex, binary, octal, a decimal with a fraction and an exponent), read as the
/// mode's patterns read one after another.
fn number(stream: &mut Stream) {
    let bytes = stream.bytes;
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let run = |from: usize, ok: &dyn Fn(u8) -> bool| (from..).find(|index| !ok(byte(*index))).unwrap_or(from);
    let digits = |from: usize| run(from, &|byte: u8| byte.is_ascii_digit() || byte == b'_');
    let at = stream.pos;
    // 0x.1p3, 0x1f, 0b1, 0o7
    if byte(at) == b'0' && matches!(byte(at + 1), b'x' | b'X') {
        let hex = |byte: u8| byte.is_ascii_hexdigit() || byte == b'_';
        if byte(at + 2) == b'.' && run(at + 3, &hex) > at + 3 && matches!(byte(run(at + 3, &hex)), b'p' | b'P') {
            let mut end = run(at + 3, &hex) + 1;
            if matches!(byte(end), b'+' | b'-') {
                end += 1;
            }
            if digits(end) > end {
                stream.pos = digits(end);
            }
        }
        let from = stream.pos;
        if byte(from) == b'0' && matches!(byte(from + 1), b'x' | b'X') && run(from + 2, &hex) > from + 2 {
            stream.pos = run(from + 2, &hex);
        }
    }
    let from = stream.pos;
    if byte(from) == b'0' && matches!(byte(from + 1), b'b' | b'B') && run(from + 2, &|byte: u8| matches!(byte, b'0' | b'1' | b'_')) > from + 2 {
        stream.pos = run(from + 2, &|byte: u8| matches!(byte, b'0' | b'1' | b'_'));
    }
    let from = stream.pos;
    if byte(from) == b'0' && matches!(byte(from + 1), b'o' | b'O') && run(from + 2, &|byte: u8| matches!(byte, b'0'..=b'7' | b'_')) > from + 2 {
        stream.pos = run(from + 2, &|byte: u8| matches!(byte, b'0'..=b'7' | b'_'));
    }
    // `((\d[_\d]*)?\.(?!\.)(\d[_\d]*)?|\d[_\d]*\.(?!\.)\d[_\d]*)?([Eef][+-]?[_\d]+)?`
    let from = stream.pos;
    let whole = if byte(from).is_ascii_digit() { digits(from) } else { from };
    let mut end = from;
    if byte(whole) == b'.' && byte(whole + 1) != b'.' {
        end = if byte(whole + 1).is_ascii_digit() { digits(whole + 1) } else { whole + 1 };
    }
    if matches!(byte(end), b'E' | b'e' | b'f' | b'F') {
        let mut after = end + 1;
        if matches!(byte(after), b'+' | b'-') {
            after += 1;
        }
        let exponent = run(after, &|byte: u8| byte.is_ascii_digit() || byte == b'_');
        if exponent > after {
            end = exponent;
        }
    }
    stream.pos = end;
    // `\d[_\d]*(e[+-]?\d+)?`
    let from = stream.pos;
    if byte(from).is_ascii_digit() {
        let mut end = digits(from);
        if matches!(byte(end), b'e' | b'E') {
            let mut after = end + 1;
            if matches!(byte(after), b'+' | b'-') {
                after += 1;
            }
            let exponent = run(after, &|byte: u8| byte.is_ascii_digit());
            if exponent > after {
                end = exponent;
            }
        }
        stream.pos = end;
    }
}

fn string(stream: &mut Stream, state: &mut Julia, end: &'static str) -> Option<Kind> {
    if stream.eat(|byte| byte == b'\\').is_some() {
        stream.next();
    } else {
        if stream.take(end) {
            state.reading = Reading::Code;
            state.leaving = true;
            return Some(Kind::String);
        }
        stream.eat(|byte| byte == b'`' || byte == b'"');
    }
    stream.eat_while(|byte| !matches!(byte, b'\\' | b'`' | b'"'));
    Some(Kind::String)
}

/// A type annotation: to a delimiter, its `{…}` parameters with it.
fn annotation(stream: &mut Stream, state: &mut Julia) -> Option<Kind> {
    stream.eat_while(|byte| !b",;{}()= \t\r".contains(&byte));
    if stream.take("{") {
        state.parameters += 1;
    } else if state.parameters > 0 && stream.take("}") {
        state.parameters -= 1;
    }
    if state.parameters > 0 {
        let rest = stream.bytes.get(stream.pos..).unwrap_or_default();
        match rest.iter().position(|byte| *byte == b'{' || *byte == b'}') {
            Some(found) if found > 0 => stream.pos += found,
            _ => {
                stream.next();
            }
        }
    } else {
        state.reading = Reading::Code;
    }
    Some(Kind::Property)
}

/// A `#= … =#` comment (nested ones counted).
fn comment(stream: &mut Stream, state: &mut Julia) -> Option<Kind> {
    if stream.take("#=") {
        state.comments += 1;
    }
    let rest = stream.bytes.get(stream.pos..).unwrap_or_default();
    match rest.windows(2).position(|pair| pair == b"#=" || pair == b"=#") {
        Some(found) => stream.pos += found,
        None => stream.skip_to_end(),
    }
    if stream.take("=#") {
        state.comments = state.comments.saturating_sub(1);
        if state.comments == 0 {
            state.reading = Reading::Code;
        }
    }
    Some(Kind::Comment)
}

/// A char literal after its `'`: one character or an escape, then `'`; anything else an error.
fn character(stream: &mut Stream, state: &mut Julia) -> Option<Kind> {
    let bytes = stream.bytes;
    let at = stream.pos;
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let closes = |end: usize| (byte(end) == b'\'').then_some(end + 1);
    let run = |from: usize, most: usize, ok: fn(u8) -> bool| (from..from + most).take_while(|index| ok(byte(*index))).count();
    let end = if byte(at) == b'\\' {
        let octal = run(at + 1, 3, |byte| (b'0'..=b'7').contains(&byte));
        let hex = if byte(at + 1) == b'x' { run(at + 2, 2, |byte| byte.is_ascii_hexdigit()) } else { 0 };
        let unicode = if byte(at + 1) == b'u' { run(at + 2, 4, |byte| byte.is_ascii_hexdigit()) } else { 0 };
        let wide = if byte(at + 1) == b'U' { run(at + 2, 8, |byte| byte.is_ascii_hexdigit()) } else { 0 };
        (octal > 0)
            .then(|| closes(at + 1 + octal))
            .flatten()
            .or_else(|| (hex > 0).then(|| closes(at + 2 + hex)).flatten())
            .or_else(|| b"abefnrtv0%?'\"\\".contains(&byte(at + 1)).then(|| closes(at + 2)).flatten())
            .or_else(|| (unicode > 0).then(|| closes(at + 2 + unicode)).flatten())
            .or_else(|| (wide >= 5).then(|| closes(at + 2 + wide)).flatten())
    } else if byte(at) != b'\'' && byte(at) != 0 {
        let width = std::str::from_utf8(bytes.get(at..).unwrap_or_default()).ok().and_then(|rest| rest.chars().next()).map_or(1, char::len_utf8);
        closes(at + width)
    } else {
        None
    };
    if let Some(end) = end {
        stream.pos = end;
        state.leaving = true;
        state.reading = Reading::Code;
        return Some(Kind::String);
    }
    match bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == b'\'')) {
        Some(found) if found > 0 => stream.pos = at + found,
        _ => stream.skip_to_end(),
    }
    if stream.take("'") {
        state.reading = Reading::Code;
    }
    Some(Kind::Operator)
}
