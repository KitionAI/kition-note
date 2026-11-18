//! Assorted languages, as their CodeMirror 5 modes color them, each ported token for token:
//! Yacas, MUMPS, Pig Latin, and FCL (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Assorted {
    /// An open block comment, an open string.
    comment: bool,
    string: Option<u8>,
    /// MUMPS: whether the line's label is being read, and where its command is.
    label: bool,
    command_mode: i8,
}

fn yacas_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'$' || byte == b'\''
}

fn yacas_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'$' || byte == b'\''
}

/// `[a-zA-Z$'][a-zA-Z0-9$']*` at `at`: where it ends.
fn yacas_name(stream: &Stream, at: usize) -> Option<usize> {
    yacas_name_start(stream.byte(at))
        .then(|| (at + 1..stream.bytes.len()).find(|end| !yacas_name_char(stream.byte(*end))).unwrap_or(stream.bytes.len()))
}

fn digits_from(stream: &Stream, at: usize) -> usize {
    (at..stream.bytes.len()).find(|end| !stream.byte(*end).is_ascii_digit()).unwrap_or(stream.bytes.len().max(at))
}

/// Yacas, as CodeMirror 5's yacas mode colors it: names variable-2 (a call's name a variable,
/// `x_y` patterns variable-3), `2 #` qualifiers strings, numbers, strings, brackets, symbols
/// operators, `//` and `/* */` comments.
pub fn yacas(text: &str, state: &mut Assorted, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return block_comment(stream, state);
        }
        if state.string.is_some() {
            return quoted(stream, state, b'"');
        }
        let at = stream.pos;
        match stream.next()? {
            b'"' => return quoted(stream, state, b'"'),
            b'/' if stream.eat(|byte| byte == b'*').is_some() => {
                state.comment = true;
                return block_comment(stream, state);
            }
            b'/' if stream.eat(|byte| byte == b'/').is_some() => {
                stream.skip_to_end();
                return Some(Kind::Comment);
            }
            _ => stream.pos = at,
        }
        // `\d+ *#`
        let digits = digits_from(stream, at);
        let hash = (digits..stream.bytes.len()).find(|end| stream.byte(*end) != b' ').unwrap_or(stream.bytes.len());
        if digits > at && stream.byte(hash) == b'#' {
            stream.pos = hash + 1;
            return Some(Kind::String);
        }
        if let Some(end) = yacas_number(stream) {
            stream.pos = end;
            return Some(Kind::Number);
        }
        // `name?_name`
        let name = yacas_name(stream, at);
        let blank = name.unwrap_or(at);
        if stream.byte(blank) == b'_'
            && let Some(end) = yacas_name(stream, blank + 1)
        {
            stream.pos = end;
            return Some(Kind::Variable);
        }
        if b"[]{}()".contains(&stream.peek()) {
            stream.next();
            return Some(Kind::Punctuation);
        }
        if let Some(end) = name {
            let open = (end..stream.bytes.len()).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(stream.bytes.len());
            stream.pos = if stream.byte(open) == b'(' { open } else { end };
            return Some(Kind::Variable);
        }
        stream.next();
        Some(Kind::Operator)
    });
}

/// `(?:\.\d+|\d+\.\d*|\d+)(?:[eE][+-]?\d+)?`
fn yacas_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let whole = digits_from(stream, at);
    let mantissa = if stream.byte(at) == b'.' {
        let end = digits_from(stream, at + 1);
        (end > at + 1).then_some(end)?
    } else if whole > at {
        if stream.byte(whole) == b'.' { digits_from(stream, whole + 1) } else { whole }
    } else {
        return None;
    };
    if matches!(stream.byte(mantissa), b'e' | b'E') {
        let sign = if matches!(stream.byte(mantissa + 1), b'+' | b'-') { mantissa + 2 } else { mantissa + 1 };
        let end = digits_from(stream, sign);
        if end > sign {
            return Some(end);
        }
    }
    Some(mantissa)
}

