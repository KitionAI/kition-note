//! `property.set`, `property.delete`, `property.rename`: lossless edits of the property block.
//! Only the key's own lines change, in the key's own style. Nested maps, block scalars, comments and the like are refused
//! (`note.property-complex`), never rewritten.

use kition_note_markdown::{Entry, Frontmatter, Index, Style, sub};

use crate::{Edit, EditError, Reason, Result, eol_of, minimal, not_found};

/// A value a command may write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    /// As written in the request's JSON.
    Number(String),
    Str(String),
    List(Vec<String>),
}

fn trim_eol(s: &str) -> &str {
    s.trim_end_matches('\n').trim_end_matches('\r')
}

/// Whether `s` reads back as the same string when written plain.
fn plain_safe(s: &str) -> bool {
    !s.is_empty()
        && s.trim() == s
        && !s.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.contains('\n')
        && !s.ends_with(':')
        && !matches!(s.to_ascii_lowercase().as_str(), "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off")
        && s.parse::<f64>().is_err()
        && !s.starts_with("0x")
        && !s.starts_with("0o")
}

fn double(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `s` as a scalar in `style`, falling back to double quotes when plain would change its meaning.
fn scalar_in(style: &Style, s: &str) -> String {
    match style {
        Style::Double => double(s),
        Style::Single if !s.contains('\n') => format!("'{}'", s.replace('\'', "''")),
        _ if plain_safe(s) => s.to_owned(),
        _ => double(s),
    }
}

fn item_style(item: &str) -> Style {
    let item = item.trim();
    if item.starts_with('"') {
        Style::Double
    } else if item.starts_with('\'') {
        Style::Single
    } else {
        Style::Plain
    }
}

fn flow_item(style: &Style, s: &str) -> String {
    if matches!(style, Style::Plain) && plain_safe(s) && !s.contains([',', '[', ']']) { s.to_owned() } else { scalar_in(&Style::Double, s) }
}

/// The text of a scalar as written: quotes removed and escapes undone.
pub fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if let Some(inner) = t.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')) {
        inner.replace("''", "'")
    } else {
        t.to_owned()
    }
}

/// A key as written: plain when YAML reads it back as the same key, double-quoted otherwise.
fn key_text(key: &str) -> String {
    if plain_safe(key) && !key.contains(['#', ':']) { key.to_owned() } else { double(key) }
}

fn scalar_text(style: &Style, value: &Value) -> String {
    match value {
        Value::Str(s) => scalar_in(style, s),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.clone(),
        Value::List(items) => format!("[{}]", items.iter().map(|item| flow_item(&Style::Plain, item)).collect::<Vec<_>>().join(", ")),
    }
}

fn render_entry(key: &str, value: &Value) -> String {
    match value {
        Value::Null => format!("{}:", key_text(key)),
        other => format!("{}: {}", key_text(key), scalar_text(&Style::Plain, other)),
    }
}

fn block(ix: &Index) -> Option<&Frontmatter> {
    ix.frontmatter.as_ref()
}

fn refuse_unreadable(fm: &Frontmatter, key: &str) -> Result<()> {
    if fm.duplicates.iter().any(|duplicate| duplicate == key) {
        return Err(EditError::coded(
            Reason::Refused,
            "note.frontmatter-duplicate-key",
            format!("the key {key:?} is written more than once; delete one with property.delete and occurrence"),
        ));
    }
    if let Some(invalid) = &fm.invalid {
        return Err(EditError::coded(
            Reason::Refused,
            "note.frontmatter-invalid",
            format!("the property block is not valid YAML (line {}: {}); fix it in the text", invalid.line, invalid.message),
        ));
    }
    Ok(())
}

fn editable<'a>(fm: &'a Frontmatter, key: &str) -> Result<Option<&'a Entry>> {
    refuse_unreadable(fm, key)?;
    let Some(entry) = fm.entries.iter().find(|entry| entry.key == key) else { return Ok(None) };
    if let Style::Complex(why) = entry.style {
        return Err(EditError::coded(
            Reason::Refused,
            "note.property-complex",
            format!("the value of {key:?} is a {why}; the engine reads it but does not rewrite it, so edit it in the text"),
        ));
    }
    Ok(Some(entry))
}

