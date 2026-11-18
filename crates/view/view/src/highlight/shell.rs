//! Shell, as CodeMirror 5's shell mode colors it: keywords, common commands builtins, `NAME=` a
//! definition, an option (`-v`, `--all`) an attribute, numbers. Strings, `$name`, `${…}`, and
//! `$(…)` nest inside one another through a stack of tokenizers, as the mode's do: a string's
//! text is a string, an expansion's a definition (the text's color), a `$(…)`'s plain.

use super::Kind;
use super::scan::Out;

/// A tokenizer on the stack (the top reads next).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tokenizer {
    /// After a `$`: what it expands.
    Dollar,
    /// Inside a string or an expansion, to `close`; `quote` is what opened it.
    Text { quote: u8, style: Option<Kind> },
    /// A string about to open with `quote`.
    TextStart { quote: u8 },
}

#[derive(Clone, Debug, Default)]
pub struct Shell {
    tokens: Vec<Tokenizer>,
}

const KEYWORDS: &[&str] = &[
    "if", "then", "do", "else", "elif", "while", "until", "for", "in", "esac", "fi", "fin", "fil", "done", "exit", "set", "unset", "export",
    "function",
];
const COMMANDS: &[&str] = &[
    "ab", "awk", "bash", "beep", "cat", "cc", "cd", "chown", "chmod", "chroot", "clear", "cp", "curl", "cut", "diff", "echo", "find", "gawk", "gcc",
    "get", "git", "grep", "hg", "kill", "killall", "ln", "ls", "make", "mkdir", "openssl", "mv", "nc", "nl", "node", "npm", "ping", "ps", "restart",
    "rm", "rmdir", "sed", "service", "sh", "shopt", "shred", "source", "sort", "sleep", "ssh", "start", "stop", "su", "sudo", "svn", "tee", "telnet",
    "top", "touch", "vi", "vim", "wall", "wc", "wget", "who", "write", "yes", "zsh",
];

/// Pops the stack's top tokenizer.
fn shift(shell: &mut Shell) {
    if !shell.tokens.is_empty() {
        shell.tokens.remove(0);
    }
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// A line read as CodeMirror's stream: a position, and the token read so far.
struct Stream<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Stream<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn eat(&mut self, wanted: u8) -> bool {
        if self.peek() == Some(wanted) {
            self.pos += 1;
            return true;
        }
        false
    }

    fn eat_while(&mut self, test: impl Fn(u8) -> bool) {
        while self.peek().is_some_and(&test) {
            self.pos += 1;
        }
    }
}

pub fn line(text: &str, shell: &mut Shell, out: &mut Out) {
    let mut stream = Stream { text, bytes: text.as_bytes(), pos: 0 };
    while stream.pos < stream.bytes.len() {
        let start = stream.pos;
        let style = tokenize(&mut stream, shell, start == 0);
        if stream.pos == start {
            stream.pos += 1;
        }
        out.mark(start..stream.pos, style);
    }
}

fn tokenize(stream: &mut Stream, shell: &mut Shell, sol: bool) -> Option<Kind> {
    match shell.tokens.first().copied() {
        None => base(stream, shell, sol),
        Some(Tokenizer::Dollar) => dollar(stream, shell),
        Some(Tokenizer::Text { quote, style }) => string(stream, shell, quote, style, sol),
        Some(Tokenizer::TextStart { quote }) => {
            if let Some(first) = shell.tokens.first_mut() {
                *first = Tokenizer::Text { quote, style: Some(Kind::String) };
            }
            stream.next();
            tokenize(stream, shell, sol)
        }
    }
}

fn base(stream: &mut Stream, shell: &mut Shell, sol: bool) -> Option<Kind> {
    if stream.peek().is_some_and(|byte| byte == b' ' || byte == b'\t' || byte == b'\r') {
        stream.eat_while(|byte| byte == b' ' || byte == b'\t' || byte == b'\r');
        return None;
    }
    let start = stream.pos;
    let here = stream.next()?;
    match here {
        b'\\' => {
            stream.next();
            None
        }
        b'\'' | b'"' | b'`' => {
            let style = if here == b'`' { None } else { Some(Kind::String) };
            shell.tokens.insert(0, Tokenizer::Text { quote: here, style });
            tokenize(stream, shell, sol)
        }
        b'#' => {
            if sol && stream.eat(b'!') {
                stream.pos = stream.bytes.len();
                return Some(Kind::Comment);
            }
            stream.pos = stream.bytes.len();
            Some(Kind::Comment)
        }
        b'$' => {
            shell.tokens.insert(0, Tokenizer::Dollar);
            tokenize(stream, shell, sol)
        }
        b'+' | b'=' => Some(Kind::Operator),
        b'-' => {
            stream.eat(b'-');
            stream.eat_while(word);
            Some(Kind::Property)
        }
        b'<' if stream.text.get(stream.pos..).is_some_and(|rest| rest.starts_with("<<")) => {
            stream.pos += 2;
            Some(Kind::Operator)
        }
        _ => {
            if here.is_ascii_digit() {
                stream.eat_while(|byte| byte.is_ascii_digit());
                if stream.peek().is_none_or(|byte| !word(byte)) {
                    return Some(Kind::Number);
                }
            }
            stream.eat_while(|byte| word(byte) || byte == b'-');
            let current = stream.text.get(start..stream.pos).unwrap_or("");
            if stream.peek() == Some(b'=') && current.bytes().any(word) {
                return None;
            }
            if KEYWORDS.contains(&current) {
                Some(Kind::Keyword)
            } else if COMMANDS.contains(&current) {
                Some(Kind::Property)
            } else {
                None
            }
        }
    }
}

/// Inside a string or an expansion opened by `quote`, to its close: a `$` there hands over to
/// an expansion, a quote of the other kind (inside an expansion) to a string.
fn string(stream: &mut Stream, shell: &mut Shell, quote: u8, style: Option<Kind>, sol: bool) -> Option<Kind> {
    let close = match quote {
        b'(' => b')',
        b'{' => b'}',
        other => other,
    };
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == close && !escaped {
            shift(shell);
            break;
        } else if next == b'$' && !escaped && quote != b'\'' && stream.peek() != Some(close) {
            stream.pos -= 1;
            shell.tokens.insert(0, Tokenizer::Dollar);
            break;
        } else if !escaped && quote != close && next == quote {
            shell.tokens.insert(0, Tokenizer::Text { quote, style });
            return tokenize(stream, shell, sol);
        } else if !escaped && (next == b'\'' || next == b'"') && quote != b'\'' && quote != b'"' {
            shell.tokens.insert(0, Tokenizer::TextStart { quote: next });
            stream.pos -= 1;
            break;
        }
        escaped = !escaped && next == b'\\';
    }
    style
}

/// After a `$`: `$(…)` and `${…}` and `$"…"` open what they expand; `$name` and `$1` are read.
fn dollar(stream: &mut Stream, shell: &mut Shell) -> Option<Kind> {
    if shell.tokens.len() > 1 {
        stream.eat(b'$');
    }
    let Some(here) = stream.next() else {
        shift(shell);
        return None;
    };
    if matches!(here, b'\'' | b'"' | b'(' | b'{') {
        let style = match here {
            b'(' | b'{' => None,
            _ => Some(Kind::String),
        };
        if let Some(first) = shell.tokens.first_mut() {
            *first = Tokenizer::Text { quote: here, style };
        }
        return tokenize(stream, shell, false);
    }
    if !here.is_ascii_digit() {
        stream.eat_while(word);
    }
    shift(shell);
    None
}
