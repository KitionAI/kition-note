//! LiveScript, as CodeMirror 5's livescript mode (its rule tables) colors it, ported rule by
//! rule: keywords and operators keywords, names variables (a name after `.`, `@`, a closing
//! bracket, or before `:` plain), `it`, `that`, `e`, … variables, strings, regular expressions
//! and `//` heregexes strings, `#` and `/* */` comments; numbers, constants, and the language's
//! classes plain.

use super::Kind;
use super::scan::{self, Out, Stream};

/// The rule table it reads with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiveScript {
    #[default]
    Start,
    /// After a member's dot, `@`, a closing bracket, or a string: a name there is a key.
    Key,
    Comment,
    Heregex,
    /// `'''`, `"""`, `'`, `"`, `` ` ``, `<[`: what closes it.
    Quoted(Quote),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    TripleSingle,
    TripleDouble,
    Single,
    Double,
    Backtick,
    Words,
}

/// The keyword pattern's words, in the order it tries them (`\s+` where a space is).
const KEYWORDS: &str = "this throw then try typeof! typeof continue const case catch class instanceof in import all import implements if \
is default delete debugger do for own for finally function super switch else extends export eval and arguments new not unless until while \
with of or return break let var loop";
const CONSTANTS: &str = "true false yes no on off null void undefined";
const ILLEGAL: &str = "package private protected public implements interface enum static yield";
const CLASSES: &str = "RegExp ReferenceError RangeError String SyntaxError Error EvalError Array Boolean Date Function Number Object TypeError \
URIError";
const FUNCTIONS: &str = "isNaN isFinite parseInt parseFloat Math JSON encodeURIComponent encodeURI decodeURIComponent decodeURI";
const VARIABLES: &str = "that til to from fallthrough it by e";

fn space(byte: u8) -> bool {
    byte.is_ascii_whitespace()
}

/// A name's byte (`[$\w\xAA-ￜ]`, any byte of a character past ASCII).
fn name_byte(byte: u8) -> bool {
    scan::js_word(byte) || byte == b'$' || byte >= 0x80
}

/// A name at `at`: where it ends (its bytes, and `-` before a letter).
fn name(stream: &Stream, at: usize) -> Option<usize> {
    let first = stream.byte(at);
    if !name_byte(first) || first.is_ascii_digit() || at >= stream.bytes.len() {
        return None;
    }
    let mut end = at + 1;
    loop {
        if end < stream.bytes.len() && name_byte(stream.byte(end)) {
            end += 1;
        } else if stream.byte(end) == b'-' && stream.byte(end + 1).is_ascii_alphabetic() {
            end += 2;
        } else {
            return Some(end);
        }
    }
}

/// The mode's `(?![$\w]|-[A-Za-z]|\s*:(?![:=]))` at `at`.
fn word_ends(stream: &Stream, at: usize) -> bool {
    let next = stream.byte(at);
    if scan::js_word(next) || next == b'$' || (next == b'-' && stream.byte(at + 1).is_ascii_alphabetic()) {
        return false;
    }
    let colon = (at..stream.bytes.len()).find(|index| !space(stream.byte(*index))).unwrap_or(stream.bytes.len());
    stream.byte(colon) != b':' || matches!(stream.byte(colon + 1), b':' | b'=')
}

/// The first of `words` (a space in one: any run of spaces) at the position that a word's end
/// follows: where it ends.
fn word_of(stream: &Stream, words: &str) -> Option<usize> {
    let mut list = words.split(' ').peekable();
    while let Some(word) = list.next() {
        let mut end = scan::at(stream.bytes, stream.pos, word).then_some(stream.pos + word.len());
        // `import all` and `for own` are two list items: the first, spaces, and the next.
        if let Some(second) = list.next_if(|second| (word == "import" && *second == "all") || (word == "for" && *second == "own")) {
            end = end.and_then(|after| {
                let gap = (after..stream.bytes.len()).find(|index| !space(stream.byte(*index))).unwrap_or(stream.bytes.len());
                (gap > after && scan::at(stream.bytes, gap, second)).then_some(gap + second.len())
            });
        }
        if let Some(end) = end.filter(|end| word_ends(stream, *end)) {
            return Some(end);
        }
    }
    None
}

pub fn line(text: &str, state: &mut LiveScript, out: &mut Out) {
    // The mode reads again while a rule takes nothing (a key's empty rule).
    scan::run_again(text, out, |stream| token(stream, state));
}

