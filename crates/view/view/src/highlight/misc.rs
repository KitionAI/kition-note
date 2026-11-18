//! Smaller languages, as their CodeMirror 5 modes color them, each ported token for token: Elm,
//! Pascal, Smalltalk, VHDL, SPARQL, Cypher, and D (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Misc {
    /// Elm: an open `{- -}` comment's depth, a `"""` string, a `[glsl| |]` block.
    elm_comment: usize,
    elm_string: bool,
    elm_glsl: bool,
    /// Pascal and D: an open comment (its closing byte pair) or string (its quote).
    comment: Option<[u8; 2]>,
    string: Option<u8>,
    /// Smalltalk: what it reads next, and whether a variable may come.
    smalltalk: Vec<Smalltalk>,
    expect_variable: bool,
    started: bool,
}

/// What Smalltalk reads next (innermost last; none: its code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Smalltalk {
    Comment,
    String,
    Symbol,
    Temporaries,
}

const ELM_KEYWORDS: &[&str] =
    &["case", "of", "as", "if", "then", "else", "let", "in", "type", "alias", "module", "where", "import", "exposing", "port"];

/// Elm, as CodeMirror 5's elm mode colors it: keywords, operators keywords, brackets builtins, a
/// capitalized name a variable-2, a name at the line's start a definition, numbers, strings and
/// chars, `--` and nested `{- -}` comments.
pub fn elm(text: &str, state: &mut Misc, out: &mut Out) {
    scan::run(text, out, |stream| {
        let style = elm_token(stream, state);
        if ELM_KEYWORDS.contains(&stream.current()) { Some(Kind::Keyword) } else { style }
    });
}

