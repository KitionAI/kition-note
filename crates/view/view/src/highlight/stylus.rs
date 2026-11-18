//! Stylus, as CodeMirror 5's stylus mode colors it, ported token for token with the state
//! machine it reads blocks, parentheses, `@media` blocks, keyframes, interpolations, and
//! variables with: tags, properties, values atoms (plain), `$variables` and other names
//! variable-2, `#hex` colors plain, numbers, strings, `.class` qualifiers strings, operators,
//! `@rules` plain, `//` and `/* */` comments. The mode's own styles are kept as its names until
//! the end, where each takes its color.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug)]
pub struct Stylus {
    /// What reads the next token: the mode's base tokenizer, a comment, a string, a `url(`.
    tokenize: Tokenize,
    /// The state the next token's type goes to.
    state: &'static str,
    /// The contexts, innermost last.
    contexts: Vec<&'static str>,
}

impl Default for Stylus {
    fn default() -> Self {
        Stylus { tokenize: Tokenize::Base, state: "block", contexts: vec!["block"] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tokenize {
    Base,
    Comment,
    String(u8),
    /// After `url`: its `(`.
    Url,
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

/// `word in object`, which also finds what every JavaScript object has.
fn has(list: &str, word: &str) -> bool {
    listed(list, word) || word == "constructor" || word == "__proto__"
}

fn is_tag(word: &str) -> bool {
    has(TAGS, &word.to_ascii_lowercase())
}

fn is_property(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    has(PROPERTIES, &lower) || has(FONT_PROPERTIES, &lower)
}

fn is_block_keyword(word: &str) -> bool {
    has(BLOCK_KEYWORDS, &word.to_ascii_lowercase())
}

/// The mode's `/^(a|b|i|s|col|em)$/i`.
fn short_tag(word: &str) -> bool {
    ["a", "b", "i", "s", "col", "em"].iter().any(|tag| tag.eq_ignore_ascii_case(word))
}

/// What a word is: a tag, a block keyword, a property, an atom, a keyword, a string (a capital
/// first), else a variable-2.
fn word_style(word: &str) -> &'static str {
    let lower = word.to_ascii_lowercase();
    if is_tag(word) {
        "tag"
    } else if is_block_keyword(word) {
        "block-keyword"
    } else if is_property(word) {
        "property"
    } else if has(VALUES, &lower) || has(COMMON_ATOMS, &lower) {
        "atom"
    } else if lower == "return" || has(COLORS, &lower) {
        "keyword"
    } else if word.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
        "string"
    } else {
        "variable-2"
    }
}

fn kind(style: &str) -> Option<Kind> {
    match style {
        "comment" | "meta" => Some(Kind::Comment),
        "string" | "qualifier" | "string-2" => Some(Kind::String),
        "number" => Some(Kind::Number),
        "keyword" => Some(Kind::Keyword),
        "operator" | "tag" => Some(Kind::Operator),
        "builtin" | "property" | "attribute" => Some(Kind::Property),
        "variable-2" | "variable-3" => Some(Kind::Variable),
        _ => None,
    }
}

fn space(byte: u8) -> bool {
    byte.is_ascii_whitespace()
}

/// The text from `at` is only spaces (the mode's `X`).
fn rest_blank(stream: &Stream) -> bool {
    stream.rest().iter().all(|byte| space(*byte))
}

fn line_text<'a>(stream: &Stream<'a>) -> &'a str {
    std::str::from_utf8(stream.bytes).unwrap_or("")
}

/// The line starts (after spaces) with the token (the mode's `R`).
fn starts_line(stream: &Stream) -> bool {
    stream.pos == 0 || line_text(stream).trim_start_matches([' ', '\t', '\r']).starts_with(stream.current())
}

/// The line's first word (`\s*[-_]*[a-z0-9]+[\w-]*`), the mode's `Y`.
fn first_word(text: &str) -> &str {
    let trimmed = text.trim_start();
    let bytes = trimmed.as_bytes();
    let dashes = bytes.iter().take_while(|byte| **byte == b'-' || **byte == b'_').count();
    if !bytes.get(dashes).is_some_and(u8::is_ascii_alphanumeric) {
        return "";
    }
    let end = dashes + bytes.get(dashes..).map_or(0, |rest| rest.iter().take_while(|byte| scan::js_word(**byte) || **byte == b'-').count());
    trimmed.get(..end).unwrap_or("")
}

fn indented(stream: &Stream) -> bool {
    stream.bytes.first().is_some_and(|byte| *byte == b' ' || *byte == b'\t')
}

/// `[\w-\.\[\]\'\"]`
fn name_part(byte: u8) -> bool {
    scan::js_word(byte) || b"-.[]'\"".contains(&byte)
}

