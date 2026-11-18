//! Grammar-like notations, as their CodeMirror 5 modes color them, each ported token for token:
//! EBNF and spreadsheet formulas (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

/// What is open, innermost first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    String,
    Comment,
    Class,
}

#[derive(Clone, Debug, Default)]
pub struct Ebnf {
    stack: Vec<Open>,
    quote: u8,
    /// The open comment's closer.
    close: &'static str,
}

fn ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

/// Reads a string's text: to its quote, a backslash taking the byte after it.
fn string_body(stream: &mut Stream, state: &mut Ebnf) {
    while state.stack.first() == Some(&Open::String) && !stream.eol() {
        if stream.peek() == state.quote {
            stream.next();
            state.stack.remove(0);
        } else if stream.peek() == b'\\' {
            stream.next();
            stream.next();
        } else {
            stream.next();
            stream.eat_while(|byte| !b"\\\"'".contains(&byte));
        }
    }
}

/// A stack opened at the token's start: a quote a string, `/*` and `(*` comments.
fn open_at_start(stream: &mut Stream, state: &mut Ebnf, comments: bool) {
    if !state.stack.is_empty() {
        return;
    }
    if matches!(stream.peek(), b'"' | b'\'') {
        state.quote = stream.peek();
        stream.next();
        state.stack.insert(0, Open::String);
    } else if comments && stream.take("/*") {
        state.stack.insert(0, Open::Comment);
        state.close = "*/";
    } else if comments && stream.take("(*") {
        state.stack.insert(0, Open::Comment);
        state.close = "*)";
    }
}

/// EBNF, as CodeMirror 5's ebnf mode colors it: a rule's name plain where `=` or `:` follows,
/// a name before `(` or `.` a variable, other names variable-2, strings properties, `[…]`
/// classes operators, `|`, `:`, `;` operators, `(* *)` and `/* */` comments, symbols atoms.
pub fn ebnf(text: &str, state: &mut Ebnf, out: &mut Out) {
    scan::run_again(text, out, |stream| {
        open_at_start(stream, state, true);
        match state.stack.first() {
            Some(Open::String) => {
                string_body(stream, state);
                return Some(Kind::Property);
            }
            Some(Open::Comment) => {
                while state.stack.first() == Some(&Open::Comment) && !stream.eol() {
                    if stream.take(state.close) {
                        state.stack.remove(0);
                    } else {
                        stream.next();
                        stream.eat_while(|byte| byte != b'*');
                    }
                }
                return Some(Kind::Comment);
            }
            Some(Open::Class) => {
                while state.stack.first() == Some(&Open::Class) && !stream.eol() {
                    if !stream.eat_while(|byte| byte != b']' && byte != b'\\') && !stream.take(".") {
                        state.stack.remove(0);
                    }
                }
                return Some(Kind::Operator);
            }
            None => {}
        }
        let here = stream.peek();
        match here {
            b'[' => {
                stream.next();
                state.stack.insert(0, Open::Class);
                return Some(Kind::Punctuation);
            }
            b':' | b'|' | b';' => {
                stream.next();
                return Some(Kind::Operator);
            }
            b'%' => {
                if stream.take("%%") {
                    return None;
                }
                if stream.byte(stream.pos + 1).is_ascii_alphabetic() {
                    stream.next();
                    stream.eat_while(|byte| byte.is_ascii_alphabetic());
                    return Some(Kind::Keyword);
                }
                if stream.take("%}") {
                    return None;
                }
            }
            // The mode's cases fall through: `/name` a keyword, then the symbol an atom.
            b'/' | b'\\' | b'.' | b'*' | b'-' | b'+' | b'^' => {
                if here == b'/' && stream.byte(stream.pos + 1).is_ascii_alphabetic() {
                    stream.next();
                    stream.eat_while(|byte| byte.is_ascii_alphabetic());
                    return Some(Kind::Keyword);
                }
                stream.next();
                return None;
            }
            b'$' => {
                if stream.take("$$") {
                    return Some(Kind::Property);
                }
                if stream.byte(stream.pos + 1).is_ascii_digit() {
                    stream.next();
                    stream.eat_while(|byte| byte.is_ascii_digit());
                    return Some(Kind::Variable);
                }
            }
            _ => {}
        }
        if matches!(here, b'$' | b'<') && stream.at("<<") {
            let name = (stream.pos + 2..stream.bytes.len()).find(|at| !ident_start(stream.byte(*at))).unwrap_or(stream.bytes.len());
            if name > stream.pos + 2 && stream.bytes.get(name..name + 2) == Some(b">>") {
                stream.pos = name + 2;
                return Some(Kind::Property);
            }
        }
        if stream.take("//") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if stream.take("return") {
            return Some(Kind::Operator);
        }
        if ident_start(here) {
            stream.eat_while(scan::js_word);
            if matches!(stream.peek(), b'(' | b'.') {
                return Some(Kind::Variable);
            }
            let after = stream.rest().iter().find(|byte| !byte.is_ascii_whitespace());
            return if matches!(after, Some(b':' | b'=')) { None } else { Some(Kind::Variable) };
        }
        if matches!(here, b'[' | b']' | b'(' | b')') {
            stream.next();
            return Some(Kind::Punctuation);
        }
        if !stream.eat_space() {
            stream.next();
        }
        None
    });
}

/// Spreadsheet formulas, as CodeMirror 5's spreadsheet mode colors them: a function's name
/// keyword, cell references variables, `$` builtins, numbers (a number run into letters an
/// error), strings, `\name` strings, `[…]` operators, symbols atoms (plain), brackets.
pub fn spreadsheet(text: &str, state: &mut Ebnf, out: &mut Out) {
    scan::run_again(text, out, |stream| {
        open_at_start(stream, state, false);
        match state.stack.first() {
            Some(Open::String) => {
                string_body(stream, state);
                return Some(Kind::String);
            }
            Some(Open::Class) => {
                while state.stack.first() == Some(&Open::Class) && !stream.eol() {
                    if stream.eat_while(|byte| byte != b']' && byte != b'\\') {
                        continue;
                    }
                    if stream.peek() == b'\\' && stream.byte(stream.pos + 1) != 0 {
                        stream.pos += 2;
                        continue;
                    }
                    state.stack.remove(0);
                }
                return Some(Kind::Operator);
            }
            _ => {}
        }
        let here = stream.peek();
        match here {
            b'[' => {
                stream.next();
                state.stack.insert(0, Open::Class);
                return Some(Kind::Punctuation);
            }
            b':' => {
                stream.next();
                return Some(Kind::Operator);
            }
            b'\\' => {
                stream.next();
                if stream.eat_while(|byte| byte.is_ascii_lowercase()) {
                    return Some(Kind::String);
                }
                return None;
            }
            b'.' | b',' | b';' | b'*' | b'-' | b'+' | b'^' | b'<' | b'/' | b'=' => {
                stream.next();
                return None;
            }
            b'$' => {
                stream.next();
                return Some(Kind::Property);
            }
            _ => {}
        }
        if stream.eat_while(|byte| byte.is_ascii_digit()) {
            return if stream.eat_while(scan::js_word) { Some(Kind::Operator) } else { Some(Kind::Number) };
        }
        if ident_start(here) {
            stream.eat_while(scan::js_word);
            return if matches!(stream.peek(), b'(' | b'.') { Some(Kind::Keyword) } else { Some(Kind::Variable) };
        }
        if b"[](){}".contains(&here) {
            stream.next();
            return Some(Kind::Punctuation);
        }
        if !stream.eat_space() {
            stream.next();
        }
        None
    });
}
