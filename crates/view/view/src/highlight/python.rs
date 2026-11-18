//! Python, as CodeMirror 5's python mode colors it: every name not a keyword or a builtin is a
//! variable (`self` and `cls` too), a name after `def` or `class` a definition, after `.` a
//! property, a decorator meta; an f-string's `{…}` is code.

use super::Kind;
use super::scan::{self, Out};

/// A multi-line (triple-quoted) string left open at a line's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Open {
    quote: u8,
    format: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Python {
    open: Option<Open>,
    /// An f-string's `{…}` left open (its depth of braces inside).
    braces: Vec<(Open, usize)>,
}

const KEYWORDS: &[&str] = &[
    "as", "assert", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "lambda", "pass", "raise", "return", "try", "while", "with", "yield", "in", "False", "True", "nonlocal", "None", "async", "await", "and", "or",
    "not", "is",
];
const BUILTINS: &[&str] = &[
    "abs",
    "all",
    "any",
    "bin",
    "bool",
    "bytearray",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "property",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
    "__import__",
    "NotImplemented",
    "Ellipsis",
    "__debug__",
    "ascii",
    "bytes",
    "exec",
    "print",
];

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn operator_char(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'~' | b'<' | b'>' | b'!' | b'@' | b'=')
}

fn word_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

/// The last token, for what a name after it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Last {
    Other,
    Def,
    Dot,
    Meta,
}

pub fn line(text: &str, python: &mut Python, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut last = Last::Other;
    let mut first = true;
    // An f-string's expression does not run on to the next line.
    python.braces.clear();
    if let Some(open) = python.open {
        at = string_body(bytes, 0, 0, open, true, python, out);
    }
    tokens(text, bytes, &mut at, &mut last, &mut first, python, out, None);
}

/// The tokens from `at`, up to the line's end or, inside an f-string's `{…}` (`until`), its `}`.
#[allow(clippy::too_many_arguments)]
fn tokens(text: &str, bytes: &[u8], at: &mut usize, last: &mut Last, first: &mut bool, python: &mut Python, out: &mut Out, until: Option<Open>) {
    let mut depth = 0usize;
    while *at < bytes.len() {
        let here = byte(bytes, *at);
        if here == b' ' || here == b'\t' || here == b'\r' {
            *at += 1;
            continue;
        }
        let beginning = std::mem::take(first);
        let string = string_start(bytes, *at);
        match here {
            b'#' => {
                out.push(*at..bytes.len(), Kind::Comment);
                *at = bytes.len();
                return;
            }
            b'}' if until.is_some() && depth == 0 => return,
            b'{' | b'(' | b'[' => {
                depth += 1;
                *at += 1;
                *last = Last::Other;
            }
            b'}' | b')' | b']' => {
                depth = depth.saturating_sub(1);
                *at += 1;
                *last = Last::Other;
            }
            b'.' if !byte(bytes, *at + 1).is_ascii_digit() => {
                *at += 1;
                *last = Last::Dot;
            }
            b'@' if beginning && word_start(byte(bytes, *at + 1)) => {
                let end = scan::word_end(bytes, *at + 1, b"");
                out.push(*at..end, Kind::Comment);
                *at = end;
                *last = Last::Meta;
            }
            _ if let Some((prefix, quote)) = string => {
                let format = text.get(*at..*at + prefix).is_some_and(|prefix| prefix.contains(['f', 'F']));
                let triple = byte(bytes, *at + prefix + 1) == quote && byte(bytes, *at + prefix + 2) == quote;
                let open = Open { quote, format };
                let body = *at + prefix + if triple { 3 } else { 1 };
                *at = string_body(bytes, *at, body, open, triple, python, out);
                *last = Last::Other;
                // An f-string's `{` left its expression to read here.
                while let Some((open, _)) = python.braces.last().copied() {
                    let mut inner_last = Last::Other;
                    let mut inner_first = false;
                    tokens(text, bytes, at, &mut inner_last, &mut inner_first, python, out, Some(open));
                    if byte(bytes, *at) != b'}' {
                        return;
                    }
                    python.braces.pop();
                    let triple_open = python.open.is_some();
                    *at = string_body(bytes, *at + 1, *at + 1, open, triple_open || triple, python, out);
                }
            }
            b'0'..=b'9' | b'.' => {
                let end = number(bytes, *at);
                out.push(*at..end, Kind::Number);
                *at = end;
                *last = Last::Other;
            }
            _ if word_start(here) => {
                let end = scan::word_end(bytes, *at, b"");
                let word = text.get(*at..end).unwrap_or("");
                let kind = match *last {
                    Last::Dot => Some(Kind::Property),
                    _ if KEYWORDS.contains(&word) => Some(Kind::Keyword),
                    _ if BUILTINS.contains(&word) => Some(Kind::Property),
                    Last::Def => None,
                    _ => Some(Kind::Variable),
                };
                out.mark(*at..end, kind);
                *last = if word == "def" || word == "class" { Last::Def } else { Last::Other };
                *at = end;
            }
            _ if operator_char(here) => {
                let mut end = *at;
                while operator_char(byte(bytes, end)) {
                    end += 1;
                }
                out.push(*at..end, Kind::Operator);
                *at = end;
                *last = Last::Other;
            }
            _ => {
                *at += 1;
                *last = Last::Other;
            }
        }
    }
}