fn token(stream: &mut Stream, state: &mut LiveScript) -> Option<Kind> {
    let rest_len = stream.bytes.len();
    match *state {
        LiveScript::Start => start(stream, state),
        LiveScript::Key => {
            if stream.eat_while(|byte| b".?@!".contains(&byte)) {
                return Some(Kind::Keyword);
            }
            *state = LiveScript::Start;
            if let Some(end) = name(stream, stream.pos) {
                stream.pos = end;
            }
            None
        }
        LiveScript::Comment => {
            match stream.rest().windows(2).position(|pair| pair == b"*/") {
                Some(at) => {
                    stream.pos += at + 2;
                    *state = LiveScript::Start;
                }
                None => stream.skip_to_end(),
            }
            Some(Kind::Comment)
        }
        LiveScript::Heregex => {
            // `.*?//[gimy$?]{0,4}`
            if let Some(at) = stream.rest().windows(2).position(|pair| pair == b"//") {
                stream.pos += at + 2;
                let mut flags = 0;
                while flags < 4 && b"gimy$?".contains(&stream.peek()) && !stream.eol() {
                    stream.next();
                    flags += 1;
                }
                *state = LiveScript::Start;
                return Some(Kind::String);
            }
            let spaces = (stream.pos..rest_len).find(|index| !space(stream.byte(*index))).unwrap_or(rest_len);
            if stream.byte(spaces) == b'#' && stream.byte(spaces + 1) == b'{' {
                stream.pos = spaces + 2;
                return Some(Kind::String);
            }
            if spaces > stream.pos {
                stream.pos = if stream.byte(spaces) == b'#' { rest_len } else { spaces };
                return Some(Kind::Comment);
            }
            stream.eat_while(|byte| !space(byte));
            Some(Kind::String)
        }
        LiveScript::Quoted(quote) => {
            let close: &[u8] = match quote {
                Quote::TripleSingle => b"'''",
                Quote::TripleDouble => b"\"\"\"",
                Quote::Words => b"]>",
                Quote::Single => b"'",
                Quote::Double => b"\"",
                Quote::Backtick => b"`",
            };
            let escapes = matches!(quote, Quote::Single | Quote::Double | Quote::Backtick);
            let mut at = stream.pos;
            while at < rest_len {
                if escapes && stream.byte(at) == b'\\' {
                    at += 2;
                    continue;
                }
                if stream.bytes.get(at..at + close.len()) == Some(close) {
                    stream.pos = at + close.len();
                    *state = LiveScript::Key;
                    return Some(Kind::String);
                }
                at += 1;
            }
            stream.skip_to_end();
            Some(Kind::String)
        }
    }
}

fn start(stream: &mut Stream, state: &mut LiveScript) -> Option<Kind> {
    let at = stream.pos;
    let len = stream.bytes.len();
    if stream.take("/*") {
        *state = LiveScript::Comment;
        return Some(Kind::Comment);
    }
    if stream.peek() == b'#' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    for (words, style) in
        [(KEYWORDS, Some(Kind::Keyword)), (CONSTANTS, None), (ILLEGAL, None), (CLASSES, None), (FUNCTIONS, None), (VARIABLES, Some(Kind::Variable))]
    {
        if let Some(end) = word_of(stream, words) {
            stream.pos = end;
            return style;
        }
    }
    if let Some(end) = name(stream, at) {
        let colon = (end..len).find(|index| !space(stream.byte(*index))).unwrap_or(len);
        if stream.byte(colon) == b':' && !matches!(stream.byte(colon + 1), b':' | b'=') {
            stream.pos = colon + 1;
            return None;
        }
        stream.pos = end;
        return Some(Kind::Variable);
    }
    if stream.take("...") {
        return Some(Kind::Keyword);
    }
    let spaces = (at..len).find(|index| !space(stream.byte(*index))).unwrap_or(len);
    if spaces > at && stream.byte(spaces) == b'?' {
        stream.pos = spaces + 1;
        return Some(Kind::Keyword);
    }
    if stream.peek() == b'@' || stream.take("::") || stream.take("..") {
        stream.eat_while(|byte| byte == b'@');
        *state = LiveScript::Key;
        return Some(Kind::Keyword);
    }
    if stream.take(".") {
        stream.eat_space();
        *state = LiveScript::Key;
        return Some(Kind::Keyword);
    }
    if stream.peek() == b'\\' && stream.byte(at + 1) > b' ' {
        stream.pos += 2;
        stream.eat_while(|byte| !space(byte) && !b",;)}]".contains(&byte));
        return Some(Kind::String);
    }
    for (open, quote) in [
        ("'''", Quote::TripleSingle),
        ("\"\"\"", Quote::TripleDouble),
        ("'", Quote::Single),
        ("\"", Quote::Double),
        ("`", Quote::Backtick),
        ("<[", Quote::Words),
    ] {
        if stream.take(open) {
            *state = LiveScript::Quoted(quote);
            return Some(Kind::String);
        }
    }
    if stream.take("//") {
        *state = LiveScript::Heregex;
        return Some(Kind::String);
    }
    if let Some(end) = regex(stream) {
        stream.pos = end;
        *state = LiveScript::Key;
        return Some(Kind::String);
    }
    if let Some(end) = number(stream) {
        stream.pos = end;
        return None;
    }
    if stream.take("(") || stream.take("{") || stream.take("[") {
        return None;
    }
    if stream.take(")") || stream.take("}") || stream.take("]") {
        *state = LiveScript::Key;
        return None;
    }
    if stream.eat_while(|byte| !space(byte)) {
        return Some(Kind::Keyword);
    }
    stream.eat_while(space);
    None
}

