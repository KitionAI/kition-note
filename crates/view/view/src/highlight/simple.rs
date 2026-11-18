//! Go, as CodeMirror 5's go mode colors it (every name a variable, its types keywords, its
//! builtins atoms, and a `.` the start of a number), and what Go and Rust carry between lines.

use super::Kind;
use super::scan::{self, Out};

/// What is open at a line's end: a block comment, or a string (its closing text).
#[derive(Clone, Debug, Default)]
pub struct Simple {
    pub comment: bool,
    pub string: Option<&'static str>,
}

const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    "bool",
    "byte",
    "complex64",
    "complex128",
    "float32",
    "float64",
    "int8",
    "int16",
    "int32",
    "int64",
    "string",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "int",
    "uint",
    "uintptr",
    "error",
    "rune",
    "any",
    "comparable",
];
const GO_ATOMS: &[&str] = &[
    "true", "false", "iota", "nil", "append", "cap", "close", "complex", "copy", "delete", "imag", "len", "make", "new", "panic", "print", "println",
    "real", "recover",
];

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

/// From `at` inside a block comment: to its `*/`, or the line's end (it stays open).
pub fn block_comment(text: &str, at: usize, state: &mut Simple, out: &mut Out) -> usize {
    match text.get(at..).and_then(|rest| rest.find("*/")) {
        Some(found) => {
            out.push(at..at + found + 2, Kind::Comment);
            state.comment = false;
            at + found + 2
        }
        None => {
            out.push(at..text.len(), Kind::Comment);
            state.comment = true;
            text.len()
        }
    }
}

pub fn go(text: &str, state: &mut Simple, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if state.comment {
        at = block_comment(text, 0, state, out);
    }
    if state.string.is_some() {
        match text.find('`') {
            Some(found) => {
                out.push(0..found + 1, Kind::String);
                state.string = None;
                at = found + 1;
            }
            None => {
                out.push(0..text.len(), Kind::String);
                return;
            }
        }
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        match here {
            b'"' | b'\'' => {
                let end = scan::string_end(bytes, at, here).unwrap_or(bytes.len());
                out.push(at..end, Kind::String);
                at = end;
            }
            b'`' => match text.get(at + 1..).and_then(|rest| rest.find('`')) {
                Some(found) => {
                    out.push(at..at + found + 2, Kind::String);
                    at += found + 2;
                }
                None => {
                    out.push(at..bytes.len(), Kind::String);
                    state.string = Some("`");
                    return;
                }
            },
            b'0'..=b'9' | b'.' => {
                let mut end = at + 1;
                if here == b'.' {
                    while byte(bytes, end).is_ascii_digit() || byte(bytes, end) == b'_' {
                        end += 1;
                    }
                    end = scan::exponent(bytes, end);
                } else if here == b'0' && matches!(byte(bytes, end).to_ascii_lowercase(), b'x' | b'b' | b'o') {
                    end += 1;
                    while byte(bytes, end).is_ascii_hexdigit() || byte(bytes, end) == b'_' {
                        end += 1;
                    }
                } else {
                    while byte(bytes, end).is_ascii_digit() || byte(bytes, end) == b'_' {
                        end += 1;
                    }
                    if byte(bytes, end) == b'.' {
                        end += 1;
                        while byte(bytes, end).is_ascii_digit() || byte(bytes, end) == b'_' {
                            end += 1;
                        }
                    }
                    end = scan::exponent(bytes, end);
                }
                if byte(bytes, end) == b'i' {
                    end += 1;
                }
                out.push(at..end, Kind::Number);
                at = end;
            }
            b'[' | b']' | b'{' | b'}' | b'(' | b')' | b',' | b';' | b':' => at += 1,
            b'/' if byte(bytes, at + 1) == b'/' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'/' if byte(bytes, at + 1) == b'*' => at = block_comment(text, at, state, out),
            b'+' | b'-' | b'*' | b'&' | b'^' | b'%' | b'=' | b'<' | b'>' | b'!' | b'|' | b'/' => {
                let mut end = at;
                while matches!(byte(bytes, end), b'+' | b'-' | b'*' | b'&' | b'^' | b'%' | b':' | b'=' | b'<' | b'>' | b'!' | b'|' | b'/') {
                    end += 1;
                }
                out.push(at..end, Kind::Operator);
                at = end;
            }
            b' ' | b'\t' | b'\r' => at += 1,
            _ => {
                let end = scan::word_end(bytes, at, b"$").max(at + 1);
                let word = text.get(at..end).unwrap_or("");
                if GO_KEYWORDS.contains(&word) {
                    out.push(at..end, Kind::Keyword);
                } else if !GO_ATOMS.contains(&word) && scan::word(here) {
                    out.push(at..end, Kind::Variable);
                }
                at = end;
            }
        }
    }
}