fn elm_token(stream: &mut Stream, state: &mut Misc) -> Option<Kind> {
    if state.elm_comment > 0 {
        return elm_comment(stream, state);
    }
    if state.elm_string {
        while !stream.eol() {
            if stream.next() == Some(b'"') && stream.take("\"\"") {
                state.elm_string = false;
                break;
            }
        }
        return Some(Kind::String);
    }
    if state.elm_glsl {
        while !stream.eol() {
            if stream.next() == Some(b'|') && stream.take("]") {
                state.elm_glsl = false;
                break;
            }
        }
        return Some(Kind::String);
    }
    if stream.eat_while(|byte| byte == b' ' || byte == 0x0b || byte == 0x0c) {
        return None;
    }
    let here = stream.next()?;
    let at_start = stream.pos == 1;
    match here {
        b'(' | b')' | b',' | b'[' | b']' | b'{' | b'}' => {
            if here == b'{' && stream.eat(|byte| byte == b'-').is_some() {
                state.elm_comment = 1;
                return elm_comment(stream, state);
            }
            if here == b'[' && stream.take("glsl|") {
                state.elm_glsl = true;
                return elm_token(stream, state).or(Some(Kind::String));
            }
            Some(Kind::Property)
        }
        b'\'' | b'"' => {
            if here == b'"' && stream.take("\"") {
                if stream.take("\"") {
                    state.elm_string = true;
                    return elm_token(stream, state);
                }
                return Some(Kind::String);
            }
            // Past every escaped quote ahead (as the mode skips to them), then to a quote; none
            // left, the mode's error.
            let escaped = [b'\\', here];
            while let Some(found) = stream.bytes.get(stream.pos..).and_then(|rest| rest.windows(2).position(|pair| pair == escaped)) {
                stream.pos += found + 2;
            }
            match stream.bytes.get(stream.pos..).and_then(|rest| rest.iter().position(|byte| *byte == here)) {
                Some(found) => {
                    stream.pos += found + 1;
                    Some(Kind::String)
                }
                None => {
                    stream.skip_to_end();
                    Some(Kind::Operator)
                }
            }
        }
        b'A'..=b'Z' => {
            stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            Some(Kind::Variable)
        }
        b'a'..=b'z' => {
            stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            if at_start { None } else { Some(Kind::Variable) }
        }
        b'0'..=b'9' => {
            if here == b'0' && stream.eat(|byte| byte == b'x' || byte == b'X').is_some() {
                stream.eat_while(|byte| byte.is_ascii_hexdigit());
                return Some(Kind::Number);
            }
            if here != b'0' {
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            if stream.eat(|byte| byte == b'.').is_some() {
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            if stream.eat(|byte| byte == b'e' || byte == b'E').is_some() {
                stream.eat(|byte| byte == b'-' || byte == b'+');
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            Some(Kind::Number)
        }
        _ if b"-&*+.\\/<>=?^|:".contains(&here) => {
            if here == b'-' && stream.eat(|byte| byte == b'-').is_some() {
                stream.skip_to_end();
                return Some(Kind::Comment);
            }
            stream.eat_while(|byte| b"-&*+.\\/<>=?^|:".contains(&byte));
            Some(Kind::Keyword)
        }
        b'_' => Some(Kind::Keyword),
        _ => Some(Kind::Operator),
    }
}

fn elm_comment(stream: &mut Stream, state: &mut Misc) -> Option<Kind> {
    while !stream.eol() {
        let here = stream.next();
        if here == Some(b'{') && stream.eat(|byte| byte == b'-').is_some() {
            state.elm_comment += 1;
        } else if here == Some(b'-') && stream.eat(|byte| byte == b'}').is_some() {
            state.elm_comment -= 1;
            if state.elm_comment == 0 {
                break;
            }
        }
    }
    Some(Kind::Comment)
}

const PASCAL_KEYWORDS: &str = "absolute and array asm begin case const constructor destructor div do downto else end file for function \
goto if implementation in inherited inline interface label mod nil not object of operator or packed procedure program record reintroduce \
repeat self set shl shr string then to type unit until uses var while with xor as class dispinterface except exports finalization \
finally initialization is library on out property raise resourcestring threadvar try abstract alias assembler bitpacked break cdecl \
continue cppdecl cvar default deprecated dynamic enumerator experimental export external far far16 forward generic helper implements \
index interrupt iocheck local message name near nodefault noreturn nostackframe oldfpccall otherwise overload override pascal platform \
private protected public published read register result safecall saveregisters softfloat specialize static stdcall stored strict \
unaligned unimplemented varargs virtual write";

/// A quoted string to its quote; left open only by a backslash at the line's end.
fn quoted(stream: &mut Stream, state: &mut Misc, quote: u8) -> Option<Kind> {
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

/// A block comment to `close` (two bytes, or one when the second is 0).
fn block(stream: &mut Stream, state: &mut Misc, close: [u8; 2]) -> Option<Kind> {
    let mut previous = 0;
    while let Some(here) = stream.next() {
        if (close[1] == 0 && here == close[0]) || (close[1] != 0 && previous == close[0] && here == close[1]) {
            state.comment = None;
            return Some(Kind::Comment);
        }
        previous = here;
    }
    state.comment = Some(close);
    Some(Kind::Comment)
}

/// Pascal, as CodeMirror 5's pascal mode colors it: keywords (as written), `null` an atom,
/// numbers, strings, `{ }`, `(* *)`, and `//` comments.
pub fn pascal(text: &str, state: &mut Misc, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return quoted(stream, state, quote);
        }
        if let Some(close) = state.comment {
            return block(stream, state, close);
        }
        let here = stream.next()?;
        match here {
            b'"' | b'\'' => quoted(stream, state, here),
            b'(' if stream.eat(|byte| byte == b'*').is_some() => block(stream, state, *b"*)"),
            b'{' => block(stream, state, [b'}', 0]),
            b'[' | b']' | b'(' | b')' | b',' | b';' | b':' | b'.' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'.');
                Some(Kind::Number)
            }
            b'/' if stream.eat(|byte| byte == b'/').is_some() => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if b"+-*&%=<>!?|/".contains(&here) => {
                stream.eat_while(|byte| b"+-*&%=<>!?|/".contains(&byte));
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                let word = stream.current();
                if PASCAL_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
                    Some(Kind::Keyword)
                } else if word == "null" {
                    None
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}

/// Smalltalk, as CodeMirror 5's smalltalk mode colors it: a name where a variable may come a
/// variable (a keyword when it holds `self`, `true`, …), other names plain, `#symbols`, strings,
/// `"comments"`, numbers, operators, brackets.
pub fn smalltalk(text: &str, state: &mut Misc, out: &mut Out) {
    if !state.started {
        state.started = true;
        state.expect_variable = true;
    }
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let (style, eos) = smalltalk_token(stream, state);
        state.expect_variable = eos;
        style
    });
}

fn smalltalk_token(stream: &mut Stream, state: &mut Misc) -> (Option<Kind>, bool) {
    let read_to = |stream: &mut Stream, state: &mut Misc, close: u8, mode: Smalltalk| {
        stream.eat_while(|byte| byte != close);
        if stream.eat(|byte| byte == close).is_some() {
            if state.smalltalk.last() == Some(&mode) {
                state.smalltalk.pop();
            }
        } else if state.smalltalk.last() != Some(&mode) {
            state.smalltalk.push(mode);
        }
    };
    match state.smalltalk.last().copied() {
        Some(Smalltalk::Comment) => {
            read_to(stream, state, b'"', Smalltalk::Comment);
            return (Some(Kind::Comment), true);
        }
        Some(Smalltalk::String) => {
            read_to(stream, state, b'\'', Smalltalk::String);
            return (Some(Kind::String), false);
        }
        Some(Smalltalk::Symbol) => {
            read_to(stream, state, b'\'', Smalltalk::Symbol);
            return (Some(Kind::String), false);
        }
        Some(Smalltalk::Temporaries) => {
            if stream.next() == Some(b'|') {
                state.smalltalk.pop();
                return (None, true);
            }
            stream.eat_while(|byte| byte != b'|');
            return (Some(Kind::Variable), false);
        }
        None => {}
    }
    let operator = |byte: u8| b"+-/\\*~<>=@%|&?!.,:;^".contains(&byte);
    let here = stream.next().unwrap_or(0);
    match here {
        b'"' => {
            state.smalltalk.push(Smalltalk::Comment);
            read_to(stream, state, b'"', Smalltalk::Comment);
            (Some(Kind::Comment), true)
        }
        b'\'' => {
            state.smalltalk.push(Smalltalk::String);
            read_to(stream, state, b'\'', Smalltalk::String);
            (Some(Kind::String), false)
        }
        b'#' => {
            if stream.peek() == b'\'' {
                stream.next();
                state.smalltalk.push(Smalltalk::Symbol);
                read_to(stream, state, b'\'', Smalltalk::Symbol);
                return (Some(Kind::String), false);
            }
            if stream.eat_while(|byte| !b" \t\r.{}[]()".contains(&byte)) { (Some(Kind::String), false) } else { (Some(Kind::Comment), false) }
        }
        b'$' => {
            if stream.next() == Some(b'<') {
                stream.eat_while(|byte| !b" \t\r>".contains(&byte));
                stream.next();
            }
            (Some(Kind::String), false)
        }
        b'|' if state.expect_variable => {
            state.smalltalk.push(Smalltalk::Temporaries);
            (None, false)
        }
        b'[' | b']' | b'{' | b'}' | b'(' | b')' => (Some(Kind::Punctuation), b"[{(".contains(&here)),
        _ if operator(here) => {
            stream.eat_while(operator);
            (Some(Kind::Operator), here != b';')
        }
        b'0'..=b'9' => {
            stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            (Some(Kind::Number), false)
        }
        _ if here.is_ascii_alphanumeric() || here == b'_' => {
            stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            if !state.expect_variable {
                return (None, false);
            }
            let word = stream.current();
            let keyword = ["true", "false", "nil", "self", "super", "thisContext"].iter().any(|known| word.contains(known));
            (Some(if keyword { Kind::Keyword } else { Kind::Variable }), false)
        }
        _ => (None, state.expect_variable),
    }
}

const VHDL_KEYWORDS: &str = "abs access after alias all and architecture array assert attribute begin block body buffer bus case \
component configuration constant disconnect downto else elsif end entity exit file for function generate generic group guarded if impure \
in inertial inout is label library linkage literal loop map mod nand new next nor null of on open or others out package port postponed \
procedure process pure range record register reject rem report return rol ror select severity signal sla sll sra srl subtype then to \
transport type unaffected units until use variable wait when while with xnor xor";

/// VHDL, as CodeMirror 5's vhdl mode colors it: keywords (any case), numbers, `"strings"` and
/// `'c'` characters, `--` comments, operators, `` ` `` and `$` names meta.
pub fn vhdl(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let here = stream.next()?;
        match here {
            b'`' | b'$' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                Some(Kind::Comment)
            }
            b'"' | b'\'' => {
                stream.eat_while(|byte| byte != here);
                stream.next();
                Some(Kind::String)
            }
            b'[' | b']' | b'{' | b'}' | b'(' | b')' | b',' | b';' | b':' | b'.' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'.' || byte == b'\'');
                Some(Kind::Number)
            }
            b'-' if stream.eat(|byte| byte == b'-').is_some() => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if b"&|~><!)(*#%@+/=?:;}{,.^-[]".contains(&here) => {
                stream.eat_while(|byte| b"&|~><!)(*#%@+/=?:;}{,.^-[]".contains(&byte));
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                let word = stream.current().to_ascii_lowercase();
                if VHDL_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
                    Some(Kind::Keyword)
                } else if matches!(stream.current(), "null" | "NULL" | "Null") {
                    None
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}

