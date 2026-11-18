//! reStructuredText, as CodeMirror 5's rst mode colors it: its two layers ported token for
//! token, the base (rst-base: explicit markup `..` with directives keywords, targets and
//! references links, substitutions variable-2, footnotes and citations quotes, roles, comments,
//! section rules, Python and LaTeX blocks colored by their ports) and the overlay (inline
//! literals strings, numbers, `+n` positives and `-n` negatives, web addresses links), each
//! character taking the color of the class Obsidian's style sheet puts last.

use std::sync::OnceLock;

use super::Kind;
use super::regex::Regex;
use super::scan::{self, Out, Stream};
use super::{python, tex};

/// The base's patterns (the mode's, as it builds them).
const PATTERNS: &[(&str, &str)] = &[
    ("END", "^(?:\\s*|\\W|$)"),
    ("NAME", "^(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)"),
    ("TEXT", "^(?:[^\\`]+)"),
    ("EXPLICIT", "^\\.\\.\\s+"),
    (
        "TARGET",
        "^_(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`):(?:\\s*|\\W|$)|^__:(?:\\s*|\\W|$)",
    ),
    (
        "DIRECTIVE",
        "^(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)::(?:\\s*|\\W|$)",
    ),
    (
        "SUBSTITUTION_DEF",
        "^\\|(?:[^\\s\\|](?:[^\\|]*[^\\s\\|])?)\\|\\s+(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)::(?:\\s*|\\W|$)",
    ),
    (
        "FOOTNOTE",
        "^\\[(?:\\d+|#(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)?|\\*)](?:\\s*|\\W|$)",
    ),
    (
        "CITATION",
        "^\\[(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)\\](?:\\s*|\\W|$)",
    ),
    ("SUBSTITUTION", "^\\|(?:[^\\s\\|](?:[^\\|]*[^\\s\\|])?)\\|"),
    (
        "FOOTNOTE_REF",
        "^\\[(?:\\d+|#(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)?|\\*)]_",
    ),
    (
        "CITATION_REF",
        "^\\[(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)\\]_",
    ),
    (
        "REFERENCE",
        "^(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)__?",
    ),
    ("ANONYMOUS_REF", "^`(?:[^\\`]+)`_"),
    ("ROLE_BEFORE", "^:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?):`(?:[^\\`]+)`(?:\\s*|\\W|$)"),
    ("ROLE_AFTER", "^`(?:[^\\`]+)`:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?):(?:\\s*|\\W|$)"),
    ("ROLE", "^:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?):(?:\\s*|\\W|$)"),
    (
        "DIRECTIVE_NAME",
        "^(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)",
    ),
    ("DOUBLE_COLON", "^::(?:\\s*|\\W|$)"),
    ("BARS", "^\\|(?:[^\\s\\|](?:[^\\|]*[^\\s\\|])?)\\|"),
    ("SPACES", "^\\s+"),
    (
        "DIRECTIVE_NAME_AFTER",
        "^(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)",
    ),
    ("DOUBLE_COLON_AFTER", "^::(?:\\s*|\\W|$)"),
    ("UNDERSCORE", "^_"),
    (
        "TARGET_NAME",
        "^(?:(?:[^\\W\\d_](?:[\\w!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)|`(?:[^\\W\\d_](?:[\\w\\s!\"#$%&'()\\*\\+,\\-\\./:;<=>\\?]*[^\\W_])?)`)|_",
    ),
    ("COLON_END", "^:(?:\\s*|\\W|$)"),
    ("LITERAL_MARK", "^::\\s*$"),
    ("PROMPT", "^\\s+(?:>>>|In \\[\\d+\\]:)\\s"),
];

fn pattern(name: &str) -> Option<&'static Regex> {
    static COMPILED: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let compiled = COMPILED.get_or_init(|| PATTERNS.iter().map(|(name, source)| (*name, Regex::new(source, false))).collect());
    compiled.iter().find(|(known, _)| *known == name).map(|(_, regex)| regex)
}

/// Whether the pattern matches where the stream stands, taking it when `take`.
fn found(stream: &mut Stream, name: &str, take: bool) -> bool {
    let Some((end, _)) = pattern(name).and_then(|regex| regex.match_at(stream.bytes, stream.pos)) else { return false };
    if take {
        stream.pos = end;
    }
    true
}