/// The line is `\s?\$[\w-\.\[\]\'\"]+` and nothing else.
fn variable_line(text: &str) -> bool {
    let text = text.strip_prefix(|c: char| c.is_whitespace()).unwrap_or(text);
    text.strip_prefix('$').is_some_and(|rest| !rest.is_empty() && rest.bytes().all(name_part))
}

pub fn line(text: &str, state: &mut Stylus, out: &mut Out) {
    scan::run(text, out, |stream| {
        if state.tokenize == Tokenize::Base && stream.eat_space() {
            return None;
        }
        let (style, kind_of) = match state.tokenize {
            Tokenize::Base => base(stream, state),
            Tokenize::Comment => comment(stream, state),
            Tokenize::String(quote) => string(stream, state, quote),
            Tokenize::Url => url(stream, state),
        };
        let mut style = style;
        let current = state.state;
        state.state = dispatch(current, kind_of.as_deref(), stream, state, &mut style);
        style.and_then(kind)
    });
}

fn comment(stream: &mut Stream, state: &mut Stylus) -> (Option<&'static str>, Option<String>) {
    let mut star = false;
    while let Some(here) = stream.next() {
        if star && here == b'/' {
            state.tokenize = Tokenize::Base;
            break;
        }
        star = here == b'*';
    }
    (Some("comment"), Some("comment".into()))
}

