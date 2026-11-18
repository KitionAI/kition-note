//! Erlang, as CodeMirror 5's erlang mode colors it, ported token for token: attributes
//! (`-module`), a call's name a tag (a BIF a builtin, a type test a guard), `Name/2` meta,
//! variables, atoms (plain), `?MACRO`s, records, numbers (`16#ff`, `$c`), strings, comments. Its
//! stack of open tokens decides where an attribute can start and whether `m:f(` calls a BIF.

use super::Kind;
use super::scan::{Out, Stream};

const BUILTINS: &str = "abs adler32 adler32_combine alive apply atom_to_binary atom_to_list binary_to_atom binary_to_existing_atom \
binary_to_list binary_to_term bit_size bitstring_to_list byte_size check_process_code contact_binary crc32 \
crc32_combine date decode_packet delete_module disconnect_node element erase exit float float_to_list \
garbage_collect get get_keys group_leader halt hd integer_to_list internal_bif iolist_size iolist_to_binary \
is_alive is_atom is_binary is_bitstring is_boolean is_float is_function is_integer is_list is_number is_pid \
is_port is_process_alive is_record is_reference is_tuple length link list_to_atom list_to_binary \
list_to_bitstring list_to_existing_atom list_to_float list_to_integer list_to_pid list_to_tuple load_module \
make_ref module_loaded monitor_node node node_link node_unlink nodes notalive now open_port pid_to_list \
port_close port_command port_connect port_control pre_loaded process_flag process_info processes purge_module \
put register registered round self setelement size spawn spawn_link spawn_monitor spawn_opt split_binary \
statistics term_to_binary time throw tl trunc tuple_size tuple_to_list unlink unregister whereis";
const GUARDS: &str = "is_atom is_binary is_bitstring is_boolean is_float is_function is_integer is_list is_number is_pid is_port \
is_record is_reference is_tuple atom binary bitstring boolean function integer list number pid port record \
reference tuple";
const TYPES: &[&str] = &["-type", "-spec", "-export_type", "-opaque"];
const KEYWORDS: &[&str] = &["after", "begin", "catch", "case", "cond", "end", "fun", "if", "let", "of", "query", "receive", "try", "when"];
const OPERATOR_WORDS: &[&str] = &["and", "andalso", "band", "bnot", "bor", "bsl", "bsr", "bxor", "div", "not", "or", "orelse", "rem", "xor"];
const SEPARATORS: &[&str] = &["->", ";", ","];
const OPERATORS: &[&str] = &["=", "+", "-", "*", "/", ">", ">=", "<", "=<", "=:=", "==", "=/=", "/=", "||", "<-", "!"];
const OPENS: &[&str] = &["<<", "(", "[", "{"];
const CLOSES: &[&str] = &["}", "]", ")", ">>"];

/// A token on the stack: its text and its type.
#[derive(Clone, Debug)]
struct Token {
    text: String,
    kind: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct Erlang {
    stack: Vec<Token>,
    string: bool,
    atom: bool,
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'@' || byte >= 0x80
}

/// Reads to `quote` (a backslash escaping the next byte): whether it closed.
fn quoted(stream: &mut Stream, quote: u8) -> bool {
    while let Some(here) = stream.next() {
        if here == quote {
            return true;
        }
        if here == b'\\' {
            stream.next();
        }
    }
    false
}

/// Whether `\s*/\s*[0-9]` follows (an arity: `name/2`), and where it ends.
fn arity(stream: &Stream) -> Option<usize> {
    let bytes = stream.bytes;
    let mut at = stream.pos;
    let skip = |at: &mut usize| {
        while matches!(bytes.get(*at), Some(b' ' | b'\t')) {
            *at += 1;
        }
    };
    skip(&mut at);
    if bytes.get(at) != Some(&b'/') {
        return None;
    }
    at += 1;
    skip(&mut at);
    bytes.get(at).is_some_and(u8::is_ascii_digit).then_some(at + 1)
}

/// The next byte after spaces.
fn next_significant(stream: &Stream) -> u8 {
    stream.bytes.get(stream.pos..).and_then(|rest| rest.iter().find(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))).copied().unwrap_or(0)
}

pub fn line(text: &str, state: &mut Erlang, out: &mut Out) {
    super::scan::run(text, out, |stream| {
        let kind = token(stream, state);
        push(state, stream.current(), kind);
        style(kind)
    });
}

fn style(kind: &str) -> Option<Kind> {
    match kind {
        "attribute" | "builtin" | "guard" => Some(Kind::Property),
        "comment" | "fun" => Some(Kind::Comment),
        "error" | "function" | "operator" => Some(Kind::Operator),
        "keyword" => Some(Kind::Keyword),
        "macro" | "variable" => Some(Kind::Variable),
        "number" => Some(Kind::Number),
        "record" => Some(Kind::Punctuation),
        "string" => Some(Kind::String),
        _ => None,
    }
}

