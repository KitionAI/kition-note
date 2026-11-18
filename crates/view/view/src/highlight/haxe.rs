//! Haxe, as CodeMirror 5's haxe mode colors it, ported token for token with its parser (the
//! combinators that mark a name declared inside a block or function a definition and a name
//! after `.` or an object's key a property): keywords, `true`, `null`, … atoms (plain),
//! `public`, `static`, … attributes, capitalized names types (variable-3), other names
//! variables, numbers, strings, `~/regex/`, `#conditionals` and `@:metadata` meta, `//` and
//! `/* */` comments.

use super::Kind;
use super::scan::{self, Out, Stream};

/// One of the parser's combinators, waiting on the stack.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Combinator {
    Statement,
    Expression,
    MaybeExpression,
    MaybeOperator,
    Attribute,
    Metadef,
    Metaargs,
    Importdef,
    Typedef,
    MaybeLabel,
    Property,
    ObjectProperty,
    /// A list of `what` split by `,` to `end`, and the part after each item.
    CommaSeparated(Box<Combinator>, &'static str),
    CommaSeparatedNext(Box<Combinator>, &'static str),
    Block,
    Vardef,
    VardefNext,
    ForSpec,
    ForIn,
    FunctionDef,
    TypeUse,
    TypeString,
    TypeProperty,
    FunctionArgument,
    Expect(&'static str),
    PushLex,
    PopLex,
    PushContext,
    PopContext,
}

impl Combinator {
    /// Runs as soon as it is on top (the mode's `lex` functions).
    fn immediate(&self) -> bool {
        matches!(self, Combinator::PushLex | Combinator::PopLex | Combinator::PopContext)
    }
}

#[derive(Clone, Debug)]
pub struct Haxe {
    comment: bool,
    string: Option<u8>,
    regex_allowed: bool,
    keyword_allowed: bool,
    stack: Vec<Combinator>,
    /// How many contexts (blocks, functions) are open: a name declared in one is a definition.
    contexts: usize,
}

impl Default for Haxe {
    fn default() -> Self {
        Haxe { comment: false, string: None, regex_allowed: true, keyword_allowed: true, stack: Vec::new(), contexts: 0 }
    }
}

/// A token's type (what the parser reads), style, and text.
struct Token<'a> {
    kind: &'static str,
    style: Option<&'static str>,
    content: &'a str,
}

fn keyword(word: &str) -> Option<(&'static str, &'static str)> {
    Some(match word {
        "if" | "while" => ("keyword a", "keyword"),
        "else" | "do" | "try" => ("keyword b", "keyword"),
        "return" | "break" | "continue" | "new" | "throw" => ("keyword c", "keyword"),
        "var" => ("var", "keyword"),
        "inline" | "static" | "public" | "private" => ("attribute", "attribute"),
        "using" | "import" => ("import", "keyword"),
        "cast" => ("cast", "keyword"),
        "macro" => ("macro", "keyword"),
        "function" => ("function", "keyword"),
        "catch" => ("catch", "keyword"),
        "untyped" => ("untyped", "keyword"),
        "callback" => ("cb", "keyword"),
        "for" => ("for", "keyword"),
        "switch" => ("switch", "keyword"),
        "case" => ("case", "keyword"),
        "default" => ("default", "keyword"),
        "in" => ("operator", "keyword"),
        "never" => ("property_access", "keyword"),
        "trace" => ("trace", "keyword"),
        "class" | "abstract" | "enum" | "interface" | "typedef" | "extends" | "implements" | "dynamic" => ("typedef", "keyword"),
        "true" | "false" | "null" => ("atom", "atom"),
        _ => return None,
    })
}

fn operator(byte: u8) -> bool {
    b"+-*&%=<>!?|".contains(&byte)
}

/// A string's text to its quote: whether it closed.
fn string_body(stream: &mut Stream, quote: u8) -> bool {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            return true;
        }
        escaped = !escaped && here == b'\\';
    }
    false
}

