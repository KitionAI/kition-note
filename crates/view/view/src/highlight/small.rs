//! Small languages, as their CodeMirror 5 modes color them, each ported token for token: Eiffel,
//! Sieve, and Brainfuck (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Small {
    /// Eiffel and Sieve: an open string's quote.
    string: Option<u8>,
    /// Sieve: an open `/* */` comment, a `text:` block.
    comment: bool,
    lines: bool,
    /// Brainfuck: the line's rest is a comment.
    comment_line: bool,
}

const EIFFEL_KEYWORDS: &str = "note across when variant until unique undefine then strip select retry rescue require rename \
reference redefine prefix once old obsolete loop local like is inspect infix include if frozen from external export ensure end \
elseif else do creation create check alias agent separate invariant inherit indexing feature expanded deferred class Void True \
Result Precursor False Current create attached detachable as and implies not or";

/// Whether `word` is an Eiffel number as the mode tests it: `0b…`, `0c…`, `0x…`, digits with a
/// point (`^(\d+\.\d*)` or `(\d*\.\d+)$`), or digits.
fn eiffel_number(word: &str) -> bool {
    let bytes = word.as_bytes();
    let all = |from: usize, ok: fn(&u8) -> bool| bytes.get(from..).is_some_and(|rest| !rest.is_empty() && rest.iter().all(ok));
    let lower = |index: usize| bytes.get(index).map(u8::to_ascii_lowercase);
    if bytes.first() == Some(&b'0') {
        match lower(1) {
            Some(b'b') if all(2, |byte| *byte == b'0' || *byte == b'1') => return true,
            Some(b'c') if all(2, |byte| (b'0'..=b'7').contains(byte)) => return true,
            Some(b'x') if all(2, u8::is_ascii_hexdigit) => return true,
            _ => {}
        }
    }
    let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits > 0 && bytes.get(digits) == Some(&b'.') {
        return true;
    }
    // `(\d*\.\d+)$` may start anywhere: digits after the last point end the word.
    if bytes.iter().rposition(|byte| *byte == b'.').is_some_and(|point| all(point + 1, u8::is_ascii_digit)) {
        return true;
    }
    !bytes.is_empty() && bytes.iter().all(u8::is_ascii_digit)
}

/// Eiffel, as CodeMirror 5's eiffel mode colors it: keywords, all-capital names (classes) tags,
/// numbers, other names variables, `:=` and symbol runs operators, strings (`%` escapes), `--`
/// comments.
pub fn eiffel(text: &str, state: &mut Small, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return eiffel_string(stream, state, quote);
        }
        let here = stream.next()?;
        if here == b'"' || here == b'\'' {
            state.string = Some(here);
            return eiffel_string(stream, state, here);
        }
        if here == b'-' && stream.eat(|byte| byte == b'-').is_some() {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if here == b':' && stream.eat(|byte| byte == b'=').is_some() {
            return Some(Kind::Operator);
        }
        let symbol = |byte: u8| b"=+-/*^%<>~".contains(&byte);
        if here.is_ascii_digit() {
            stream.eat_while(|byte| b"xXbBCc0123456789.".contains(&byte));
        } else if scan::js_word(here) {
            stream.eat_while(scan::js_word);
        } else if symbol(here) {
            stream.eat_while(symbol);
            return Some(Kind::Operator);
        } else {
            return None;
        }
        stream.eat(|byte| byte == b'?' || byte == b'!');
        let word = stream.current();
        Some(if EIFFEL_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
            Kind::Keyword
        } else if word.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
            && word.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            Kind::Operator
        } else if eiffel_number(word) {
            Kind::Number
        } else {
            Kind::Variable
        })
    });
}

fn eiffel_string(stream: &mut Stream, state: &mut Small, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'%';
    }
    Some(Kind::String)
}

const SIEVE_KEYWORDS: &str = "if elsif else stop require";

/// Sieve, as CodeMirror 5's sieve mode colors it: keywords, `true`/`false`/`not` atoms (plain),
/// `:tags` operators, sizes numbers, strings, `text:` blocks to a lone `.`, `#` and `/* */`
/// comments.
pub fn sieve(text: &str, state: &mut Small, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.lines {
            if stream.sol() {
                if stream.next() == Some(b'.') && stream.eol() {
                    state.lines = false;
                }
                return Some(Kind::String);
            }
            stream.eat_space();
            let comment = stream.peek() == b'#';
            stream.skip_to_end();
            return Some(if comment { Kind::Comment } else { Kind::String });
        }
        if state.comment {
            return sieve_comment(stream, state);
        }
        if let Some(quote) = state.string {
            return sieve_string(stream, state, quote);
        }
        let here = stream.next()?;
        match here {
            b'/' if stream.eat(|byte| byte == b'*').is_some() => {
                state.comment = true;
                sieve_comment(stream, state)
            }
            b'#' => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            b'"' => {
                state.string = Some(here);
                sieve_string(stream, state, here)
            }
            b'(' | b')' | b'{' | b'}' | b',' | b';' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| byte.is_ascii_digit());
                stream.eat(|byte| b"KkMmGg".contains(&byte));
                Some(Kind::Number)
            }
            b':' => {
                stream.eat_while(|byte| byte.is_ascii_alphabetic() || byte == b'_');
                stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(scan::js_word);
                let word = stream.current().to_string();
                if word == "text" && stream.eat(|byte| byte == b':').is_some() {
                    state.lines = true;
                    return Some(Kind::String);
                }
                if SIEVE_KEYWORDS.split_ascii_whitespace().any(|known| known == word) { Some(Kind::Keyword) } else { None }
            }
        }
    });
}

fn sieve_string(stream: &mut Stream, state: &mut Small, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    if !escaped {
        state.string = None;
    }
    Some(Kind::String)
}

fn sieve_comment(stream: &mut Stream, state: &mut Small) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if star && here == b'/' {
            state.comment = false;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}

/// Brainfuck, as CodeMirror 5's brainfuck mode colors it: `+` and `-` keywords, brackets, the
/// other commands plain; any other byte makes the line's rest a comment.
pub fn brainfuck(text: &str, state: &mut Small, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if stream.sol() {
            state.comment_line = false;
        }
        let here = stream.next()?;
        if !b"><+-.,[]".contains(&here) || state.comment_line {
            // Another character, which the mode reads by its UTF-16 unit: all its bytes.
            if here >= 0x80 {
                stream.eat_while(|byte| (0x80..0xc0).contains(&byte));
            }
            state.comment_line = !stream.eol();
            return Some(Kind::Comment);
        }
        match here {
            b'[' | b']' => Some(Kind::Punctuation),
            b'+' | b'-' => Some(Kind::Keyword),
            _ => None,
        }
    });
}
