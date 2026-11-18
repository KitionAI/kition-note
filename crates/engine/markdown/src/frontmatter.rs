//! The note's properties: the leading `---` block read line by line, so every top-level key knows
//! its exact source lines and the style it is written in. `saphyr` reads the block only to say whether it is valid YAML and what type each
//! key's value has; nothing is ever written back from it.

use std::ops::Range;

use crate::sub;

/// How a key's value is written, which decides how `property.set` may rewrite it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Style {
    /// `key:` with nothing after it and nothing below.
    Empty,
    Plain,
    Single,
    Double,
    /// `key: [a, b]`.
    FlowList,
    /// `key:` followed by `- item` lines; `prefix` is the first item's text before its value.
    BlockList {
        prefix: String,
    },
    /// Read, never rewritten: a nested map, a block scalar, a flow map, an anchor, alias or tag, a
    /// comment, a multi-line plain value, or a list item that is not a plain scalar.
    Complex(&'static str),
}

impl Style {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Plain => "plain",
            Self::Single => "single-quoted",
            Self::Double => "double-quoted",
            Self::FlowList => "flow-list",
            Self::BlockList { .. } => "block-list",
            Self::Complex(_) => "complex",
        }
    }
}

/// A key's value as YAML reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Typed {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
    /// A string of the form `YYYY-MM-DD`.
    Date(String),
    /// A string of the form `YYYY-MM-DDThh:mm[:ss]...`.
    DateTime(String),
    List(Vec<String>),
    /// A nested map or a list of other things: shown, never edited.
    Other,
}

impl Typed {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Boolean(_) => "boolean",
            Self::Null => "null",
            Self::Date(_) => "date",
            Self::DateTime(_) => "datetime",
            Self::List(_) => "list",
            Self::Other => "other",
        }
    }
}

/// One top-level key.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    /// The key as written, quotes included.
    pub key_range: Range<usize>,
    /// The header line and the lines below it, each with its line ending.
    pub lines: Range<usize>,
    /// The value on the header line (after `: `), without its line ending.
    pub value: Range<usize>,
    pub style: Style,
    /// Block lists: each item's line, without its line ending.
    pub items: Vec<Range<usize>>,
    /// The value as YAML reads it, when the block parses.
    pub typed: Option<Typed>,
}

/// Why the block is not valid YAML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YamlError {
    pub message: String,
    /// The line in the note, from 1.
    pub line: usize,
}

/// The leading property block.
#[derive(Clone, Debug, PartialEq)]
pub struct Frontmatter {
    /// The whole block: the opening fence to the end of the closing fence's line.
    pub range: Range<usize>,
    /// The lines between the fences.
    pub body: Range<usize>,
    pub entries: Vec<Entry>,
    /// Top-level comment lines (`# ...` at column 0).
    pub comments: Vec<Range<usize>>,
    /// Keys written more than once.
    pub duplicates: Vec<String>,
    pub invalid: Option<YamlError>,
}

impl Frontmatter {
    /// The entries named `key`, in order.
    pub fn named<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        self.entries.iter().filter(move |entry| entry.key == key)
    }
}

fn trim_eol(s: &str) -> &str {
    s.trim_end_matches('\n').trim_end_matches('\r')
}

/// The lines of `range`, each with its line ending.
pub fn line_ranges(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut at = range.start;
    while at < range.end {
        let end = sub(text, at..range.end).find('\n').map_or(range.end, |offset| at + offset + 1);
        out.push(at..end);
        at = end;
    }
    out
}

/// The block's body and the closing fence's line: `---` on the first line, closed by `---` or
/// `...`.
pub fn locate(text: &str) -> Option<(Range<usize>, Range<usize>)> {
    let first = text.find('\n')?;
    if trim_eol(sub(text, 0..first + 1)) != "---" {
        return None;
    }
    let mut at = first + 1;
    while at < text.len() {
        let end = sub(text, at..text.len()).find('\n').map_or(text.len(), |offset| at + offset + 1);
        let line = trim_eol(sub(text, at..end));
        if line == "---" || line == "..." {
            return Some((first + 1..at, at..end));
        }
        at = end;
    }
    None
}

/// Whether a plain value carries a ` #` comment outside quotes.
pub fn has_comment(value: &str) -> bool {
    let (mut single, mut double) = (false, false);
    let mut prev = ' ';
    for c in value.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && prev.is_whitespace() => return true,
            _ => {}
        }
        prev = c;
    }
    false
}

