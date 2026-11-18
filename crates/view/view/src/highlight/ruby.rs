//! Ruby, as CodeMirror 5's ruby mode colors it: keywords, a capitalized name a tag (a constant
//! or a class), the name after `def` and a block's `|params|` definitions, `@ivar` and `$global`
//! variables, `:symbol` and `label:` atoms, `.` an operator and the name after it a property,
//! strings with `#{…}` code, regular expressions.

use super::Kind;
use super::scan::{self, Out};

/// What carries from one line to the next: an open string (its quote) or `=begin` comment, and
/// the `#{…}` depth inside a string.
#[derive(Clone, Debug, Default)]
pub struct Ruby {
    string: Option<u8>,
    comment: bool,
    /// Braces open in a string's `#{…}` (the string resumes when they close).
    interpolation: Vec<(u8, usize)>,
}

const KEYWORDS: &[&str] = &[
    "alias",
    "and",
    "BEGIN",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "defined?",
    "do",
    "else",
    "elsif",
    "END",
    "end",
    "ensure",
    "false",
    "for",
    "if",
    "in",
    "module",
    "next",
    "not",
    "or",
    "redo",
    "rescue",
    "retry",
    "return",
    "self",
    "super",
    "then",
    "true",
    "undef",
    "unless",
    "until",
    "when",
    "while",
    "yield",
    "nil",
    "raise",
    "throw",
    "catch",
    "fail",
    "loop",
    "callcc",
    "caller",
    "lambda",
    "proc",
    "public",
    "protected",
    "private",
    "require",
    "load",
    "require_relative",
    "extend",
    "autoload",
    "__END__",
    "__FILE__",
    "__LINE__",
    "__dir__",
];

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn operator_char(byte: u8) -> bool {
    matches!(byte, b'=' | b'+' | b'-' | b'/' | b'*' | b':' | b'.' | b'^' | b'%' | b'<' | b'>' | b'~' | b'|')
}

/// The token before, for what a name after it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Last {
    Other,
    Def,
    Dot,
    Params,
}

pub fn line(text: &str, ruby: &mut Ruby, out: &mut Out) {
    let bytes = text.as_bytes();
    if ruby.comment {
        out.push(0..text.len(), Kind::Comment);
        if text.starts_with("=end") {
            ruby.comment = false;
        }
        return;
    }
    if text.starts_with("=begin") {
        out.push(0..text.len(), Kind::Comment);
        ruby.comment = true;
        return;
    }
    let mut at = 0;
    let mut last = Last::Other;
    if let Some(quote) = ruby.string {
        at = string(bytes, 0, 0, quote, ruby, out);
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'#' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'}' if ruby.interpolation.last().is_some_and(|(_, depth)| *depth == 0) => {
                let Some((quote, _)) = ruby.interpolation.pop() else { break };
                at = string(bytes, at, at + 1, quote, ruby, out);
            }
            b'{' => {
                if let Some((_, depth)) = ruby.interpolation.last_mut() {
                    *depth += 1;
                }
                at += 1;
                if scan::next_significant(bytes, at).is_some_and(|(_, next)| next == b'|') {
                    last = Last::Params;
                }
            }
            b'}' => {
                if let Some((_, depth)) = ruby.interpolation.last_mut() {
                    *depth = depth.saturating_sub(1);
                }
                at += 1;
            }
            b'|' if last == Last::Params => {
                // A block's parameters, to the closing `|`.
                at += 1;
                while at < bytes.len() && byte(bytes, at) != b'|' {
                    at += 1;
                }
                at += 1;
                last = Last::Other;
            }
            b'"' | b'\'' | b'`' => at = string(bytes, at, at + 1, here, ruby, out),
            b'@' | b'$' if scan::word(byte(bytes, at + 1)) || byte(bytes, at + 1) == b'@' => {
                at += 1;
                while byte(bytes, at) == b'@' {
                    at += 1;
                }
                at = scan::word_end(bytes, at, b"");
                out.push(start..at, Kind::Variable);
                last = Last::Other;
            }
            b':' if byte(bytes, at + 1) != b':' && (byte(bytes, at + 1).is_ascii_alphabetic() || byte(bytes, at + 1) == b'_') => {
                // A symbol: an atom, in the text's color.
                at = word_end(bytes, at + 1);
                last = Last::Other;
            }
            b'/' if regex_ahead(bytes, at + 1) => {
                at = scan::string_end(bytes, at, b'/').unwrap_or(bytes.len());
                out.push(start..at, Kind::String);
            }
            b'0'..=b'9' => {
                at = number(bytes, at);
                out.push(start..at, Kind::Number);
                last = Last::Other;
            }
            _ if operator_char(here) => {
                while operator_char(byte(bytes, at)) {
                    at += 1;
                }
                out.push(start..at, Kind::Operator);
                last = if here == b'.' && at == start + 1 { Last::Dot } else { Last::Other };
            }
            _ if here.is_ascii_alphabetic() || here == b'_' || here >= 0x80 => {
                at = word_end(bytes, at);
                let word = text.get(start..at).unwrap_or("");
                // `label:` is an atom with its colon (`Shop::` too: one colon, then `:Cart`).
                if byte(bytes, at) == b':' && last != Last::Dot {
                    at += 1;
                    last = Last::Other;
                    continue;
                }
                let kind = if last == Last::Dot {
                    Some(Kind::Property)
                } else if last == Last::Def {
                    None
                } else if KEYWORDS.contains(&word) {
                    Some(Kind::Keyword)
                } else if here.is_ascii_uppercase() {
                    Some(Kind::Operator)
                } else {
                    Some(Kind::Variable)
                };
                out.mark(start..at, kind);
                last = match word {
                    "def" => Last::Def,
                    "do" if scan::next_significant(bytes, at).is_some_and(|(_, next)| next == b'|') => Last::Params,
                    _ => Last::Other,
                };
            }
            _ => {
                at += 1;
                last = if last == Last::Params { Last::Params } else { Last::Other };
            }
        }
    }
}