/// A string's prefix length (`r`, `b`, `f`, `rb`, …) and quote at `at`, when one starts there.
fn string_start(bytes: &[u8], at: usize) -> Option<(usize, u8)> {
    let mut prefix = 0;
    while prefix < 2 && matches!(byte(bytes, at + prefix).to_ascii_lowercase(), b'r' | b'b' | b'u' | b'f') {
        prefix += 1;
    }
    let quote = byte(bytes, at + prefix);
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    if prefix > 0 && at > 0 && scan::word(byte(bytes, at - 1)) {
        return None;
    }
    Some((prefix, quote))
}

/// A string's text from `start`, read from `from`: to its closing quote (or quotes, `triple`), or
/// to an f-string's `{` (its expression to read next), or to the line's end (a triple-quoted one
/// stays open).
fn string_body(bytes: &[u8], start: usize, from: usize, open: Open, triple: bool, python: &mut Python, out: &mut Out) -> usize {
    let mut index = from;
    python.open = None;
    while let Some(here) = bytes.get(index) {
        match *here {
            b'\\' => index += 2,
            b'{' if open.format && byte(bytes, index + 1) == b'{' => index += 2,
            b'{' if open.format => {
                out.push(start..index, Kind::String);
                python.braces.push((open, 0));
                if triple {
                    python.open = Some(open);
                }
                return index + 1;
            }
            quote if quote == open.quote => {
                if !triple {
                    out.push(start..index + 1, Kind::String);
                    return index + 1;
                }
                if byte(bytes, index + 1) == quote && byte(bytes, index + 2) == quote {
                    out.push(start..index + 3, Kind::String);
                    return index + 3;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    out.push(start..bytes.len(), Kind::String);
    if triple {
        python.open = Some(open);
    }
    bytes.len()
}

/// The end of a number at `at`: `0x…`, `0o…`, `0b…`, or digits with a fraction, an exponent, and
/// an imaginary `j`.
fn number(bytes: &[u8], at: usize) -> usize {
    let lower = |index: usize| byte(bytes, index).to_ascii_lowercase();
    let mut end = at;
    if lower(at) == b'0' && matches!(lower(at + 1), b'x' | b'o' | b'b') {
        end = at + 2;
        while lower(end).is_ascii_hexdigit() || lower(end) == b'_' {
            end += 1;
        }
        return end;
    }
    while lower(end).is_ascii_digit() || lower(end) == b'_' {
        end += 1;
    }
    if lower(end) == b'.' {
        end += 1;
        while lower(end).is_ascii_digit() || lower(end) == b'_' {
            end += 1;
        }
    }
    end = scan::exponent(bytes, end);
    if matches!(lower(end), b'j' | b'l') {
        end += 1;
    }
    end.max(at + 1)
}
