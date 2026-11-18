//! Oz, as CodeMirror 5's oz mode colors it, ported token for token: keywords (`[]` too), `true`,
//! `nil`, … atoms (plain), a `proc`'s, `fun`'s, or `meth`'s name plain and a `class`'s name
//! variable-3, operators, other words (and lone brackets) variables, numbers (`~` for
//! negative), strings, `%` and `/* */` comments, braces brackets.

use super::Kind;
use super::scan::{self, Out, Stream};

/// What Oz reads next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Code,
    String(u8),
    Comment,
    /// The name after `class`, `meth`, `proc`/`fun`.
    Class,
    Method,
    Procedure,
}

#[derive(Clone, Debug, Default)]
pub struct Oz {
    mode: Mode,
    /// After `proc {` (the name comes next).
    passed_brace: bool,
}

const ATOMS: &str = "true|false|nil|unit";
const OPENING: &str = "local|proc|fun|case|class|if|cond|or|dis|choice|not|thread|try|raise|lock|for|suchthat|meth|functor";
const MIDDLE: &str = "in|then|else|of|elseof|elsecase|elseif|catch|finally|with|require|prepare|import|export|define|do";
const COMMON: &str = "andthen|at|attr|declare|feat|from|lex|mod|div|mode|orelse|parser|prod|prop|scanner|self|syn|token";
const LONG_OPERATORS: &[&str] = &[":::", "...", "=<:", ">=:"];
const OPERATORS: &[&str] = &["<-", ":=", "=<", ">=", "<=", "<:", ">:", "=:", "\\=", "\\=:", "!!", "==", "::"];

pub fn line(text: &str, state: &mut Oz, out: &mut Out) {
    scan::run_again(text, out, |stream| match state.mode {
        Mode::Code => code(stream, state),
        Mode::String(quote) => string(stream, state, quote),
        Mode::Comment => comment(stream, state),
        Mode::Class | Mode::Method => {
            if stream.eat_space() {
                return None;
            }
            let class = state.mode == Mode::Class;
            name(stream, |byte| if class { byte.is_ascii_uppercase() } else { byte.is_ascii_alphabetic() }, false);
            state.mode = Mode::Code;
            if class { Some(Kind::Variable) } else { None }
        }
        Mode::Procedure => {
            if stream.eat_space() {
                return None;
            }
            if !state.passed_brace && stream.take("{") {
                state.passed_brace = true;
                return Some(Kind::Punctuation);
            }
            if state.passed_brace {
                name(stream, |byte| byte.is_ascii_uppercase(), true);
                state.passed_brace = false;
            }
            state.mode = Mode::Code;
            None
        }
    });
}

/// `([first][A-Za-z0-9_]*)|(`.+`)` (or `$` when `dollar`), if there.
fn name(stream: &mut Stream, first: impl Fn(u8) -> bool, dollar: bool) {
    if first(stream.peek()) {
        stream.next();
        stream.eat_while(scan::js_word);
    } else if stream.peek() == b'`'
        && let Some(last) = stream.rest().iter().rposition(|byte| *byte == b'`').filter(|last| *last >= 2)
    {
        stream.pos += last + 1;
    } else if dollar && stream.peek() == b'$' {
        stream.next();
    }
}

fn code(stream: &mut Stream, state: &mut Oz) -> Option<Kind> {
    if stream.eat_space() {
        return None;
    }
    if stream.take("{") || stream.take("}") {
        return Some(Kind::Punctuation);
    }
    if stream.take("[]") {
        return Some(Kind::Keyword);
    }
    if LONG_OPERATORS.iter().chain(OPERATORS).any(|operator| stream.take(operator)) {
        return Some(Kind::Operator);
    }
    if stream.take_word_of(ATOMS, false) {
        return None;
    }
    let at = stream.pos;
    if stream.take_word_of(OPENING, false) {
        state.mode = match stream.bytes.get(at..stream.pos) {
            Some(b"proc" | b"fun") => Mode::Procedure,
            Some(b"class") => Mode::Class,
            Some(b"meth") => Mode::Method,
            _ => Mode::Code,
        };
        return Some(Kind::Keyword);
    }
    if stream.take_word_of(MIDDLE, false) || stream.take_word_of(COMMON, false) || stream.take_word_of("end", false) {
        return Some(Kind::Keyword);
    }
    let here = stream.next()?;
    if here == b'"' || here == b'\'' {
        state.mode = Mode::String(here);
        return string(stream, state, here);
    }
    if here == b'~' || here.is_ascii_digit() {
        if here == b'~' {
            if !stream.peek().is_ascii_digit() {
                return None;
            }
            if stream.next() == Some(b'0') && hex(stream) {
                return Some(Kind::Number);
            }
        } else if here == b'0' && hex(stream) {
            return Some(Kind::Number);
        }
        // `[0-9]*(\.[0-9]+)?([eE][~+]?[0-9]+)?`, which may take nothing.
        stream.eat_while(|byte| byte.is_ascii_digit());
        if stream.peek() == b'.' && stream.byte(stream.pos + 1).is_ascii_digit() {
            stream.next();
            stream.eat_while(|byte| byte.is_ascii_digit());
        }
        if matches!(stream.peek(), b'e' | b'E') {
            let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'~' | b'+'));
            if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
        return Some(Kind::Number);
    }
    if here == b'%' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here == b'/' && stream.eat(|byte| byte == b'*').is_some() {
        state.mode = Mode::Comment;
        return comment(stream, state);
    }
    if b"^@!|<>#~.*-+\\/,=".contains(&here) {
        return Some(Kind::Operator);
    }
    stream.eat_while(scan::js_word);
    Some(Kind::Variable)
}

/// `[xX][0-9a-fA-F]+` at the position, taken if there.
fn hex(stream: &mut Stream) -> bool {
    if matches!(stream.peek(), b'x' | b'X') && stream.byte(stream.pos + 1).is_ascii_hexdigit() {
        stream.next();
        stream.eat_while(|byte| byte.is_ascii_hexdigit());
        return true;
    }
    false
}

/// A string to its quote; open past the line only by a backslash at its end.
fn string(stream: &mut Stream, state: &mut Oz, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    let mut closed = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            closed = true;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    if closed || !escaped {
        state.mode = Mode::Code;
    }
    Some(Kind::String)
}

fn comment(stream: &mut Stream, state: &mut Oz) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if star && here == b'/' {
            state.mode = Mode::Code;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}