/// A run of patterns (a role, a substitution, a directive) and where in it the next token is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Phase {
    pattern: Option<&'static str>,
    stage: u8,
}

/// What reads the next token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Reader {
    #[default]
    Text,
    Explicit,
    Comment,
    Literal,
    /// An indented block in another language.
    Python,
    Latex,
}

#[derive(Clone, Debug, Default)]
pub struct Rst {
    reader: Reader,
    phase: Phase,
    latex_role: bool,
    latex_inline: bool,
    python_next: bool,
    python: python::Python,
    latex: tex::Latex,
    /// This line's ranges the base styles without a color (quotes, rules), where the overlay's
    /// colors do not show either.
    masked: Vec<std::ops::Range<usize>>,
    /// Per byte of the line: whether a `_` comes later in the run of name bytes from it (a
    /// reference's first form cannot match where none does; the check saves trying its pattern,
    /// whose backtracking costs a long word's length at every byte).
    underscore_ahead: Vec<bool>,
}

impl Rst {
    fn to(&mut self, reader: Reader, phase: Option<(&'static str, u8)>) {
        self.reader = reader;
        self.phase = phase.map_or_else(Phase::default, |(pattern, stage)| Phase { pattern: Some(pattern), stage });
    }
}

fn style(name: &str) -> Option<Kind> {
    match name {
        "meta" | "comment" => Some(Kind::Comment),
        "keyword" => Some(Kind::Keyword),
        "string" | "string-2" | "positive" => Some(Kind::String),
        "variable-2" => Some(Kind::Variable),
        "link" => Some(Kind::Link),
        "number" => Some(Kind::Number),
        "negative" => Some(Kind::Operator),
        _ => None,
    }
}

pub fn line(text: &str, state: &mut Rst, out: &mut Out) {
    let mut base = Out::default();
    state.masked.clear();
    // The bytes `[\w!"#$%&'()*+,\-./:;<=>?]` (the reference name's middle) take.
    let name_byte = |byte: u8| scan::js_word(byte) || b"!\"#$%&'()*+,-./:;<=>?".contains(&byte);
    let bytes = text.as_bytes();
    state.underscore_ahead = vec![false; bytes.len() + 1];
    for index in (0..bytes.len()).rev() {
        let next = bytes.get(index + 1).copied();
        let ahead = next.is_some_and(|next| name_byte(next) && (next == b'_' || state.underscore_ahead.get(index + 1).copied().unwrap_or(false)));
        if let Some(slot) = state.underscore_ahead.get_mut(index) {
            *slot = ahead;
        }
    }
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        let mut kind = None;
        for _ in 0..10 {
            kind = token(text, &mut stream, state, &mut base);
            if stream.pos > stream.start {
                break;
            }
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
            kind = None;
        }
        base.mark(stream.start..stream.pos, kind);
    }
    let base = base.finish();
    let overlay = overlay_line(text);
    // Per byte, the color of the class Obsidian's style sheet puts last (or that a more specific
    // rule picks: a quote's and a link's): both layers' classes are on the text.
    let mut at = 0;
    while at < text.len() {
        let in_base = base.iter().find(|(range, _)| range.contains(&at));
        let in_overlay = overlay.iter().find(|(range, _, _)| range.contains(&at));
        let masked = state.masked.iter().find(|range| range.contains(&at));
        let mut end = text.len();
        for boundary in base
            .iter()
            .flat_map(|(range, _)| [range.start, range.end])
            .chain(overlay.iter().flat_map(|(range, _, _)| [range.start, range.end]))
            .chain(state.masked.iter().flat_map(|range| [range.start, range.end]))
        {
            if boundary > at && boundary < end {
                end = boundary;
            }
        }
        let base_kind = in_base.map(|(_, kind)| (*kind, priority(*kind)));
        let overlay_kind = in_overlay.map(|(_, kind, rank)| (*kind, *rank));
        let winner = match (masked, base_kind, overlay_kind) {
            (Some(_), _, _) => None,
            (None, Some(base), Some(overlay)) => Some(if overlay.1 > base.1 { overlay.0 } else { base.0 }),
            (None, Some(base), None) => Some(base.0),
            (None, None, Some(overlay)) => Some(overlay.0),
            (None, None, None) => None,
        };
        if let Some(kind) = winner {
            out.push(at..end, kind);
        }
        at = end;
    }
}