const SPARQL_BUILTINS: &str = "str lang langmatches datatype bound sameterm isiri isuri iri uri bnode count sum min max avg sample \
group_concat rand abs ceil floor round concat substr strlen replace ucase lcase encode_for_uri contains strstarts strends strbefore \
strafter year month day hours minutes seconds timezone tz now uuid struuid md5 sha1 sha256 sha384 sha512 coalesce if strlang strdt \
isnumeric regex exists isblank isliteral a bind";
const SPARQL_KEYWORDS: &str = "base prefix select distinct reduced construct describe ask from named where order limit offset filter \
optional graph by asc desc as having undef values group minus in not service silent using insert delete union true false with data copy \
to move add create drop clear load";

/// A prefixed name's local part (`:name`, with `.`, `-`, escapes, and `%` codes).
fn sparql_local(stream: &mut Stream) {
    loop {
        let here = stream.peek();
        let next = stream.look(1);
        let dot = here == b'.' && (scan::word(next) || matches!(next, b'-' | b'\\' | b'%'));
        if dot || scan::word(here) || matches!(here, b':' | b'-') {
            stream.pos += 1;
        } else if here == b'\\' && b"-\\_~.!$&'()*+,;=/?#@%".contains(&next) && next != 0 {
            stream.pos += 2;
        } else if here == b'%' && next.is_ascii_hexdigit() && stream.look(2).is_ascii_hexdigit() {
            stream.pos += 3;
        } else {
            break;
        }
    }
}

