//! The ML family (OCaml, F#, Standard ML, and the bare mode), as CodeMirror 5's mllike mode
//! colors it: its keywords and each
//! dialect's extra words (keywords, builtins, types), `~label`s, numbers, strings, `{| |}` long
//! strings, nested `(* *)` comments (and F#'s `//`); every operator character an operator.

use super::Kind;
use super::scan::{self, Out, Stream};

/// Which dialect's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    OCaml,
    FSharp,
    Sml,
    /// The mode with no dialect's words.
    Plain,
}

const KEYWORDS: &str = "as do else end exception fun functor if in include let of open rec struct then type val while with";
const OCAML_KEYWORDS: &str = "and assert begin class constraint done downto external function initializer lazy match method module \
mutable new nonrec object private sig to try value virtual when";
const OCAML_BUILTINS: &str = "raise failwith true false asr land lor lsl lsr lxor mod or raise_notrace trace exit print_string \
print_endline List";
const OCAML_TYPES: &str = "int float bool char string unit";
const FSHARP_KEYWORDS: &str = "abstract assert base begin class default delegate done downcast downto elif extern finally for function \
global inherit inline interface internal lazy match member module mutable namespace new null override private public return select \
static to try upcast use void when yield atomic break checked component const constraint constructor continue eager event external \
fixed method mixin object parallel process protected pure sealed tailcall trait virtual volatile";
const FSHARP_BUILTINS: &str = "List Seq Map Set Option int string not true false raise failwith";
const SML_KEYWORDS: &str = "abstype and andalso case datatype fn handle infix infixr local nonfix op orelse raise withtype eqtype \
sharing sig signature structure where true false";
const SML_BUILTINS: &str = "int real string char bool";

#[derive(Clone, Debug, Default)]
pub struct Ml {
    string: bool,
    long_string: bool,
    comments: usize,
}

fn style(word: &str, dialect: Dialect) -> Kind {
    let has = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
    match dialect {
        Dialect::OCaml if has(OCAML_BUILTINS) => Kind::Property,
        Dialect::OCaml if has(OCAML_TYPES) => Kind::Property,
        Dialect::OCaml if has(KEYWORDS) || has(OCAML_KEYWORDS) => Kind::Keyword,
        Dialect::FSharp if has(FSHARP_BUILTINS) => Kind::Property,
        Dialect::FSharp if has(KEYWORDS) || has(FSHARP_KEYWORDS) => Kind::Keyword,
        Dialect::Sml if has(SML_BUILTINS) => Kind::Property,
        Dialect::Sml if has(KEYWORDS) || has(SML_KEYWORDS) => Kind::Keyword,
        Dialect::Plain if has(KEYWORDS) => Kind::Keyword,
        _ => Kind::Variable,
    }
}

pub fn line(text: &str, state: &mut Ml, dialect: Dialect, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.string {
            return string(stream, state);
        }
        if state.long_string {
            return long_string(stream, state);
        }
        if state.comments > 0 {
            return comment(stream, state);
        }
        token(stream, state, dialect)
    });
}

fn token(stream: &mut Stream, state: &mut Ml, dialect: Dialect) -> Option<Kind> {
    let here = stream.next()?;
    if here == b'"' {
        state.string = true;
        return string(stream, state);
    }
    if here == b'{' && stream.eat(|byte| byte == b'|').is_some() {
        state.long_string = true;
        return long_string(stream, state);
    }
    if here == b'(' && stream.eat(|byte| byte == b'*').is_some() {
        state.comments += 1;
        return comment(stream, state);
    }
    if here == b'~' || here == b'?' {
        stream.eat_while(scan::word);
        return Some(Kind::Variable);
    }
    if here == b'`' {
        stream.eat_while(scan::word);
        return None;
    }
    if here == b'/' && matches!(dialect, Dialect::FSharp | Dialect::Sml) && stream.eat(|byte| byte == b'/').is_some() {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here.is_ascii_digit() {
        if here == b'0' && stream.eat(|byte| byte == b'b' || byte == b'B').is_some() {
            stream.eat_while(|byte| byte == b'0' || byte == b'1');
        }
        if here == b'0' && stream.eat(|byte| byte == b'x' || byte == b'X').is_some() {
            stream.eat_while(|byte| byte.is_ascii_hexdigit());
        }
        if here == b'0' && stream.eat(|byte| byte == b'o' || byte == b'O').is_some() {
            stream.eat_while(|byte| (b'0'..=b'7').contains(&byte));
        } else {
            stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'_');
            if stream.eat(|byte| byte == b'.').is_some() {
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
            if stream.eat(|byte| byte == b'e' || byte == b'E').is_some() {
                stream.eat_while(|byte| byte.is_ascii_digit() || byte == b'-' || byte == b'+');
            }
        }
        return Some(Kind::Number);
    }
    if b"+-*&%=<>!?|@.~:".contains(&here) {
        return Some(Kind::Operator);
    }
    if scan::word(here) {
        stream.eat_while(scan::word);
        return Some(style(stream.current(), dialect));
    }
    None
}

fn string(stream: &mut Stream, state: &mut Ml) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            state.string = false;
            return Some(Kind::String);
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn long_string(stream: &mut Stream, state: &mut Ml) -> Option<Kind> {
    let mut previous = 0;
    while state.long_string
        && let Some(here) = stream.next()
    {
        if previous == b'|' && here == b'}' {
            state.long_string = false;
        }
        previous = here;
    }
    Some(Kind::String)
}

fn comment(stream: &mut Stream, state: &mut Ml) -> Option<Kind> {
    let mut previous = 0;
    while state.comments > 0
        && let Some(here) = stream.next()
    {
        if previous == b'(' && here == b'*' {
            state.comments += 1;
        }
        if previous == b'*' && here == b')' {
            state.comments -= 1;
        }
        previous = here;
    }
    Some(Kind::Comment)
}
