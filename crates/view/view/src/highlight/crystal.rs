//! Crystal, as CodeMirror 5's crystal mode colors it, ported token for token: keywords, a
//! definition's name after `def`, `class`, and the like, constants tags, `@ivars` variable-2,
//! `:symbols` and chars atoms, a name after `.` a property, numbers, strings and `%w( )` literals
//! with `#{…}` code, `{% %}` and `{{ }}` macros meta, `#` comments.

use super::Kind;
use super::scan::{Out, Stream};

const KEYWORDS: &[&str] = &[
    "abstract",
    "alias",
    "as",
    "asm",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "do",
    "else",
    "elsif",
    "end",
    "ensure",
    "enum",
    "extend",
    "for",
    "fun",
    "if",
    "include",
    "instance_sizeof",
    "lib",
    "macro",
    "module",
    "next",
    "of",
    "out",
    "pointerof",
    "private",
    "protected",
    "rescue",
    "return",
    "require",
    "select",
    "sizeof",
    "struct",
    "super",
    "then",
    "type",
    "typeof",
    "uninitialized",
    "union",
    "unless",
    "until",
    "when",
    "while",
    "with",
    "yield",
    "__DIR__",
    "__END_LINE__",
    "__FILE__",
    "__LINE__",
];
const ATOMS: &[&str] = &["true", "false", "nil", "self"];
const DEFINERS: &[&str] = &["def", "fun", "macro", "class", "module", "struct", "lib", "enum", "union", "do", "for"];
const OPENERS: &[&str] = &["if", "unless", "case", "while", "until", "begin", "then"];

/// A tokenizer on the mode's stack.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reader {
    /// `{% %}` (`%`) or `{{ }}` (`{`): a macro, past its opening once started.
    Macro(u8, bool),
    /// Brackets (or `#{`) read as code to their closing text; its style for the delimiters.
    Nest(&'static str, &'static str, Option<Kind>, bool),
    /// A method's name after `def` or `fun`.
    Method,
    /// A macro's name.
    MacroName,
    /// A type's name after `class`, `module`, and the like.
    TypeName,
    /// A string to its closing byte: its style, and whether `#{…}` and escapes count.
    Quoted(u8, Option<Kind>, bool),
    /// A heredoc to its terminator line: whether it interpolates.
    Heredoc(String, bool),
}

#[derive(Clone, Debug, Default)]
pub struct Crystal {
    readers: Vec<Reader>,
    last: String,
    /// The last styled token's style (`operator` as `Some(Some(Operator))`, none yet as `None`).
    last_style: Option<Option<Kind>>,
    blocks: Vec<String>,
    /// Whether this token counts as the last one though its style has no color (or is meta).
    remember: bool,
}

fn lower_start(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte == b'_' || byte >= 0x80
}

fn upper_start(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte == b'_' || byte >= 0x80
}

fn name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn take_name(stream: &mut Stream, start: fn(u8) -> bool) -> bool {
    if !start(stream.peek()) || stream.eol() {
        return false;
    }
    stream.eat_while(name);
    true
}

/// `[-+/%|&^]`, `**?`, `<<`, `>>`.
fn arithmetic(stream: &mut Stream) -> bool {
    let (a, b) = (stream.peek(), stream.look(1));
    if b"-+/%|&^".contains(&a) && a != 0 {
        stream.pos += 1;
    } else if a == b'*' {
        stream.pos += if b == b'*' { 2 } else { 1 };
    } else if (a == b'<' || a == b'>') && (b == b'<' || b == b'>') {
        stream.pos += 2;
    } else {
        return false;
    }
    true
}

/// `=~`, `!~`, `===`, `<=>`, `<`, `<=`, `>`, `>=`, `=`, `==`, `!`, `!=`, `||`, `&&`, `~`.
fn comparison(stream: &mut Stream) -> bool {
    for operator in ["=~", "!~", "===", "<=>"] {
        if stream.take(operator) {
            return true;
        }
    }
    let (a, b) = (stream.peek(), stream.look(1));
    if b"<>=!".contains(&a) && a != 0 {
        stream.pos += if b == b'=' { 2 } else { 1 };
    } else if (a == b'|' || a == b'&') && b == a {
        stream.pos += 2;
    } else if a == b'~' {
        stream.pos += 1;
    } else {
        return false;
    }
    true
}

/// `[]`, `[]?`, `[]=`.
fn index_operator(stream: &mut Stream) -> bool {
    if !stream.take("[]") {
        return false;
    }
    stream.eat(|byte| byte == b'?' || byte == b'=');
    true
}

