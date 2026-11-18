//! TeX, as CodeMirror 5's stex mode colors it: a LaTeX fence (`latex`), and math in math mode: commands, `^`, `_`, and `&`
//! tags, letters variables, numbers, brackets, `%` comments; the `$` delimiters keywords (drawn
//! in math's color).

use super::Kind;
use super::scan::{self, Out, Stream};

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

/// A math source's spans (`text` with its `$` delimiters).
pub fn line(text: &str, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'$' => {
                while byte(bytes, at) == b'$' {
                    at += 1;
                }
                out.push(start..at, Kind::Keyword);
            }
            b'\\' if byte(bytes, at + 1).is_ascii_alphabetic() || byte(bytes, at + 1) == b'@' => {
                at += 1;
                while byte(bytes, at).is_ascii_alphabetic() || byte(bytes, at) == b'@' {
                    at += 1;
                }
                out.push(start..at, Kind::Operator);
            }
            b'\\' => {
                at = (at + 2).min(bytes.len());
                out.push(start..at, Kind::Operator);
            }
            _ if here.is_ascii_alphabetic() => {
                while byte(bytes, at).is_ascii_alphabetic() {
                    at += 1;
                }
                out.push(start..at, Kind::Variable);
            }
            b'^' | b'_' | b'&' => {
                at += 1;
                out.push(start..at, Kind::Operator);
            }
            b'+' | b'-' | b'<' | b'>' | b'|' | b'=' | b',' | b'/' | b'@' | b'!' | b'*' | b':' | b';' | b'\'' | b'"' | b'`' | b'~' | b'#' | b'?' => {
                at += 1
            }
            b'0'..=b'9' | b'.' => {
                while byte(bytes, at).is_ascii_digit() {
                    at += 1;
                }
                if byte(bytes, at) == b'.' {
                    at += 1;
                    while byte(bytes, at).is_ascii_digit() {
                        at += 1;
                    }
                }
                out.push(start..at.max(start + 1), Kind::Number);
                at = at.max(start + 1);
            }
            b'{' | b'}' | b'[' | b']' | b'(' | b')' => {
                at += 1;
                out.push(start..at, Kind::Punctuation);
            }
            b'%' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            _ => {
                // Anything else is the mode's error.
                let width = text.get(at..).and_then(|rest| rest.chars().next()).map_or(1, char::len_utf8);
                at += width;
                out.push(start..at, Kind::Operator);
            }
        }
    }
}

/// The spans of one line of math source.
pub fn spans(text: &str) -> Vec<(std::ops::Range<usize>, Kind)> {
    let mut out = Out::default();
    line(text, &mut out);
    out.finish()
}

/// A LaTeX command's argument styles, by its name (the mode's plugins): each bracket's style.
fn plugin(name: &str) -> Option<&'static [Option<Kind>]> {
    Some(match name {
        "importmodule" => &[Some(Kind::String), Some(Kind::Property)],
        "documentclass" => &[None, None],
        "usepackage" | "begin" | "end" | "label" | "ref" | "eqref" | "cite" | "bibitem" | "Bibitem" | "RBibitem" => &[None],
        _ => return None,
    })
}

/// A command being read: its argument styles (none: the mode's DEFAULT) and its brackets so far.
#[derive(Clone, Debug)]
struct Command {
    styles: Option<&'static [Option<Kind>]>,
    brackets: usize,
}

