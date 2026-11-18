//! Small formats, as their CodeMirror 5 modes color them, each ported token for token: Solr
//! queries, troff, RPM spec files, ASCII-armored PGP, HXML, and literate Haskell (see each
//! function).

use super::Kind;
use super::more::{self, More};
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Formats {
    /// Solr: an open string.
    string: bool,
    /// RPM spec: inside `%if`, inside a macro's `( )`.
    control_flow: bool,
    macro_parameters: bool,
    /// ASCII armor: where it is, and the armor's type.
    armor: Armor,
    armor_type: String,
    /// HXML: an open `'` string.
    hxml_string: bool,
    /// Literate Haskell: the code's own state.
    haskell: More,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Armor {
    #[default]
    Top,
    Headers,
    Header,
    Body,
    End,
}

fn solr_term(byte: u8) -> bool {
    !byte.is_ascii_whitespace() && !b"|!+-*?~^&:()[]{}\"\\".contains(&byte)
}

/// Whether JavaScript's `parseFloat(text).toString()` gives `text` back.
fn js_float_round_trips(text: &str) -> bool {
    if text == "NaN" || text == "Infinity" {
        return true;
    }
    let canonical = text.bytes().all(|byte| byte.is_ascii_digit() || b".e+-".contains(&byte));
    let Some(value) = text.parse::<f64>().ok().filter(|value| canonical && value.is_finite()) else { return false };
    let shown = if value != 0.0 && !(1e-6..1e21).contains(&value.abs()) {
        let exponent = format!("{value:e}");
        match exponent.split_once('e') {
            Some((mantissa, power)) if !power.starts_with('-') => format!("{mantissa}e+{power}"),
            _ => exponent,
        }
    } else {
        format!("{value}")
    };
    shown == text
}

/// Solr queries, as CodeMirror 5's solr mode colors them: `OR`, `AND`, `NOT`, `TO` and symbols
/// operators, numbers, a term before `:` (a field) plain, other terms and quoted phrases
/// strings.
pub fn solr(text: &str, state: &mut Formats, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.string {
            return solr_string(stream, state);
        }
        let here = stream.next()?;
        if here == b'"' {
            return solr_string(stream, state);
        }
        if b"|!+-*?~^&".contains(&here) {
            if here == b'|' || here == b'&' {
                stream.eat(|byte| byte == here);
            }
            return Some(Kind::Operator);
        }
        if !solr_term(here) {
            return None;
        }
        stream.eat_while(solr_term);
        let term = stream.current();
        if ["OR", "AND", "NOT", "TO"].iter().any(|word| word.eq_ignore_ascii_case(term)) {
            Some(Kind::Operator)
        } else if js_float_round_trips(term) {
            Some(Kind::Number)
        } else if stream.peek() == b':' {
            None
        } else {
            Some(Kind::String)
        }
    });
}

/// A phrase to its quote; open past the line only by a backslash at its end.
fn solr_string(stream: &mut Stream, state: &mut Formats) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    state.string = escaped;
    Some(Kind::String)
}

/// troff, as CodeMirror 5's troff mode colors it: `\` escapes strings, `.\"` comments, a
/// request's name (`.B `, `.PP`) an attribute, `.TH`/`.SH` lines quotes (plain).
pub fn troff(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let sol = stream.sol();
        let here = stream.next()?;
        if here == b'\\' {
            let simple = ["fB", "fR", "fI", "u", "d", "%", "&"].iter().any(|escape| stream.take(escape));
            if !simple {
                if stream.take("m[") {
                    if let Some(close) = stream.rest().iter().position(|byte| *byte == b']') {
                        stream.pos += close;
                    }
                    stream.next();
                } else if stream.take("s+") || stream.take("s-") {
                    stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'-');
                } else if stream.take("(") || stream.take("*(") {
                    stream.eat_while(|byte| scan::js_word(byte) || byte == b'-');
                }
            }
            return Some(Kind::String);
        }
        if sol && (here == b'.' || here == b'\'') && stream.take("\\") && stream.take("\"") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if sol && here == b'.' {
            if stream.take("B ") || stream.take("I ") || stream.take("R ") {
                return Some(Kind::Property);
            }
            if ["TH ", "SH ", "SS ", "HP "].iter().any(|request| stream.take(request)) {
                stream.skip_to_end();
                return None;
            }
            // Two capitals or two lower-case letters, each read as far as it matches (a capital
            // then a lower-case letter leaves both read).
            let upper = stream.eat(|byte| byte.is_ascii_uppercase()).is_some() && stream.eat(|byte| byte.is_ascii_uppercase()).is_some();
            if upper || (stream.eat(|byte| byte.is_ascii_lowercase()).is_some() && stream.eat(|byte| byte.is_ascii_lowercase()).is_some()) {
                return Some(Kind::Property);
            }
        }
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'-');
        None
    });
}

