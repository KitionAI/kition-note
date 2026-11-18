//! TOML, ini (`properties`), and diff, as CodeMirror 5's toml, properties, and diff modes color
//! them: a TOML key a property (a quoted one too), its value's string, number; an ini comment; a
//! diff's added line green (a string's color), its removed line red, its `@@` line meta.

use super::Kind;
use super::scan::Out;

/// A TOML array left open at a line's end (its values go on).
#[derive(Clone, Debug, Default)]
pub struct Toml {
    arrays: usize,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

pub fn line(text: &str, toml: &mut Toml, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut key = toml.arrays == 0;
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        match here {
            b'"' | b'\'' => {
                at += 1;
                while at < bytes.len() && byte(bytes, at) != here {
                    at += if byte(bytes, at) == b'\\' { 2 } else { 1 };
                }
                at = (at + 1).min(bytes.len());
                out.push(start..at, if key { Kind::Property } else { Kind::String });
            }
            b']' if toml.arrays > 0 => {
                toml.arrays -= 1;
                at += 1;
                out.push(start..at, Kind::Punctuation);
            }
            b'[' if key => {
                // A table's header: an atom, in the text's color.
                at = text.get(at..).and_then(|rest| rest.find(']')).map_or(bytes.len(), |found| at + found + 1);
                if byte(bytes, at) == b']' {
                    at += 1;
                }
            }
            b'#' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b' ' | b'\t' | b'\r' => at += 1,
            b'=' if key => {
                at += 1;
                key = false;
            }
            _ if key => {
                while at < bytes.len() && !matches!(byte(bytes, at), b'=' | b' ') {
                    at += 1;
                }
                out.push(start..at, Kind::Property);
            }
            b'[' => {
                toml.arrays += 1;
                at += 1;
                out.push(start..at, Kind::Punctuation);
            }
            _ => {
                let rest = text.get(at..).unwrap_or("");
                if rest.starts_with("true") || rest.starts_with("false") {
                    at += if rest.starts_with("true") { 4 } else { 5 };
                    continue;
                }
                let digits = |from: usize| {
                    let mut end = from;
                    while byte(bytes, end).is_ascii_digit() {
                        end += 1;
                    }
                    end
                };
                let sign = usize::from(here == b'-');
                let whole = digits(at + sign);
                if whole > at + sign {
                    // A date (`1979-05-27T07:32:00Z`) is an atom; a number is a number.
                    let date = whole - at - sign == 4 && byte(bytes, whole) == b'-';
                    let mut end = whole;
                    if date {
                        while matches!(byte(bytes, end), b'0'..=b'9' | b'-' | b':' | b'.' | b'T') {
                            end += 1;
                        }
                        if byte(bytes, end) == b'Z' {
                            at = end + 1;
                            continue;
                        }
                        end = whole;
                    }
                    if byte(bytes, end) == b'.' && byte(bytes, end + 1).is_ascii_digit() {
                        end = digits(end + 1);
                    }
                    out.push(at..end, Kind::Number);
                    at = end;
                    continue;
                }
                at += 1;
            }
        }
    }
}

/// An ini or `.properties` line: only its comment (a line starting `#`, `!`, or `;`) is colored.
pub fn properties(text: &str, out: &mut Out) {
    let trimmed = text.trim_start();
    if trimmed.starts_with(['#', '!', ';']) {
        out.push(text.len() - trimmed.len()..text.len(), Kind::Comment);
    }
}

/// A diff's line: an added line (`+`) green, a removed one (`-`) red, a hunk's `@@` meta, each up
/// to its trailing spaces.
pub fn diff(text: &str, out: &mut Out) {
    let kind = match text.as_bytes().first() {
        Some(b'+') => Kind::String,
        Some(b'-') => Kind::Operator,
        Some(b'@') => Kind::Comment,
        _ => return,
    };
    out.push(0..text.trim_end().len(), kind);
}
