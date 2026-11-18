//! VBScript, as CodeMirror 5's vbscript mode colors it, ported token for token: keywords and
//! block words, `And`, `Not`, … and symbols operators, `True`, `Nothing`, … atoms (plain), the
//! runtime's functions builtins, its objects, constants, and members variables, other names
//! variables, `.member` names, numbers, strings, `'` and `Rem` comments, anything else an error.

use super::Kind;
use super::scan::{self, Out, Stream};

pub fn line(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        let style = token(stream);
        if stream.current() != "." {
            return style;
        }
        // A member: the dot and the name after it together, a variable (else the mode's error).
        match token(stream) {
            Some(Kind::Variable | Kind::Property | Kind::Keyword) => Some(Kind::Variable),
            _ => Some(Kind::Operator),
        }
    });
}

/// A number at the position: where it ends.
fn number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let lower = |index: usize| stream.byte(index).to_ascii_lowercase();
    let digit_or_dot = |byte: u8| byte.is_ascii_digit() || byte == b'.';
    let prefixed = lower(at) == b'&' && matches!(lower(at + 1), b'h' | b'o');
    let body = if prefixed { at + 2 } else { at };
    if !digit_or_dot(stream.byte(body)) {
        return None;
    }
    let run = (body..).find(|index| !digit_or_dot(stream.byte(*index))).unwrap_or(body);
    if lower(run).is_ascii_lowercase() || lower(run) == b'_' {
        return None;
    }
    let digits = |from: usize| (from..).find(|index| !stream.byte(*index).is_ascii_digit()).unwrap_or(from);
    let eat = |end: usize, letter: u8| if lower(end) == letter { end + 1 } else { end };
    // Floats: `\d*\.\d+`, `\d+\.\d*`, `\.\d+` (then `J`).
    let whole = digits(at);
    if stream.byte(whole) == b'.' && digits(whole + 1) > whole + 1 {
        return Some(eat(digits(whole + 1), b'j'));
    }
    if whole > at && stream.byte(whole) == b'.' {
        return Some(eat(digits(whole + 1), b'j'));
    }
    // Integers: `&H…`, `&O…`, `[1-9]\d*F?` (then `J`), `0` not before a digit or `x` (then `L`).
    let hex = (at + 2..).find(|index| !stream.byte(*index).is_ascii_hexdigit()).unwrap_or(at + 2);
    let octal = (at + 2..).find(|index| !(b'0'..=b'7').contains(&stream.byte(*index))).unwrap_or(at + 2);
    let integer = if prefixed && lower(at + 1) == b'h' && hex > at + 2 {
        hex
    } else if prefixed && lower(at + 1) == b'o' && octal > at + 2 {
        octal
    } else if (b'1'..=b'9').contains(&stream.byte(at)) {
        eat(if stream.byte(whole) == b'F' { whole + 1 } else { whole }, b'j')
    } else if stream.byte(at) == b'0' && !stream.byte(at + 1).is_ascii_digit() && lower(at + 1) != b'x' {
        at + 1
    } else {
        return None;
    };
    Some(eat(integer, b'l'))
}

