//! Stack languages, as their CodeMirror 5 modes color them, each ported token for token: Forth
//! and Factor (see each function).

use std::ops::Range;

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Forth {
    /// Whether a `:` definition is being compiled.
    compiling: bool,
    /// The words the code defined, upper-cased.
    words: Vec<String>,
}

const FORTH_CORE: &str = "INVERT AND OR XOR 2* 2/ LSHIFT RSHIFT 0= = 0< < > U< MIN MAX 2DROP 2DUP 2OVER 2SWAP ?DUP DEPTH DROP \
DUP OVER ROT SWAP >R R> R@ + - 1+ 1- ABS NEGATE S>D * M* UM* FM/MOD SM/REM UM/MOD */ */MOD / /MOD MOD HERE , @ ! CELL+ CELLS \
C, C@ C! CHARS 2@ 2! ALIGN ALIGNED +! ALLOT CHAR [CHAR] [ ] BL FIND EXECUTE IMMEDIATE COUNT LITERAL STATE ; DOES> >BODY \
EVALUATE SOURCE >IN <# # #S #> HOLD SIGN BASE >NUMBER HEX DECIMAL FILL MOVE . CR EMIT SPACE SPACES TYPE U. .R U.R ACCEPT \
TRUE FALSE <> U> 0<> 0> NIP TUCK ROLL PICK 2>R 2R@ 2R> WITHIN UNUSED MARKER I J TO COMPILE, [COMPILE] SAVE-INPUT \
RESTORE-INPUT PAD ERASE 2LITERAL DNEGATE D- D+ D0< D0= D2* D2/ D< D= DMAX DMIN D>S DABS M+ M*/ D. D.R 2ROT DU< CATCH \
THROW FREE RESIZE ALLOCATE CS-PICK CS-ROLL GET-CURRENT SET-CURRENT FORTH-WORDLIST GET-ORDER SET-ORDER PREVIOUS \
SEARCH-WORDLIST WORDLIST FIND ALSO ONLY FORTH DEFINITIONS ORDER -TRAILING /STRING SEARCH COMPARE CMOVE CMOVE> BLANK \
SLITERAL";
const FORTH_IMMEDIATE: &str = "IF ELSE THEN BEGIN WHILE REPEAT UNTIL RECURSE [IF] [ELSE] [THEN] ?DO DO LOOP +LOOP UNLOOP \
LEAVE EXIT AGAIN CASE OF ENDOF ENDCASE";
const FORTH_DEFINERS: &str = "VARIABLE 2VARIABLE CONSTANT 2CONSTANT CREATE POSTPONE VALUE WORD";

fn space(byte: u8) -> bool {
    byte.is_ascii_whitespace()
}

/// Where a run of `ok` bytes from `at` ends.
fn run_of(stream: &Stream, at: usize, ok: impl Fn(u8) -> bool) -> usize {
    let len = stream.bytes.len();
    (at..len).find(|end| !ok(stream.byte(*end))).unwrap_or(len.max(at))
}

/// Whether JavaScript's `Number(text) - 68719476735` is neither zero nor NaN (the mode's test
/// for a number): a decimal (sign, digits, a fraction, an exponent), `Infinity`, or a `0x`,
/// `0o`, `0b` integer.
fn forth_number(text: &str) -> bool {
    const ZERO_AT: u128 = 68_719_476_735;
    let lower = text.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = lower.strip_prefix(prefix) {
            // Too long for `u128` is a number far from the one that makes zero.
            return !digits.is_empty() && digits.chars().all(|digit| digit.is_digit(radix)) && u128::from_str_radix(digits, radix) != Ok(ZERO_AT);
        }
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned == "Infinity" {
        return true;
    }
    let bytes = unsigned.as_bytes();
    let digits = |from: usize| (from..bytes.len()).find(|index| !bytes.get(*index).is_some_and(u8::is_ascii_digit)).unwrap_or(bytes.len().max(from));
    let mut at = digits(0);
    let mut any = at > 0;
    if bytes.get(at) == Some(&b'.') {
        let fraction = digits(at + 1);
        any = any || fraction > at + 1;
        at = fraction;
    }
    if !any {
        return false;
    }
    if bytes.get(at).is_some_and(|byte| byte.eq_ignore_ascii_case(&b'e')) {
        let sign = if bytes.get(at + 1).is_some_and(|byte| *byte == b'+' || *byte == b'-') { at + 2 } else { at + 1 };
        let end = digits(sign);
        if end == sign {
            return false;
        }
        at = end;
    }
    at == bytes.len() && unsigned.parse::<f64>().is_ok_and(|number| number != ZERO_AT as f64)
}

