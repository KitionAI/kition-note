//! Rust, as CodeMirror 5's rust mode (a simple mode: an ordered list of patterns) colors it:
//! names variables, `name!` macros variables too, the name after `let`, `fn`, `struct`, … a
//! definition, primitive types atoms, `true`, `Some`, `Ok` builtins, attributes meta.

use super::Kind;
use super::scan::{self, Out};
use super::simple::{Simple, block_comment};

const KEYWORDS: &[&str] = &[
    "abstract", "alignof", "as", "async", "await", "box", "break", "continue", "const", "crate", "do", "dyn", "else", "enum", "extern", "fn", "for",
    "final", "if", "impl", "in", "loop", "macro", "match", "mod", "move", "offsetof", "override", "priv", "proc", "pub", "pure", "ref", "return",
    "self", "sizeof", "static", "struct", "super", "trait", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual", "where", "while",
    "yield",
];
const ATOMS: &[&str] =
    &["Self", "isize", "usize", "char", "bool", "u8", "u16", "u32", "u64", "f16", "f32", "f64", "i8", "i16", "i32", "i64", "str", "Option"];
const BUILTINS: &[&str] = &["true", "false", "Some", "None", "Ok", "Err"];
const DEFINERS: &[&str] = &["let", "fn", "enum", "mod", "struct", "type", "union"];

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

pub fn line(text: &str, state: &mut Simple, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if state.comment {
        at = block_comment(text, 0, state, out);
    }
    if let Some(close) = state.string {
        at = string_rest(text, at, at, close, state, out);
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let rest = text.get(at..).unwrap_or("");
        // A byte string's `b`, a raw string's `r` and its hashes.
        let prefix = usize::from(here == b'b' && matches!(byte(bytes, at + 1), b'"' | b'r' | b'\''));
        if byte(bytes, at + prefix) == b'"' && (prefix == 0 || !scan::word(byte(bytes, at.wrapping_sub(1)))) {
            at = string_rest(text, at, at + prefix + 1, "\"", state, out);
            continue;
        }
        if byte(bytes, at + prefix) == b'r' && (at == 0 || !scan::word(byte(bytes, at - 1))) {
            let hashes = bytes.get(at + prefix + 1..).map_or(0, |after| after.iter().take_while(|byte| **byte == b'#').count());
            if byte(bytes, at + prefix + 1 + hashes) == b'"' {
                let close: &'static str = match hashes {
                    0 => "\"",
                    1 => "\"#",
                    2 => "\"##",
                    _ => "\"###",
                };
                at = string_rest(text, at, at + prefix + 2 + hashes, close, state, out);
                continue;
            }
        }
        if let Some(end) = char_literal(bytes, at + prefix).filter(|_| prefix == 0 || byte(bytes, at + 1) == b'\'') {
            out.push(at..end, Kind::String);
            at = end;
            continue;
        }
        if let Some(end) = number(bytes, at).filter(|_| at == 0 || !scan::word(byte(bytes, at - 1))) {
            out.push(at..end, Kind::Number);
            at = end;
            continue;
        }
        if here.is_ascii_alphabetic() || here == b'_' {
            let end = scan::word_end(bytes, at, b"");
            let word = text.get(at..end).unwrap_or("");
            // `let mut name`, `fn name`, …: the keyword, then the name defined.
            if DEFINERS.contains(&word) {
                let mut keyword_end = end;
                if word == "let"
                    && let Some((next, _)) = scan::next_significant(bytes, end).filter(|(next, _)| *next > end)
                    && scan::at(bytes, next, "mut")
                    && !scan::word(byte(bytes, next + 3))
                {
                    keyword_end = next + 3;
                }
                if let Some((name, first)) = scan::next_significant(bytes, keyword_end).filter(|(name, _)| *name > keyword_end)
                    && (first.is_ascii_alphabetic() || first == b'_')
                {
                    out.push(at..keyword_end, Kind::Keyword);
                    at = scan::word_end(bytes, name, b"");
                    continue;
                }
            }
            if let Some(keyword) = KEYWORDS.iter().find(|keyword| rest.starts_with(**keyword) && !scan::word(byte(bytes, at + keyword.len()))) {
                out.push(at..at + keyword.len(), Kind::Keyword);
                at += keyword.len();
                continue;
            }
            if ATOMS.contains(&word) {
                at = end;
                continue;
            }
            if BUILTINS.contains(&word) {
                out.push(at..end, Kind::Property);
                at = end;
                continue;
            }
        }
        if here == b'#'
            && (byte(bytes, at + 1) == b'[' || (byte(bytes, at + 1) == b'!' && byte(bytes, at + 2) == b'['))
            && let Some(close) = rest.rfind(']')
        {
            out.push(at..at + close + 1, Kind::Comment);
            at += close + 1;
            continue;
        }
        if rest.starts_with("//") {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        if rest.starts_with("/*") {
            at = block_comment(text, at, state, out);
            continue;
        }
        if matches!(here, b'-' | b'+' | b'/' | b'*' | b'=' | b'<' | b'>' | b'!') {
            let mut end = at;
            while matches!(byte(bytes, end), b'-' | b'+' | b'/' | b'*' | b'=' | b'<' | b'>' | b'!') {
                end += 1;
            }
            out.push(at..end, Kind::Operator);
            at = end;
            continue;
        }
        if here.is_ascii_alphabetic() || here == b'_' {
            let end = scan::word_end(bytes, at, b"");
            let end = if byte(bytes, end) == b'!' { end + 1 } else { end };
            out.push(at..end, Kind::Variable);
            at = end;
            continue;
        }
        at += if here >= 0x80 { scan::word_end(bytes, at, b"").max(at + 1) - at } else { 1 };
    }
}