/// What the LaTeX reader reads next: text, a command's arguments, or math to its delimiter.
#[derive(Clone, Debug, Default)]
enum Reading {
    #[default]
    Text,
    Arguments,
    Math(&'static str),
}

#[derive(Clone, Debug, Default)]
pub struct Latex {
    commands: Vec<Command>,
    reading: Reading,
}

/// LaTeX, as CodeMirror 5's stex mode colors it: commands tags, their brackets, `$`, `\[`, and
/// `\(` math (keyword delimiters, math inside), `%` comments.
pub fn latex(text: &str, state: &mut Latex, out: &mut Out) {
    if text.is_empty() {
        // A blank line ends what was open.
        *state = Latex::default();
        return;
    }
    super::scan::run(text, out, |stream| match state.reading {
        Reading::Text => latex_text(stream, state),
        Reading::Arguments => latex_arguments(stream, state),
        Reading::Math(end) => latex_math(stream, state, end),
    });
}

fn latex_text(stream: &mut Stream, state: &mut Latex) -> Option<Kind> {
    let letter = |byte: u8| byte.is_ascii_alphabetic() || byte == b'@';
    if stream.peek() == b'\\' && letter(stream.look(1)) {
        stream.pos += 1;
        stream.eat_while(letter);
        let name = stream.current().get(1..).unwrap_or("");
        state.commands.push(Command { styles: plugin(name), brackets: 0 });
        state.reading = Reading::Arguments;
        return Some(Kind::Operator);
    }
    if stream.peek() == b'\\' && b"$&%#{}_,;!/\\".contains(&stream.look(1)) {
        stream.pos += 2;
        return Some(Kind::Operator);
    }
    for (open, close) in [("\\[", "\\]"), ("\\(", "\\)"), ("$$", "$$"), ("$", "$")] {
        if stream.take(open) {
            state.reading = Reading::Math(close);
            return Some(Kind::Keyword);
        }
    }
    let here = stream.next()?;
    match here {
        b'%' => {
            stream.skip_to_end();
            Some(Kind::Comment)
        }
        b'}' | b']' => {
            if state.commands.is_empty() {
                return Some(Kind::Operator);
            }
            state.reading = Reading::Arguments;
            Some(Kind::Punctuation)
        }
        b'{' | b'[' => {
            state.commands.push(Command { styles: None, brackets: 0 });
            Some(Kind::Punctuation)
        }
        b'0'..=b'9' => {
            stream.eat_while(|byte| scan::word(byte) || matches!(byte, b'.' | b'%'));
            None
        }
        _ => {
            stream.eat_while(|byte| scan::word(byte) || byte == b'-');
            // The innermost command with argument styles styles its argument.
            let command = state.commands.iter().rev().find(|command| command.styles.is_some());
            command.and_then(|command| command.styles?.get(command.brackets.checked_sub(1)?).copied().flatten())
        }
    }
}

/// After a command's name: its brackets, or the text going on (the command done).
fn latex_arguments(stream: &mut Stream, state: &mut Latex) -> Option<Kind> {
    let here = stream.peek();
    if here == b'{' || here == b'[' {
        if let Some(command) = state.commands.last_mut()
            && command.styles.is_some()
        {
            command.brackets += 1;
        }
        stream.pos += 1;
        state.reading = Reading::Text;
        return Some(Kind::Punctuation);
    }
    if matches!(here, b' ' | b'\t' | b'\r') {
        stream.pos += 1;
        return None;
    }
    state.reading = Reading::Text;
    state.commands.pop();
    latex_text(stream, state)
}

/// Math, to its closing delimiter.
fn latex_math(stream: &mut Stream, state: &mut Latex, end: &'static str) -> Option<Kind> {
    if stream.eat_space() {
        return None;
    }
    if stream.take(end) {
        state.reading = Reading::Text;
        return Some(Kind::Keyword);
    }
    let here = stream.peek();
    let next = stream.look(1);
    if here == b'\\' && (next.is_ascii_alphabetic() || next == b'@') {
        stream.pos += 1;
        stream.eat_while(|byte| byte.is_ascii_alphabetic() || byte == b'@');
        return Some(Kind::Operator);
    }
    if here.is_ascii_alphabetic() {
        stream.eat_while(|byte| byte.is_ascii_alphabetic());
        return Some(Kind::Variable);
    }
    if here == b'\\' && b"$&%#{}_,;!/".contains(&next) {
        stream.pos += 2;
        return Some(Kind::Operator);
    }
    if b"^_&".contains(&here) {
        stream.pos += 1;
        return Some(Kind::Operator);
    }
    if b"+-<>|=,/@!*:;'\"`~#?".contains(&here) {
        stream.pos += 1;
        return None;
    }
    // `\d+\.\d*|\d*\.\d+|\d+`
    let digits = |from: usize| (from..).find(|index| !stream.bytes.get(*index).is_some_and(u8::is_ascii_digit)).unwrap_or(from);
    let whole = digits(stream.pos);
    let end_of_number =
        if stream.bytes.get(whole) == Some(&b'.') && (whole > stream.pos || digits(whole + 1) > whole + 1) { digits(whole + 1) } else { whole };
    if end_of_number > stream.pos {
        stream.pos = end_of_number;
        return Some(Kind::Number);
    }
    stream.pos += 1;
    match here {
        b'{' | b'}' | b'[' | b']' | b'(' | b')' => Some(Kind::Punctuation),
        b'%' => {
            stream.skip_to_end();
            Some(Kind::Comment)
        }
        _ => {
            // The mode's error: a whole character.
            while stream.peek() >= 0x80 && stream.peek() < 0xC0 {
                stream.pos += 1;
            }
            Some(Kind::Operator)
        }
    }
}