fn string(stream: &mut Stream, state: &mut Stylus, quote: u8) -> (Option<&'static str>, Option<String>) {
    let mut escaped = false;
    let mut last = None;
    while let Some(here) = stream.next() {
        last = Some(here);
        if here == quote && !escaped {
            if quote == b')' {
                stream.back_up(1);
            }
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    if last == Some(quote) || !(escaped || quote == b')') {
        state.tokenize = Tokenize::Base;
    }
    (Some("string"), Some("string".into()))
}

fn url(stream: &mut Stream, state: &mut Stylus) -> (Option<&'static str>, Option<String>) {
    stream.next();
    let quoted = stream.rest().iter().find(|byte| !space(**byte)).is_some_and(|byte| b"\"')".contains(byte));
    state.tokenize = if quoted { Tokenize::Base } else { Tokenize::String(b')') };
    (None, Some("(".into()))
}

fn base(stream: &mut Stream, state: &mut Stylus) -> (Option<&'static str>, Option<String>) {
    let here = stream.peek();
    let word = |stream: &Stream, from: usize| {
        (from..stream.bytes.len()).find(|end| !(scan::js_word(stream.byte(*end)) || stream.byte(*end) == b'-')).unwrap_or(stream.bytes.len())
    };
    if stream.take("//") {
        stream.skip_to_end();
        return (Some("comment"), Some("comment".into()));
    }
    if stream.take("/*") {
        state.tokenize = Tokenize::Comment;
        return comment(stream, state);
    }
    if here == b'"' || here == b'\'' {
        stream.next();
        state.tokenize = Tokenize::String(here);
        return string(stream, state, here);
    }
    if here == b'@' {
        stream.next();
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'\\' || byte == b'-');
        return (Some("def"), Some(stream.current().to_string()));
    }
    if here == b'#' {
        stream.next();
        let hex = (stream.pos..stream.bytes.len()).find(|end| !stream.byte(*end).is_ascii_hexdigit()).unwrap_or(stream.bytes.len()) - stream.pos;
        let fits = |length: usize| {
            let end = stream.pos + length;
            length <= hex && !scan::js_word(stream.byte(end)) && stream.byte(end) != b'-'
        };
        if let Some(length) = [8, 6, 4, 3].into_iter().find(|length| fits(*length)) {
            stream.pos += length;
            return (Some("atom"), Some("atom".into()));
        }
        if stream.peek().is_ascii_alphabetic() {
            stream.pos = word(stream, stream.pos + 1);
            return (Some("builtin"), Some("hash".into()));
        }
    }
    let after_hash = stream.pos;
    // `^\-(moz|ms|o|webkit)-`
    if stream.peek() == b'-' {
        let rest = stream.rest();
        if let Some(prefix) = ["moz-", "ms-", "o-", "webkit-"]
            .iter()
            .find(|prefix| rest.get(1..1 + prefix.len()).is_some_and(|text| text.eq_ignore_ascii_case(prefix.as_bytes())))
        {
            stream.pos += 1 + prefix.len();
            return (Some("meta"), Some("vendor-prefixes".into()));
        }
    }
    // `^-?[0-9]?\.?[0-9]`
    let sign = if stream.peek() == b'-' { after_hash + 1 } else { after_hash };
    let number = [(true, true), (true, false), (false, true), (false, false)].into_iter().find_map(|(digit, dot)| {
        let mut at = sign;
        if digit {
            if !stream.byte(at).is_ascii_digit() {
                return None;
            }
            at += 1;
        }
        if dot {
            if stream.byte(at) != b'.' {
                return None;
            }
            at += 1;
        }
        stream.byte(at).is_ascii_digit().then_some(at + 1)
    });
    if let Some(end) = number {
        stream.pos = end;
        stream.eat_while(|byte| byte.is_ascii_alphabetic() || byte == b'%');
        return (Some("number"), Some("unit".into()));
    }
    if stream.peek() == b'!' {
        stream.next();
        let important = ["important", "optional"]
            .iter()
            .find(|word| stream.rest().get(..word.len()).is_some_and(|text| text.eq_ignore_ascii_case(word.as_bytes())));
        if let Some(word) = important {
            stream.pos += word.len();
            return (Some("keyword"), Some("important".into()));
        }
        return (Some("operator"), Some("important".into()));
    }
    if stream.peek() == b'.' && stream.byte(stream.pos + 1).is_ascii_alphabetic() {
        stream.pos = word(stream, stream.pos + 2);
        return (Some("qualifier"), Some("qualifier".into()));
    }
    if stream.take_word_of("domain|regexp|url-prefix|url", false) {
        if stream.peek() == b'(' {
            state.tokenize = Tokenize::Url;
        }
        return (Some("property"), Some("word".into()));
    }
    // `^[a-z][\w-]*\(` a mixin's name, `^(\+|-)[a-z][\w-]*\(` a block mixin's.
    for (sign, kind_of) in [(0, "mixin"), (1, "block-mixin")] {
        let start = stream.pos + sign;
        if (sign == 0 || matches!(stream.peek(), b'+' | b'-')) && stream.byte(start).is_ascii_alphabetic() {
            let end = word(stream, start + 1);
            if stream.byte(end) == b'(' {
                stream.pos = end;
                return (Some("keyword"), Some(kind_of.into()));
            }
        }
    }
    if line_text(stream).trim_start().starts_with('&') {
        let dashes = (stream.pos..stream.bytes.len()).find(|end| !matches!(stream.byte(*end), b'-' | b'_')).unwrap_or(stream.bytes.len());
        if dashes > stream.pos && stream.byte(dashes).is_ascii_lowercase() {
            stream.pos = word(stream, dashes + 1);
            return (Some("qualifier"), Some("qualifier".into()));
        }
    }
    if matches!(stream.peek(), b'/' | b'&') && {
        let next = stream.byte(stream.pos + 1);
        b"-_:.#".contains(&next) || next.is_ascii_lowercase()
    } {
        stream.next();
        return (Some("variable-3"), Some("reference".into()));
    }
    if stream.peek() == b'&' && stream.bytes.get(stream.pos + 1..).is_some_and(|rest| rest.iter().all(|byte| space(*byte))) {
        stream.skip_to_end();
        return (Some("variable-3"), Some("reference".into()));
    }
    if stream.take_word_of("in|and|or|not|is not|is a|is|isnt|defined|if unless", false) {
        return (Some("operator"), Some("operator".into()));
    }
    // `^\$?[-_]*[a-z0-9]+[\w-]*`
    let dollar = if stream.peek() == b'$' { stream.pos + 1 } else { stream.pos };
    let dashes = (dollar..stream.bytes.len()).find(|end| !matches!(stream.byte(*end), b'-' | b'_')).unwrap_or(stream.bytes.len());
    if stream.byte(dashes).is_ascii_alphanumeric() {
        stream.pos = word(stream, dashes + 1);
        let member = matches!(stream.peek(), b'.' | b'[') && {
            let next = stream.byte(stream.pos + 1);
            scan::js_word(next) || b"-'\"]".contains(&next)
        };
        if member && !is_tag(stream.current()) {
            stream.take(".");
            return (Some("variable-2"), Some("variable-name".into()));
        }
        return (Some("variable-2"), Some("word".into()));
    }
    if let Some(length) = operator(stream.rest()) {
        stream.pos += length;
        return (Some("operator"), Some(stream.current().to_string()));
    }
    let here = stream.next().unwrap_or(0);
    if b":;,{}[]()".contains(&here) {
        return (None, Some((here as char).to_string()));
    }
    (None, None)
}

/// `([.]{2,3}|&&|\|\||\*\*|[?!=:]?=|[-+*\/%<>]=?|\?:|\~)` at the text's start: its length.
fn operator(rest: &[u8]) -> Option<usize> {
    let byte = |index: usize| rest.get(index).copied().unwrap_or(0);
    if byte(0) == b'.' && byte(1) == b'.' {
        return Some(if byte(2) == b'.' { 3 } else { 2 });
    }
    for pair in [b"&&", b"||", b"**"] {
        if rest.starts_with(pair) {
            return Some(2);
        }
    }
    if b"?!=:".contains(&byte(0)) && byte(1) == b'=' {
        return Some(2);
    }
    if byte(0) == b'=' {
        return Some(1);
    }
    if b"-+*/%<>".contains(&byte(0)) {
        return Some(if byte(1) == b'=' { 2 } else { 1 });
    }
    if rest.starts_with(b"?:") {
        return Some(2);
    }
    (byte(0) == b'~').then_some(1)
}

fn top(state: &Stylus) -> &'static str {
    state.contexts.last().copied().unwrap_or("block")
}