/// Forth, as CodeMirror 5's forth mode colors it: the standard's core words builtins (any
/// case), its control words keywords, words the code defines variables, `\` and `( )`
/// comments, `." "`, `S" "`, `.( )` strings, numbers, other words atoms (plain); a definition's
/// name plain. A token takes the spaces after it.
pub fn forth(text: &str, state: &mut Forth, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        forth_token(stream, state)
    });
}

fn forth_token(stream: &mut Stream, state: &mut Forth) -> Option<Kind> {
    let at = stream.pos;
    let word_end = run_of(stream, at, |byte| !space(byte));
    let word = std::str::from_utf8(stream.bytes.get(at..word_end).unwrap_or_default()).unwrap_or("").to_string();
    let after_word = run_of(stream, word_end, space);
    // `\s+(\S+)(\s|$)+` after `prefix`: where it ends and the name.
    let named = |stream: &Stream, prefix_end: usize| {
        let gap = run_of(stream, prefix_end, space);
        let name_end = run_of(stream, gap, |byte| !space(byte));
        (gap > prefix_end && name_end > gap).then(|| {
            let name = std::str::from_utf8(stream.bytes.get(gap..name_end).unwrap_or_default()).unwrap_or("").to_ascii_uppercase();
            (run_of(stream, name_end, space), name)
        })
    };
    if !state.compiling {
        let upper = word.to_ascii_uppercase();
        if (word == "]" || upper == ":NONAME") && (word_end == stream.bytes.len() || space(stream.byte(word_end))) {
            stream.pos = (word_end + 1).min(stream.bytes.len());
            state.compiling = true;
            return Some(Kind::Property);
        }
        if stream.peek() == b':'
            && let Some((end, name)) = named(stream, at + 1)
        {
            state.words.push(name);
            state.compiling = true;
            stream.pos = end;
            return None;
        }
        for definer in FORTH_DEFINERS.split_ascii_whitespace() {
            if stream.rest().get(..definer.len()).is_some_and(|text| text.eq_ignore_ascii_case(definer.as_bytes()))
                && let Some((end, name)) = named(stream, at + definer.len())
            {
                state.words.push(name);
                stream.pos = end;
                return None;
            }
        }
        for tick in ["'", "[']"] {
            if stream.at(tick)
                && let Some((end, _)) = named(stream, at + tick.len())
            {
                stream.pos = end;
                return Some(Kind::Property);
            }
        }
    } else {
        if matches!(stream.peek(), b';' | b'[') {
            let next = stream.byte(at + 1);
            if at + 1 == stream.bytes.len() || space(next) {
                state.compiling = false;
                stream.pos = at + 1;
                return Some(Kind::Property);
            }
        }
        if stream.at("POSTPONE")
            && let Some((end, _)) = named(stream, at + "POSTPONE".len())
        {
            stream.pos = end;
            return Some(Kind::Property);
        }
    }
    stream.pos = after_word;
    let upper = word.to_ascii_uppercase();
    let listed = |list: &str| list.split_ascii_whitespace().any(|known| known == upper);
    if state.words.iter().rev().any(|known| *known == upper) {
        return Some(Kind::Variable);
    }
    if word == "\\" {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if listed(FORTH_CORE) {
        return Some(Kind::Property);
    }
    if listed(FORTH_IMMEDIATE) {
        return Some(Kind::Keyword);
    }
    let read_to = |stream: &mut Stream, close: u8| {
        stream.eat_while(|byte| byte != close);
        stream.eat(|byte| byte == close);
    };
    match word.as_str() {
        "(" => {
            read_to(stream, b')');
            Some(Kind::Comment)
        }
        ".(" => {
            read_to(stream, b')');
            Some(Kind::String)
        }
        "S\"" | ".\"" | "C\"" => {
            read_to(stream, b'"');
            Some(Kind::String)
        }
        _ if forth_number(&word) => Some(Kind::Number),
        _ => None,
    }
}

/// What Factor reads next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Factor {
    #[default]
    Start,
    Vocabulary,
    String,
    /// After `STRING:`, to a line starting `;`.
    Lines,
    /// A `"""` string.
    Triple,
    /// A word's `( -- )` stack effect.
    Stack,
}

