//! Mathematica (the Wolfram Language), as CodeMirror 5's mathematica mode colors it, ported
//! token for token: names keywords, `x_` patterns, `#` slots and `name: …` lines variables,
//! `\[Alpha]` named characters variables, `f::usage` meta, `a::b` messages strings, numbers
//! (with `^^` bases, `` ` `` precision, `*^` exponents), strings, nested `(* *)` comments,
//! operators, brackets, and anything else an error.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Mathematica {
    /// How many `(*` are open.
    comment: usize,
    /// A string runs on.
    string: bool,
}

fn name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'$'
}

fn name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'$'
}

/// `[a-zA-Z$][a-zA-Z0-9$]*` at `at`: where it ends.
fn name(stream: &Stream, at: usize) -> Option<usize> {
    name_start(stream.byte(at)).then(|| (at + 1..).find(|end| !name_char(stream.byte(*end))).unwrap_or(at + 1))
}

fn run_of(stream: &Stream, at: usize, ok: impl Fn(u8) -> bool) -> usize {
    let len = stream.bytes.len();
    (at..len).find(|end| !ok(stream.byte(*end))).unwrap_or(len.max(at))
}

/// `\.\d+|\d+\.\d*|\d+` at `at`.
fn mantissa(stream: &Stream, at: usize) -> Option<usize> {
    let digits = run_of(stream, at, |byte| byte.is_ascii_digit());
    if stream.byte(at) == b'.' {
        let end = run_of(stream, at + 1, |byte| byte.is_ascii_digit());
        return (end > at + 1).then_some(end);
    }
    if digits == at {
        return None;
    }
    if stream.byte(digits) == b'.' {
        return Some(run_of(stream, digits + 1, |byte| byte.is_ascii_digit()));
    }
    Some(digits)
}

/// A precision mark (`` `(`?mantissa)? ``) and an exponent (`\*\^[+-]?\d+`), each optional.
fn suffixes(stream: &Stream, at: usize) -> usize {
    let mut end = at;
    if stream.byte(end) == b'`' {
        end += 1;
        let tick = if stream.byte(end) == b'`' { end + 1 } else { end };
        if let Some(after) = mantissa(stream, tick).or_else(|| mantissa(stream, end)) {
            end = after;
        }
    }
    if stream.byte(end) == b'*' && stream.byte(end + 1) == b'^' {
        let sign = if matches!(stream.byte(end + 2), b'+' | b'-') { end + 3 } else { end + 2 };
        let digits = run_of(stream, sign, |byte| byte.is_ascii_digit());
        if digits > sign {
            end = digits;
        }
    }
    end
}

fn number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let base = run_of(stream, at, |byte| byte.is_ascii_digit());
    if base > at && stream.byte(base) == b'^' && stream.byte(base + 1) == b'^' {
        let digits = base + 2;
        let word = |from: usize| run_of(stream, from, scan::js_word);
        let end = if stream.byte(digits) == b'.' && word(digits + 1) > digits + 1 {
            Some(word(digits + 1))
        } else if word(digits) > digits {
            let whole = word(digits);
            Some(if stream.byte(whole) == b'.' { word(whole + 1) } else { whole })
        } else {
            None
        };
        if let Some(end) = end {
            return Some(suffixes(stream, end));
        }
    }
    mantissa(stream, at).map(|end| suffixes(stream, end))
}

/// A name with `` `context `` parts after it: where it ends.
fn qualified(stream: &Stream, at: usize) -> Option<usize> {
    let mut end = name(stream, at)?;
    while stream.byte(end) == b'`' && name_char(stream.byte(end + 1)) {
        end = run_of(stream, end + 1, name_char);
    }
    Some(end)
}

pub fn line(text: &str, state: &mut Mathematica, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.string {
            return string(stream, state);
        }
        if state.comment > 0 {
            return comment(stream, state);
        }
        token(stream, state)
    });
}

fn token(stream: &mut Stream, state: &mut Mathematica) -> Option<Kind> {
    let at = stream.pos;
    match stream.next()? {
        b'"' => return string(stream, state),
        b'(' if stream.eat(|byte| byte == b'*').is_some() => {
            state.comment += 1;
            return comment(stream, state);
        }
        _ => stream.back_up(1),
    }
    if let Some(end) = number(stream) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    for prefix in ["In[", "Out["] {
        if stream.at(prefix) {
            let digits = run_of(stream, at + prefix.len(), |byte| byte.is_ascii_digit());
            if stream.byte(digits) == b']' {
                stream.pos = digits + 1;
                return None;
            }
        }
    }
    if let Some(end) = qualified(stream, at)
        && stream.bytes.get(end..end + 2) == Some(b"::")
    {
        if stream.bytes.get(end + 2..end + 7) == Some(b"usage") {
            stream.pos = end + 7;
            return Some(Kind::Comment);
        }
        if let Some(message) = name(stream, end + 2) {
            stream.pos = if stream.byte(message) == b':' { message + 1 } else { message };
            return Some(Kind::String);
        }
    }
    if let Some(end) = name(stream, at) {
        // `name :` and a name or another byte after it: the line's rest.
        let colon = run_of(stream, end, |byte| byte.is_ascii_whitespace());
        let after = stream.byte(colon + 1);
        if stream.byte(colon) == b':' && colon + 1 < stream.bytes.len() && (name_start(after) || !b":=>~@^&*)[]'?,|".contains(&after)) {
            stream.skip_to_end();
            return Some(Kind::Variable);
        }
        let blanks = run_of(stream, end, |byte| byte == b'_');
        if blanks > end {
            stream.pos = name(stream, blanks).unwrap_or(blanks);
            return Some(Kind::Variable);
        }
    }
    let blanks = run_of(stream, at, |byte| byte == b'_');
    if blanks > at
        && let Some(end) = name(stream, blanks)
    {
        stream.pos = end;
        return Some(Kind::Variable);
    }
    if stream.at("\\[")
        && let Some(end) = name(stream, at + 2)
        && stream.byte(end) == b']'
    {
        stream.pos = end + 1;
        return Some(Kind::Variable);
    }
    if b"[]{}()".contains(&stream.peek()) {
        stream.next();
        return Some(Kind::Punctuation);
    }
    if stream.peek() == b'#' {
        stream.pos = match name(stream, at + 1) {
            Some(end) => end,
            None => {
                let hashes = run_of(stream, at, |byte| byte == b'#');
                if stream.byte(hashes).is_ascii_digit() { hashes + 1 } else { hashes }
            }
        };
        return Some(Kind::Variable);
    }
    let tick = usize::from(stream.peek() == b'`');
    if let Some(mut end) = name(stream, at + tick) {
        while stream.byte(end) == b'`'
            && let Some(next) = name(stream, end + 1)
        {
            end = next;
        }
        stream.pos = if stream.byte(end) == b'`' { end + 1 } else { end };
        return Some(Kind::Keyword);
    }
    // An operator, or anything else the mode's error: the same color.
    stream.next();
    Some(Kind::Operator)
}

fn string(stream: &mut Stream, state: &mut Mathematica) -> Option<Kind> {
    let mut escaped = false;
    state.string = true;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            state.string = false;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn comment(stream: &mut Stream, state: &mut Mathematica) -> Option<Kind> {
    let mut previous = 0;
    while state.comment > 0 {
        let Some(here) = stream.next() else { break };
        if previous == b'(' && here == b'*' {
            state.comment += 1;
        }
        if previous == b'*' && here == b')' {
            state.comment -= 1;
        }
        previous = here;
    }
    Some(Kind::Comment)
}