/// The colon ending a key on a header line: followed by white space or the line's end.
fn key_colon(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    for (index, &byte) in bytes.iter().enumerate() {
        match (quote, byte) {
            (None, b'"' | b'\'') if index == 0 => quote = Some(byte),
            (Some(open), _) if byte == open => quote = None,
            (None, b':') if bytes.get(index + 1).is_none_or(|next| matches!(next, b' ' | b'\t' | b'\r')) => return Some(index),
            _ => {}
        }
    }
    None
}

/// Reads the property block, if the note has one.
pub fn scan(text: &str, line_of: impl Fn(usize) -> usize) -> Option<Frontmatter> {
    let (body, close) = locate(text)?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut comments = Vec::new();
    for line in line_ranges(text, body.clone()) {
        let s = trim_eol(sub(text, line.clone()));
        match s.chars().next() {
            None => {
                if let Some(entry) = entries.last_mut() {
                    entry.lines.end = line.end;
                }
            }
            Some('#') => comments.push(line.clone()),
            Some(c) if c.is_whitespace() || s.starts_with("- ") || s == "-" => {
                if let Some(entry) = entries.last_mut() {
                    entry.lines.end = line.end;
                }
            }
            Some(_) => {
                let Some(colon) = key_colon(s) else {
                    entries.push(Entry {
                        key: s.to_owned(),
                        key_range: line.start..line.start + s.len(),
                        lines: line.clone(),
                        value: line.start + s.len()..line.start + s.len(),
                        style: Style::Complex("not a key"),
                        items: Vec::new(),
                        typed: None,
                    });
                    continue;
                };
                let raw_key = sub(s, 0..colon);
                let key = raw_key.trim().trim_matches('"').trim_matches('\'').to_owned();
                let after = sub(s, colon + 1..s.len());
                let lead = after.len() - after.trim_start().len();
                let value_start = line.start + colon + 1 + lead;
                let value_end = (line.start + s.len()).max(value_start);
                entries.push(Entry {
                    key,
                    key_range: line.start..line.start + raw_key.trim_end().len(),
                    lines: line.clone(),
                    value: value_start..value_end,
                    style: Style::Empty,
                    items: Vec::new(),
                    typed: None,
                });
            }
        }
    }
    for entry in &mut entries {
        // Trailing blank lines are not part of an entry.
        loop {
            let lines = line_ranges(text, entry.lines.clone());
            match lines.last() {
                Some(last) if lines.len() > 1 && trim_eol(sub(text, last.clone())).trim().is_empty() => entry.lines.end = last.start,
                _ => break,
            }
        }
        classify(text, entry);
    }
    let mut duplicates = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if entries.get(..index).is_some_and(|before| before.iter().any(|other| other.key == entry.key)) && !duplicates.contains(&entry.key) {
            duplicates.push(entry.key.clone());
        }
    }
    let mut invalid = None;
    // saphyr refuses duplicated keys as invalid YAML: those are reported as duplicates instead.
    if duplicates.is_empty() {
        match typed(sub(text, body.clone())) {
            Ok(values) => {
                for entry in &mut entries {
                    entry.typed = values.iter().find(|(key, _)| *key == entry.key).map(|(_, value)| value.clone());
                }
            }
            Err((message, line)) => invalid = Some(YamlError { message, line: line_of(body.start) + 1 + line }),
        }
    }
    Some(Frontmatter { range: 0..close.end, body, entries, comments, duplicates, invalid })
}