fn lex<'a>(stream: &mut Stream<'a>, state: &mut Haxe) -> Token<'a> {
    let token = |kind, style| Token { kind, style, content: "" };
    if state.comment {
        return block_comment(stream, state);
    }
    if let Some(quote) = state.string {
        if string_body(stream, quote) {
            state.string = None;
        }
        return token("string", Some("string"));
    }
    let Some(here) = stream.next() else { return token("variable", None) };
    if here == b'"' || here == b'\'' {
        if !string_body(stream, here) {
            state.string = Some(here);
        }
        return token("string", Some("string"));
    }
    let single = |stream: &Stream<'a>| Token { kind: single_kind(here), style: None, content: stream.current_static() };
    if b"[]{}(),;:.".contains(&here) {
        return single(stream);
    }
    if here == b'0' && stream.eat(|byte| byte == b'x' || byte == b'X').is_some() {
        stream.eat_while(|byte| byte.is_ascii_hexdigit());
        return token("number", Some("number"));
    }
    if here.is_ascii_digit() || (here == b'-' && stream.eat(|byte| byte.is_ascii_digit()).is_some()) {
        // `\d*(?:\.\d*(?!\.))?(?:[eE][+\-]?\d+)?`
        stream.eat_while(|byte| byte.is_ascii_digit());
        if stream.peek() == b'.' {
            let digits = (stream.pos + 1..stream.bytes.len()).find(|at| !stream.byte(*at).is_ascii_digit()).unwrap_or(stream.bytes.len());
            // `\.\d*(?!\.)`: the digits give one back when a point follows them.
            if stream.byte(digits) != b'.' {
                stream.pos = digits;
            } else if digits > stream.pos + 1 {
                stream.pos = digits - 1;
            }
        }
        if matches!(stream.peek(), b'e' | b'E') {
            let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'+' | b'-'));
            if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
        return token("number", Some("number"));
    }
    if state.regex_allowed && here == b'~' && stream.eat(|byte| byte == b'/').is_some() {
        string_body(stream, b'/');
        stream.eat_while(|byte| b"gimsu".contains(&byte));
        return token("regexp", Some("string-2"));
    }
    if here == b'/' {
        if stream.eat(|byte| byte == b'*').is_some() {
            state.comment = true;
            return block_comment(stream, state);
        }
        if stream.eat(|byte| byte == b'/').is_some() {
            stream.skip_to_end();
            return token("comment", Some("comment"));
        }
        stream.eat_while(operator);
        return Token { kind: "operator", style: None, content: stream.current_static() };
    }
    if here == b'#' {
        stream.skip_to_end();
        return token("conditional", Some("meta"));
    }
    if here == b'@' {
        stream.eat(|byte| byte == b':');
        stream.eat_while(scan::js_word);
        return token("metadata", Some("meta"));
    }
    if operator(here) {
        stream.eat_while(operator);
        return Token { kind: "operator", style: None, content: stream.current_static() };
    }
    if here.is_ascii_uppercase() {
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'<' || byte == b'>');
        return Token { kind: "type", style: Some("variable-3"), content: stream.current_static() };
    }
    stream.eat_while(scan::js_word);
    let word = stream.current_static();
    match keyword(word).filter(|_| state.keyword_allowed) {
        Some((kind, style)) => Token { kind, style: Some(style), content: word },
        None => Token { kind: "variable", style: Some("variable"), content: word },
    }
}

fn single_kind(byte: u8) -> &'static str {
    match byte {
        b'[' => "[",
        b']' => "]",
        b'{' => "{",
        b'}' => "}",
        b'(' => "(",
        b')' => ")",
        b',' => ",",
        b';' => ";",
        b':' => ":",
        _ => ".",
    }
}

fn block_comment<'a>(stream: &mut Stream<'a>, state: &mut Haxe) -> Token<'a> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if here == b'/' && star {
            state.comment = false;
            break;
        }
        star = here == b'*';
    }
    Token { kind: "comment", style: Some("comment"), content: "" }
}

impl<'a> Stream<'a> {
    /// The token's text, for as long as the line lives.
    fn current_static(&self) -> &'a str {
        self.bytes.get(self.start..self.pos).and_then(|text| std::str::from_utf8(text).ok()).unwrap_or("")
    }
}

pub fn line(text: &str, state: &mut Haxe, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let token = lex(stream, state);
        if token.kind == "comment" {
            return style(token.style);
        }
        state.regex_allowed = matches!(token.kind, "operator" | "keyword c" | "[" | "{" | "}" | "(" | "," | ";" | ":");
        state.keyword_allowed = token.kind != ".";
        let marked = parse(state, token.kind, token.content);
        style(marked.or(token.style))
    });
}

fn style(style: Option<&str>) -> Option<Kind> {
    match style? {
        "keyword" => Some(Kind::Keyword),
        "attribute" | "property" => Some(Kind::Property),
        "variable" | "variable-2" | "variable-3" => Some(Kind::Variable),
        "string" | "string-2" => Some(Kind::String),
        "number" => Some(Kind::Number),
        "comment" | "meta" => Some(Kind::Comment),
        _ => None,
    }
}