pub fn line(text: &str, state: &mut Crystal, out: &mut Out) {
    super::scan::run(text, out, |stream| {
        // A reader may read nothing (a `def` before `{{`): the mode reads again, as CodeMirror
        // allows a few times.
        let mut style = None;
        for _ in 0..10 {
            state.remember = false;
            style = read(stream, state);
            // A styled token is the last one (comments aside); so is one whose style has no
            // color here (a definition, an atom, meta), which its reader marks.
            if (style.is_some() && style != Some(Kind::Comment)) || state.remember {
                state.last = stream.current().to_string();
                state.last_style = Some(style);
            }
            if stream.pos > stream.start {
                break;
            }
        }
        style
    });
}

fn read(stream: &mut Stream, state: &mut Crystal) -> Option<Kind> {
    match state.readers.last().cloned() {
        None => base(stream, state),
        Some(Reader::Macro(kind, started)) => {
            let open = if kind == b'%' { "{%" } else { "{{" };
            let close = if kind == b'%' { "%}" } else { "}}" };
            if !started && stream.take(open) {
                if let Some(last) = state.readers.last_mut() {
                    *last = Reader::Macro(kind, true);
                }
                return remember(state, Some(Kind::Comment));
            }
            if stream.take(close) {
                state.readers.pop();
                return remember(state, Some(Kind::Comment));
            }
            base(stream, state)
        }
        Some(Reader::Nest(open, close, style, started)) => {
            if !started && stream.take(open) {
                if let Some(last) = state.readers.last_mut() {
                    *last = Reader::Nest(open, close, style, true);
                }
                return if style.is_some() { remember(state, style) } else { None };
            }
            let got = base(stream, state);
            if stream.current() == close {
                state.readers.pop();
                state.remember = style.is_some();
                return style;
            }
            got
        }
        Some(Reader::Method) => {
            if stream.eat_space() {
                return None;
            }
            if take_name(stream, lower_start) {
                stream.eat(|byte| byte == b'!' || byte == b'?');
            } else if !arithmetic(stream) && !comparison(stream) {
                index_operator(stream);
            }
            state.readers.pop();
            remember(state, None)
        }
        Some(Reader::MacroName) => {
            if stream.eat_space() {
                return None;
            }
            if take_name(stream, lower_start) {
                if stream.current() == "def" {
                    return Some(Kind::Keyword);
                }
                stream.eat(|byte| byte == b'?' || byte == b'!');
            }
            state.readers.pop();
            remember(state, None)
        }
        Some(Reader::TypeName) => {
            if stream.eat_space() {
                return None;
            }
            take_name(stream, upper_start);
            state.readers.pop();
            remember(state, None)
        }
        Some(Reader::Quoted(close, style, interpolates)) => quoted(stream, state, close, style, interpolates),
        Some(Reader::Heredoc(end, interpolates)) => heredoc(stream, state, &end, interpolates),
    }
}

/// A token the mode styles though its style has no color here (or is meta): still the last one.
fn remember(state: &mut Crystal, style: Option<Kind>) -> Option<Kind> {
    state.remember = true;
    style
}