/// Where a kind's class stands in Obsidian's style sheet (later wins; a link's rule is more
/// specific than any).
fn priority(kind: Kind) -> u8 {
    match kind {
        Kind::Comment => 2,
        Kind::Punctuation => 4,
        Kind::Number => 5,
        Kind::String => 6,
        Kind::Operator => 7,
        Kind::Variable => 8,
        Kind::Property => 9,
        Kind::Keyword => 10,
        Kind::Link => 11,
    }
}

/// The overlay: inline markup and numbers, over every line: each span's kind and where its class
/// stands (`+n` and `-n` before every code class).
fn overlay_line(text: &str) -> Vec<(std::ops::Range<usize>, Kind, u8)> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let regexes = overlay_patterns();
    let mut stream = Stream { bytes, start: 0, pos: 0 };
    let ends_word = |stream: &Stream| {
        // `\W+|$` after the token.
        stream.eol() || !scan::js_word(stream.peek())
    };
    let matched = |stream: &mut Stream, index: usize, take: bool| {
        let Some((end, _)) = regexes.get(index).and_then(|regex| regex.match_at(stream.bytes, stream.pos)) else { return false };
        if take {
            stream.pos = end;
        }
        true
    };
    while !stream.eol() {
        stream.start = stream.pos;
        let mut kind = None;
        let mut done = false;
        for (index, name) in ["strong", "em", "string-2"].iter().enumerate() {
            // (A match not before a word's end is still taken.)
            if matched(&mut stream, index, true) && ends_word(&stream) {
                kind = style(name).map(|kind| (kind, priority(kind)));
                done = true;
                break;
            }
        }
        if !done {
            for (index, name) in [(3, "number"), (4, "positive"), (5, "negative"), (6, "link")] {
                if matched(&mut stream, index, true) {
                    kind = style(name).map(|kind| (kind, if matches!(name, "positive" | "negative") { 1 } else { priority(kind) }));
                    done = true;
                    break;
                }
            }
        }
        if !done {
            while stream.next().is_some() && !(0..7).any(|index| matched(&mut stream, index, false)) {}
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        if let Some((kind, rank)) = kind {
            spans.push((stream.start..stream.pos, kind, rank));
        }
    }
    spans
}

fn overlay_patterns() -> &'static Vec<Regex> {
    static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        [
            r"^\*\*[^\*\s](?:[^\*]*[^\*\s])?\*\*",
            r"^\*[^\*\s](?:[^\*]*[^\*\s])?\*",
            r"^``[^`\s](?:[^`]*[^`\s])``",
            r"^(?:[\d]+(?:[\.,]\d+)*)",
            r"^(?:\s\+[\d]+(?:[\.,]\d+)*)",
            r"^(?:\s\-[\d]+(?:[\.,]\d+)*)",
            r"^[Hh][Tt][Tt][Pp][Ss]?://(?:[\d\w.-]+)\.(?:\w{2,6})(?:/[\d\w\#\%\&\-\.\,\/\:\=\?\~]+)*",
        ]
        .iter()
        .map(|source| Regex::new(source, false))
        .collect()
    })
}

/// A section's rule: one punctuation character four or more times, then spaces only.
fn rule_line(text: &str) -> bool {
    let trimmed = text.trim_end();
    let bytes = trimmed.as_bytes();
    let Some(first) = bytes.first() else { return false };
    first.is_ascii_punctuation() && bytes.len() >= 4 && bytes.iter().all(|byte| byte == first)
}

fn token(text: &str, stream: &mut Stream, state: &mut Rst, out: &mut Out) -> Option<Kind> {
    match state.reader {
        Reader::Text => text_token(text, stream, state, out),
        Reader::Explicit => explicit(stream, state),
        Reader::Comment | Reader::Literal => {
            if stream.eol() || stream.eat_space() {
                stream.skip_to_end();
                return Some(Kind::Comment);
            }
            state.to(Reader::Text, None);
            None
        }
        Reader::Python | Reader::Latex => {
            if stream.sol() {
                if !stream.eat_space() {
                    state.to(Reader::Text, None);
                }
                return None;
            }
            let at = stream.pos;
            let mut inner = Out::default();
            let rest = text.get(at..).unwrap_or("");
            if state.reader == Reader::Python {
                python::line(rest, &mut state.python, &mut inner);
            } else {
                tex::latex(rest, &mut state.latex, &mut inner);
            }
            out.extend_shifted(inner, at);
            stream.skip_to_end();
            None
        }
    }
}