/// A string to its quote; open past the line only when the line ends inside it.
fn quoted(stream: &mut Stream, state: &mut Assorted, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    state.string = Some(quote);
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn block_comment(stream: &mut Stream, state: &mut Assorted) -> Option<Kind> {
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

/// MUMPS, as CodeMirror 5's mumps mode colors it: a line's label (to the first space) a tag,
/// commands variable-2 where a command may come, `$functions` builtins, names variables,
/// numbers, quotes strings, symbols operators, `;` comments, anything else an error.
pub fn mumps(text: &str, state: &mut Assorted, out: &mut Out) {
    scan::run(text, out, |stream| {
        let style = mumps_token(stream, state);
        if state.label { Some(Kind::Operator) } else { style }
    });
}

fn mumps_token(stream: &mut Stream, state: &mut Assorted) -> Option<Kind> {
    if stream.sol() {
        state.label = true;
        state.command_mode = 0;
    }
    let here = stream.peek();
    if here == b' ' || here == b'\t' {
        state.label = false;
        if state.command_mode == 0 {
            state.command_mode = 1;
        } else if state.command_mode < 0 || state.command_mode == 2 {
            state.command_mode = 0;
        }
    } else if here != b'.' && state.command_mode > 0 {
        state.command_mode = if here == b':' { -1 } else { 2 };
    }
    if here == b'(' || here == b'\t' {
        state.label = false;
    }
    if here == b';' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if let Some(end) = mumps_number(stream) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    if here == b'"' {
        // The mode's `skipTo('"')` finds the quote it stands on: the quote alone.
        stream.next();
        return Some(Kind::String);
    }
    let pair = stream.bytes.get(stream.pos..stream.pos + 2).unwrap_or_default();
    if matches!(pair, b"'=" | b"<=" | b">=" | b"'>" | b"'<" | b"[]" | b")]" | b"|]" | b"(]") {
        stream.pos += 2;
        return Some(Kind::Operator);
    }
    if b"+-*/&#!_?\\<>='[]".contains(&here) {
        stream.next();
        return Some(Kind::Operator);
    }
    if b".,:".contains(&here) {
        stream.next();
        return None;
    }
    if here == b'(' || here == b')' {
        stream.next();
        return Some(Kind::Punctuation);
    }
    if state.command_mode > 0 && stream.take_word_of(MUMPS_COMMANDS, true) {
        return Some(Kind::Variable);
    }
    if stream.take_word_of(MUMPS_FUNCTIONS, true) {
        return Some(Kind::Property);
    }
    if here == b'%' || here.is_ascii_alphabetic() {
        stream.next();
        stream.eat_while(|byte| byte.is_ascii_alphanumeric());
        return Some(Kind::Variable);
    }
    if here == b'$' || here == b'^' {
        stream.next();
        return Some(Kind::Property);
    }
    if here == b'@' {
        stream.next();
        return Some(Kind::String);
    }
    if scan::js_word(here) {
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'%');
        return Some(Kind::Variable);
    }
    stream.next();
    Some(Kind::Operator)
}

/// `[-+]?\d+(\.\d+)?([eE][-+]?\d+)?`
fn mumps_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let sign = if matches!(stream.byte(at), b'+' | b'-') { at + 1 } else { at };
    let whole = digits_from(stream, sign);
    if whole == sign {
        return None;
    }
    let mut end = whole;
    if stream.byte(end) == b'.' && stream.byte(end + 1).is_ascii_digit() {
        end = digits_from(stream, end + 1);
    }
    if matches!(stream.byte(end), b'e' | b'E') {
        let digits = if matches!(stream.byte(end + 1), b'+' | b'-') { end + 2 } else { end + 1 };
        let exponent = digits_from(stream, digits);
        if exponent > digits {
            end = exponent;
        }
    }
    Some(end)
}

fn pig_operator(byte: u8) -> bool {
    b"*+-%<>=&?:/!|".contains(&byte)
}

/// Pig Latin, as CodeMirror 5's pig mode colors it: keywords (any case, not before `)` or `.`),
/// builtins, types, and other names variables, numbers, symbols
/// operators, `--` and `/* */` comments, and quoted strings errors.
pub fn pig(text: &str, state: &mut Assorted, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return block_comment(stream, state);
        }
        if let Some(quote) = state.string {
            return pig_string(stream, state, quote);
        }
        let here = stream.next()?;
        match here {
            b'"' | b'\'' => pig_string(stream, state, here),
            b'[' | b']' | b'{' | b'}' | b'(' | b')' | b',' | b';' | b'.' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
                Some(Kind::Number)
            }
            b'/' if stream.eat(|byte| byte == b'*').is_some() => {
                state.comment = true;
                block_comment(stream, state)
            }
            b'-' if stream.eat(|byte| byte == b'-').is_some() => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if pig_operator(here) => {
                stream.eat_while(pig_operator);
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(|byte| scan::js_word(byte) || byte == b'$');
                let listed = |list: &str, word: &str| list.split_ascii_whitespace().any(|known| known == word);
                if listed(PIG_KEYWORDS, &stream.current().to_ascii_uppercase())
                    && stream.eat(|byte| byte == b')').is_none()
                    && stream.eat(|byte| byte == b'.').is_none()
                {
                    return Some(Kind::Keyword);
                }
                // Builtins (variable-2), types (variable-3), and other names: one color.
                Some(Kind::Variable)
            }
        }
    });
}

/// A quoted string, the mode's error; open past the line only by a backslash at its end.
fn pig_string(stream: &mut Stream, state: &mut Assorted, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    let mut closed = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            closed = true;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    state.string = (!closed && escaped).then_some(quote);
    Some(Kind::Operator)
}