fn push(state: &mut Stylus, context: &'static str) -> &'static str {
    state.contexts.push(context);
    context
}

fn pop(state: &mut Stylus) -> &'static str {
    if state.contexts.len() > 1 {
        state.contexts.pop();
    }
    top(state)
}

/// Leaves the context and reads the token in the one around it (the mode's `E`).
fn reapply(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    pop(state);
    let context = top(state);
    dispatch(context, kind_of, stream, state, style)
}

fn at_media(kind_of: &str) -> bool {
    ["@font-face", "@media", "@supports", "@document", "@-moz-document"].iter().any(|rule| kind_of.contains(rule))
}

fn at_keyframes(kind_of: &str) -> bool {
    ["@keyframes", "@-moz-keyframes", "@-ms-keyframes", "@-o-keyframes", "@-webkit-keyframes"].iter().any(|rule| kind_of.ends_with(rule))
}

fn dispatch(context: &'static str, kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    match context {
        "parens" => parens(kind_of, stream, state, style),
        "vendorPrefixes" => {
            if kind_of == Some("word") {
                *style = Some("property");
                return push(state, "block");
            }
            pop(state)
        }
        "pseudo" => {
            if is_property(first_word(line_text(stream))) {
                return reapply(kind_of, stream, state, style);
            }
            stream.eat_while(|byte| byte.is_ascii_lowercase() || byte == b'-');
            *style = Some("variable-3");
            if rest_blank(stream) { push(state, "block") } else { pop(state) }
        }
        "atBlock" => at_block(kind_of, stream, state, style),
        "atBlock_parens" => at_block_parens(kind_of, stream, state, style),
        "keyframes" => keyframes(kind_of, stream, state, style),
        "interpolation" => interpolation(kind_of, stream, state, style),
        "extend" => {
            if matches!(kind_of, Some("[" | "=")) {
                return "extend";
            }
            if kind_of == Some("word") {
                *style = Some(word_style(stream.current()));
                return "extend";
            }
            pop(state)
        }
        "variableName" => {
            let current = stream.current();
            if matches!(kind_of, Some("string" | "[" | "]")) || current.starts_with(['.', '$']) {
                if current.len() > 1
                    && current.starts_with('.')
                    && current.as_bytes().get(1).is_some_and(|byte| scan::js_word(*byte) || *byte == b'-')
                {
                    *style = Some("variable-2");
                }
                return "variableName";
            }
            reapply(kind_of, stream, state, style)
        }
        _ => block(kind_of, stream, state, style),
    }
}

/// `{` before a name (the mode's `A`).
fn interpolation_start(kind_of: Option<&str>, stream: &Stream) -> bool {
    kind_of == Some("{") && {
        let rest = stream.rest();
        let from = rest.iter().take_while(|byte| space(**byte)).count();
        let from = if rest.get(from) == Some(&b'$') { from + 1 } else { from };
        rest.get(from).is_some_and(|byte| scan::js_word(*byte) || *byte == b'-')
    }
}

/// `:` before a lower-case name (the mode's `O`).
fn pseudo_start(kind_of: Option<&str>, stream: &Stream) -> bool {
    kind_of == Some(":") && (stream.peek().is_ascii_lowercase() || stream.peek() == b'-')
}

/// The mode's `U`.
fn opens_block(kind_of: Option<&str>, stream: &Stream) -> bool {
    (rest_blank(stream) && matches!(kind_of, Some("{" | "]" | "hash" | "qualifier"))) || kind_of == Some("block-mixin")
}