/// A string's text from `start`, read from `from`: to `close`, or the line's end (it stays open).
fn string_rest(text: &str, start: usize, from: usize, close: &'static str, state: &mut Simple, out: &mut Out) -> usize {
    let bytes = text.as_bytes();
    let raw = close != "\"" || (start < from && bytes.get(start..from).is_some_and(|open| open.contains(&b'r')));
    let mut index = from;
    state.string = None;
    while index < bytes.len() {
        if !raw && byte(bytes, index) == b'\\' {
            index += 2;
            continue;
        }
        if scan::at(bytes, index, close) {
            out.push(start..index + close.len(), Kind::String);
            return index + close.len();
        }
        index += 1;
    }
    out.push(start..bytes.len(), Kind::String);
    state.string = Some(close);
    bytes.len()
}

/// The end of a character literal at `at` (`'a'`, `'\n'`, `'\x41'`, `'\u{…}'`); a lifetime is none.
fn char_literal(bytes: &[u8], at: usize) -> Option<usize> {
    if byte(bytes, at) != b'\'' {
        return None;
    }
    let mut index = at + 1;
    if byte(bytes, index) == b'\\' {
        index += 2;
        while byte(bytes, index) != b'\'' && index < at + 12 && index < bytes.len() {
            index += 1;
        }
    } else {
        let width = match byte(bytes, index) {
            0 | b'\'' => return None,
            lead if lead >= 0xf0 => 4,
            lead if lead >= 0xe0 => 3,
            lead if lead >= 0xc0 => 2,
            _ => 1,
        };
        index += width;
    }
    (byte(bytes, index) == b'\'').then_some(index + 1)
}

/// The end of a number at `at`, in the mode's pattern: a float (an exponent or a fraction, then
/// `f32` or `f64`), or an integer (`0b`, `0o`, `0x`, or digits) with an integer type after.
fn number(bytes: &[u8], at: usize) -> Option<usize> {
    if !byte(bytes, at).is_ascii_digit() {
        return None;
    }
    let digits = |from: usize, ok: fn(u8) -> bool| {
        let mut end = from;
        while ok(byte(bytes, end)) || byte(bytes, end) == b'_' {
            end += 1;
        }
        end
    };
    let integer = digits(at, |byte| byte.is_ascii_digit());
    let mut float = None;
    if matches!(byte(bytes, integer), b'e' | b'E') {
        let exponent = scan::exponent(bytes, integer);
        if exponent > integer {
            float = Some(exponent);
        }
    } else if byte(bytes, integer) == b'.' && byte(bytes, integer + 1).is_ascii_digit() {
        let fraction = digits(integer + 1, |byte| byte.is_ascii_digit());
        float = Some(scan::exponent(bytes, fraction));
    }
    if let Some(end) = float {
        return Some(if scan::at(bytes, end, "f32") || scan::at(bytes, end, "f64") { end + 3 } else { end });
    }
    let end = match (byte(bytes, at), byte(bytes, at + 1)) {
        (b'0', b'b') => digits(at + 2, |byte| byte == b'0' || byte == b'1'),
        (b'0', b'o') => digits(at + 2, |byte| (b'0'..=b'7').contains(&byte)),
        (b'0', b'x') => digits(at + 2, |byte| byte.is_ascii_hexdigit()),
        _ => integer,
    };
    for suffix in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "isize", "usize"] {
        if scan::at(bytes, end, suffix) {
            return Some(end + suffix.len());
        }
    }
    Some(end)
}
