//! HTTP messages, as CodeMirror 5's http mode colors them, ported token for token: the request
//! line's method and protocol keywords and its path a string, a status line's protocol and its
//! code (by class), header names plain and their values strings, and what breaks that shape an
//! error. Obsidian never runs the mode's blank line, so a body reads as more headers.

use super::Kind;
use super::scan::{self, Out, Stream};

/// What the mode reads next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Http {
    #[default]
    Start,
    Status,
    StatusText,
    Path,
    Protocol,
    Header,
}

pub fn line(text: &str, state: &mut Http, out: &mut Out) {
    scan::run(text, out, |stream| {
        if *state != Http::Header && stream.eat_space() {
            return None;
        }
        token(stream, state)
    });
}

fn failed(stream: &mut Stream, state: &mut Http) -> Option<Kind> {
    stream.skip_to_end();
    *state = Http::Header;
    Some(Kind::Operator)
}

/// `HTTP/\d\.\d` at the position: where it ends.
fn protocol(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    (stream.rest().starts_with(b"HTTP/")
        && stream.byte(at + 5).is_ascii_digit()
        && stream.byte(at + 6) == b'.'
        && stream.byte(at + 7).is_ascii_digit())
    .then_some(at + 8)
}

fn token(stream: &mut Stream, state: &mut Http) -> Option<Kind> {
    match *state {
        Http::Start => {
            if let Some(end) = protocol(stream) {
                stream.pos = end;
                *state = Http::Status;
                return Some(Kind::Keyword);
            }
            let end = (stream.pos..).find(|at| !stream.byte(*at).is_ascii_uppercase()).unwrap_or(stream.pos);
            if end > stream.pos && matches!(stream.byte(end), b' ' | b'\t') {
                stream.pos = end;
                *state = Http::Path;
                return Some(Kind::Keyword);
            }
            failed(stream, state)
        }
        Http::Status => {
            let end = (stream.pos..).find(|at| !stream.byte(*at).is_ascii_digit()).unwrap_or(stream.pos);
            if end == stream.pos {
                return failed(stream, state);
            }
            let code: u32 = stream
                .bytes
                .get(stream.pos..end)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| digits.parse().ok())
                .unwrap_or(0);
            stream.pos = end;
            *state = Http::StatusText;
            // `positive …` (1xx to 3xx), `negative …` (4xx, 5xx), else an error.
            Some(if (100..400).contains(&code) { Kind::String } else { Kind::Operator })
        }
        Http::StatusText => {
            stream.skip_to_end();
            *state = Http::Header;
            None
        }
        Http::Path => {
            stream.eat_while(|byte| !byte.is_ascii_whitespace());
            *state = Http::Protocol;
            Some(Kind::String)
        }
        Http::Protocol => {
            if protocol(stream) == Some(stream.bytes.len()) {
                stream.skip_to_end();
                *state = Http::Header;
                return Some(Kind::Keyword);
            }
            failed(stream, state)
        }
        Http::Header => {
            if stream.sol() && stream.eat(|byte| byte == b' ' || byte == b'\t').is_none() {
                return match stream.rest().iter().position(|byte| *byte == b':') {
                    Some(colon) => {
                        stream.pos += colon + 1;
                        None
                    }
                    None => {
                        stream.skip_to_end();
                        Some(Kind::Operator)
                    }
                };
            }
            stream.skip_to_end();
            Some(Kind::String)
        }
    }
}