/// `property.set`: writes `value` into `key`'s lines in the key's style; a new key goes last in
/// the block; a note without a block gets one.
pub fn set(text: &str, ix: &Index, key: &str, value: &Value) -> Result<Vec<Edit>> {
    if key.trim().is_empty() || key.contains('\n') {
        return Err(crate::invalid("key must be a non-empty single-line string"));
    }
    let Some(fm) = block(ix) else {
        return Ok(vec![Edit::insert(0, format!("---\n{}\n---\n", render_entry(key, value)))]);
    };
    let Some(entry) = editable(fm, key)? else {
        let close_line = ix.lines.line(ix.lines.line_of(fm.body.end)).unwrap_or(fm.body.end..fm.body.end);
        let eol = eol_of(text, close_line);
        return Ok(vec![Edit::insert(fm.body.end, format!("{}{eol}", render_entry(key, value)))]);
    };
    let eol = eol_of(text, entry.lines.clone());
    match (&entry.style, value) {
        (Style::BlockList { prefix }, Value::List(items)) => {
            let first = entry.items.first().map_or(entry.value.end, |line| line.start);
            let last = entry.items.last().map_or(entry.value.end, |line| line.end);
            if items.is_empty() {
                return Ok(vec![Edit::new(entry.value.start..last, " []")]);
            }
            let olds: Vec<(&str, String)> = entry
                .items
                .iter()
                .map(|line| {
                    let s = trim_eol(sub(text, line.clone()));
                    (s, unquote(sub(s, prefix.len().min(s.len())..s.len())))
                })
                .collect();
            let style = olds.first().map_or(Style::Plain, |(line, _)| item_style(sub(line, prefix.len().min(line.len())..line.len())));
            let lines: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(index, item)| match olds.get(index) {
                    Some((line, old)) if old == item => (*line).to_owned(),
                    _ => format!("{prefix}{}", scalar_in(&style, item)),
                })
                .collect();
            Ok(minimal(text, first..last, &lines.join(eol)))
        }
        (Style::FlowList, Value::List(items)) => {
            let written = sub(text, entry.value.clone()).trim();
            let inner = written.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).unwrap_or_default();
            let separator = if inner.contains(", ") || !inner.contains(',') { ", " } else { "," };
            let style = item_style(inner.split(',').next().unwrap_or_default());
            let pad = if inner.starts_with(' ') { " " } else { "" };
            let body: Vec<String> = items.iter().map(|item| flow_item(&style, item)).collect();
            let new = if body.is_empty() { "[]".to_owned() } else { format!("[{pad}{}{pad}]", body.join(separator)) };
            Ok(minimal(text, entry.value.clone(), &new))
        }
        (Style::BlockList { .. } | Style::FlowList, _) | (_, Value::List(_)) => {
            // The value changes kind: the entry is written anew, a list in flow style.
            let lines = entry.lines.start..entry.lines.end - if sub(text, entry.lines.clone()).ends_with('\n') { eol.len() } else { 0 };
            Ok(minimal(text, lines, &render_entry(key, value)))
        }
        (style, value) => {
            let new = scalar_text(style, value);
            if entry.value.is_empty() {
                // `key:` becomes `key: value`, on the header line only.
                let header = trim_eol(sub(text, entry.lines.clone()));
                let at = entry.lines.start + header.len();
                let space = if header.ends_with([' ', '\t']) { "" } else { " " };
                if matches!(value, Value::Null) {
                    return Ok(Vec::new());
                }
                Ok(vec![Edit::insert(at, format!("{space}{new}"))])
            } else {
                let current = sub(text, entry.value.clone());
                // A plain value that reads as the same thing stays as written.
                if matches!(style, Style::Plain | Style::Single | Style::Double)
                    && matches!(value, Value::Str(s) if unquote(current) == *s && *style != Style::Plain)
                {
                    return Ok(Vec::new());
                }
                Ok(minimal(text, entry.value.clone(), &new))
            }
        }
    }
}

fn occurrence_of<'a>(fm: &'a Frontmatter, key: &str, occurrence: Option<usize>) -> Result<&'a Entry> {
    let all: Vec<&Entry> = fm.entries.iter().filter(|entry| entry.key == key).collect();
    match (occurrence, all.as_slice()) {
        (_, []) => Err(not_found(format!("the note has no property {key:?}"))),
        (None, [one]) => Ok(one),
        (None, many) => Err(EditError::new(Reason::Ambiguous, format!("{key:?} is written {} times; give occurrence (from 1)", many.len()))),
        (Some(n), all) => n
            .checked_sub(1)
            .and_then(|index| all.get(index))
            .copied()
            .ok_or_else(|| EditError::new(Reason::OutOfRange, format!("{key:?} is written {} times, so there is no occurrence {n}", all.len()))),
    }
}

/// `property.delete`: removes the key's lines (one of a duplicated key's with `occurrence`).
pub fn delete(ix: &Index, key: &str, occurrence: Option<usize>) -> Result<Vec<Edit>> {
    let fm = block(ix).ok_or_else(|| not_found("the note has no properties"))?;
    let entry = occurrence_of(fm, key, occurrence)?;
    Ok(vec![Edit::delete(entry.lines.clone())])
}

/// `property.rename`: changes only the key token.
pub fn rename(ix: &Index, key: &str, to: &str) -> Result<Vec<Edit>> {
    if to.trim().is_empty() || to.contains('\n') {
        return Err(crate::invalid("to must be a non-empty single-line string"));
    }
    let fm = block(ix).ok_or_else(|| not_found("the note has no properties"))?;
    refuse_unreadable(fm, key)?;
    if to != key && fm.entries.iter().any(|entry| entry.key == to) {
        return Err(EditError::coded(Reason::Refused, "note.frontmatter-duplicate-key", format!("the note already has a property {to:?}")));
    }
    let entry = occurrence_of(fm, key, None)?;
    if to == key {
        return Ok(Vec::new());
    }
    Ok(vec![Edit::new(entry.key_range.clone(), key_text(to))])
}