fn block(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    // The line's text, read only when a rule needs it (it costs the line's length).
    let bytes = stream.bytes;
    let text = || std::str::from_utf8(bytes).unwrap_or("");
    let blank = rest_blank(stream);
    if (kind_of == Some("comment") && starts_line(stream)) || (kind_of == Some(",") && blank) || kind_of == Some("mixin") {
        return push(state, "block");
    }
    if interpolation_start(kind_of, stream) {
        return push(state, "interpolation");
    }
    if blank && kind_of == Some("]") && !text().trim_start().starts_with(['.', '#', ':', '[', '*', '&']) && !is_tag(first_word(text())) {
        return push(state, "block");
    }
    if opens_block(kind_of, stream) {
        return push(state, "block");
    }
    if kind_of == Some("}") && blank {
        return push(state, "block");
    }
    if kind_of == Some("variable-name") {
        return push(state, "variableName");
    }
    if kind_of == Some("=") {
        return push(state, "block");
    }
    if kind_of == Some("*")
        && (blank || {
            let rest = stream.rest();
            let from = rest.iter().take_while(|byte| space(**byte)).count();
            rest.get(from).is_some_and(|byte| b",.#[:{".contains(byte))
        })
    {
        *style = Some("tag");
        return push(state, "block");
    }
    if pseudo_start(kind_of, stream) {
        return push(state, "pseudo");
    }
    let Some(kind_of) = kind_of else { return top(state) };
    if at_media(kind_of) {
        return push(state, if blank { "block" } else { "atBlock" });
    }
    if at_keyframes(kind_of) {
        return push(state, "keyframes");
    }
    if kind_of.contains("@extend") {
        return push(state, "extend");
    }
    if kind_of.starts_with('@') {
        if indented(stream) && is_property(stream.current().get(1..).unwrap_or("")) {
            *style = Some("variable-2");
            return "block";
        }
        return push(state, "block");
    }
    if kind_of == "reference" && blank {
        return push(state, "block");
    }
    if kind_of == "(" {
        return push(state, "parens");
    }
    if kind_of == "vendor-prefixes" {
        return push(state, "vendorPrefixes");
    }
    if kind_of == "word" {
        let current = stream.current().to_string();
        let mut word = word_style(&current);
        *style = Some(word);
        if word == "property" {
            if starts_line(stream) {
                return push(state, "block");
            }
            *style = Some("atom");
            return "block";
        }
        if word == "tag" {
            if ["embed", "menu", "pre", "progress", "sub", "table"].iter().any(|part| current.contains(part)) && is_property(first_word(text())) {
                *style = Some("atom");
                return "block";
            }
            if bracketed(text(), &current) {
                *style = Some("atom");
                return "block";
            }
            let starts = starts_line(stream);
            if short_tag(&current)
                && ((starts && text().contains('='))
                    || !(starts
                        || text().trim_start().starts_with('.')
                        || text().starts_with(['#', '&', '[', '/', '>', '*'])
                        || is_tag(first_word(text()))))
            {
                *style = Some("variable-2");
                if is_block_keyword(first_word(text())) {
                    return "block";
                }
                return push(state, "block");
            }
            if blank {
                return push(state, "block");
            }
        }
        if word == "block-keyword" {
            word = "keyword";
            *style = Some(word);
            if !starts_line(stream) {
                return "block";
            }
            return push(state, "block");
        }
        if word == "variable-2" && variable_line(text()) {
            return push(state, "block");
        }
    }
    top(state)
}

/// The line has `[` (and spaces) before the word or the word (and spaces) before `]`.
fn bracketed(text: &str, word: &str) -> bool {
    let open = text.match_indices('[').any(|(at, _)| text.get(at + 1..).is_some_and(|rest| rest.trim_start().starts_with(word)));
    let close = text.match_indices(word).any(|(at, _)| text.get(at + word.len()..).is_some_and(|rest| rest.trim_start().starts_with(']')));
    open || close
}

fn parens(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    if kind_of == Some("(") {
        return push(state, "parens");
    }
    if kind_of == Some(")") {
        let outer = state.contexts.len().checked_sub(2).and_then(|index| state.contexts.get(index)).copied();
        if outer == Some("parens") {
            return pop(state);
        }
        // The mode weighs the line to pick the block's indentation; either way, a block.
        return push(state, "block");
    }
    if let Some(kind_of) = kind_of
        && kind_of.starts_with('@')
        && is_property(stream.current().get(1..).unwrap_or(""))
    {
        *style = Some("variable-2");
    }
    if kind_of == Some("word") {
        let current = stream.current();
        let mut word = word_style(current);
        if word == "tag" && short_tag(current) {
            word = "variable-2";
        }
        if word == "property" || current == "to" {
            word = "atom";
        }
        *style = Some(word);
    }
    if kind_of == Some("variable-name") {
        return push(state, "variableName");
    }
    if pseudo_start(kind_of, stream) {
        return push(state, "pseudo");
    }
    top(state)
}

fn at_block(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    if kind_of == Some("(") {
        return push(state, "atBlock_parens");
    }
    if opens_block(kind_of, stream) {
        return push(state, "block");
    }
    if interpolation_start(kind_of, stream) {
        return push(state, "interpolation");
    }
    if kind_of == Some("word") {
        let current = stream.current();
        let lower = current.to_ascii_lowercase();
        let word = if ["only", "not", "and", "or"].contains(&lower.as_str()) {
            "keyword"
        } else if listed(DOCUMENT_TYPES, &lower) {
            "tag"
        } else if listed(MEDIA_TYPES, &lower) {
            "attribute"
        } else if listed(MEDIA_FEATURES, &lower) {
            "property"
        } else if listed(NON_STANDARD_PROPERTIES, &lower) {
            "string-2"
        } else {
            word_style(current)
        };
        *style = Some(word);
        if word == "tag" && rest_blank(stream) {
            return push(state, "block");
        }
    }
    if kind_of == Some("operator") && ["not", "and", "or"].contains(&stream.current()) {
        *style = Some("keyword");
    }
    top(state)
}