fn base(stream: &mut Stream, state: &mut Crystal) -> Option<Kind> {
    if stream.eat_space() {
        return None;
    }
    if state.last != "\\" && stream.at("{%") {
        state.readers.push(Reader::Macro(b'%', false));
        return read(stream, state);
    }
    if state.last != "\\" && stream.at("{{") {
        state.readers.push(Reader::Macro(b'{', false));
        return read(stream, state);
    }
    if stream.peek() == b'#' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if take_name(stream, lower_start) {
        stream.eat(|byte| byte == b'?' || byte == b'!');
        let word = stream.current().to_string();
        let bare = word.trim_end_matches(['?', '!']);
        if stream.eat(|byte| byte == b':').is_some() {
            return remember(state, None);
        }
        if state.last == "." {
            return Some(Kind::Property);
        }
        if KEYWORDS.contains(&bare) {
            if DEFINERS.contains(&bare) {
                let lib_fun = bare == "fun" && state.blocks.iter().any(|block| block == "lib");
                let abstract_def = bare == "def" && state.last == "abstract";
                if !lib_fun && !abstract_def {
                    state.blocks.push(word.clone());
                }
            } else if matches!(state.last_style, None | Some(Some(Kind::Operator))) && OPENERS.contains(&bare) {
                state.blocks.push(word.clone());
            } else if word == "end" {
                state.blocks.pop();
            }
            match word.as_str() {
                "def" | "fun" => state.readers.push(Reader::Method),
                "macro" => state.readers.push(Reader::MacroName),
                "class" | "module" | "struct" | "lib" | "enum" | "union" => state.readers.push(Reader::TypeName),
                _ => {}
            }
            return Some(Kind::Keyword);
        }
        if ATOMS.contains(&bare) {
            return remember(state, None);
        }
        return Some(Kind::Variable);
    }
    if stream.eat(|byte| byte == b'@').is_some() {
        if stream.peek() == b'[' {
            state.readers.push(Reader::Nest("[", "]", Some(Kind::Comment), false));
            return read(stream, state);
        }
        stream.eat(|byte| byte == b'@');
        if !take_name(stream, lower_start) {
            take_name(stream, upper_start);
        }
        return Some(Kind::Variable);
    }
    if take_name(stream, upper_start) {
        return Some(Kind::Operator);
    }
    if stream.eat(|byte| byte == b':').is_some() {
        if stream.eat(|byte| byte == b'"').is_some() {
            state.readers.push(Reader::Quoted(b'"', None, false));
            return read(stream, state);
        }
        if take_name(stream, lower_start) || take_name(stream, upper_start) || arithmetic(stream) || comparison(stream) || index_operator(stream) {
            return remember(state, None);
        }
        stream.eat(|byte| byte == b':');
        return Some(Kind::Operator);
    }
    if stream.eat(|byte| byte == b'"').is_some() {
        state.readers.push(Reader::Quoted(b'"', Some(Kind::String), true));
        return read(stream, state);
    }
    if stream.peek() != b'%' {
        return other(stream, state);
    }
    // `%r(…)`, `%w(…)`, `%q(…)`, `%(…)`, `%name`, `%`.
    let (close, style, interpolates) = if stream.take("%r") {
        (stream.next().unwrap_or(0), Some(Kind::String), true)
    } else if stream.take("%w") || stream.take("%q") {
        (stream.next().unwrap_or(0), Some(Kind::String), false)
    } else if !scan_word_or_space(stream.look(1)) && stream.look(1) != b'=' && stream.look(1) != 0 {
        stream.pos += 1;
        (stream.next().unwrap_or(0), Some(Kind::String), true)
    } else if lower_start(stream.look(1)) || stream.look(1).is_ascii_alphabetic() {
        stream.pos += 1;
        stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80);
        return remember(state, Some(Kind::Comment));
    } else {
        stream.pos += 1;
        return Some(Kind::Operator);
    };
    let close = match close {
        b'[' => b']',
        b'{' => b'}',
        b'(' => b')',
        b'<' => b'>',
        other => other,
    };
    state.readers.push(Reader::Quoted(close, style, interpolates));
    read(stream, state)
}

fn scan_word_or_space(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || matches!(byte, b' ' | b'\t' | b'\r')
}