/// Factor, as CodeMirror 5's factor mode (a simple mode) colors it, rule by rule: `!` comments,
/// strings, numbers, `:`, `M:`, `NAME:` definitions (the keyword, the defined name plain),
/// `USING:` and `IN:` vocabularies tags, a stack effect meta, words ending in `[`, `{` and
/// `t`, `f`, `if`, … keywords, `foo?`, `foo>`, `>foo` builtins, other words variables.
pub fn factor(text: &str, state: &mut Factor, out: &mut Out) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        for (range, kind) in factor_token(&mut stream, state) {
            out.mark(range, kind);
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
    }
}

fn factor_token(stream: &mut Stream, state: &mut Factor) -> Vec<(Range<usize>, Option<Kind>)> {
    let at = stream.pos;
    let bytes = stream.bytes;
    let len = bytes.len();
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let run_while = |from: usize, ok: &dyn Fn(u8) -> bool| (from..len).find(|end| !ok(byte(*end))).unwrap_or(len.max(from));
    let non_space = |from: usize| run_while(from, &|next| !space(next));
    let spaces = |from: usize| run_while(from, &space);
    let starts = |from: usize, prefix: &str| scan::at(bytes, from, prefix);
    let run = non_space(at);
    let word = bytes.get(at..run).unwrap_or_default();
    let take = |stream: &mut Stream, end: usize, kind: Option<Kind>| {
        stream.pos = end;
        vec![(at..end, kind)]
    };
    match *state {
        Factor::Start => {}
        Factor::Vocabulary => {
            if byte(at) == b';' {
                *state = Factor::Start;
                return take(stream, at + 1, Some(Kind::Keyword));
            }
            if run > at {
                return take(stream, run, Some(Kind::Operator));
            }
            return take(stream, spaces(at).max(at + 1), None);
        }
        Factor::String | Factor::Triple => {
            let close: &[u8] = if *state == Factor::String { b"\"" } else { b"\"\"\"" };
            let mut index = at;
            while index < len {
                if byte(index) == b'\\' {
                    index += 2;
                    continue;
                }
                if bytes.get(index..index + close.len()) == Some(close) {
                    *state = Factor::Start;
                    return take(stream, index + close.len(), Some(Kind::String));
                }
                index += 1;
            }
            return take(stream, len, Some(Kind::String));
        }
        Factor::Lines => {
            if byte(at) == b';' {
                *state = Factor::Start;
                return take(stream, at + 1, Some(Kind::Keyword));
            }
            return take(stream, len, Some(Kind::String));
        }
        Factor::Stack => {
            if byte(at) == b')' {
                *state = Factor::Start;
                return take(stream, at + 1, Some(Kind::Punctuation));
            }
            if starts(at, "--") {
                return take(stream, at + 2, Some(Kind::Punctuation));
            }
            if run > at {
                return take(stream, run, Some(Kind::Comment));
            }
            return take(stream, spaces(at).max(at + 1), None);
        }
    }
    // `#?!.*`
    let bang = if byte(at) == b'#' { at + 1 } else { at };
    if byte(bang) == b'!' {
        return take(stream, len, Some(Kind::Comment));
    }
    if starts(at, "\"\"\"") {
        *state = Factor::Triple;
        return take(stream, at + 3, Some(Kind::String));
    }
    if starts(at, "STRING:") && space(byte(at + 7)) {
        *state = Factor::Lines;
        stream.pos = at + 8;
        return vec![(at..at + 7, Some(Kind::Keyword))];
    }
    // `\S*?"`: to the first quote in the word.
    if let Some(quote) = word.iter().position(|byte| *byte == b'"') {
        *state = Factor::String;
        return take(stream, at + quote + 1, Some(Kind::String));
    }
    if let Some(end) = factor_number(stream) {
        return take(stream, end, Some(Kind::Number));
    }
    // `((?:GENERIC)|\:?\:)(\s+)(\S+)(\s+)(\()`, then `(M\:)(\s+)(\S+)(\s+)(\S+)`.
    let head = if starts(at, "GENERIC") {
        Some(at + 7)
    } else if starts(at, "::") && space(byte(at + 2)) {
        Some(at + 2)
    } else if starts(at, ":") {
        Some(at + 1)
    } else {
        None
    };
    if let Some(head) = head {
        let gap = spaces(head);
        let name = non_space(gap);
        let open = spaces(name);
        if gap > head && name > gap && open > name && byte(open) == b'(' {
            *state = Factor::Stack;
            stream.pos = open + 1;
            return vec![(at..head, Some(Kind::Keyword)), (gap..name, None), (open..open + 1, Some(Kind::Punctuation))];
        }
    }
    if starts(at, "M:") {
        let gap = spaces(at + 2);
        let name = non_space(gap);
        let second = spaces(name);
        let end = non_space(second);
        if gap > at + 2 && name > gap && second > name && end > second {
            stream.pos = end;
            return vec![(at..at + 2, Some(Kind::Keyword)), (gap..name, None), (second..end, Some(Kind::Operator))];
        }
    }
    if starts(at, "USING:") {
        *state = Factor::Vocabulary;
        return take(stream, at + 6, Some(Kind::Keyword));
    }
    // `(USE\:|IN\:)(\s+)(\S+)`, then `(\S+\:)(\s+)(\S+)`, each to a space or the end.
    for (prefix, second) in [("USE:", Some(Kind::Operator)), ("IN:", Some(Kind::Operator)), ("", None)] {
        let head =
            if prefix.is_empty() { (run > at + 1 && byte(run - 1) == b':').then_some(run) } else { starts(at, prefix).then_some(at + prefix.len()) };
        let Some(head) = head else { continue };
        let gap = spaces(head);
        let end = non_space(gap);
        if gap > head && end > gap {
            stream.pos = end;
            return vec![(at..head, Some(Kind::Keyword)), (gap..end, second)];
        }
    }
    if run > at {
        let keyword = matches!(
            word,
            b";" | b"\\" | b"t" | b"f" | b"if" | b"loop" | b"while" | b"until" | b"do" | b"PRIVATE>" | b"<PRIVATE" | b"." | b"]" | b"}"
        ) || word.ends_with(b"[")
            || word.ends_with(b"{");
        if keyword {
            return take(stream, run, Some(Kind::Keyword));
        }
        let last = word.last().copied().unwrap_or(0);
        let first = word.first().copied().unwrap_or(0);
        if word.len() >= 2 && (b")>.*?".contains(&last) || b")><".contains(&first)) {
            return take(stream, run, Some(Kind::Property));
        }
        if word.len() == 1 && b"+-=/*<>".contains(&first) {
            return take(stream, run, Some(Kind::Keyword));
        }
        return take(stream, run, Some(Kind::Variable));
    }
    take(stream, spaces(at).max(at + 1), None)
}

/// `0x[\d,a-f]+`, `0o[0-7]+`, `0b[0,1]+`, or `-?\d+.?\d*` before a space (backtracking as the
/// pattern does): where it ends.
fn factor_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    for (prefix, ok) in [("0x", b"0123456789,abcdef".as_slice()), ("0o", b"01234567"), ("0b", b"0,1")] {
        if stream.at(prefix) {
            let end = run_of(stream, at + 2, |byte| ok.contains(&byte));
            if end > at + 2 {
                return Some(end);
            }
        }
    }
    let sign = if stream.byte(at) == b'-' { at + 1 } else { at };
    let digits = run_of(stream, sign, |byte| byte.is_ascii_digit());
    let before_space = |end: usize| end < stream.bytes.len() && space(stream.byte(end));
    for whole in (sign + 1..=digits).rev() {
        let dots: &[usize] = if whole < stream.bytes.len() { &[1, 0] } else { &[0] };
        for dot in dots {
            let fraction = whole + dot;
            let most = run_of(stream, fraction, |byte| byte.is_ascii_digit());
            if let Some(end) = (fraction..=most).rev().find(|end| before_space(*end)) {
                return Some(end);
            }
        }
    }
    None
}