/// Feeds a token to the parser: what it marks the token (a definition, a property), if anything.
fn parse(state: &mut Haxe, kind: &str, content: &str) -> Option<&'static str> {
    let mut marked = None;
    loop {
        let combinator = state.stack.pop().unwrap_or(Combinator::Statement);
        if run(state, combinator, kind, content, &mut marked) {
            while state.stack.last().is_some_and(Combinator::immediate) {
                if let Some(next) = state.stack.pop() {
                    run(state, next, kind, content, &mut marked);
                }
            }
            return marked;
        }
    }
}

/// Pushes `list` to run in its order.
fn pass(state: &mut Haxe, list: Vec<Combinator>) {
    state.stack.extend(list.into_iter().rev());
}

/// Declares a name: inside a context, a definition.
fn declare(state: &Haxe, marked: &mut Option<&'static str>) {
    if state.contexts > 0 {
        *marked = Some("def");
    }
}

fn is_atomic(kind: &str) -> bool {
    matches!(kind, "atom" | "number" | "variable" | "string" | "regexp")
}

/// Runs a combinator on the token: whether it took it.
fn run(state: &mut Haxe, combinator: Combinator, kind: &str, content: &str, marked: &mut Option<&'static str>) -> bool {
    use Combinator as C;
    let cont = |state: &mut Haxe, list: Vec<Combinator>| {
        pass(state, list);
        true
    };
    let lex = || C::PushLex;
    let upper = content.as_bytes().first().is_some_and(u8::is_ascii_uppercase);
    match combinator {
        C::PushLex | C::PopLex => false,
        C::PushContext => {
            state.contexts += 1;
            false
        }
        C::PopContext => {
            state.contexts = state.contexts.saturating_sub(1);
            false
        }
        C::Statement => match kind {
            "@" => cont(state, vec![C::Metadef]),
            "var" => cont(state, vec![lex(), C::Vardef, C::Expect(";"), C::PopLex]),
            "keyword a" => cont(state, vec![lex(), C::Expression, C::Statement, C::PopLex]),
            "keyword b" => cont(state, vec![lex(), C::Statement, C::PopLex]),
            "{" => cont(state, vec![lex(), C::PushContext, C::Block, C::PopLex, C::PopContext]),
            ";" => true,
            "attribute" => cont(state, vec![C::Attribute]),
            "function" => cont(state, vec![C::FunctionDef]),
            "for" => cont(state, vec![lex(), C::Expect("("), lex(), C::ForSpec, C::Expect(")"), C::PopLex, C::Statement, C::PopLex]),
            "variable" => cont(state, vec![lex(), C::MaybeLabel]),
            "switch" => cont(state, vec![lex(), C::Expression, lex(), C::Expect("{"), C::Block, C::PopLex, C::PopLex]),
            "case" => cont(state, vec![C::Expression, C::Expect(":")]),
            "default" => cont(state, vec![C::Expect(":")]),
            "catch" => {
                cont(state, vec![lex(), C::PushContext, C::Expect("("), C::FunctionArgument, C::Expect(")"), C::Statement, C::PopLex, C::PopContext])
            }
            "import" => cont(state, vec![C::Importdef, C::Expect(";")]),
            "typedef" => cont(state, vec![C::Typedef]),
            _ => {
                pass(state, vec![lex(), C::Expression, C::Expect(";"), C::PopLex]);
                false
            }
        },
        C::Expression => {
            if is_atomic(kind) || kind == "type" {
                return cont(state, vec![C::MaybeOperator]);
            }
            match kind {
                "function" => cont(state, vec![C::FunctionDef]),
                "keyword c" => cont(state, vec![C::MaybeExpression]),
                "(" => cont(state, vec![lex(), C::MaybeExpression, C::Expect(")"), C::PopLex, C::MaybeOperator]),
                "operator" => cont(state, vec![C::Expression]),
                "[" => cont(state, vec![lex(), C::CommaSeparated(Box::new(C::MaybeExpression), "]"), C::PopLex, C::MaybeOperator]),
                "{" => cont(state, vec![lex(), C::CommaSeparated(Box::new(C::ObjectProperty), "}"), C::PopLex, C::MaybeOperator]),
                _ => true,
            }
        }
        C::MaybeExpression => {
            if kind.contains([';', '}', ')', ']', ',']) {
                false
            } else {
                pass(state, vec![C::Expression]);
                false
            }
        }
        C::MaybeOperator => {
            if kind == "operator" && (content.contains("++") || content.contains("--")) {
                return cont(state, vec![C::MaybeOperator]);
            }
            match kind {
                "operator" | ":" => cont(state, vec![C::Expression]),
                ";" => false,
                "(" => cont(state, vec![lex(), C::CommaSeparated(Box::new(C::Expression), ")"), C::PopLex, C::MaybeOperator]),
                "." => cont(state, vec![C::Property, C::MaybeOperator]),
                "[" => cont(state, vec![lex(), C::Expression, C::Expect("]"), C::PopLex, C::MaybeOperator]),
                _ => false,
            }
        }
        C::Attribute => match kind {
            "attribute" => cont(state, vec![C::Attribute]),
            "function" => cont(state, vec![C::FunctionDef]),
            "var" => cont(state, vec![C::Vardef]),
            _ => false,
        },
        C::Metadef => match kind {
            ":" | "variable" => cont(state, vec![C::Metadef]),
            "(" => cont(state, vec![lex(), C::CommaSeparated(Box::new(C::Metaargs), ")"), C::PopLex, C::Statement]),
            _ => false,
        },
        C::Metaargs => kind == "variable",
        C::Importdef => {
            if kind == "variable" && upper {
                return true;
            }
            if matches!(kind, "variable" | "property" | ".") || content == "*" {
                return cont(state, vec![C::Importdef]);
            }
            false
        }
        C::Typedef => (kind == "variable" || kind == "type") && upper,
        C::MaybeLabel => {
            if kind == ":" {
                return cont(state, vec![C::PopLex, C::Statement]);
            }
            pass(state, vec![C::MaybeOperator, C::Expect(";"), C::PopLex]);
            false
        }
        C::Property => {
            if kind == "variable" {
                *marked = Some("property");
                return true;
            }
            false
        }
        C::ObjectProperty => {
            if kind == "variable" {
                *marked = Some("property");
            }
            if is_atomic(kind) {
                return cont(state, vec![C::Expect(":"), C::Expression]);
            }
            false
        }
        C::CommaSeparated(what, end) => {
            if kind == end {
                return true;
            }
            pass(state, vec![*what.clone(), C::CommaSeparatedNext(what, end)]);
            false
        }
        C::CommaSeparatedNext(what, end) => {
            if kind == "," {
                return cont(state, vec![*what.clone(), C::CommaSeparatedNext(what, end)]);
            }
            if kind == end {
                return true;
            }
            cont(state, vec![C::Expect(end)])
        }
        C::Block => {
            if kind == "}" {
                return true;
            }
            pass(state, vec![C::Statement, C::Block]);
            false
        }
        C::Vardef => {
            if kind == "variable" {
                declare(state, marked);
                return cont(state, vec![C::TypeUse, C::VardefNext]);
            }
            true
        }
        C::VardefNext => {
            if content == "=" {
                return cont(state, vec![C::Expression, C::VardefNext]);
            }
            if kind == "," {
                return cont(state, vec![C::Vardef]);
            }
            false
        }
        C::ForSpec => {
            if kind == "variable" {
                declare(state, marked);
                return cont(state, vec![C::ForIn, C::Expression]);
            }
            false
        }
        C::ForIn => content == "in",
        C::FunctionDef => {
            if kind == "variable" || kind == "type" {
                declare(state, marked);
                return cont(state, vec![C::FunctionDef]);
            }
            if content == "new" {
                return cont(state, vec![C::FunctionDef]);
            }
            if kind == "(" {
                return cont(
                    state,
                    vec![
                        lex(),
                        C::PushContext,
                        C::CommaSeparated(Box::new(C::FunctionArgument), ")"),
                        C::PopLex,
                        C::TypeUse,
                        C::Statement,
                        C::PopContext,
                    ],
                );
            }
            false
        }
        C::TypeUse => {
            if kind == ":" {
                return cont(state, vec![C::TypeString]);
            }
            false
        }
        C::TypeString => match kind {
            "type" | "variable" => true,
            "{" => cont(state, vec![lex(), C::CommaSeparated(Box::new(C::TypeProperty), "}"), C::PopLex]),
            _ => false,
        },
        C::TypeProperty => {
            if kind == "variable" {
                return cont(state, vec![C::TypeUse]);
            }
            false
        }
        C::FunctionArgument => {
            if kind == "variable" {
                declare(state, marked);
                return cont(state, vec![C::TypeUse]);
            }
            false
        }
        C::Expect(wanted) => {
            if kind == wanted {
                return true;
            }
            if wanted == ";" {
                return false;
            }
            cont(state, vec![C::Expect(wanted)])
        }
    }
}