/// A regular expression literal (`/…/` with classes and escapes, then up to four flags).
fn regex(stream: &Stream) -> Option<usize> {
    let len = stream.bytes.len();
    if stream.peek() != b'/' {
        return None;
    }
    let mut at = stream.pos + 1;
    while at < len {
        match stream.byte(at) {
            b'/' => {
                let mut end = at + 1;
                while end < at + 5 && end < len && b"gimy$".contains(&stream.byte(end)) {
                    end += 1;
                }
                return Some(end);
            }
            b'\\' if at + 1 < len => at += 2,
            b'\\' => return None,
            b'[' => {
                at += 1;
                loop {
                    match stream.byte(at) {
                        b']' if at < len => break,
                        b'\\' if at + 1 < len => at += 2,
                        _ if at >= len => return None,
                        b'\\' => return None,
                        _ => at += 1,
                    }
                }
                at += 1;
            }
            _ => at += 1,
        }
    }
    None
}

/// The mode's number: `0x…`, `<radix>r…`, or digits with a fraction and an exponent, then any
/// word bytes.
fn number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let len = stream.bytes.len();
    let run = |from: usize, ok: &dyn Fn(u8) -> bool| (from..len).find(|end| !ok(stream.byte(*end))).unwrap_or(len.max(from));
    let digit_or_underscore = |byte: u8| byte.is_ascii_digit() || byte == b'_';
    if stream.at("0x") && stream.byte(at + 2).is_ascii_hexdigit() {
        return Some(run(at + 3, &|byte| byte.is_ascii_hexdigit() || byte == b'_'));
    }
    // `(?:[2-9]|[12]\d|3[0-6])r[\da-zA-Z][\da-zA-Z_]*`
    let radix_end = match (stream.byte(at), stream.byte(at + 1)) {
        (b'1' | b'2', b'0'..=b'9') | (b'3', b'0'..=b'6') if stream.byte(at + 2) == b'r' => Some(at + 2),
        (b'2'..=b'9', b'r') => Some(at + 1),
        _ => None,
    };
    if let Some(r) = radix_end
        && stream.byte(r + 1).is_ascii_alphanumeric()
    {
        return Some(run(r + 2, &|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
    }
    let mantissa = if stream.byte(at).is_ascii_digit() {
        let whole = run(at + 1, &digit_or_underscore);
        if stream.byte(whole) == b'.' && stream.byte(whole + 1).is_ascii_digit() { run(whole + 2, &digit_or_underscore) } else { whole }
    } else if stream.byte(at) == b'.' && stream.byte(at + 1).is_ascii_digit() {
        run(at + 2, &digit_or_underscore)
    } else {
        return None;
    };
    let mut end = mantissa;
    if stream.byte(end) == b'e' {
        let sign = if matches!(stream.byte(end + 1), b'+' | b'-') { end + 2 } else { end + 1 };
        if stream.byte(sign).is_ascii_digit() {
            end = run(sign + 1, &digit_or_underscore);
        }
    }
    Some(run(end, &|byte| scan::js_word(byte) || byte == b'$'))
}