const RPM_ARCHES: &[&str] = &[
    "i386", "i586", "i686", "x86_64", "ppc64le", "ppc64", "ppc", "ia64", "s390x", "s390", "sparc64", "sparcv9", "sparc", "noarch", "alphaev6",
    "alpha", "hppa", "mipsel",
];
const RPM_SECTIONS: &str = "debug_package|package|description|prep|build|install|files|clean|changelog|preinstall|preun|\
postinstall|postun|pretrans|posttrans|pre|post|triggerin|triggerun|verifyscript|check|triggerpostun|triggerprein|trigger";

/// Takes the first of `list` (`|` between them) the position starts with, after `prefix`.
fn take_prefixed(stream: &mut Stream, prefix: &str, list: &str) -> bool {
    if !stream.at(prefix) {
        return false;
    }
    let at = stream.pos + prefix.len();
    let found = list.split('|').find(|item| scan::at(stream.bytes, at, item));
    if let Some(item) = found {
        stream.pos = at + item.len();
    }
    found.is_some()
}

/// RPM spec files, as CodeMirror 5's rpm-spec mode colors them: `Name:` headers and `%section`
/// lines plain, macros keywords, `$VAR` and `%{…}` definitions plain, architectures numbers,
/// `%if` conditions' operators and numbers, `#` comments.
pub fn rpm_spec(text: &str, state: &mut Formats, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.peek() == b'#' {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        let word =
            |stream: &Stream, from: usize| (from..stream.bytes.len()).find(|at| !scan::js_word(stream.byte(*at))).unwrap_or(stream.bytes.len());
        if stream.sol() {
            let name = (0..stream.bytes.len()).find(|at| !(stream.byte(*at).is_ascii_alphanumeric() || b"()".contains(&stream.byte(*at))));
            if let Some(colon) = name.filter(|colon| *colon > 0 && stream.byte(*colon) == b':') {
                stream.pos = colon + 1;
                return None;
            }
            if take_prefixed(stream, "%", RPM_SECTIONS) {
                return None;
            }
        }
        if stream.peek() == b'$' {
            let end = word(stream, stream.pos + 1);
            if end > stream.pos + 1 {
                stream.pos = end;
                return None;
            }
            if stream.byte(stream.pos + 1) == b'{' {
                let end = word(stream, stream.pos + 2);
                if end > stream.pos + 2 && stream.byte(end) == b'}' {
                    stream.pos = end + 1;
                    return None;
                }
            }
        }
        if take_prefixed(stream, "%", "else|endif") {
            return Some(Kind::Keyword);
        }
        if take_prefixed(stream, "%", "ifnarch|ifarch|if") {
            state.control_flow = true;
            return Some(Kind::Keyword);
        }
        if state.control_flow {
            if ["!", "?", "<=", "<", ">=", ">", "==", "&&", "||"].iter().any(|operator| stream.take(operator)) {
                return Some(Kind::Operator);
            }
            if stream.eat_while(|byte| byte.is_ascii_digit()) {
                return Some(Kind::Number);
            }
        }
        if let Some(arch) = RPM_ARCHES.iter().find(|arch| stream.at(arch)) {
            stream.pos += arch.len();
            if stream.eol() {
                state.control_flow = false;
            }
            return Some(Kind::Number);
        }
        if stream.peek() == b'%' {
            let end = word(stream, stream.pos + 1);
            if end > stream.pos + 1 {
                stream.pos = end;
                if stream.take("(") {
                    state.macro_parameters = true;
                }
                return Some(Kind::Keyword);
            }
        }
        if state.macro_parameters {
            if stream.eat_while(|byte| byte.is_ascii_digit()) {
                return Some(Kind::Number);
            }
            if stream.take(")") {
                state.macro_parameters = false;
                return Some(Kind::Keyword);
            }
        }
        // `%\{\??[\w \-\:\!]+\}`
        if stream.at("%{") {
            let at = stream.pos + 2;
            let from = if stream.byte(at) == b'?' { at + 1 } else { at };
            let end = (from..stream.bytes.len()).find(|index| !(scan::js_word(stream.byte(*index)) || b" -:!".contains(&stream.byte(*index))));
            if let Some(end) = end.filter(|end| *end > from && stream.byte(*end) == b'}') {
                stream.pos = end + 1;
                if stream.eol() {
                    state.control_flow = false;
                }
                return None;
            }
        }
        stream.next();
        None
    });
}

