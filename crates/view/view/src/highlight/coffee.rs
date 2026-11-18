//! CoffeeScript, as CodeMirror 5's coffeescript mode colors it, ported token for token:
//! keywords, word operators (`is`, `and`), atoms (`yes`, `null`), `@name` and a name after `.`
//! properties, numbers, strings, `///` and `/…/` regular expressions, `#` and `###` comments.
//! Its scopes follow the indentation, so a line dedented to no open scope's column is the mode's
//! error (its leading spaces in the error's color).

use super::Kind;
use super::scan::{Out, Stream};

const OPERATOR_WORDS: &[&str] = &["and", "or", "not", "is", "isnt", "in", "instanceof", "typeof"];
const BLOCKS: &[&str] = &["for", "while", "loop", "if", "unless", "else", "switch", "try", "catch", "finally", "class"];
const KEYWORDS: &[&str] = &[
    "for", "while", "loop", "if", "unless", "else", "switch", "try", "catch", "finally", "class", "break", "by", "continue", "debugger", "delete",
    "do", "in", "of", "new", "return", "then", "this", "@", "throw", "when", "until", "extends",
];
const ATOMS: &[&str] = &["Infinity", "NaN", "undefined", "null", "true", "false", "on", "off", "yes", "no"];

/// The editor's indent unit, which the mode's scopes step by.
const INDENT_UNIT: usize = 4;

#[derive(Clone, Copy, Debug)]
struct Scope {
    offset: usize,
    /// `coffee` (an indented block, as `c`), or the bracket that closes it.
    kind: u8,
}

/// What the reader reads next: code, a string (or regular expression) to its delimiter, or a
/// `###` comment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Reading {
    #[default]
    Code,
    String(String, bool),
    Comment,
}

#[derive(Clone, Debug)]
pub struct Coffee {
    reading: Reading,
    scopes: Vec<Scope>,
    property: bool,
    dedent: bool,
}

impl Default for Coffee {
    fn default() -> Coffee {
        Coffee { reading: Reading::Code, scopes: vec![Scope { offset: 0, kind: b'c' }], property: false, dedent: false }
    }
}

impl Coffee {
    /// The innermost scope (the top level's when none is open).
    fn scope(&self) -> Scope {
        self.scopes.last().copied().unwrap_or(Scope { offset: 0, kind: b'c' })
    }
}