fn classify(text: &str, entry: &mut Entry) {
    if matches!(entry.style, Style::Complex(_)) {
        return;
    }
    let value = sub(text, entry.value.clone()).trim();
    let header_end = sub(text, entry.lines.clone()).find('\n').map_or(entry.lines.end, |offset| entry.lines.start + offset + 1);
    let below: Vec<Range<usize>> = line_ranges(text, header_end..entry.lines.end)
        .into_iter()
        .filter(|line| !trim_eol(sub(text, line.clone())).trim().is_empty())
        .map(|line| line.start..line.start + trim_eol(sub(text, line.clone())).len())
        .collect();
    entry.style = if value.is_empty() {
        if below.is_empty() {
            Style::Empty
        } else if below.iter().all(|line| {
            let s = sub(text, line.clone()).trim_start();
            s.starts_with("- ") || s == "-"
        }) {
            let first = below.first().map_or("", |line| sub(text, line.clone()));
            let indent = first.len() - first.trim_start().len();
            let prefix = sub(first, 0..indent + 2).to_owned();
            let plain = below.iter().all(|line| {
                let s = sub(text, line.clone());
                let item = s.trim_start().trim_start_matches('-').trim();
                s.starts_with(&prefix) && !has_comment(item) && !item.starts_with(['[', '{', '&', '*', '!', '|', '>'])
            });
            entry.items = below;
            if plain { Style::BlockList { prefix } } else { Style::Complex("block list item is not a plain scalar") }
        } else {
            Style::Complex("nested map")
        }
    } else if !below.is_empty() {
        Style::Complex(if value.starts_with(['|', '>']) { "block scalar" } else { "multi-line value" })
    } else if value.starts_with(['|', '>']) {
        Style::Complex("block scalar")
    } else if value.starts_with('{') {
        Style::Complex("flow map")
    } else if value.starts_with(['&', '*', '!']) {
        Style::Complex("anchor, alias or tag")
    } else if has_comment(value) {
        Style::Complex("comment")
    } else if value.starts_with('[') {
        let inner = value.strip_prefix('[').and_then(|rest| rest.strip_suffix(']'));
        match inner {
            Some(inner) if !inner.contains(['[', '{']) => Style::FlowList,
            _ => Style::Complex("nested flow list"),
        }
    } else if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Style::Double
    } else if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        Style::Single
    } else {
        Style::Plain
    };
}

fn is_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 10 && bytes.iter().enumerate().all(|(index, byte)| if index == 4 || index == 7 { *byte == b'-' } else { byte.is_ascii_digit() })
}

fn string_value(s: &str) -> Typed {
    if is_date(s) {
        Typed::Date(s.to_owned())
    } else if s.len() > 10 && s.get(..10).is_some_and(is_date) && s.as_bytes().get(10).is_some_and(|byte| *byte == b'T' || *byte == b' ') {
        Typed::DateTime(s.to_owned())
    } else {
        Typed::String(s.to_owned())
    }
}

/// Each top-level key's value, or the parse error with its line inside the block (from 0).
fn typed(body: &str) -> Result<Vec<(String, Typed)>, (String, usize)> {
    use saphyr::{LoadableYamlNode, Scalar, Yaml};
    let docs = Yaml::load_from_str(body).map_err(|error| (error.info().to_owned(), error.marker().line().saturating_sub(1)))?;
    let mut out = Vec::new();
    match docs.first() {
        None | Some(Yaml::Value(Scalar::Null)) => {}
        Some(Yaml::Mapping(map)) => {
            for (key, value) in map {
                let key = match key {
                    Yaml::Value(Scalar::String(s)) => s.to_string(),
                    Yaml::Value(Scalar::Integer(n)) => n.to_string(),
                    Yaml::Value(Scalar::Boolean(b)) => b.to_string(),
                    _ => continue,
                };
                let value = match value {
                    Yaml::Value(Scalar::String(s)) => string_value(s),
                    Yaml::Value(Scalar::Integer(n)) => Typed::Number(n.to_string()),
                    Yaml::Value(Scalar::FloatingPoint(n)) => Typed::Number(n.to_string()),
                    Yaml::Value(Scalar::Boolean(b)) => Typed::Boolean(*b),
                    Yaml::Value(Scalar::Null) => Typed::Null,
                    Yaml::Sequence(items) => {
                        let strings: Option<Vec<String>> = items
                            .iter()
                            .map(|item| match item {
                                Yaml::Value(Scalar::String(s)) => Some(s.to_string()),
                                Yaml::Value(Scalar::Integer(n)) => Some(n.to_string()),
                                Yaml::Value(Scalar::FloatingPoint(n)) => Some(n.to_string()),
                                Yaml::Value(Scalar::Boolean(b)) => Some(b.to_string()),
                                _ => None,
                            })
                            .collect();
                        strings.map_or(Typed::Other, Typed::List)
                    }
                    _ => Typed::Other,
                };
                out.push((key, value));
            }
        }
        Some(_) => return Err(("the property block is not a map of keys".to_owned(), 0)),
    }
    Ok(out)
}