/// `-----<word> (.*)?-----\s*$` from the line's start: the armor's type, if the line is one.
fn armor_line<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let rest = text.strip_prefix("-----")?.strip_prefix(word)?.strip_prefix(' ')?;
    let trimmed = rest.trim_end();
    trimmed.strip_suffix("-----")
}

/// ASCII-armored PGP, as CodeMirror 5's asciiarmor mode colors it: `-----BEGIN …-----` and a
/// matching `-----END …-----` tags, header names plain and their values strings, the first
/// line after the headers and any line outside the armor errors.
pub fn armor(text: &str, state: &mut Formats, out: &mut Out) {
    // The mode's `a`: the line's rest an error, unless it is blank.
    let rest_error = |stream: &mut Stream| {
        let blank = stream.rest().iter().all(u8::is_ascii_whitespace);
        stream.skip_to_end();
        (!blank).then_some(Kind::Operator)
    };
    scan::run(text, out, |stream| match state.armor {
        Armor::Top => {
            if stream.sol()
                && let Some(kind) = armor_line(text, "BEGIN")
            {
                state.armor = Armor::Headers;
                state.armor_type = kind.to_string();
                stream.skip_to_end();
                return Some(Kind::Operator);
            }
            rest_error(stream)
        }
        Armor::Headers => {
            if stream.sol() {
                let name = stream.rest().iter().take_while(|byte| scan::js_word(**byte)).count();
                if name > 0 && stream.byte(name) == b':' {
                    stream.pos = name + 1;
                    state.armor = Armor::Header;
                    return None;
                }
            }
            let style = rest_error(stream);
            if style.is_some() {
                state.armor = Armor::Body;
            }
            style
        }
        Armor::Header => {
            stream.skip_to_end();
            state.armor = Armor::Headers;
            Some(Kind::String)
        }
        Armor::Body => {
            if stream.sol()
                && let Some(kind) = armor_line(text, "END")
            {
                stream.skip_to_end();
                if kind != state.armor_type {
                    return Some(Kind::Operator);
                }
                state.armor = Armor::End;
                return Some(Kind::Operator);
            }
            if stream.eat_while(|byte| byte.is_ascii_alphanumeric() || b"+/=".contains(&byte)) {
                return None;
            }
            stream.next();
            Some(Kind::Operator)
        }
        Armor::End => rest_error(stream),
    });
}

/// HXML, as CodeMirror 5's hxml mode colors it: a line's `-flag` a variable, `--flag` and `-D`
/// keywords, `'…'` strings, `#` comments.
pub fn hxml(text: &str, state: &mut Formats, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.peek() == b'#' {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if stream.sol() && stream.peek() == b'-' {
            stream.next();
            let mut style = Kind::Variable;
            if stream.take("-") {
                style = Kind::Keyword;
            }
            if stream.take("D") {
                style = Kind::Keyword;
            }
            stream.eat_while(|byte| byte.is_ascii_alphabetic());
            return Some(style);
        }
        if !state.hxml_string && stream.take("'") {
            state.hxml_string = true;
        }
        if state.hxml_string {
            match stream.rest().iter().position(|byte| *byte == b'\'') {
                Some(quote) => stream.pos += quote,
                None => stream.skip_to_end(),
            }
            if stream.take("'") {
                state.hxml_string = false;
            }
            return Some(Kind::String);
        }
        stream.next();
        None
    });
}

/// Literate Haskell, as CodeMirror 5's haskell-literate mode colors it: a line starting `>` is
/// code (the `>` meta), colored as Haskell; any other line a comment.
pub fn literate_haskell(text: &str, state: &mut Formats, out: &mut Out) {
    let Some(code) = text.strip_prefix('>') else {
        out.push(0..text.len(), Kind::Comment);
        return;
    };
    out.push(0..1, Kind::Comment);
    let mut inner = Out::default();
    more::haskell(code, &mut state.haskell, &mut inner);
    out.extend_shifted(inner, 1);
}