fn text_token(text: &str, stream: &mut Stream, state: &mut Rst, out: &mut Out) -> Option<Kind> {
    let phase = state.phase;
    if stream.sol() && found(stream, "PROMPT", false) {
        state.python = python::Python::default();
        state.to(Reader::Python, None);
        return None;
    }
    if stream.sol() && found(stream, "EXPLICIT", true) {
        state.to(Reader::Explicit, None);
        return style("meta");
    }
    if stream.sol() && rule_line(text) {
        stream.skip_to_end();
        state.to(Reader::Text, None);
        return None;
    }
    if phase.pattern == Some("ROLE_BEFORE") || found(stream, "ROLE_BEFORE", false) {
        let stage = phase.stage;
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Text, Some(("ROLE_BEFORE", stage)));
        match stage {
            0 => {
                next(state, 1);
                stream.take(":");
                return style("meta");
            }
            1 => {
                next(state, 2);
                found(stream, "NAME", true);
                if stream.current().starts_with("math") || stream.current().starts_with("latex") {
                    state.latex_role = true;
                }
                return style("keyword");
            }
            2 => {
                next(state, 3);
                stream.take(":`");
                return style("meta");
            }
            3 => {
                if std::mem::take(&mut state.latex_role) {
                    state.latex_inline = true;
                    state.latex = tex::Latex::default();
                }
                if state.latex_inline {
                    if stream.peek() == b'`' {
                        next(state, 4);
                        state.latex_inline = false;
                        return None;
                    }
                    let at = stream.pos;
                    let end = stream.rest().iter().position(|byte| *byte == b'`').map_or(text.len(), |found| at + found);
                    let mut inner = Out::default();
                    tex::latex(text.get(at..end).unwrap_or(""), &mut state.latex, &mut inner);
                    out.extend_shifted(inner, at);
                    stream.pos = end;
                    return None;
                }
                next(state, 4);
                found(stream, "TEXT", true);
                return style("string");
            }
            4 => {
                next(state, 5);
                stream.take("`");
                return style("meta");
            }
            5 => {
                next(state, 6);
                found(stream, "END", true);
                return None;
            }
            _ => {
                state.to(Reader::Text, None);
                return None;
            }
        }
    }
    if phase.pattern == Some("ROLE_AFTER") || found(stream, "ROLE_AFTER", false) {
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Text, Some(("ROLE_AFTER", stage)));
        return match phase.stage {
            0 => {
                next(state, 1);
                stream.take("`");
                style("meta")
            }
            1 => {
                next(state, 2);
                found(stream, "TEXT", true);
                style("string")
            }
            2 => {
                next(state, 3);
                stream.take("`:");
                style("meta")
            }
            3 => {
                next(state, 4);
                found(stream, "NAME", true);
                style("keyword")
            }
            4 => {
                next(state, 5);
                stream.take(":");
                style("meta")
            }
            5 => {
                next(state, 6);
                found(stream, "END", true);
                None
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if phase.pattern == Some("ROLE") || found(stream, "ROLE", false) {
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Text, Some(("ROLE", stage)));
        return match phase.stage {
            0 => {
                next(state, 1);
                stream.take(":");
                style("meta")
            }
            1 => {
                next(state, 2);
                found(stream, "NAME", true);
                style("keyword")
            }
            2 => {
                next(state, 3);
                stream.take(":");
                style("meta")
            }
            3 => {
                next(state, 4);
                found(stream, "END", true);
                None
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if phase.pattern == Some("SUBSTITUTION") || found(stream, "SUBSTITUTION", false) {
        return match phase.stage {
            0 => {
                state.to(Reader::Text, Some(("SUBSTITUTION", 1)));
                found(stream, "BARS", true);
                style("variable-2")
            }
            1 => {
                state.to(Reader::Text, Some(("SUBSTITUTION", 2)));
                stream.take("_");
                stream.take("_");
                style("link")
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if found(stream, "FOOTNOTE_REF", true) || found(stream, "CITATION_REF", true) {
        state.masked.push(stream.start..stream.pos);
        state.to(Reader::Text, None);
        return None;
    }
    let may_reference = stream.peek() == b'`' || state.underscore_ahead.get(stream.pos).copied().unwrap_or(false);
    if may_reference && found(stream, "REFERENCE", true) {
        state.to(Reader::Text, None);
        let next = stream.peek();
        return (stream.eol() || !scan::js_word(next) || next >= 0x80).then(|| style("link")).flatten();
    }
    if phase.pattern == Some("ANONYMOUS_REF") || found(stream, "ANONYMOUS_REF", false) {
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Text, Some(("ANONYMOUS_REF", stage)));
        return match phase.stage {
            0 => {
                if stream.eol() || !scan::js_word(stream.peek()) {
                    next(state, 1);
                } else {
                    found(stream, "ANONYMOUS_REF", true);
                }
                None
            }
            1 => {
                next(state, 2);
                stream.take("`");
                style("link")
            }
            2 => {
                next(state, 3);
                found(stream, "TEXT", true);
                None
            }
            3 => {
                next(state, 4);
                stream.take("`_");
                style("link")
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if found(stream, "LITERAL_MARK", true) {
        state.to(Reader::Literal, None);
        return None;
    }
    if stream.next().is_some() {
        state.to(Reader::Text, None);
    }
    None
}

/// After `..`: a substitution's definition, a directive, a target, a footnote or citation, or a
/// comment.
fn explicit(stream: &mut Stream, state: &mut Rst) -> Option<Kind> {
    let phase = state.phase;
    if phase.pattern == Some("SUBSTITUTION_DEF") || found(stream, "SUBSTITUTION_DEF", false) {
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Explicit, Some(("SUBSTITUTION_DEF", stage)));
        return match phase.stage {
            0 => {
                next(state, 1);
                found(stream, "BARS", true);
                style("variable-2")
            }
            1 => {
                next(state, 2);
                found(stream, "SPACES", true);
                None
            }
            2 => {
                next(state, 3);
                found(stream, "DIRECTIVE_NAME_AFTER", true);
                style("keyword")
            }
            3 => {
                next(state, 4);
                found(stream, "DOUBLE_COLON_AFTER", true);
                style("meta")
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if phase.pattern == Some("DIRECTIVE") || found(stream, "DIRECTIVE", false) {
        let next = |state: &mut Rst, stage: u8| state.to(Reader::Explicit, Some(("DIRECTIVE", stage)));
        return match phase.stage {
            0 => {
                next(state, 1);
                found(stream, "DIRECTIVE_NAME", true);
                let name = stream.current();
                if name.starts_with("math") || name.starts_with("latex") {
                    state.latex_role = true;
                } else if name.starts_with("python") {
                    state.python_next = true;
                }
                style("keyword")
            }
            1 => {
                next(state, 2);
                found(stream, "DOUBLE_COLON", true);
                let rest = stream.rest();
                let latex = rest.starts_with(b"latex") && rest.get(5..).is_some_and(|after| after.iter().all(u8::is_ascii_whitespace));
                if latex {
                    stream.skip_to_end();
                }
                if latex || std::mem::take(&mut state.latex_role) {
                    state.latex = tex::Latex::default();
                    state.to(Reader::Latex, None);
                }
                style("meta")
            }
            2 => {
                next(state, 3);
                let rest = stream.rest();
                let python = rest.starts_with(b"python") && rest.get(6..).is_some_and(|after| after.iter().all(u8::is_ascii_whitespace));
                if python {
                    stream.skip_to_end();
                }
                if python || std::mem::take(&mut state.python_next) {
                    state.python = python::Python::default();
                    state.to(Reader::Python, None);
                }
                None
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if phase.pattern == Some("TARGET") || found(stream, "TARGET", false) {
        return match phase.stage {
            0 => {
                state.to(Reader::Explicit, Some(("TARGET", 1)));
                found(stream, "UNDERSCORE", true);
                found(stream, "TARGET_NAME", true);
                style("link")
            }
            1 => {
                state.to(Reader::Explicit, Some(("TARGET", 2)));
                found(stream, "COLON_END", true);
                style("meta")
            }
            _ => {
                state.to(Reader::Text, None);
                None
            }
        };
    }
    if found(stream, "FOOTNOTE", true) || found(stream, "CITATION", true) {
        state.masked.push(stream.start..stream.pos);
        state.to(Reader::Text, None);
        return None;
    }
    stream.eat_space();
    if stream.eol() {
        state.to(Reader::Text, None);
        return None;
    }
    stream.skip_to_end();
    state.to(Reader::Comment, None);
    style("comment")
}
