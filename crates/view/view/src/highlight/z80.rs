//! Z80 assembly, as CodeMirror 5's z80 mode colors it, ported token for token: on an indented
//! line instructions and jumps keywords, registers and conditions after them plain, numbers
//! after a register, undocumented registers errors; `.directives` and `#defines` plain, `;`
//! comments, strings, `'c'`, `$hex`, and `%binary` numbers. An unindented line's words (its
//! labels) are plain.

use super::Kind;
use super::scan::{self, Out};

/// The mode's patterns, each written out as the words it takes (in any case).
const INSTRUCTIONS: &str = "adc add and bit ccf cp cpd cpdr cpi cpir cpl daa dec di djnz ei ex exx halt im in inc ind indr ini \
inir ld ldd lddr ldi ldir neg nop or otdr otir out outd outi pop push res rl rla rlc rlca rld rr rra rrc rrca rrd rst sbc scf set \
sla sra srl sub xor";
const JUMPS: &str = "b_call b_jump bcall bjump call jp jr ret reti retn";
const REGISTERS: &str = "a af b bc c d de e h hl i ix iy l r sp";
const CONDITIONS: &str = "c m nc nz p pe po z";
const UNDOCUMENTED: &str = "hx hy ixh ixl iyh iyl lx ly slia sll";

/// What came last on the line: an instruction, a jump, a register, or a directive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Z80 {
    #[default]
    None,
    Instruction,
    Jump,
    Operand,
    Directive,
}

fn listed(list: &str, word: &str) -> bool {
    list.split_ascii_whitespace().any(|known| known.eq_ignore_ascii_case(word))
}

/// `[\da-f]+h`, `[0-7]+o`, `[01]+b`, or `\d+d?`, the whole word (any case).
fn number(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let all = |digits: &str, ok: fn(char) -> bool| !digits.is_empty() && digits.chars().all(ok);
    let suffixed = |suffix: char, ok: fn(char) -> bool| lower.strip_suffix(suffix).is_some_and(|digits| all(digits, ok));
    suffixed('h', |digit| digit.is_ascii_hexdigit())
        || suffixed('o', |digit| ('0'..='7').contains(&digit))
        || suffixed('b', |digit| digit == '0' || digit == '1')
        || all(&lower, |digit| digit.is_ascii_digit())
        || suffixed('d', |digit| digit.is_ascii_digit())
}

pub fn line(text: &str, state: &mut Z80, out: &mut Out) {
    let indented = text.starts_with([' ', '\t']);
    scan::run(text, out, |stream| {
        if stream.pos == 0 {
            *state = Z80::None;
        }
        if stream.eat_space() {
            return None;
        }
        if stream.eat_while(scan::js_word) {
            let word = stream.current();
            if !indented {
                return None;
            }
            if matches!(*state, Z80::Instruction | Z80::Operand) && listed(REGISTERS, word) {
                *state = Z80::Operand;
                return None;
            }
            if *state == Z80::Jump && listed(CONDITIONS, word) {
                *state = Z80::Operand;
                return None;
            }
            if listed(INSTRUCTIONS, word) {
                *state = Z80::Instruction;
                return Some(Kind::Keyword);
            }
            if listed(JUMPS, word) {
                *state = Z80::Jump;
                return Some(Kind::Keyword);
            }
            if *state == Z80::Operand && number(word) {
                return Some(Kind::Number);
            }
            if listed(UNDOCUMENTED, word) {
                return Some(Kind::Operator);
            }
            return None;
        }
        if stream.take(";") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if stream.take("\"") {
            while let Some(here) = stream.next() {
                if here == b'"' {
                    break;
                }
                if here == b'\\' {
                    stream.next();
                }
            }
            return Some(Kind::String);
        }
        if stream.take("'") {
            // `\\?.'`: with the backslash, else without it.
            let at = stream.pos;
            let quote = |at: usize| (at < stream.bytes.len() && stream.byte(at) == b'\'').then_some(at + 1);
            let end = if stream.byte(at) == b'\\' { quote(at + 2) } else { None }.or_else(|| quote(at + 1).filter(|_| at < stream.bytes.len()));
            if let Some(end) = end {
                stream.pos = end;
                return Some(Kind::Number);
            }
            return None;
        }
        if stream.take(".") || (stream.sol() && stream.take("#")) {
            *state = Z80::Directive;
            stream.eat_while(scan::js_word);
            return None;
        }
        if stream.take("$") {
            return stream.eat_while(|byte| byte.is_ascii_hexdigit()).then_some(Kind::Number);
        }
        if stream.take("%") {
            return stream.eat_while(|byte| byte == b'0' || byte == b'1').then_some(Kind::Number);
        }
        stream.next();
        None
    });
}
