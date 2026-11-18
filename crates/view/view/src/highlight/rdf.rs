//! RDF syntaxes, as their CodeMirror 5 modes color them, each ported token for token: Turtle and
//! N-Triples (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Turtle {
    /// An open string's quote.
    string: Option<u8>,
}

fn turtle_operator(byte: u8) -> bool {
    b"*+-<>=&|".contains(&byte)
}

/// Turtle, as CodeMirror 5's turtle mode colors it: `<iris>` atoms (plain), `prefix:` names
/// variables and the colon an operator, `@prefix`, `@base`, `a` meta, other words keywords (a
/// capitalized one a comment), strings, `#` comments.
pub fn turtle(text: &str, state: &mut Turtle, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return turtle_string(stream, state, quote);
        }
        let here = stream.next()?;
        // `<` not before a space or `=`: an IRI to `>`.
        if here == b'<' && !(stream.peek().is_ascii_whitespace() || stream.peek() == b'=' || stream.at(NBSP)) {
            while !(stream.eol() || stream.peek().is_ascii_whitespace() || stream.peek() == b'>' || stream.at(NBSP)) {
                stream.next();
            }
            stream.eat(|byte| byte == b'>');
            return None;
        }
        match here {
            b'"' | b'\'' => {
                state.string = Some(here);
                turtle_string(stream, state, here)
            }
            b'{' | b'}' | b'(' | b')' | b',' | b'.' | b';' | b'[' | b']' => None,
            b'#' => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if turtle_operator(here) => {
                stream.eat_while(turtle_operator);
                None
            }
            b':' => Some(Kind::Operator),
            _ => {
                stream.eat_while(scan::js_word);
                if stream.peek() == b':' {
                    return Some(Kind::Variable);
                }
                let word = stream.current();
                // `@prefix`, `@base`, `a` meta; a capitalized word a comment: one color.
                if ["@prefix", "@base", "a"].iter().any(|meta| meta.eq_ignore_ascii_case(word)) || here.is_ascii_uppercase() {
                    Some(Kind::Comment)
                } else {
                    Some(Kind::Keyword)
                }
            }
        }
    });
}

/// A no-break space, which an IRI stops at too.
const NBSP: &str = "\u{a0}";

fn turtle_string(stream: &mut Stream, state: &mut Turtle, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

/// N-Triples (and N-Quads), as CodeMirror 5's ntriples mode colors them: `<iris>`, `#fragments`,
/// and `^^<types>` variables, `_:blank` nodes builtins, literals strings with their `@language`.
pub fn ntriples(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        let here = stream.next()?;
        match here {
            b'<' => {
                stream.eat_while(|byte| byte != b'#' && byte != b'>');
                if stream.peek() != b'#' {
                    stream.next();
                }
                Some(Kind::Variable)
            }
            b'#' => {
                stream.eat_while(|byte| byte != b'>' && byte != b' ');
                Some(Kind::Variable)
            }
            b'>' => Some(Kind::Variable),
            b'_' => {
                stream.eat_while(|byte| byte != b' ');
                stream.next();
                Some(Kind::Property)
            }
            b'"' => {
                stream.eat_while(|byte| byte != b'"');
                stream.next();
                Some(Kind::String)
            }
            b'@' => {
                stream.eat_while(|byte| byte != b' ');
                stream.next();
                Some(Kind::String)
            }
            b'^' => {
                stream.next();
                stream.eat_while(|byte| byte != b'>');
                stream.next();
                Some(Kind::Variable)
            }
            _ => None,
        }
    });
}
