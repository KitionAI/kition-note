//! Verilog and SystemVerilog, as CodeMirror 5's verilog mode colors them, ported token for
//! token: keywords, names variables, `` `directives `` and `#delays` definitions, `$system`
//! tasks and operators meta, sized and real numbers, strings, `//` and `/* */` comments.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Verilog {
    /// An open `/* */` comment.
    comment: bool,
    /// A string left open by a backslash at its line's end.
    string: bool,
}

const KEYWORDS: &str = "accept_on alias always always_comb always_ff always_latch and assert assign assume automatic before \
begin bind bins binsof bit break buf bufif0 bufif1 byte case casex casez cell chandle checker class clocking cmos config const \
constraint context continue cover covergroup coverpoint cross deassign default defparam design disable dist do edge else end \
endcase endchecker endclass endclocking endconfig endfunction endgenerate endgroup endinterface endmodule endpackage \
endprimitive endprogram endproperty endspecify endsequence endtable endtask enum event eventually expect export extends extern \
final first_match for force foreach forever fork forkjoin function generate genvar global highz0 highz1 if iff ifnone \
ignore_bins illegal_bins implements implies import incdir include initial inout input inside instance int integer interconnect \
interface intersect join join_any join_none large let liblist library local localparam logic longint macromodule matches medium \
modport module nand negedge nettype new nexttime nmos nor noshowcancelled not notif0 notif1 null or output package packed \
parameter pmos posedge primitive priority program property protected pull0 pull1 pulldown pullup pulsestyle_ondetect \
pulsestyle_onevent pure rand randc randcase randsequence rcmos real realtime ref reg reject_on release repeat restrict return \
rnmos rpmos rtran rtranif0 rtranif1 s_always s_eventually s_nexttime s_until s_until_with scalared sequence shortint shortreal \
showcancelled signed small soft solve specify specparam static string strong strong0 strong1 struct super supply0 supply1 \
sync_accept_on sync_reject_on table tagged task this throughout time timeprecision timeunit tran tranif0 tranif1 tri tri0 tri1 \
triand trior trireg type typedef union unique unique0 unsigned until until_with untyped use uwire var vectored virtual void wait \
wait_order wand weak weak0 weak1 while wildcard wire with within wor xnor xor";

fn name(byte: u8) -> bool {
    scan::js_word(byte) || byte == b'$'
}

fn digits(stream: &Stream, from: usize) -> usize {
    (from..).find(|at| !(stream.byte(*at).is_ascii_digit() || stream.byte(*at) == b'_')).unwrap_or(from)
}

/// A real number: `\d[\d_]*(\.\d[\d_]*)?E-?[\d_]+`, else `\d[\d_]*\.\d[\d_]*` (any case).
fn real(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    if !stream.byte(at).is_ascii_digit() {
        return None;
    }
    let whole = digits(stream, at + 1);
    let fraction = (stream.byte(whole) == b'.' && stream.byte(whole + 1).is_ascii_digit()).then(|| digits(stream, whole + 2));
    let exponent = |from: usize| {
        if !stream.byte(from).eq_ignore_ascii_case(&b'e') {
            return None;
        }
        let sign = if stream.byte(from + 1) == b'-' { from + 2 } else { from + 1 };
        let end = digits(stream, sign);
        (end > sign).then_some(end)
    };
    fraction.and_then(exponent).or_else(|| exponent(whole)).or(fraction)
}

/// A based number: `\d*\s*'s?<base>\s*<first><rest or _>*` (any case).
fn based(stream: &Stream, base: u8, digit: impl Fn(u8) -> bool) -> Option<usize> {
    let mut at = stream.pos;
    while stream.byte(at).is_ascii_digit() {
        at += 1;
    }
    while stream.byte(at).is_ascii_whitespace() {
        at += 1;
    }
    if stream.byte(at) != b'\'' {
        return None;
    }
    at += 1;
    if stream.byte(at).eq_ignore_ascii_case(&b's') && stream.byte(at + 1).eq_ignore_ascii_case(&base) {
        at += 1;
    }
    if !stream.byte(at).eq_ignore_ascii_case(&base) {
        return None;
    }
    at += 1;
    while stream.byte(at).is_ascii_whitespace() {
        at += 1;
    }
    let lower = |at: usize| stream.byte(at).to_ascii_lowercase();
    if !digit(lower(at)) {
        return None;
    }
    at += 1;
    while digit(lower(at)) || lower(at) == b'_' {
        at += 1;
    }
    Some(at)
}

fn number(stream: &Stream) -> Option<usize> {
    real(stream)
        .or_else(|| based(stream, b'd', |byte| byte.is_ascii_digit()))
        .or_else(|| based(stream, b'b', |byte| b"xz01".contains(&byte)))
        .or_else(|| based(stream, b'o', |byte| b"xz01234567".contains(&byte)))
        .or_else(|| based(stream, b'h', |byte| byte.is_ascii_hexdigit() || b"xz?".contains(&byte)))
        .or_else(|| stream.byte(stream.pos).is_ascii_digit().then(|| digits(stream, stream.pos + 1)))
}

pub fn line(text: &str, state: &mut Verilog, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return comment(stream, state);
        }
        if state.string {
            return string(stream, state);
        }
        token(stream, state)
    });
}

fn token(stream: &mut Stream, state: &mut Verilog) -> Option<Kind> {
    let here = stream.peek();
    if b",;:.".contains(&here) {
        stream.next();
        return None;
    }
    if b"[]{}()".contains(&here) {
        stream.next();
        return Some(Kind::Punctuation);
    }
    match here {
        b'`' => {
            stream.next();
            // A directive: a definition (the mode reads on to a `(` and backs up again).
            stream.eat_while(name);
            return None;
        }
        b'$' => {
            stream.next();
            return stream.eat_while(name).then_some(Kind::Comment);
        }
        b'#' => {
            stream.next();
            stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'_' || byte == b'.');
            return None;
        }
        b'@' => {
            stream.next();
            stream.eat_while(|byte| byte == b'@');
            return None;
        }
        b'"' => {
            stream.next();
            return string(stream, state);
        }
        b'/' => {
            stream.next();
            if stream.eat(|byte| byte == b'*').is_some() {
                return comment(stream, state);
            }
            if stream.eat(|byte| byte == b'/').is_some() {
                stream.skip_to_end();
                return Some(Kind::Comment);
            }
            stream.back_up(1);
        }
        _ => {}
    }
    if let Some(end) = number(stream) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    if stream.eat_while(|byte| b"+-*/!~&|^%=?:<>".contains(&byte)) {
        return Some(Kind::Comment);
    }
    if stream.eat_while(name) {
        let word = stream.current();
        return Some(if KEYWORDS.split_ascii_whitespace().any(|known| known == word) { Kind::Keyword } else { Kind::Variable });
    }
    stream.next();
    None
}

/// A string to its quote; left open only by a backslash at the line's end.
fn string(stream: &mut Stream, state: &mut Verilog) -> Option<Kind> {
    let mut escaped = false;
    let mut closed = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            closed = true;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    state.string = !closed && escaped;
    Some(Kind::String)
}

fn comment(stream: &mut Stream, state: &mut Verilog) -> Option<Kind> {
    let mut star = false;
    state.comment = true;
    while let Some(here) = stream.next() {
        if here == b'/' && star {
            state.comment = false;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}