/// One token's type (the mode's `tokenizer`).
fn token(stream: &mut Stream, state: &mut Erlang) -> &'static str {
    if state.string {
        state.string = !quoted(stream, b'"');
        return "string";
    }
    if state.atom {
        state.atom = !quoted(stream, b'\'');
        return "atom";
    }
    if stream.eat_space() {
        return "whitespace";
    }
    let bytes = stream.bytes;
    // An attribute, where a form starts: `-name`.
    if state.stack.is_empty() && stream.peek() == b'-' {
        let mut at = stream.pos + 1;
        while matches!(bytes.get(at), Some(b' ' | b'\t')) {
            at += 1;
        }
        if bytes.get(at).is_some_and(|byte| byte.is_ascii_lowercase() || *byte >= 0x80) {
            while bytes.get(at).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte >= 0x80) {
                at += 1;
            }
            stream.pos = at;
            return if TYPES.contains(&stream.current()) { "type" } else { "attribute" };
        }
    }
    let Some(here) = stream.next() else { return "whitespace" };
    match here {
        b'%' => {
            stream.skip_to_end();
            return "comment";
        }
        b':' => return "colon",
        b'?' => {
            stream.eat_space();
            stream.eat_while(word);
            return "macro";
        }
        b'#' => {
            stream.eat_space();
            stream.eat_while(word);
            return "record";
        }
        b'$' => {
            if stream.next() != Some(b'\\') {
                return "number";
            }
            return if escape(stream) { "number" } else { "error" };
        }
        b'.' => return "dot",
        b'\'' => {
            state.atom = !quoted(stream, b'\'');
            if !state.atom {
                if let Some(end) = arity(stream) {
                    stream.pos = end;
                    return "fun";
                }
                if matches!(next_significant(stream), b'(' | b':') {
                    return "function";
                }
            }
            return "atom";
        }
        b'"' => {
            state.string = !quoted(stream, b'"');
            return "string";
        }
        _ => {}
    }
    if here.is_ascii_uppercase() || here == b'_' {
        stream.eat_while(word);
        return "variable";
    }
    if here.is_ascii_lowercase() || here >= 0x80 {
        stream.eat_while(word);
        if let Some(end) = arity(stream) {
            stream.pos = end;
            return "fun";
        }
        let name = stream.current();
        if KEYWORDS.contains(&name) {
            return "keyword";
        }
        if OPERATOR_WORDS.contains(&name) {
            return "operator";
        }
        let has = |list: &str| list.split_ascii_whitespace().any(|known| known == name);
        if next_significant(stream) == b'(' {
            let qualified = state.stack.last().is_some_and(|last| last.text == ":")
                && state.stack.len().checked_sub(2).and_then(|at| state.stack.get(at)).is_none_or(|before| before.text != "erlang");
            if !has(BUILTINS) || qualified {
                return if has(GUARDS) { "guard" } else { "function" };
            }
            return "builtin";
        }
        if next_significant(stream) == b':' {
            return if name == "erlang" { "builtin" } else { "function" };
        }
        return if matches!(name, "true" | "false") { "boolean" } else { "atom" };
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| byte.is_ascii_digit());
        if stream.eat(|byte| byte == b'#').is_some() {
            if !stream.eat_while(|byte| byte.is_ascii_alphanumeric()) {
                stream.back_up(1);
            }
        } else if stream.eat(|byte| byte == b'.').is_some() {
            if stream.eat_while(|byte| byte.is_ascii_digit()) {
                if stream.eat(|byte| byte == b'e' || byte == b'E').is_some() {
                    if stream.eat(|byte| byte == b'-' || byte == b'+').is_some() {
                        if !stream.eat_while(|byte| byte.is_ascii_digit()) {
                            stream.back_up(2);
                        }
                    } else if !stream.eat_while(|byte| byte.is_ascii_digit()) {
                        stream.back_up(1);
                    }
                }
            } else {
                stream.back_up(1);
            }
        }
        return "number";
    }
    if longest(stream, b"<([{", OPENS, true) {
        return "open_paren";
    }
    if longest(stream, b">)]}", CLOSES, true) {
        return "close_paren";
    }
    if longest(stream, b"->,;", SEPARATORS, false) {
        return "separator";
    }
    if longest(stream, b"+-*/<>=|:!", OPERATORS, false) {
        return "operator";
    }
    "none"
}

