//! More languages, as their CodeMirror 5 modes color them, each ported token for token:
//! Modelica, ECL, and Dylan (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Varied {
    comment: bool,
    string: Option<u8>,
    /// Dylan: how deep the open `/* */` comment is nested.
    depth: usize,
    /// ECL: whether the next token is the line's first.
    start_of_line: bool,
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

/// A string to its quote; open past the line only by a backslash at its end.
fn quoted(stream: &mut Stream, state: &mut Varied, quote: u8) -> Option<Kind> {
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
    Some(Kind::String)
}

fn block_comment(stream: &mut Stream, state: &mut Varied) -> Option<Kind> {
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

/// Modelica, as CodeMirror 5's modelica mode colors it: keywords, builtins, `Real`, `Integer`, …
/// atoms (plain), names and `'quoted names'` variables, numbers, strings, operators, `//` and
/// `/* */` comments, anything else an error.
pub fn modelica(text: &str, state: &mut Varied, out: &mut Out) {
    scan::run(text, out, |stream| {
        if state.comment {
            return block_comment(stream, state);
        }
        if state.string.is_some() {
            return modelica_string(stream, state);
        }
        if stream.eat_space() {
            return None;
        }
        let here = stream.next()?;
        if here == b'/' && stream.eat(|byte| byte == b'/').is_some() {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if here == b'/' && stream.eat(|byte| byte == b'*').is_some() {
            state.comment = true;
            return block_comment(stream, state);
        }
        let pair = [here, stream.peek()];
        if [b":=", b"<=", b">=", b"==", b"<>", b".+", b".-", b".*", b"./", b".^"].iter().any(|operator| **operator == pair) {
            stream.next();
            return Some(Kind::Operator);
        }
        if b";=(:),{}.*<>+-/^[]".contains(&here) {
            return Some(Kind::Operator);
        }
        if here.is_ascii_alphabetic() || here == b'_' {
            stream.eat_while(scan::js_word);
            let word = stream.current();
            return if listed(MODELICA_KEYWORDS, word) {
                Some(Kind::Keyword)
            } else if listed(MODELICA_BUILTINS, word) {
                Some(Kind::Property)
            } else if listed(MODELICA_ATOMS, word) {
                None
            } else {
                Some(Kind::Variable)
            };
        }
        if here == b'\'' && stream.peek() != 0 && stream.peek() != b'\'' {
            stream.eat_while(|byte| byte != b'\'');
            return if stream.eat(|byte| byte == b'\'').is_some() { Some(Kind::Variable) } else { Some(Kind::Operator) };
        }
        if here == b'"' {
            state.string = Some(b'"');
            return modelica_string(stream, state);
        }
        if here.is_ascii_digit() {
            stream.eat_while(|byte| byte.is_ascii_digit());
            if stream.eat(|byte| byte == b'.').is_some() {
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            if stream.eat(|byte| byte == b'e' || byte == b'E').is_some() {
                stream.eat(|byte| byte == b'-' || byte == b'+');
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            return Some(Kind::Number);
        }
        Some(Kind::Operator)
    });
}

/// A Modelica string to its quote (it runs on past the line's end).
fn modelica_string(stream: &mut Stream, state: &mut Varied) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

/// ECL, as CodeMirror 5's ecl mode colors it: keywords, actions and functions variables, types
/// (a type with a size too) variable-3, attributes builtins, other words (`true`, `null`, …) plain,
/// numbers, strings, operators, `#` lines meta, `//` and `/* */` comments.
pub fn ecl(text: &str, state: &mut Varied, out: &mut Out) {
    state.start_of_line = true;
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let style = ecl_token(stream, state);
        if style != Some(Kind::Comment) {
            state.start_of_line = false;
        }
        style
    });
}

fn ecl_token(stream: &mut Stream, state: &mut Varied) -> Option<Kind> {
    if state.comment {
        return block_comment(stream, state);
    }
    if let Some(quote) = state.string {
        return ecl_string(stream, state, quote);
    }
    let here = stream.next()?;
    if here == b'#' && state.start_of_line {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here == b'"' || here == b'\'' {
        return ecl_string(stream, state, here);
    }
    if b"[]{}(),;:.".contains(&here) {
        return None;
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
        return Some(Kind::Number);
    }
    if here == b'/' {
        if stream.eat(|byte| byte == b'*').is_some() {
            state.comment = true;
            return block_comment(stream, state);
        }
        if stream.eat(|byte| byte == b'/').is_some() {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
    }
    let operator = |byte: u8| b"+-*&%=<>!?|/".contains(&byte);
    if operator(here) {
        stream.eat_while(operator);
        return Some(Kind::Operator);
    }
    stream.eat_while(|byte| scan::js_word(byte) || byte == b'$');
    let word = stream.current().to_ascii_lowercase();
    if listed(ECL_KEYWORDS, &word) {
        return Some(Kind::Keyword);
    }
    if listed(ECL_VARIABLES, &word) || listed(ECL_FUNCTIONS, &word) || listed(ECL_TYPES, &word) {
        return Some(Kind::Variable);
    }
    if listed(ECL_BUILTINS, &word) {
        return Some(Kind::Property);
    }
    // A type with a size after it (`string20`, `unsigned_8`).
    let base = word.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
    if base.len() > 1 && listed(ECL_TYPES, base) {
        return Some(Kind::Variable);
    }
    None
}

/// An ECL string to its quote; open past the line only by a backslash at its end.
fn ecl_string(stream: &mut Stream, state: &mut Varied, quote: u8) -> Option<Kind> {
    quoted(stream, state, quote)
}

const DYLAN_NAME: &[u8] = b"-_?!*@<>$%";

fn dylan_name(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || DYLAN_NAME.contains(&byte)
}

/// The Dylan words by what they are, the later list winning for a word in two.
const DYLAN_KEYWORDS: &str = "if block begin method case for select when unless until while iterate profiling dynamic-bind finally \
exception cleanup else elseif afterwards above below by from handler in instance let local otherwise slot subclass then to keyed-by \
virtual";
const DYLAN_DEFINITIONS: &str = "class C-subtype C-mapped-subtype interface module library macro C-struct C-union C-function \
C-callable-wrapper method function C-variable C-address constant variable generic domain C-pointer-type table";
const DYLAN_CALLS: &str = "signal error cerror break check-type abort";

/// Dylan, as CodeMirror 5's dylan mode colors it: statement words keywords, definition words
/// plain, signaling calls builtins, `name:` keywords atoms (plain), `<class>` tags, `*global*`
/// variable-2 and `$constant` variable-3, other names variables, `#t`, `#key`, … atoms,
/// numbers, strings, operators, brackets, `//` and nested `/* */` comments.
pub fn dylan(text: &str, state: &mut Varied, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.depth > 0 {
            return dylan_comment(stream, state);
        }
        if let Some(quote) = state.string {
            return quoted(stream, state, quote);
        }
        dylan_token(stream, state)
    });
}

fn dylan_comment(stream: &mut Stream, state: &mut Varied) -> Option<Kind> {
    let mut star = false;
    let mut slash = false;
    while let Some(here) = stream.next() {
        if here == b'/' && star {
            state.depth -= 1;
            if state.depth == 0 {
                break;
            }
        } else if here == b'*' && slash {
            state.depth += 1;
        }
        star = here == b'*';
        slash = here == b'/';
    }
    Some(Kind::Comment)
}

/// A Dylan number at the position: where it ends.
fn dylan_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let len = stream.bytes.len();
    let sign = if matches!(stream.byte(at), b'+' | b'-') { at + 1 } else { at };
    let digits = |from: usize| (from..len).find(|end| !stream.byte(*end).is_ascii_digit()).unwrap_or(len.max(from));
    let exponent = |from: usize| {
        if !b"esdxESDX".contains(&stream.byte(from)) || from >= len {
            return None;
        }
        let sign = if matches!(stream.byte(from + 1), b'+' | b'-') { from + 2 } else { from + 1 };
        let end = digits(sign);
        (end > sign).then_some(end)
    };
    let whole = digits(sign);
    if stream.byte(whole) == b'.' {
        let fraction = digits(whole + 1);
        return Some(exponent(fraction).unwrap_or(fraction));
    }
    if whole > sign {
        return Some(exponent(whole).unwrap_or(whole));
    }
    None
}

fn dylan_token(stream: &mut Stream, state: &mut Varied) -> Option<Kind> {
    let here = stream.peek();
    let len = stream.bytes.len();
    if here == b'\'' || here == b'"' {
        stream.next();
        return quoted(stream, state, here);
    }
    if here == b'/' {
        stream.next();
        if stream.eat(|byte| byte == b'*').is_some() {
            state.depth = 1;
            return dylan_comment(stream, state);
        }
        if stream.eat(|byte| byte == b'/').is_some() {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        stream.back_up(1);
    } else if matches!(here, b'+' | b'-' | b'.') || here.is_ascii_digit() {
        if let Some(end) = dylan_number(stream) {
            stream.pos = end;
            return Some(Kind::Number);
        }
    } else {
        match here {
            b'#' => {
                stream.next();
                let next = stream.peek();
                match next {
                    b'"' => {
                        stream.next();
                        return quoted(stream, state, b'"');
                    }
                    b'b' | b'x' | b'o' => {
                        stream.next();
                        stream.eat_while(|byte| match next {
                            b'b' => byte == b'0' || byte == b'1',
                            b'x' => byte.is_ascii_hexdigit(),
                            _ => (b'0'..=b'7').contains(&byte),
                        });
                        return Some(Kind::Number);
                    }
                    b'#' => {
                        stream.next();
                        return Some(Kind::Punctuation);
                    }
                    b'[' | b'(' => {
                        stream.next();
                        return Some(Kind::Punctuation);
                    }
                    _ => {}
                }
                for word in ["f", "t", "all-keys", "include", "key", "next", "rest"] {
                    if stream.rest().get(..word.len()).is_some_and(|text| text.eq_ignore_ascii_case(word.as_bytes())) {
                        stream.pos += word.len();
                        return None;
                    }
                }
                stream.eat_while(|byte| byte.is_ascii_alphabetic() || byte == b'-');
                return Some(Kind::Operator);
            }
            b'~' => {
                stream.next();
                if stream.eat(|byte| byte == b'=').is_some() {
                    stream.eat(|byte| byte == b'=');
                }
                return Some(Kind::Operator);
            }
            b':' => {
                stream.next();
                if stream.eat(|byte| byte == b'=').is_some() {
                    return Some(Kind::Operator);
                }
                if stream.eat(|byte| byte == b':').is_some() {
                    return Some(Kind::Punctuation);
                }
            }
            _ => {
                if b"[](){}".contains(&here) {
                    stream.next();
                    return Some(Kind::Punctuation);
                }
                if here == b'.' || here == b',' {
                    stream.next();
                    return Some(Kind::Punctuation);
                }
                if stream.take("end") {
                    return Some(Kind::Keyword);
                }
            }
        }
    }
    // `name:`, `<name>`, `*name*`, `$name`, `with-…`/`without-…`.
    let at = stream.pos;
    let run = (at..len).find(|end| !dylan_name(stream.byte(*end))).unwrap_or(len);
    if run > at && stream.byte(run) == b':' {
        stream.pos = run + 1;
        return None;
    }
    for (open, close, style) in [(b'<', b'>', Kind::Operator), (b'*', b'*', Kind::Variable)] {
        if stream.byte(at) == open {
            let run = (at + 1..len).find(|end| !dylan_name(stream.byte(*end))).unwrap_or(len);
            if let Some(last) = (at + 2..run).rev().find(|end| stream.byte(*end) == close) {
                stream.pos = last + 1;
                return Some(style);
            }
        }
    }
    if stream.byte(at) == b'$' && dylan_name(stream.byte(at + 1)) && at + 1 < len {
        stream.pos = (at + 1..len).find(|end| !dylan_name(stream.byte(*end))).unwrap_or(len);
        return Some(Kind::Variable);
    }
    for prefix in ["without-", "with-"] {
        if stream.at(prefix) && dylan_name(stream.byte(at + prefix.len())) && at + prefix.len() < len {
            stream.pos = (at + prefix.len()..len).find(|end| !dylan_name(stream.byte(*end))).unwrap_or(len);
            return None;
        }
    }
    if b"+-*/^=<>&|".contains(&stream.peek()) {
        stream.next();
        return Some(Kind::Operator);
    }
    if stream.take("define") {
        return None;
    }
    stream.eat_while(|byte| scan::js_word(byte) || byte == b'-');
    let word = stream.current();
    if listed(DYLAN_CALLS, word) {
        return Some(Kind::Property);
    }
    if listed(DYLAN_DEFINITIONS, word) {
        return None;
    }
    if listed(DYLAN_KEYWORDS, word) {
        return Some(Kind::Keyword);
    }
    if stream.pos > at && dylan_name(stream.byte(at)) {
        return Some(Kind::Variable);
    }
    stream.next();
    Some(Kind::Variable)
}

const MODELICA_KEYWORDS: &str = "algorithm and annotation assert block break class connect connector constant constrainedby der discrete each else elseif \
elsewhen encapsulated end enumeration equation expandable extends external false final flow for function if import impure in \
initial inner input loop model not operator or outer output package parameter partial protected public pure record redeclare \
replaceable return stream then true type when while within";

const MODELICA_BUILTINS: &str = "abs acos actualStream asin atan atan2 cardinality ceil cos cosh delay div edge exp floor getInstanceName homotopy inStream \
integer log log10 mod pre reinit rem semiLinear sign sin sinh spatialDistribution sqrt tan tanh";

const MODELICA_ATOMS: &str = "Real Boolean Integer String";

const ECL_KEYWORDS: &str = "abs acos allnodes ascii asin asstring atan atan2 ave case choose choosen choosesets clustersize combine correlation cos cosh \
count covariance cron dataset dedup define denormalize distribute distributed distribution ebcdic enth error evaluate event \
eventextra eventname exists exp failcode failmessage fetch fromunicode getisvalid global graph group hash hash32 hash64 \
hashcrc hashmd5 having if index intformat isvalid iterate join keyunicode length library limit ln local log loop map matched \
matchlength matchposition matchtext matchunicode max merge mergejoin min nolocal nonempty normalize parse pipe power preload \
process project pull random range rank ranked realformat recordof regexfind regexreplace regroup rejected rollup round \
roundup row rowdiff sample set sin sinh sizeof soapcall sort sorted sqrt stepped stored sum table tan tanh thisnode topn \
tounicode transfer trim truncate typeof ungroup unicodeorder variance which workunit xmldecode xmlencode xmltext xmlunicode";

const ECL_VARIABLES: &str =
    "apply assert build buildindex evaluate fail keydiff keypatch loadxml nothor notify output parallel sequential soapcall wait";

const ECL_FUNCTIONS: &str = "__compressed__ all and any as atmost before beginc++ best between case const counter csv descend encrypt end endc++ endmacro \
except exclusive expire export extend false few first flat from full function group header heading hole ifblock import in \
interface joined keep keyed last left limit load local locale lookup macro many maxcount maxlength min skew module named \
nocase noroot noscan nosort not of only opt or outer overwrite packed partition penalty physicallength pipe quote record \
relationship repeat return right scan self separator service shared skew skip sql store terminator thor threshold token \
transform trim true type unicodeorder unsorted validate virtual whole wild within xml xpath";

const ECL_TYPES: &str = "ascii big_endian boolean data decimal ebcdic integer pattern qstring real record rule set of string token udecimal unicode \
unsigned varstring varunicode";

const ECL_BUILTINS: &str = "checkpoint deprecated failcode failmessage failure global independent onwarning persist priority recovery stored success \
wait when";