/// What `base` reads when no `%` comes: heredocs, chars, numbers, operators, brackets.
fn other(stream: &mut Stream, state: &mut Crystal) -> Option<Kind> {
    let bytes = stream.bytes;
    if stream.at("<<-") {
        let quoted = stream.look(3) == b'\'';
        let from = stream.pos + 3 + usize::from(quoted);
        if bytes.get(from).is_some_and(u8::is_ascii_uppercase) {
            let end = (from..).find(|index| !bytes.get(*index).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')).unwrap_or(from);
            if !quoted || bytes.get(end) == Some(&b'\'') {
                let terminator = String::from_utf8_lossy(bytes.get(from..end).unwrap_or_default()).to_string();
                stream.pos = end + usize::from(quoted);
                state.readers.push(Reader::Heredoc(terminator, !quoted));
                return read(stream, state);
            }
        }
    }
    if stream.eat(|byte| byte == b'\'').is_some() {
        // One character or an escape, then the quote.
        if stream.peek() == b'\\' {
            let next = stream.look(1);
            if b"befnrtv0'\"".contains(&next) && next != 0 {
                stream.pos += 2;
            } else if (b'0'..=b'7').contains(&next) && (b'0'..=b'7').contains(&stream.look(2)) && (b'0'..=b'7').contains(&stream.look(3)) {
                stream.pos += 4;
            } else if next == b'u' {
                let from = stream.pos + 2;
                let hex = (from..from + 4).take_while(|index| bytes.get(*index).is_some_and(u8::is_ascii_hexdigit)).count();
                if hex == 4 {
                    stream.pos = from + 4;
                } else if bytes.get(from) == Some(&b'{') {
                    let digits = (from + 1..from + 7).take_while(|index| bytes.get(*index).is_some_and(u8::is_ascii_hexdigit)).count();
                    if digits > 0 && bytes.get(from + 1 + digits) == Some(&b'}') {
                        stream.pos = from + 2 + digits;
                    }
                }
            }
        } else if stream.peek() != b'\'' && !stream.eol() {
            stream.next();
            while (0x80..0xC0).contains(&stream.peek()) {
                stream.pos += 1;
            }
        }
        stream.eat(|byte| byte == b'\'');
        return remember(state, None);
    }
    if stream.eat(|byte| byte == b'0').is_some() {
        if stream.eat(|byte| byte == b'x').is_some() {
            stream.eat_while(|byte| byte.is_ascii_hexdigit() || byte == b'_');
        } else if stream.eat(|byte| byte == b'o').is_some() {
            stream.eat_while(|byte| (b'0'..=b'7').contains(&byte) || byte == b'_');
        } else if stream.eat(|byte| byte == b'b').is_some() {
            stream.eat_while(|byte| byte == b'0' || byte == b'1' || byte == b'_');
        }
        return Some(Kind::Number);
    }
    if stream.eat(|byte| byte.is_ascii_digit()).is_some() {
        stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'_');
        if stream.peek() == b'.' && (stream.look(1).is_ascii_digit() || stream.look(1) == b'_') {
            stream.pos += 1;
            stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'_');
        }
        if matches!(stream.peek(), b'e' | b'E') {
            let sign = usize::from(matches!(stream.look(1), b'+' | b'-'));
            if stream.bytes.get(stream.pos + 1 + sign).is_some_and(u8::is_ascii_digit) {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
        return Some(Kind::Number);
    }
    if arithmetic(stream) {
        stream.eat(|byte| byte == b'=');
        return Some(Kind::Operator);
    }
    if comparison(stream) || stream.take("...") || stream.take(".") || stream.take("->") || stream.eat(|byte| byte == b'?' || byte == b':').is_some()
    {
        return Some(Kind::Operator);
    }
    if let Some(open) = [("(", ")"), ("{", "}"), ("[", "]")].iter().find(|(open, _)| stream.at(open)) {
        state.readers.push(Reader::Nest(open.0, open.1, None, false));
        return read(stream, state);
    }
    if stream.eat(|byte| byte == b'\\').is_some() {
        stream.next();
        return remember(state, Some(Kind::Comment));
    }
    stream.next();
    while (0x80..0xC0).contains(&stream.peek()) {
        stream.pos += 1;
    }
    None
}

fn quoted(stream: &mut Stream, state: &mut Crystal, close: u8, style: Option<Kind>, interpolates: bool) -> Option<Kind> {
    // An atom's string (`:"…"`) has no color but is styled.
    state.remember = true;
    let mut escaped = false;
    while !stream.eol() {
        if escaped {
            stream.next();
            escaped = false;
            continue;
        }
        if stream.at("{%") {
            state.readers.push(Reader::Macro(b'%', false));
            return style;
        }
        if stream.at("{{") {
            state.readers.push(Reader::Macro(b'{', false));
            return style;
        }
        if interpolates && stream.at("#{") {
            state.readers.push(Reader::Nest("#{", "}", Some(Kind::Comment), false));
            return style;
        }
        let here = stream.next().unwrap_or(0);
        if here == close {
            state.readers.pop();
            return style;
        }
        escaped = interpolates && here == b'\\';
    }
    style
}

fn heredoc(stream: &mut Stream, state: &mut Crystal, end: &str, interpolates: bool) -> Option<Kind> {
    if stream.sol() {
        stream.eat_space();
        if stream.take(end) {
            state.readers.pop();
            return Some(Kind::String);
        }
    }
    let mut escaped = false;
    while !stream.eol() {
        if escaped {
            stream.next();
            escaped = false;
            continue;
        }
        if stream.at("{%") {
            state.readers.push(Reader::Macro(b'%', false));
            return Some(Kind::String);
        }
        if stream.at("{{") {
            state.readers.push(Reader::Macro(b'{', false));
            return Some(Kind::String);
        }
        if interpolates && stream.at("#{") {
            state.readers.push(Reader::Nest("#{", "}", Some(Kind::Comment), false));
            return Some(Kind::String);
        }
        escaped = interpolates && stream.next() == Some(b'\\');
    }
    Some(Kind::String)
}