/// After `$\`: an escape the mode knows (`\n`, `\077`, `\^A`, `\x41`, `\x{41}`).
fn escape(stream: &mut Stream) -> bool {
    let bytes = stream.bytes;
    let at = stream.pos;
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let octal = (at..at + 3).take_while(|index| (b'0'..=b'7').contains(&byte(*index))).count();
    let end = if octal > 0 {
        at + octal
    } else if b"bdefnrstv\\\"'".contains(&byte(at)) && byte(at) != 0 {
        at + 1
    } else if byte(at) == b'^' && byte(at + 1).is_ascii_alphabetic() {
        at + 2
    } else if byte(at) == b'x' && byte(at + 1).is_ascii_alphanumeric() && byte(at + 2).is_ascii_alphanumeric() {
        at + 3
    } else if byte(at) == b'x' && byte(at + 1) == b'{' {
        let close = (at + 2..).find(|index| !byte(*index).is_ascii_alphanumeric()).unwrap_or(at + 2);
        if close > at + 2 && byte(close) == b'}' { close + 1 } else { return false }
    } else {
        return false;
    };
    stream.pos = end;
    true
}

/// The mode's bracket and operator readers: a one-byte token in `set` grown to the longest of
/// `known` (brackets: the first known prefix as it grows; operators: the longest run's longest
/// known prefix). Leaves one byte read when none is known.
fn longest(stream: &mut Stream, set: &[u8], known: &[&str], bracket: bool) -> bool {
    if stream.pos != stream.start + 1 || !set.contains(&stream.look(-1)) {
        return false;
    }
    if bracket {
        stream.back_up(1);
        while set.contains(&stream.peek()) && !stream.eol() {
            stream.pos += 1;
            if known.contains(&stream.current()) {
                return true;
            }
        }
        stream.pos = stream.start + 1;
        return false;
    }
    while set.contains(&stream.peek()) && !stream.eol() {
        stream.pos += 1;
    }
    // No known word is longer than the longest: backing down starts there.
    let longest = known.iter().map(|word| word.len()).max().unwrap_or(0);
    stream.pos = stream.pos.min(stream.start + longest);
    while stream.pos > stream.start {
        if known.contains(&stream.current()) {
            return true;
        }
        stream.pos -= 1;
    }
    stream.pos = stream.start + 1;
    false
}

/// Keeps the stack of open tokens as the mode does (comments and spaces stay off it).
fn push(state: &mut Erlang, text: &str, kind: &'static str) {
    if kind == "comment" || kind == "whitespace" {
        return;
    }
    let stack = &mut state.stack;
    let top = stack.len().saturating_sub(1);
    if top > 0 && stack.get(top).is_some_and(|last| last.kind == "record") && kind == "dot" {
        stack.pop();
    } else {
        if top > 0 && stack.get(top).is_some_and(|last| last.kind == "group") {
            stack.pop();
        }
        stack.push(Token { text: text.to_string(), kind });
    }
    let Some(last) = stack.last().cloned() else { return };
    if last.kind == "dot" {
        stack.clear();
        return;
    }
    let top = stack.len() - 1;
    if top > 1 && last.kind == "fun" && stack.get(top - 1).is_some_and(|before| before.text == "fun") {
        stack.truncate(top - 1);
        return;
    }
    let rules: &[(char, &[&str])] = match last.text.as_str() {
        "}" => &[('g', &["{"])],
        "]" => &[('i', &["["])],
        ")" => &[('i', &["("])],
        ">>" => &[('i', &["<<"])],
        "end" => &[('i', &["begin", "case", "fun", "if", "receive", "try"])],
        "," => &[('e', &["begin", "try", "when", "->", ",", "(", "[", "{", "<<"])],
        "->" => &[('r', &["when"]), ('m', &["try", "if", "case", "receive"])],
        ";" => &[('E', &["case", "fun", "if", "receive", "try", "when"])],
        "catch" => &[('e', &["try"])],
        "of" => &[('e', &["case"])],
        "after" => &[('e', &["receive", "try"])],
        _ => return,
    };
    let newest = stack.len() - 1;
    for (rule, tokens) in rules {
        if let Some(found) = (0..newest).rev().find(|index| stack.get(*index).is_some_and(|token| tokens.contains(&token.text.as_str()))) {
            let kept = stack.get(found).cloned();
            let last = stack.get(newest).cloned();
            stack.truncate(found);
            match rule {
                'm' => stack.extend(kept.into_iter().chain(last)),
                'r' => stack.extend(last),
                'g' => stack.push(Token { text: "group".to_string(), kind: "group" }),
                'E' | 'e' => stack.extend(kept),
                _ => {}
            }
            return;
        }
    }
    if rules.last().is_some_and(|(rule, _)| *rule == 'E') {
        stack.clear();
    }
}
