//! TTCN-3 and its configuration files, as CodeMirror 5's ttcn and ttcn-cfg modes color them,
//! each ported token for token (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Ttcn {
    comment: bool,
    string: Option<u8>,
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

/// A string to its quote (then a `b`, `h`, or `o` after it); it runs on past the line's end.
fn string(stream: &mut Stream, state: &mut Ttcn, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    state.string = Some(quote);
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            if matches!(stream.peek().to_ascii_lowercase(), b'b' | b'h' | b'o') {
                stream.next();
            }
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

/// TTCN-3, as CodeMirror 5's ttcn mode colors it: keywords, builtins and types builtins,
/// operations plain, verdicts, booleans, and `null`/`omit` strings, names variables, numbers,
/// strings, `#` lines and `%macros` atoms, punctuation, `//` and `/* */` comments.
pub fn ttcn(text: &str, state: &mut Ttcn, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return comment(stream, state);
        }
        if let Some(quote) = state.string {
            return string(stream, state, quote);
        }
        let here = stream.next()?;
        if here == b'"' || here == b'\'' {
            return string(stream, state, here);
        }
        if b"[]{}(),;\\:?.".contains(&here) {
            return Some(Kind::Punctuation);
        }
        if here == b'#' {
            stream.skip_to_end();
            return None;
        }
        if here == b'%' {
            stream.eat_while(scan::js_word);
            return None;
        }
        if here.is_ascii_digit() {
            stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
            return Some(Kind::Number);
        }
        if here == b'/' {
            if stream.eat(|byte| byte == b'*').is_some() {
                state.comment = true;
                return comment(stream, state);
            }
            if stream.eat(|byte| byte == b'/').is_some() {
                stream.skip_to_end();
                return Some(Kind::Comment);
            }
        }
        let operator = |byte: u8| b"+-*&@=<>!/".contains(&byte);
        if operator(here) {
            if here == b'@' && (stream.take("try") || stream.take("catch") || stream.take("lazy")) {
                return Some(Kind::Keyword);
            }
            stream.eat_while(operator);
            return Some(Kind::Operator);
        }
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'$' || byte >= 0x80);
        let word = stream.current();
        if listed(TTCN_KEYWORDS, word) {
            Some(Kind::Keyword)
        } else if listed(TTCN_BUILTIN, word) {
            Some(Kind::Property)
        } else if [TTCN_TIMER_OPS, TTCN_CONFIG_OPS, TTCN_VERDICT_OPS, TTCN_PORT_OPS, TTCN_SUT_OPS, TTCN_FUNCTION_OPS]
            .iter()
            .any(|list| listed(list, word))
        {
            None
        } else if [TTCN_VERDICT_CONSTS, TTCN_BOOLEAN_CONSTS, TTCN_OTHER_CONSTS].iter().any(|list| listed(list, word)) {
            Some(Kind::String)
        } else if listed(TTCN_TYPES, word) || listed(TTCN_VISIBILITY_MODIFIERS, word) {
            Some(Kind::Property)
        } else if listed(TTCN_TEMPLATE_MATCH, word) {
            None
        } else {
            Some(Kind::Variable)
        }
    });
}

fn comment(stream: &mut Stream, state: &mut Ttcn) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if here == b'/' && star {
            state.comment = false;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}

/// TTCN-3 configuration files, as CodeMirror 5's ttcn-cfg mode colors them as Obsidian loads it
/// (by the mode's name, without its MIME type's word lists): `[SECTIONS]` numbers, names
/// variables, numbers, strings, `:` and `=` punctuation, `|` operators, `#` comments.
pub fn ttcn_cfg(text: &str, state: &mut Ttcn, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.string {
            return string(stream, state, quote);
        }
        let here = stream.next()?;
        if here == b'"' || here == b'\'' {
            return string(stream, state, here);
        }
        if here == b':' || here == b'=' {
            return Some(Kind::Punctuation);
        }
        if here == b'#' {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if here.is_ascii_digit() {
            stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
            return Some(Kind::Number);
        }
        if here == b'|' {
            stream.eat_while(|byte| byte == b'|');
            return Some(Kind::Operator);
        }
        if here == b'[' {
            stream.eat_while(|byte| scan::js_word(byte) || byte == b']');
            return Some(Kind::Number);
        }
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'$');
        Some(Kind::Variable)
    });
}

const TTCN_KEYWORDS: &str = "activate address alive all alt altstep and and4b any break case component const continue control deactivate display do else \
encode enumerated except exception execute extends extension external for from function goto group if import in infinity \
inout interleave label language length log match message mixed mod modifies module modulepar mtc noblock not not4b nowait of \
on optional or or4b out override param pattern port procedure record recursive rem repeat return runs select self sender set \
signature system template testcase to type union value valueof var variant while with xor xor4b";

const TTCN_BUILTIN: &str = "bit2hex bit2int bit2oct bit2str char2int char2oct encvalue decomp decvalue float2int float2str hex2bit hex2int hex2oct \
hex2str int2bit int2char int2float int2hex int2oct int2str int2unichar isbound ischosen ispresent isvalue lengthof log2str \
oct2bit oct2char oct2hex oct2int oct2str regexp replace rnd sizeof str2bit str2float str2hex str2int str2oct substr \
unichar2int unichar2char enum2int";

const TTCN_TYPES: &str = "anytype bitstring boolean char charstring default float hexstring integer objid octetstring universal verdicttype timer";

const TTCN_TIMER_OPS: &str = "read running start stop timeout";

const TTCN_PORT_OPS: &str = "call catch check clear getcall getreply halt raise receive reply send trigger";

const TTCN_CONFIG_OPS: &str = "create connect disconnect done kill killed map unmap";

const TTCN_VERDICT_OPS: &str = "getverdict setverdict";

const TTCN_SUT_OPS: &str = "action";

const TTCN_FUNCTION_OPS: &str = "apply derefers refers";

const TTCN_VERDICT_CONSTS: &str = "error fail inconc none pass";

const TTCN_BOOLEAN_CONSTS: &str = "true false";

const TTCN_OTHER_CONSTS: &str = "null NULL omit";

const TTCN_VISIBILITY_MODIFIERS: &str = "private public friend";

const TTCN_TEMPLATE_MATCH: &str = "complement ifpresent subset superset permutation";