#[cfg(test)]
mod tests {
    use kition_note_markdown::index;

    use super::*;
    use crate::apply;

    fn run(text: &str, key: &str, value: Value) -> String {
        apply(text, &set(text, &index(text), key, &value).unwrap())
    }

    fn refused(text: &str, key: &str) -> Option<&'static str> {
        set(text, &index(text), key, &Value::Str("B".into())).unwrap_err().code
    }

    #[test]
    fn scalars_keep_their_quotes() {
        assert_eq!(run("---\na: x\nb: \"y\"\nc: 'z'\n---\n", "b", Value::Str("q".into())), "---\na: x\nb: \"q\"\nc: 'z'\n---\n");
        assert_eq!(run("---\na: x\n---\n", "a", Value::Str("true".into())), "---\na: \"true\"\n---\n");
        assert_eq!(run("---\na:\n---\n", "a", Value::Str("v".into())), "---\na: v\n---\n");
        assert_eq!(run("---\na: 1\n---\n", "a", Value::Number("2.5".into())), "---\na: 2.5\n---\n");
        assert_eq!(run("---\na: x\n---\n", "a", Value::Bool(false)), "---\na: false\n---\n");
    }

    #[test]
    fn lists_keep_their_style() {
        let list = |items: &[&str]| Value::List(items.iter().map(|item| (*item).to_owned()).collect());
        assert_eq!(run("---\ntags:\n  - a\n  - b\nx: 1\n---\n", "tags", list(&["a", "b", "c"])), "---\ntags:\n  - a\n  - b\n  - c\nx: 1\n---\n");
        assert_eq!(run("---\ntags: [a, b]\n---\n", "tags", list(&["a", "c"])), "---\ntags: [a, c]\n---\n");
        assert_eq!(run("---\ntags:\n- \"a\"\n---\n", "tags", list(&["a", "b c"])), "---\ntags:\n- \"a\"\n- \"b c\"\n---\n");
        assert_eq!(run("---\ntags:\n  - a\n  - b\n---\n", "tags", list(&["b"])), "---\ntags:\n  - b\n---\n");
        assert_eq!(run("---\ntags:\n  - a\n---\n", "tags", list(&[])), "---\ntags: []\n---\n");
        assert_eq!(run("---\ntags: a\n---\n", "tags", list(&["a", "b"])), "---\ntags: [a, b]\n---\n");
    }

    #[test]
    fn complex_values_are_refused() {
        let text = "---\nauthor:\n  name: Ann\nsummary: |\n  Two\n  lines\nx: 1 # one\ny: &a 3\n---\n";
        for key in ["author", "summary", "x", "y"] {
            assert_eq!(refused(text, key), Some("note.property-complex"), "{key}");
        }
        assert_eq!(refused("---\ntags: [a]\ntags: [b]\n---\n", "tags"), Some("note.frontmatter-duplicate-key"));
        assert_eq!(refused("---\ntitle: [unclosed\n---\n", "title"), Some("note.frontmatter-invalid"));
    }

    #[test]
    fn new_keys_and_new_blocks() {
        assert_eq!(run("---\na: 1\n---\nBody\n", "b", Value::Bool(true)), "---\na: 1\nb: true\n---\nBody\n");
        assert_eq!(run("Body\n", "b", Value::Number("2".into())), "---\nb: 2\n---\nBody\n");
        assert_eq!(run("---\r\na: x\r\n---\r\n", "a", Value::Str("y".into())), "---\r\na: y\r\n---\r\n");
        assert_eq!(run("---\r\na: x\r\n---\r\n", "b", Value::Str("y".into())), "---\r\na: x\r\nb: y\r\n---\r\n");
    }

    #[test]
    fn delete_and_rename_touch_only_their_key() {
        let text = "---\ntags: [a]\ntitle: \"T\"\ntags: [b]\n---\n";
        assert!(delete(&index(text), "tags", None).is_err());
        assert_eq!(apply(text, &delete(&index(text), "tags", Some(2)).unwrap()), "---\ntags: [a]\ntitle: \"T\"\n---\n");
        assert_eq!(delete(&index(text), "tags", Some(3)).unwrap_err().reason, Reason::OutOfRange);
        let text = "---\ntitle: T\nlist:\n  - a\n---\n";
        assert_eq!(apply(text, &delete(&index(text), "list", None).unwrap()), "---\ntitle: T\n---\n");
        assert_eq!(apply(text, &rename(&index(text), "title", "name").unwrap()), "---\nname: T\nlist:\n  - a\n---\n");
        assert_eq!(rename(&index(text), "title", "list").unwrap_err().code, Some("note.frontmatter-duplicate-key"));
        assert_eq!(apply(text, &rename(&index(text), "title", "a: b").unwrap()), "---\n\"a: b\": T\nlist:\n  - a\n---\n");
    }
}