const FCL_KEYWORDS: &str = "term method accu rule then is and or if default var_input var_output fuzzify defuzzify function_block \
ruleblock end_ruleblock end_defuzzify end_function_block end_fuzzify end_var";
const FCL_ATOMS: &str = "true false nan real min max cog cogs";

fn fcl_operator(byte: u8) -> bool {
    b"+-*&^%:=<>!|/".contains(&byte)
}

/// FCL (fuzzy control language), as CodeMirror 5's fcl mode colors it: keywords and atoms (any
/// case), numbers, other words (and lone symbols) variables, operator runs, `//`, `/* */`, and
/// `(* *)` comments.
pub fn fcl(text: &str, state: &mut Assorted, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return fcl_comment(stream, state);
        }
        let here = stream.next()?;
        if here.is_ascii_digit() || here == b'.' {
            let at = stream.pos;
            let exponent = |from: usize| {
                if !matches!(stream.byte(from), b'e' | b'E') {
                    return from;
                }
                let digits = if matches!(stream.byte(from + 1), b'+' | b'-') { from + 2 } else { from + 1 };
                let end = digits_from(stream, digits);
                if end > digits { end } else { from }
            };
            stream.pos = match here {
                b'.' => {
                    let end = digits_from(stream, at);
                    if end > at { exponent(end) } else { at }
                }
                b'0' if matches!(stream.byte(at), b'x' | b'X') && stream.byte(at + 1).is_ascii_hexdigit() => {
                    (at + 1..stream.bytes.len()).find(|end| !stream.byte(*end).is_ascii_hexdigit()).unwrap_or(stream.bytes.len())
                }
                b'0' if stream.byte(at) == b'0' && (b'0'..=b'7').contains(&stream.byte(at + 1)) => {
                    (at + 1..stream.bytes.len()).find(|end| !(b'0'..=b'7').contains(&stream.byte(*end))).unwrap_or(stream.bytes.len())
                }
                b'0' => at,
                _ => {
                    let whole = digits_from(stream, at);
                    let fraction = if stream.byte(whole) == b'.' { digits_from(stream, whole + 1) } else { whole };
                    exponent(fraction)
                }
            };
            return Some(Kind::Number);
        }
        if (here == b'/' || here == b'(') && stream.eat(|byte| byte == b'*').is_some() {
            state.comment = true;
            return fcl_comment(stream, state);
        }
        if (here == b'/' || here == b'(') && stream.eat(|byte| byte == b'/').is_some() {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if fcl_operator(here) {
            stream.eat_while(fcl_operator);
            return Some(Kind::Operator);
        }
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'$' || byte >= 0x80);
        let word = stream.current().to_ascii_lowercase();
        let listed = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
        if listed(FCL_KEYWORDS) {
            Some(Kind::Keyword)
        } else if listed(FCL_ATOMS) {
            None
        } else {
            Some(Kind::Variable)
        }
    });
}

/// A comment to `*/` or `*)`.
fn fcl_comment(stream: &mut Stream, state: &mut Assorted) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if star && (here == b'/' || here == b')') {
            state.comment = false;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}

const PIG_KEYWORDS: &str = "VOID IMPORT RETURNS DEFINE LOAD FILTER FOREACH ORDER CUBE DISTINCT COGROUP JOIN CROSS UNION SPLIT INTO IF OTHERWISE ALL AS \
BY USING INNER OUTER ONSCHEMA PARALLEL PARTITION GROUP AND OR NOT GENERATE FLATTEN ASC DESC IS STREAM THROUGH STORE \
MAPREDUCE SHIP CACHE INPUT OUTPUT STDERROR STDIN STDOUT LIMIT SAMPLE LEFT RIGHT FULL EQ GT LT GTE LTE NEQ MATCHES TRUE FALSE \
DUMP";

const MUMPS_FUNCTIONS: &str = "$ascii|$char|$data|$ecode|$estack|$etrap|$extract|$find|$fnumber|$get|$horolog|$io|$increment|$job|$justify|$length|$name|\
$next|$order|$piece|$qlength|$qsubscript|$query|$quit|$random|$reverse|$select|$stack|$test|$text|$translate|$view|$x|$y|$a|\
$c|$d|$e|$ec|$es|$et|$f|$fn|$g|$h|$i|$j|$l|$n|$na|$o|$p|$q|$ql|$qs|$r|$re|$s|$st|$t|$tr|$v|$z";

const MUMPS_COMMANDS: &str = "break|close|do|else|for|goto|halt|hang|if|job|kill|lock|merge|new|open|quit|read|set|tcommit|trollback|tstart|use|view|write|\
xecute|b|c|d|e|f|g|h|i|j|k|l|m|n|o|q|r|s|tc|tro|ts|u|v|w|x";