/// SPARQL, as CodeMirror 5's sparql mode colors it: keywords and builtins (any case), `?vars`,
/// `<iri>`s and prefixed names atoms, strings, brackets, operators, `#` comments.
pub fn sparql(text: &str, state: &mut Misc, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return sparql_string(stream, state, quote);
        }
        let here = stream.next()?;
        let operator = |byte: u8| b"*+-<>=&|^/!?".contains(&byte);
        match here {
            b'$' | b'?' => {
                if here == b'?' && matches!(stream.peek(), b' ' | b'\t' | b'\r') {
                    return Some(Kind::Operator);
                }
                stream.eat_while(|byte| scan::word(byte) && byte != 0);
                Some(Kind::Variable)
            }
            b'<' if !matches!(stream.peek(), b' ' | b'\t' | b'\r' | b'=') => {
                stream.eat_while(|byte| !b" \t\r>".contains(&byte));
                stream.eat(|byte| byte == b'>');
                None
            }
            b'"' | b'\'' => sparql_string(stream, state, here),
            b'{' | b'}' | b'(' | b')' | b',' | b'.' | b';' | b'[' | b']' => Some(Kind::Punctuation),
            b'#' => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            b'^' => {
                if stream.peek() == b'^' {
                    stream.next();
                } else {
                    stream.eat_while(operator);
                }
                Some(Kind::Operator)
            }
            _ if operator(here) => {
                stream.eat_while(operator);
                Some(Kind::Operator)
            }
            b':' => {
                sparql_local(stream);
                None
            }
            b'@' => {
                stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
                Some(Kind::Comment)
            }
            _ => {
                stream.eat_while(scan::word);
                if stream.eat(|byte| byte == b':').is_some() {
                    sparql_local(stream);
                    return None;
                }
                let word = stream.current().to_ascii_lowercase();
                if SPARQL_BUILTINS.split_ascii_whitespace().any(|known| known == word) {
                    Some(Kind::Property)
                } else if SPARQL_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
                    Some(Kind::Keyword)
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}

fn sparql_string(stream: &mut Stream, state: &mut Misc, quote: u8) -> Option<Kind> {
    state.string = Some(quote);
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

const CYPHER_BUILTINS: &str = "abs acos allshortestpaths asin atan atan2 avg ceil coalesce collect cos cot count degrees e endnode exp \
extract filter floor haversin head id keys labels last left length log log10 lower ltrim max min node nodes percentilecont \
percentiledisc pi radians rand range reduce rel relationship relationships replace reverse right round rtrim shortestpath sign sin size \
split sqrt startnode stdev stdevp str substring sum tail tan timestamp tofloat toint tostring trim type upper";
const CYPHER_OPERATORS: &str = "all and any contains exists has in none not or single xor";
const CYPHER_KEYWORDS: &str = "as asc ascending assert by case commit constraint create csv cypher delete desc descending detach distinct \
drop else end ends explain false fieldterminator foreach from headers in index is join limit load match merge null on optional order \
periodic profile remove return scan set skip start starts then true union unique unwind using when where with call yield access active \
assign alter catalog change copy constraints current database databases dbms default deny element elements exists grant graph graphs if \
indexes label labels management name names new node nodes not of or password populated privileges property read relationship \
relationships replace required revoke role roles show status stop suspended to traverse type types user users write";

/// Cypher, as CodeMirror 5's cypher mode colors it: clauses keywords and functions builtins (any
/// case), `name:` labels and properties atoms, strings, `//` comments; numbers are names there.
pub fn cypher(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let here = stream.next()?;
        match here {
            b'"' | b'\'' => {
                let rest = stream.bytes.get(stream.pos..).unwrap_or_default();
                if let Some(found) = rest.iter().position(|byte| *byte == here) {
                    stream.pos += found + 1;
                }
                Some(Kind::String)
            }
            b'{' | b'}' | b'(' | b')' | b',' | b'.' | b';' | b'[' | b']' => None,
            b'/' if stream.eat(|byte| byte == b'/').is_some() => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if b"*+-<>=&|~%^".contains(&here) => {
                stream.eat_while(|byte| b"*+-<>=&|~%^".contains(&byte));
                None
            }
            _ => {
                stream.eat_while(scan::word);
                if stream.eat(|byte| byte == b':').is_some() {
                    stream.eat_while(|byte| scan::word(byte) || byte == b'-');
                    return None;
                }
                let word = stream.current().to_ascii_lowercase();
                let has = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
                if has(CYPHER_BUILTINS) {
                    Some(Kind::Property)
                } else if has(CYPHER_OPERATORS) {
                    None
                } else if has(CYPHER_KEYWORDS) {
                    Some(Kind::Keyword)
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}

const D_KEYWORDS: &str = "abstract alias align asm assert auto break case cast cdouble cent cfloat const continue debug default \
delegate delete deprecated export extern final finally function goto immutable import inout invariant is lazy macro module new nothrow \
override package pragma private protected public pure ref return shared short static super synchronized template this throw typedef \
typeid typeof volatile __FILE__ __LINE__ __gshared __traits __vector __parameters body catch class do else enum for foreach \
foreach_reverse if in interface mixin out scope struct switch try union unittest version while with";
const D_BUILTINS: &str = "bool byte char creal dchar double float idouble ifloat int ireal long real short ubyte ucent uint ulong ushort \
wchar wstring void size_t sizediff_t";
const D_ATOMS: &str = "exit failure success true false null";

/// D, as CodeMirror 5's d mode colors it: keywords, builtin types, atoms, `@attributes` meta,
/// numbers, strings (`"`, `'`, `` ` ``), `//`, `/* */`, and `/+ +/` comments.
pub fn d(text: &str, state: &mut Misc, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return quoted(stream, state, quote);
        }
        if let Some(close) = state.comment {
            return block(stream, state, close);
        }
        let here = stream.next()?;
        match here {
            b'@' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                Some(Kind::Comment)
            }
            b'"' | b'\'' | b'`' => quoted(stream, state, here),
            b'[' | b']' | b'{' | b'}' | b'(' | b')' | b',' | b';' | b':' | b'.' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'.');
                Some(Kind::Number)
            }
            b'/' if stream.eat(|byte| byte == b'+').is_some() => block(stream, state, *b"+/"),
            b'/' if stream.eat(|byte| byte == b'*').is_some() => block(stream, state, *b"*/"),
            b'/' if stream.eat(|byte| byte == b'/').is_some() => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            _ if b"+-*&%=<>!?|/".contains(&here) => {
                stream.eat_while(|byte| b"+-*&%=<>!?|/".contains(&byte));
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                let word = stream.current();
                let has = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
                if has(D_KEYWORDS) {
                    Some(Kind::Keyword)
                } else if has(D_BUILTINS) {
                    Some(Kind::Property)
                } else if has(D_ATOMS) {
                    None
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}