/// Whether `word` (one of `list`) starts at the position, as a whole word.
fn word_at(stream: &Stream, list: &[&str]) -> Option<usize> {
    let rest = stream.bytes.get(stream.pos..)?;
    list.iter()
        .find(|word| rest.starts_with(word.as_bytes()) && !rest.get(word.len()).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_'))
        .map(|word| word.len())
}

fn identifier_end(bytes: &[u8], at: usize) -> usize {
    let start = |byte: u8| byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$';
    if !bytes.get(at).is_some_and(|byte| start(*byte)) {
        return at;
    }
    (at..).find(|index| !bytes.get(*index).is_some_and(|byte| start(*byte) || byte.is_ascii_digit())).unwrap_or(at)
}

pub fn line(text: &str, state: &mut Coffee, out: &mut Out) {
    let indentation = text.bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count();
    super::scan::run(text, out, |stream| {
        let (style, styled) = token(stream, state, indentation);
        // A styled token (an indent or a dedent too) decides whether a property follows.
        if styled && style != Some(Kind::Comment) {
            state.property = style == Some(Kind::Punctuation) && stream.current() == ".";
        }
        style
    });
}

/// The outcome of a token: its style, or that it is an indent or a dedent.
enum Read {
    Style(Option<Kind>),
    Indent,
    Dedent,
}

fn token(stream: &mut Stream, state: &mut Coffee, indentation: usize) -> (Option<Kind>, bool) {
    let read = match state.reading.clone() {
        Reading::Code => code(stream, state, indentation),
        Reading::String(end, regex) => Read::Style(string(stream, state, &end, regex)),
        Reading::Comment => Read::Style(block_comment(stream, state)),
    };
    let current = stream.current().to_string();
    if current == "return" {
        state.dedent = true;
    }
    if (matches!(current.as_str(), "->" | "=>") && stream.eol()) || matches!(read, Read::Indent) {
        open(state, b'c');
    }
    if let Some(close) = [(b'[', b']'), (b'(', b')'), (b'{', b'}')].iter().find(|(open, _)| current.as_bytes() == [*open]).map(|(_, close)| *close) {
        open(state, close);
    }
    if BLOCKS.iter().any(|block| {
        current.starts_with(block) && !current.as_bytes().get(block.len()).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    }) {
        open(state, b'c');
    }
    if current == "then" {
        dedent(state, indentation);
    }
    if matches!(read, Read::Dedent) && dedent(state, indentation) {
        return (Some(Kind::Operator), true);
    }
    if let Some(close) = b"])}".iter().find(|close| current.as_bytes() == [**close]) {
        while state.scope().kind == b'c' && state.scopes.len() > 1 {
            state.scopes.pop();
        }
        if state.scope().kind == *close {
            state.scopes.pop();
        }
    }
    if state.dedent && stream.eol() {
        if state.scope().kind == b'c' && state.scopes.len() > 1 {
            state.scopes.pop();
        }
        state.dedent = false;
    }
    match read {
        Read::Style(style) => (style, style.is_some()),
        Read::Indent | Read::Dedent => (None, true),
    }
}

/// Opens a scope (`kind`: `c` for an indented block, else its closing bracket).
fn open(state: &mut Coffee, kind: u8) {
    let offset = state.scopes.iter().rev().find(|scope| scope.kind == b'c' || scope.kind == b'}').map_or(0, |scope| scope.offset + INDENT_UNIT);
    state.scopes.push(Scope { offset, kind });
}

/// Closes scopes down to the line's indentation; true when none opened at it (an error).
fn dedent(state: &mut Coffee, indentation: usize) -> bool {
    if state.scopes.len() < 2 {
        return false;
    }
    if state.scope().kind == b'c' {
        if !state.scopes.iter().any(|scope| scope.offset == indentation) {
            return true;
        }
        while state.scopes.len() > 1 && state.scope().offset != indentation {
            state.scopes.pop();
        }
    } else {
        state.scopes.pop();
    }
    false
}

fn code(stream: &mut Stream, state: &mut Coffee, indentation: usize) -> Read {
    if stream.sol() {
        let offset = state.scope().offset;
        if stream.eat_space() {
            let kind = state.scope().kind;
            return if offset < indentation && kind == b'c' {
                Read::Indent
            } else if indentation < offset {
                Read::Dedent
            } else {
                Read::Style(None)
            };
        }
        if offset > 0 {
            dedent(state, indentation);
        }
    }
    if stream.eat_space() {
        return Read::Style(None);
    }
    Read::Style(base(stream, state))
}

fn base(stream: &mut Stream, state: &mut Coffee) -> Option<Kind> {
    let bytes = stream.bytes;
    if stream.take("####") {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if stream.take("###") {
        state.reading = Reading::Comment;
        return block_comment(stream, state);
    }
    if stream.peek() == b'#' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if let Some(end) = number(bytes, stream.pos) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    for quote in ["'''", "\"\"\"", "'", "\""] {
        if stream.take(quote) {
            state.reading = Reading::String(quote.to_string(), false);
            return string(stream, state, quote, false);
        }
    }
    if stream.take("///") {
        state.reading = Reading::String("///".to_string(), true);
        return string(stream, state, "///", true);
    }
    if stream.peek() == b'/' {
        let rest = bytes.get(stream.pos + 1..).unwrap_or_default();
        if rest.contains(&b'/') {
            stream.pos += 1;
            state.reading = Reading::String("/".to_string(), true);
            return string(stream, state, "/", true);
        }
    }
    if let Some(length) = operator(bytes, stream.pos) {
        stream.pos += length;
        return Some(Kind::Operator);
    }
    if let Some(length) = word_at(stream, OPERATOR_WORDS) {
        stream.pos += length;
        return Some(Kind::Operator);
    }
    let here = stream.peek();
    if b"()[]{},:`=;".contains(&here) {
        stream.pos += 1;
        return Some(Kind::Punctuation);
    }
    if here == b'.' {
        let dots = (stream.pos..stream.pos + 3).take_while(|index| bytes.get(*index) == Some(&b'.')).count();
        stream.pos += dots;
        return Some(Kind::Punctuation);
    }
    if let Some(length) = word_at(stream, ATOMS) {
        stream.pos += length;
        return None;
    }
    if here == b'@' && identifier_end(bytes, stream.pos + 1) > stream.pos + 1 {
        stream.pos = identifier_end(bytes, stream.pos + 1);
        return Some(Kind::Property);
    }
    if state.property && identifier_end(bytes, stream.pos) > stream.pos {
        stream.pos = identifier_end(bytes, stream.pos);
        return Some(Kind::Property);
    }
    if let Some(length) = word_at(stream, KEYWORDS) {
        stream.pos += length;
        return Some(Kind::Keyword);
    }
    if here == b'@' && stream.look(1).is_ascii_digit() {
        stream.pos += 1;
        return Some(Kind::Keyword);
    }
    if identifier_end(bytes, stream.pos) > stream.pos {
        stream.pos = identifier_end(bytes, stream.pos);
        return Some(Kind::Variable);
    }
    stream.next();
    while (0x80..0xC0).contains(&stream.peek()) {
        stream.pos += 1;
    }
    Some(Kind::Operator)
}

/// The mode's operators: `->`, `=>`, `++`, `+=`, `--`, `**`, `//`, `==`, `!=`, `<<=`, `>>=`, `%=`,
/// `&=`, `|=`, `^=`, `~`, `!`, `?`, `or=`, `&&=`, `?=`.
fn operator(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let (a, b, c) = (byte(at), byte(at + 1), byte(at + 2));
    Some(match a {
        b'-' if b == b'>' => 2,
        b'=' if b == b'>' => 2,
        b'+' => 1 + usize::from(b == b'+' || b == b'='),
        b'-' => 1 + usize::from(b == b'-' || b == b'='),
        b'*' => 1 + usize::from(b == b'*' || b == b'='),
        b'/' => 1 + usize::from(b == b'/' || b == b'='),
        b'=' | b'!' if b == b'=' => 2,
        b'<' => {
            let mut end = 1;
            if b == b'>' || b == b'<' {
                end += 1;
            }
            if byte(at + end) == b'=' {
                end += 1;
            }
            end
        }
        b'>' => {
            let mut end = 1;
            if b == b'>' {
                end += 1;
            }
            if byte(at + end) == b'=' {
                end += 1;
            }
            end
        }
        b'%' | b'&' | b'|' | b'^' => 1 + usize::from(b == b'='),
        b'~' | b'!' => 1,
        b'?' => 1 + usize::from(b == b'='),
        b'o' if b == b'r' && c == b'=' => 3,
        b'a' if b == b'n' && c == b'd' && byte(at + 3) == b'=' => 4,
        _ => return None,
    })
}

/// A number at `at`, read as the mode's patterns read one after another (a float's trailing `.`
/// given back when a second `.` follows, as in a range).
fn number(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let digits = |from: usize| (from..).find(|index| !byte(*index).is_ascii_digit()).unwrap_or(from);
    let signed = if byte(at) == b'-' { at + 1 } else { at };
    if !(byte(signed).is_ascii_digit() || byte(signed) == b'.') {
        return None;
    }
    let exponent = |end: usize| {
        if matches!(byte(end), b'e' | b'E') {
            let sign = if matches!(byte(end + 1), b'+' | b'-') { end + 2 } else { end + 1 };
            if digits(sign) > sign {
                return digits(sign);
            }
        }
        end
    };
    // Floats, each pattern read after the one before: `-?\d*\.\d+(e…)?`, `-?\d+\.\d*`, `-?\.\d+`.
    let mut pos = at;
    let mut float = false;
    let read = |pos: usize| {
        let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
        let whole = digits(signed);
        (byte(whole) == b'.' && digits(whole + 1) > whole + 1).then(|| exponent(digits(whole + 1)))
    };
    if let Some(end) = read(pos) {
        pos = end;
        float = true;
    }
    {
        let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
        let whole = digits(signed);
        if whole > signed && byte(whole) == b'.' {
            pos = digits(whole + 1);
            float = true;
        }
    }
    {
        let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
        if byte(signed) == b'.' && digits(signed + 1) > signed + 1 {
            pos = digits(signed + 1);
            float = true;
        }
    }
    if float {
        if byte(pos) == b'.' {
            pos -= 1;
        }
        return Some(pos);
    }
    // Integers: `-?0x[0-9a-f]+`, `-?[1-9]\d*(e…)?`, `-?0` not before a digit or `x`.
    let mut pos = at;
    let mut integer = false;
    let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
    if byte(signed) == b'0' && matches!(byte(signed + 1), b'x' | b'X') {
        let end = (signed + 2..).find(|index| !byte(*index).is_ascii_hexdigit()).unwrap_or(signed + 2);
        if end > signed + 2 {
            pos = end;
            integer = true;
        }
    }
    let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
    if (b'1'..=b'9').contains(&byte(signed)) {
        pos = exponent(digits(signed));
        integer = true;
    }
    let signed = if byte(pos) == b'-' { pos + 1 } else { pos };
    if byte(signed) == b'0' && !byte(signed + 1).is_ascii_digit() && !matches!(byte(signed + 1), b'x' | b'X') {
        pos = signed + 1;
        integer = true;
    }
    integer.then_some(pos)
}

/// A string or regular expression to `end`; a regular expression ends with its line.
fn string(stream: &mut Stream, state: &mut Coffee, end: &str, regex: bool) -> Option<Kind> {
    let style = Kind::String;
    while !stream.eol() {
        stream.eat_while(|byte| !b"'\"/\\".contains(&byte));
        if stream.eat(|byte| byte == b'\\').is_some() {
            stream.next();
            if regex && stream.eol() {
                return Some(style);
            }
        } else {
            if stream.take(end) {
                state.reading = Reading::Code;
                return Some(style);
            }
            stream.eat(|byte| b"'\"/".contains(&byte));
        }
    }
    if regex {
        state.reading = Reading::Code;
    }
    Some(style)
}

fn block_comment(stream: &mut Stream, state: &mut Coffee) -> Option<Kind> {
    while !stream.eol() {
        stream.eat_while(|byte| byte != b'#');
        if stream.take("###") {
            state.reading = Reading::Code;
            break;
        }
        stream.eat_while(|byte| byte == b'#');
    }
    Some(Kind::Comment)
}
