//! YAML, as CodeMirror 5's yaml mode colors it: a key an atom (in the text's color), its `: ` and
//! a list's `- ` meta, a value's number or boolean (the whole rest of the line) a number or a
//! keyword, quoted strings, comments, a block literal's lines strings.

use super::Kind;
use super::scan::Out;

#[derive(Clone, Debug, Default)]
pub struct Yaml {
    /// In a block literal (`|`, `>`) under a key at this column.
    literal: Option<usize>,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

pub fn line(text: &str, yaml: &mut Yaml, out: &mut Out) {
    let bytes = text.as_bytes();
    let indent = bytes.iter().take_while(|byte| **byte == b' ').count();
    if let Some(column) = yaml.literal {
        if indent > column && indent < bytes.len() {
            out.push(indent..bytes.len(), Kind::String);
            return;
        }
        if indent < bytes.len() {
            yaml.literal = None;
        } else {
            return;
        }
    }
    let mut at = 0;
    let mut pair = false;
    let mut pair_start = false;
    let mut key_column = 0;
    let mut inline = 0usize;
    let trimmed = text.trim_end();
    if trimmed == "---" || trimmed == "..." {
        return;
    }
    // A list item's `- ` at the line's start (its indentation too).
    let mut dash = indent;
    while byte(bytes, dash) == b'-' && matches!(byte(bytes, dash + 1), b' ' | b'\t') {
        dash += 1;
        while matches!(byte(bytes, dash), b' ' | b'\t') {
            dash += 1;
        }
    }
    if dash > indent {
        out.push(0..dash, Kind::Comment);
        at = dash;
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        if here == b'#' && (at == 0 || matches!(byte(bytes, at - 1), b' ' | b'\t')) {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        if here == b'\'' || here == b'"' {
            let mut end = at + 1;
            while end < bytes.len() && byte(bytes, end) != here {
                if byte(bytes, end) == b'\\' {
                    end += 1;
                }
                end += 1;
            }
            let end = (end + 1).min(bytes.len());
            out.push(at..end, Kind::String);
            at = end;
            pair_start = false;
            continue;
        }
        if matches!(here, b'{' | b'}' | b'[' | b']') {
            if matches!(here, b'{' | b'[') {
                inline += 1;
            } else {
                inline = inline.saturating_sub(1);
            }
            out.push(at..at + 1, Kind::Comment);
            at += 1;
            continue;
        }
        // A separator in an inline list or map.
        if inline > 0 && here == b',' {
            out.push(at..at + 1, Kind::Comment);
            at += 1;
            continue;
        }
        if pair_start {
            let rest = text.get(at..).unwrap_or("");
            let spaced = rest.trim_start_matches([' ', '\t']);
            let lead = rest.len() - spaced.len();
            if spaced.starts_with('|') || spaced.starts_with('>') {
                out.push(at..text.len(), Kind::Comment);
                yaml.literal = Some(key_column);
                return;
            }
            if (spaced.starts_with('&') || spaced.starts_with('*')) && spaced.len() > 1 {
                let name = spaced.get(1..).unwrap_or("");
                let length = name.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))).unwrap_or(name.len());
                if length > 0 {
                    out.push(at..at + lead + 1 + length, Kind::Variable);
                    at += lead + 1 + length;
                    continue;
                }
            }
            let value = spaced.trim_end();
            let numeric = value.strip_prefix('-').unwrap_or(value);
            if !numeric.is_empty() && numeric.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b',') {
                out.push(at..text.len(), Kind::Number);
                return;
            }
            // A boolean is the whole rest of the line.
            if ["true", "false", "on", "off", "yes", "no"].iter().any(|word| rest.eq_ignore_ascii_case(word)) {
                out.push(at..text.len(), Kind::Keyword);
                return;
            }
        }
        if !pair && let Some(length) = key(text.get(at..).unwrap_or("")) {
            // A key is the mode's atom: the text's color.
            at += length;
            pair = true;
            key_column = indent;
            continue;
        }
        if pair && here == b':' {
            let mut end = at + 1;
            while matches!(byte(bytes, end), b' ' | b'\t') {
                end += 1;
            }
            out.push(at..end, Kind::Comment);
            at = end;
            pair_start = true;
            continue;
        }
        pair_start = false;
        at += 1;
    }
}

/// A key's length at the start of `rest` (its text up to a `:` followed by a space or the end).
fn key(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let first = *bytes.first()?;
    if matches!(first, b' ' | b'\t' | b',' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>' | b'\'' | b'"' | b'%' | b'@' | b'`')
    {
        return None;
    }
    let mut index = 0;
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'#' => return None,
            b':' if matches!(bytes.get(index + 1), None | Some(b' ' | b'\t')) => return Some(index),
            _ => index += 1,
        }
    }
    None
}