fn at_block_parens(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    match kind_of {
        Some("{" | "}") => top(state),
        Some(")") => push(state, if rest_blank(stream) { "block" } else { "atBlock" }),
        Some("word") => {
            let lower = stream.current().to_ascii_lowercase();
            let mut word = word_style(&lower);
            if lower.starts_with("max") || lower.starts_with("min") {
                word = "property";
            }
            if word == "tag" {
                word = if short_tag(&lower) { "variable-2" } else { "atom" };
            }
            *style = Some(word);
            top(state)
        }
        _ => at_block(kind_of, stream, state, style),
    }
}

fn keyframes(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    if !indented(stream)
        && ((kind_of == Some("}") && starts_line(stream)) || matches!(kind_of, Some("]" | "hash" | "qualifier")) || is_tag(stream.current()))
    {
        return reapply(kind_of, stream, state, style);
    }
    match kind_of {
        Some("{") => push(state, "keyframes"),
        Some("}") => {
            if starts_line(stream) {
                pop(state)
            } else {
                push(state, "keyframes")
            }
        }
        Some("unit")
            if {
                let current = stream.current();
                current.strip_suffix('%').is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            } =>
        {
            push(state, "keyframes")
        }
        Some("word") => {
            let word = word_style(stream.current());
            *style = Some(word);
            if word == "block-keyword" {
                *style = Some("keyword");
                return push(state, "keyframes");
            }
            top(state)
        }
        Some(rule) if at_media(rule) => push(state, if rest_blank(stream) { "block" } else { "atBlock" }),
        Some("mixin") => push(state, "block"),
        _ => top(state),
    }
}