/// The next token's style.
fn token(stream: &mut Stream) -> Option<Kind> {
    if stream.eat_space() {
        return None;
    }
    if stream.peek() == b'\'' || stream.take_word_of("rem", true) {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if let Some(end) = number(stream) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    if stream.take("\"") {
        // To the next quote (a doubled quote is two strings); never past the line.
        while !stream.eol() {
            stream.eat_while(|byte| byte != b'\'' && byte != b'"');
            if stream.take("\"") {
                break;
            }
            stream.eat(|byte| byte == b'\'' || byte == b'"');
        }
        return Some(Kind::String);
    }
    if stream.take("<>") || stream.take("<=") || stream.take(">=") {
        return Some(Kind::Operator);
    }
    if b"+-*/&\\^<>=".contains(&stream.peek()) {
        stream.next();
        return Some(Kind::Operator);
    }
    if stream.take_word_of(OPERATOR_WORDS, true) {
        return Some(Kind::Operator);
    }
    if matches!(stream.peek(), b'.' | b',') {
        stream.next();
        return None;
    }
    if matches!(stream.peek(), b'(' | b')') {
        stream.next();
        return Some(Kind::Punctuation);
    }
    for words in [RESUME, "do", OPENING, MIDDLE, "end", CLOSING, KEYWORDS] {
        if stream.take_word_of(words, true) {
            return Some(Kind::Keyword);
        }
    }
    if stream.take_word_of(ATOMS, true) {
        return None;
    }
    if stream.take_word_of(MEMBERS, true) {
        return Some(Kind::Variable);
    }
    if stream.take_word_of(BUILTINS, true) {
        return Some(Kind::Property);
    }
    if stream.take_word_of(OBJECTS, true) {
        return Some(Kind::Variable);
    }
    if stream.peek().is_ascii_alphabetic() {
        stream.eat_while(scan::js_word);
        return Some(Kind::Variable);
    }
    stream.next();
    Some(Kind::Operator)
}

const OPERATOR_WORDS: &str = "and|or|not|xor|is|mod|eqv|imp";

const OBJECTS: &str = "WScript|err|debug|RegExp|vbBlack|vbRed|vbGreen|vbYellow|vbBlue|vbMagenta|vbCyan|vbWhite|vbBinaryCompare|vbTextCompare|\
vbSunday|vbMonday|vbTuesday|vbWednesday|vbThursday|vbFriday|vbSaturday|vbUseSystemDayOfWeek|vbFirstJan1|\
vbFirstFourDays|vbFirstFullWeek|vbGeneralDate|vbLongDate|vbShortDate|vbLongTime|vbShortTime|vbObjectError|vbOKOnly|\
vbOKCancel|vbAbortRetryIgnore|vbYesNoCancel|vbYesNo|vbRetryCancel|vbCritical|vbQuestion|vbExclamation|vbInformation|\
vbDefaultButton1|vbDefaultButton2|vbDefaultButton3|vbDefaultButton4|vbApplicationModal|vbSystemModal|vbOK|vbCancel|\
vbAbort|vbRetry|vbIgnore|vbYes|vbNo|vbCr|VbCrLf|vbFormFeed|vbLf|vbNewLine|vbNullChar|vbNullString|vbTab|vbVerticalTab|\
vbUseDefault|vbTrue|vbFalse|vbEmpty|vbNull|vbInteger|vbLong|vbSingle|vbDouble|vbCurrency|vbDate|vbString|vbObject|\
vbError|vbBoolean|vbVariant|vbDataObject|vbDecimal|vbByte|vbArray";

const MEMBERS: &str = "clear|execute|raise|replace|test|write|writeline|close|open|state|eof|update|addnew|end|createobject|quit|description|\
firstindex|global|helpcontext|helpfile|ignorecase|length|number|pattern|source|value|count";

const BUILTINS: &str = "abs|array|asc|atn|cbool|cbyte|ccur|cdate|cdbl|chr|cint|clng|cos|csng|cstr|date|dateadd|datediff|datepart|dateserial|\
datevalue|day|escape|eval|execute|exp|filter|formatcurrency|formatdatetime|formatnumber|formatpercent|getlocale|\
getobject|getref|hex|hour|inputbox|instr|instrrev|int|fix|isarray|isdate|isempty|isnull|isnumeric|isobject|join|\
lbound|lcase|left|len|loadpicture|log|ltrim|rtrim|trim|maths|mid|minute|month|monthname|msgbox|now|oct|replace|rgb|\
right|rnd|round|scriptengine|scriptenginebuildversion|scriptenginemajorversion|scriptengineminorversion|second|\
setlocale|sgn|sin|space|split|sqr|strcomp|string|strreverse|tan|time|timer|timeserial|timevalue|typename|ubound|ucase|\
unescape|vartype|weekday|weekdayname|year";

const OPENING: &str = "class|sub|select|while|if|function|property|with|for";

const MIDDLE: &str = "else|elseif|case";

const CLOSING: &str = "next|loop|wend";

const ATOMS: &str = "true|false|nothing|empty|null";

const KEYWORDS: &str = "dim|redim|then|until|randomize|byval|byref|new|property|exit|in|const|private|public|get|set|let|stop|\
on error resume next|on error goto 0|option explicit|call|me";

const RESUME: &str = "on error resume next|exit";