/// The end of a number at `at`: hex, or digits with a fraction (a `.` only before a digit, so
/// `1..5` is a range) and an exponent.
fn number(bytes: &[u8], at: usize) -> usize {
    let digits = |from: usize, ok: fn(u8) -> bool| {
        let mut end = from;
        while ok(byte(bytes, end)) || byte(bytes, end) == b'_' {
            end += 1;
        }
        end
    };
    if byte(bytes, at) == b'0' && matches!(byte(bytes, at + 1), b'x' | b'X') {
        return digits(at + 2, |byte| byte.is_ascii_hexdigit());
    }
    let mut end = digits(at, |byte| byte.is_ascii_digit());
    if byte(bytes, end) == b'.' && byte(bytes, end + 1).is_ascii_digit() {
        end = digits(end + 1, |byte| byte.is_ascii_digit());
    }
    scan::exponent(bytes, end)
}

/// The end of a name at `at` (a method's `?` or `!` with it).
fn word_end(bytes: &[u8], at: usize) -> usize {
    let end = scan::word_end(bytes, at, b"");
    if matches!(byte(bytes, end), b'?' | b'!') && byte(bytes, end + 1) != b'=' { end + 1 } else { end }
}

/// Whether a `/` at `at - 1` opens a regular expression: its closing `/` is on the line, outside
/// brackets, as the mode looks ahead.
fn regex_ahead(bytes: &[u8], at: usize) -> bool {
    let mut depth = 0i32;
    let mut index = at;
    while let Some(next) = bytes.get(index) {
        match next {
            b'\\' => index += 1,
            b'[' | b'{' | b'(' => depth += 1,
            b']' | b'}' | b')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            b'/' if depth == 0 => return true,
            _ => {}
        }
        index += 1;
    }
    false
}

/// A string's text from `start`, read from `from`: to its closing quote, or a `#{` (its code to
/// read next), or the line's end (it stays open).
fn string(bytes: &[u8], start: usize, from: usize, quote: u8, ruby: &mut Ruby, out: &mut Out) -> usize {
    let mut index = from;
    ruby.string = None;
    while let Some(here) = bytes.get(index) {
        match *here {
            b'\\' => index += 2,
            b'#' if quote != b'\'' && byte(bytes, index + 1) == b'{' => {
                out.push(start..index + 2, Kind::String);
                ruby.interpolation.push((quote, 0));
                return index + 2;
            }
            close if close == quote => {
                out.push(start..index + 1, Kind::String);
                return index + 1;
            }
            _ => index += 1,
        }
    }
    out.push(start..bytes.len(), Kind::String);
    ruby.string = Some(quote);
    bytes.len()
}