fn interpolation(kind_of: Option<&str>, stream: &mut Stream, state: &mut Stylus, style: &mut Option<&'static str>) -> &'static str {
    if kind_of == Some("{") {
        pop(state);
        push(state, "block");
    }
    if kind_of == Some("}") {
        // The mode weighs the line to pick the block's indentation; either way, a block.
        return push(state, "block");
    }
    if kind_of == Some("variable-name") {
        return push(state, "variableName");
    }
    if kind_of == Some("word") {
        let word = word_style(stream.current());
        *style = Some(if word == "tag" { "atom" } else { word });
    }
    top(state)
}

/// Tag names.
const TAGS: &str = "a abbr address area article aside audio b base bdi bdo bgsound blockquote body br button canvas caption cite code col \
colgroup data datalist dd del details dfn div dl dt em embed fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 head \
header hgroup hr html i iframe img input ins kbd keygen label legend li link main map mark marquee menu menuitem meta meter \
nav nobr noframes noscript object ol optgroup option output p param pre progress q rp rt ruby s samp script section select \
small source span strong style sub summary sup table tbody td textarea tfoot th thead time tr track u ul var video";

const DOCUMENT_TYPES: &str = "domain regexp url-prefix url";

const MEDIA_TYPES: &str = "all aural braille handheld print projection screen tty tv embossed";

const MEDIA_FEATURES: &str = "width min-width max-width height min-height max-height device-width min-device-width max-device-width device-height \
min-device-height max-device-height aspect-ratio min-aspect-ratio max-aspect-ratio device-aspect-ratio \
min-device-aspect-ratio max-device-aspect-ratio color min-color max-color color-index min-color-index max-color-index \
monochrome min-monochrome max-monochrome resolution min-resolution max-resolution scan grid dynamic-range \
video-dynamic-range";

const PROPERTIES: &str = "align-content align-items align-self alignment-adjust alignment-baseline anchor-point animation animation-delay \
animation-direction animation-duration animation-fill-mode animation-iteration-count animation-name animation-play-state \
animation-timing-function appearance azimuth backface-visibility background background-attachment background-clip \
background-color background-image background-origin background-position background-repeat background-size baseline-shift \
binding bleed bookmark-label bookmark-level bookmark-state bookmark-target border border-bottom border-bottom-color \
border-bottom-left-radius border-bottom-right-radius border-bottom-style border-bottom-width border-collapse border-color \
border-image border-image-outset border-image-repeat border-image-slice border-image-source border-image-width border-left \
border-left-color border-left-style border-left-width border-radius border-right border-right-color border-right-style \
border-right-width border-spacing border-style border-top border-top-color border-top-left-radius border-top-right-radius \
border-top-style border-top-width border-width bottom box-decoration-break box-shadow box-sizing break-after break-before \
break-inside caption-side clear clip color color-profile column-count column-fill column-gap column-rule column-rule-color \
column-rule-style column-rule-width column-span column-width columns content counter-increment counter-reset crop cue \
cue-after cue-before cursor direction display dominant-baseline drop-initial-after-adjust drop-initial-after-align \
drop-initial-before-adjust drop-initial-before-align drop-initial-size drop-initial-value elevation empty-cells fit \
fit-position flex flex-basis flex-direction flex-flow flex-grow flex-shrink flex-wrap float float-offset flow-from flow-into \
font font-feature-settings font-family font-kerning font-language-override font-size font-size-adjust font-stretch \
font-style font-synthesis font-variant font-variant-alternates font-variant-caps font-variant-east-asian \
font-variant-ligatures font-variant-numeric font-variant-position font-weight grid grid-area grid-auto-columns \
grid-auto-flow grid-auto-position grid-auto-rows grid-column grid-column-end grid-column-start grid-row grid-row-end \
grid-row-start grid-template grid-template-areas grid-template-columns grid-template-rows hanging-punctuation height hyphens \
icon image-orientation image-rendering image-resolution inline-box-align justify-content left letter-spacing line-break \
line-height line-stacking line-stacking-ruby line-stacking-shift line-stacking-strategy list-style list-style-image \
list-style-position list-style-type margin margin-bottom margin-left margin-right margin-top marker-offset marks \
marquee-direction marquee-loop marquee-play-count marquee-speed marquee-style max-height max-width min-height min-width \
move-to nav-down nav-index nav-left nav-right nav-up object-fit object-position opacity order orphans outline outline-color \
outline-offset outline-style outline-width overflow overflow-style overflow-wrap overflow-x overflow-y padding \
padding-bottom padding-left padding-right padding-top page page-break-after page-break-before page-break-inside page-policy \
pause pause-after pause-before perspective perspective-origin pitch pitch-range play-during position presentation-level \
punctuation-trim quotes region-break-after region-break-before region-break-inside region-fragment rendering-intent resize \
rest rest-after rest-before richness right rotation rotation-point ruby-align ruby-overhang ruby-position ruby-span \
shape-image-threshold shape-inside shape-margin shape-outside size speak speak-as speak-header speak-numeral \
speak-punctuation speech-rate stress string-set tab-size table-layout target target-name target-new target-position \
text-align text-align-last text-decoration text-decoration-color text-decoration-line text-decoration-skip \
text-decoration-style text-emphasis text-emphasis-color text-emphasis-position text-emphasis-style text-height text-indent \
text-justify text-outline text-overflow text-shadow text-size-adjust text-space-collapse text-transform \
text-underline-position text-wrap top transform transform-origin transform-style transition transition-delay \
transition-duration transition-property transition-timing-function unicode-bidi vertical-align visibility voice-balance \
voice-duration voice-family voice-pitch voice-range voice-rate voice-stress voice-volume volume white-space widows width \
will-change word-break word-spacing word-wrap z-index clip-path clip-rule mask enable-background filter flood-color \
flood-opacity lighting-color stop-color stop-opacity pointer-events color-interpolation color-interpolation-filters \
color-rendering fill fill-opacity fill-rule image-rendering marker marker-end marker-mid marker-start shape-rendering stroke \
stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-opacity stroke-width \
text-rendering baseline-shift dominant-baseline glyph-orientation-horizontal glyph-orientation-vertical text-anchor \
writing-mode font-smoothing osx-font-smoothing";

const NON_STANDARD_PROPERTIES: &str = "scrollbar-arrow-color scrollbar-base-color scrollbar-dark-shadow-color scrollbar-face-color scrollbar-highlight-color \
scrollbar-shadow-color scrollbar-3d-light-color scrollbar-track-color shape-inside searchfield-cancel-button \
searchfield-decoration searchfield-results-button searchfield-results-decoration zoom";

const FONT_PROPERTIES: &str = "font-family src unicode-range font-variant font-feature-settings font-stretch font-weight font-style";

/// Color names (the mode colors them keywords).
const COLORS: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue \
chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkkhaki \
darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkturquoise \
darkviolet deeppink deepskyblue dimgray dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold \
goldenrod gray grey green greenyellow honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen \
lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightpink lightsalmon lightseagreen \
lightskyblue lightslategray lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue \
mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue \
mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen \
paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue \
saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray snow springgreen steelblue tan \
teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen";

const VALUES: &str = "above absolute activeborder additive activecaption afar after-white-space ahead alias all all-scroll alphabetic alternate \
always amharic amharic-abegede antialiased appworkspace arabic-indic armenian asterisks attr auto avoid avoid-column \
avoid-page avoid-region background backwards baseline below bidi-override binary bengali blink block block-axis bold bolder \
border border-box both bottom break break-all break-word bullets button button-bevel buttonface buttonhighlight buttonshadow \
buttontext calc cambodian capitalize caps-lock-indicator caption captiontext caret cell center checkbox circle cjk-decimal \
cjk-earthly-branch cjk-heavenly-stem cjk-ideographic clear clip close-quote col-resize collapse column compact condensed \
contain content contents content-box context-menu continuous copy counter counters cover crop cross crosshair currentcolor \
cursive cyclic dashed decimal decimal-leading-zero default default-button destination-atop destination-in destination-out \
destination-over devanagari disc discard disclosure-closed disclosure-open document dot-dash dot-dot-dash dotted double down \
e-resize ease ease-in ease-in-out ease-out element ellipse ellipsis embed end ethiopic ethiopic-abegede \
ethiopic-abegede-am-et ethiopic-abegede-gez ethiopic-abegede-ti-er ethiopic-abegede-ti-et ethiopic-halehame-aa-er \
ethiopic-halehame-aa-et ethiopic-halehame-am-et ethiopic-halehame-gez ethiopic-halehame-om-et ethiopic-halehame-sid-et \
ethiopic-halehame-so-et ethiopic-halehame-ti-er ethiopic-halehame-ti-et ethiopic-halehame-tig ethiopic-numeric ew-resize \
expanded extends extra-condensed extra-expanded fantasy fast fill fixed flat flex footnotes forwards from geometricPrecision \
georgian graytext groove gujarati gurmukhi hand hangul hangul-consonant hebrew help hidden hide high higher highlight \
highlighttext hiragana hiragana-iroha horizontal hsl hsla icon ignore inactiveborder inactivecaption inactivecaptiontext \
infinite infobackground infotext inherit initial inline inline-axis inline-block inline-flex inline-table inset inside \
intrinsic invert italic japanese-formal japanese-informal justify kannada katakana katakana-iroha keep-all khmer \
korean-hangul-formal korean-hanja-formal korean-hanja-informal landscape lao large larger left level lighter line-through \
linear linear-gradient lines list-item listbox listitem local logical loud lower lower-alpha lower-armenian lower-greek \
lower-hexadecimal lower-latin lower-norwegian lower-roman lowercase ltr malayalam match matrix matrix3d \
media-controls-background media-current-time-display media-fullscreen-button media-mute-button media-play-button \
media-return-to-realtime-button media-rewind-button media-seek-back-button media-seek-forward-button media-slider \
media-sliderthumb media-time-remaining-display media-volume-slider media-volume-slider-container media-volume-sliderthumb \
medium menu menulist menulist-button menulist-text menulist-textfield menutext message-box middle min-intrinsic mix \
mongolian monospace move multiple myanmar n-resize narrower ne-resize nesw-resize no-close-quote no-drop no-open-quote \
no-repeat none normal not-allowed nowrap ns-resize numbers numeric nw-resize nwse-resize oblique octal open-quote \
optimizeLegibility optimizeSpeed oriya oromo outset outside outside-shape overlay overline padding padding-box painted page \
paused persian perspective plus-darker plus-lighter pointer polygon portrait pre pre-line pre-wrap preserve-3d progress \
push-button radial-gradient radio read-only read-write read-write-plaintext-only rectangle region relative repeat \
repeating-linear-gradient repeating-radial-gradient repeat-x repeat-y reset reverse rgb rgba ridge right rotate rotate3d \
rotateX rotateY rotateZ round row-resize rtl run-in running s-resize sans-serif scale scale3d scaleX scaleY scaleZ scroll \
scrollbar scroll-position se-resize searchfield searchfield-cancel-button searchfield-decoration searchfield-results-button \
searchfield-results-decoration semi-condensed semi-expanded separate serif show sidama simp-chinese-formal \
simp-chinese-informal single skew skewX skewY skip-white-space slide slider-horizontal slider-vertical \
sliderthumb-horizontal sliderthumb-vertical slow small small-caps small-caption smaller solid somali source-atop source-in \
source-out source-over space spell-out square square-button standard start static status-bar stretch stroke sub \
subpixel-antialiased super sw-resize symbolic symbols table table-caption table-cell table-column table-column-group \
table-footer-group table-header-group table-row table-row-group tamil telugu text text-bottom text-top textarea textfield \
thai thick thin threeddarkshadow threedface threedhighlight threedlightshadow threedshadow tibetan tigre tigrinya-er \
tigrinya-er-abegede tigrinya-et tigrinya-et-abegede to top trad-chinese-formal trad-chinese-informal translate translate3d \
translateX translateY translateZ transparent ultra-condensed ultra-expanded underline up upper-alpha upper-armenian \
upper-greek upper-hexadecimal upper-latin upper-norwegian upper-roman uppercase urdu url var vertical vertical-text visible \
visibleFill visiblePainted visibleStroke visual w-resize wait wave wider window windowframe windowtext words x-large x-small \
xor xx-large xx-small bicubic optimizespeed grayscale row row-reverse wrap wrap-reverse column-reverse flex-start flex-end \
space-between space-around unset";

const BLOCK_KEYWORDS: &str = "for if else unless from to";

const COMMON_ATOMS: &str = "null true false href title type not-allowed readonly disabled";
