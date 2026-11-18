//! APL, as CodeMirror 5's apl mode colors it, ported token for token: names (and any other
//! symbol) keywords, the reduce, scan, each, and inner product operators operators, primitive
//! functions, the arrow and zilde plain, numbers (`¯` for negative), strings, `⍝` and `#`
//! comments.

use super::Kind;
use super::scan::{self, Out};

/// The operators: `.`, `/`, `\`, and reduce first, scan first, each, power.
const OPERATORS: &str = "./\\\u{233f}\u{2340}\u{a8}\u{2363}";
/// The primitive functions: `+`, minus, times, divide, ceiling, floor, … (the mode's list).
const FUNCTIONS: &str = "+\u{2212}\u{d7}\u{f7}\u{2308}\u{230a}\u{2223}\u{2373}?\u{22c6}\u{235f}\u{25cb}!\u{2339}<\u{2264}=>\u{2265}\
\u{2260}\u{2261}\u{2262}\u{2208}\u{2377}\u{222a}\u{2229}\u{223c}\u{2228}\u{2227}\u{2371}\u{2372}\u{2374},\u{236a}\u{233d}\u{2296}\
\u{2349}\u{2191}\u{2193}\u{2282}\u{2283}\u{2337}\u{234b}\u{2352}\u{22a4}\u{22a5}\u{2355}\u{234e}\u{22a3}\u{22a2}";
/// Zilde, the empty vector.
const ZILDE: char = '\u{236c}';
/// The assignment arrow.
const ARROW: char = '\u{2190}';
/// The lamp, which starts a comment.
const LAMP: char = '\u{235d}';
/// The jot, which with a `.` after it is the outer product.
const JOT: char = '\u{2218}';
/// The high minus, which starts a negative number.
const HIGH_MINUS: char = '\u{af}';

fn eat_while(chars: &mut std::iter::Peekable<std::str::CharIndices>, ok: impl Fn(char) -> bool) {
    while chars.next_if(|(_, next)| ok(*next)).is_some() {}
}

/// (The mode's monadic and dyadic function names take no color of their own, so what it tracks
/// for them is left out.)
pub fn line(text: &str, out: &mut Out) {
    let mut chars = text.char_indices().peekable();
    while let Some((start, here)) = chars.next() {
        if matches!(here, ' ' | '\t' | '\r') {
            continue;
        }
        let kind = match here {
            '"' | '\'' => {
                eat_while(&mut chars, |next| next != here);
                chars.next();
                Some(Kind::String)
            }
            '[' | '{' | '(' | ']' | '}' | ')' | ZILDE => None,
            _ if here == HIGH_MINUS || here.is_ascii_digit() => {
                eat_while(&mut chars, |next| next.is_ascii() && (scan::js_word(next as u8) || next == '.'));
                Some(Kind::Number)
            }
            _ if OPERATORS.contains(here) => Some(Kind::Operator),
            ARROW => None,
            _ if FUNCTIONS.contains(here) => None,
            LAMP | '#' => {
                while chars.next().is_some() {}
                Some(Kind::Comment)
            }
            JOT if chars.peek().is_some_and(|(_, next)| *next == '.') => {
                chars.next();
                None
            }
            _ => {
                eat_while(&mut chars, |next| next.is_ascii() && (scan::js_word(next as u8) || next == '$'));
                Some(Kind::Keyword)
            }
        };
        let end = chars.peek().map_or(text.len(), |(at, _)| *at);
        out.mark(start..end, kind);
    }
}
