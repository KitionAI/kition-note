//! JavaScript, TypeScript, and JSON, as CodeMirror 5's javascript mode colors them: a name being
//! declared (a variable, a parameter, a function, a class) is a definition in the text's color,
//! a name used is a variable, a name after `.` or an object's key or a class's member is a
//! property, and in TypeScript a name in a type is a type. A light parse tracks what each bracket
//! opened, as the mode's does.

use super::Kind;
use super::scan::{self, Out};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    JavaScript,
    TypeScript,
    Json,
    /// JavaScript with JSX elements.
    Jsx,
    /// TypeScript with JSX elements.
    Tsx,
}

/// What a bracket opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctx {
    Block,
    Object,
    Class,
    Interface,
    Enum,
    Params,
    /// An arrow function's parameters (its return type is read as an expression, as the mode's).
    Arrow,
    Paren,
    ForHead,
    Bracket,
    TypeArgs,
    Template,
    Pattern,
    Import,
    /// An `export { … }` list.
    Export,
    /// A function type's parameters, in a type.
    TypeParams,
    /// A JSX tag's attributes (`true` for a closing tag).
    JsxTag(bool),
    /// A JSX element's children.
    JsxChildren,
    /// A JSX `{…}` expression.
    JsxExpr,
}

/// What a name is next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Expect {
    #[default]
    Expr,
    Def,
    Property,
    Member,
    TypeName,
}

/// The token before, for what a `/`, a `{`, or a `:` means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Prev {
    #[default]
    Start,
    Op,
    Arrow,
    Open,
    CloseParen,
    CloseBrace,
    Comma,
    Semi,
    Colon,
    Value,
    Keyword(&'static str),
}

/// What the next `(` or `{` opens, from the words before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Pending {
    #[default]
    None,
    Params,
    Class,
    Interface,
    Enum,
}

#[derive(Clone, Debug, Default)]
pub struct Js {
    stack: Vec<Ctx>,
    expect: Expect,
    prev: Prev,
    pending: Pending,
    /// In a type: the bracket depth it began at, and whether a `{` at that depth ends it (a
    /// return type before a function's body, a class's `implements`).
    types: Option<(usize, bool)>,
    /// A `type` alias named: its `=` starts the type.
    alias: bool,
    /// A function's parameters closed with a `:` next: its return type.
    returns: bool,
    /// In a `var`, `let`, or `const` declaration at this depth.
    decl: Option<usize>,
    /// In an `import` statement.
    import: bool,
    /// The import statement named its default import.
    import_default: bool,
    /// The last name was one being declared or a member's (a `:` after it starts a type).
    named: bool,
    /// The last name may take type arguments (`new Map<…>`, a generic declaration).
    generic: bool,
    after_new: bool,
    comment: bool,
    template: bool,
    /// The last token was a name in a type (a name right after it is not a type's).
    type_word: bool,
    /// The last line ended a class's method signature with no body: the mode misreads the next
    /// member's modifier as a variable.
    signature: bool,
    /// A `(` met a `,` first (no argument before it): the mode's parser then only waits for a
    /// `)`, so no name after a `.` is a property until one comes.
    swallowing: bool,
}

const KEYWORDS: &[&str] = &[
    "if",
    "while",
    "with",
    "else",
    "do",
    "try",
    "finally",
    "return",
    "break",
    "continue",
    "new",
    "delete",
    "void",
    "throw",
    "debugger",
    "var",
    "const",
    "let",
    "function",
    "catch",
    "for",
    "switch",
    "case",
    "default",
    "in",
    "typeof",
    "instanceof",
    "this",
    "class",
    "super",
    "yield",
    "export",
    "import",
    "extends",
    "await",
];
const ATOMS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity"];
const CLASS_MODIFIERS: &[&str] =
    &["static", "get", "set", "async", "public", "private", "protected", "readonly", "abstract", "declare", "override", "accessor"];
const TS_STATEMENTS: &[&str] = &["type", "interface", "enum", "declare", "abstract", "namespace", "module"];
const TYPE_KEYWORDS: &[&str] = &["typeof", "keyof", "infer", "extends", "is", "readonly", "unique", "in", "new", "as"];

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn operator_char(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'&' | b'%' | b'=' | b'<' | b'>' | b'!' | b'?' | b'|' | b'~' | b'^' | b'@')
}

fn word_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' || byte >= 0x80
}

fn word_end(bytes: &[u8], at: usize) -> usize {
    scan::word_end(bytes, at, b"$")
}

impl Js {
    fn top(&self) -> Option<Ctx> {
        self.stack.last().copied()
    }

    fn in_types(&self) -> bool {
        self.types.is_some() || self.top() == Some(Ctx::TypeArgs)
    }

    /// Pops brackets up to and including the nearest of `kinds`, if there is one open.
    fn close(&mut self, kinds: &[Ctx]) -> Option<Ctx> {
        let found = self.stack.iter().rposition(|ctx| kinds.contains(ctx))?;
        let ctx = self.stack.get(found).copied();
        self.stack.truncate(found);
        if self.types.is_some_and(|(depth, _)| depth > self.stack.len()) {
            self.types = None;
        }
        if self.decl.is_some_and(|depth| depth > self.stack.len()) {
            self.decl = None;
        }
        ctx
    }

    /// Whether a `/` here starts a regular expression (an expression may start), not a division.
    fn expression_allowed(&self) -> bool {
        match self.prev {
            Prev::Start | Prev::Op | Prev::Arrow | Prev::Open | Prev::Comma | Prev::Semi | Prev::Colon | Prev::CloseBrace => true,
            Prev::Keyword(word) => {
                matches!(word, "return" | "typeof" | "case" | "do" | "else" | "new" | "delete" | "void" | "throw" | "in" | "of" | "instanceof")
                    || matches!(word, "yield" | "await" | "export" | "default")
            }
            Prev::CloseParen | Prev::Value => false,
        }
    }

    /// Whether a `{` here opens an object (an expression may start), not a block.
    fn object_allowed(&self) -> bool {
        match self.prev {
            Prev::Op | Prev::Open | Prev::Comma | Prev::Colon => true,
            Prev::Keyword(word) => {
                matches!(word, "return" | "typeof" | "case" | "new" | "delete" | "void" | "throw" | "in" | "yield" | "await" | "default")
            }
            _ => false,
        }
    }
}

pub fn line(text: &str, js: &mut Js, flavor: Flavor, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if js.comment {
        match text.find("*/") {
            Some(found) => {
                out.push(0..found + 2, Kind::Comment);
                js.comment = false;
                at = found + 2;
            }
            None => {
                out.push(0..text.len(), Kind::Comment);
                return;
            }
        }
    }
    if js.template {
        at = template(bytes, at, at, js, out);
    }
    let jsx = matches!(flavor, Flavor::Jsx | Flavor::Tsx);
    while at < bytes.len() {
        let here = byte(bytes, at);
        let next = byte(bytes, at + 1);
        match js.top() {
            Some(Ctx::JsxTag(closing)) => {
                at = jsx_tag(bytes, at, closing, js, out);
                continue;
            }
            Some(Ctx::JsxChildren) => {
                at = jsx_children(bytes, at, js, out);
                continue;
            }
            _ => {}
        }
        if jsx && here == b'<' && (next.is_ascii_alphabetic() || next == b'>') && js.expression_allowed() {
            out.push(at..at + 1, Kind::Punctuation);
            js.stack.push(Ctx::JsxTag(false));
            at = jsx_name(bytes, at + 1, out);
            continue;
        }
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'/' if next == b'/' => {
                out.push(at..bytes.len(), Kind::Comment);
                break;
            }
            b'/' if next == b'*' => match text.get(at + 2..).and_then(|rest| rest.find("*/")) {
                Some(found) => {
                    out.push(at..at + 2 + found + 2, Kind::Comment);
                    at += found + 4;
                }
                None => {
                    out.push(at..bytes.len(), Kind::Comment);
                    js.comment = true;
                    break;
                }
            },
            b'"' | b'\'' => {
                let end = scan::string_end(bytes, at, here).unwrap_or(bytes.len());
                let key = scan::followed_by(bytes, end, b':') && (flavor == Flavor::Json || js.expect == Expect::Member);
                out.push(at..end, if key { Kind::Property } else { Kind::String });
                if js.expect == Expect::Member {
                    js.expect = Expect::Expr;
                    js.named = true;
                } else {
                    js.named = false;
                }
                js.prev = Prev::Value;
                at = end;
            }
            b'`' => {
                at = template(bytes, at, at + 1, js, out);
                js.prev = Prev::Value;
            }
            b'0'..=b'9' => {
                let end = number(bytes, at);
                out.push(at..end, Kind::Number);
                js.prev = Prev::Value;
                js.named = false;
                at = end;
            }
            b'.' if next.is_ascii_digit() => {
                let end = number(bytes, at);
                out.push(at..end, Kind::Number);
                js.prev = Prev::Value;
                at = end;
            }
            b'.' => {
                if next == b'.' && byte(bytes, at + 2) == b'.' {
                    // A spread is the mode's meta; what it spreads, a value.
                    out.push(at..at + 3, Kind::Comment);
                    js.prev = Prev::Op;
                    if js.expect == Expect::Member {
                        js.expect = Expect::Expr;
                    }
                    at += 3;
                } else {
                    if !js.swallowing {
                        js.expect = Expect::Property;
                    }
                    js.prev = Prev::Op;
                    at += 1;
                }
            }
            b'#' if word_start(next) => {
                let end = word_end(bytes, at + 1);
                out.push(at..end, Kind::Property);
                if js.expect == Expect::Member {
                    js.expect = Expect::Expr;
                    js.named = true;
                    if scan::followed_by(bytes, end, b'(') {
                        js.pending = Pending::Params;
                    }
                } else {
                    js.expect = Expect::Expr;
                }
                js.prev = Prev::Value;
                at = end;
            }
            _ if word_start(here) => {
                let end = word_end(bytes, at);
                let kind = word(text, bytes, at, end, js, flavor);
                out.mark(at..end, kind);
                at = end;
            }
            b'/' => {
                if js.expression_allowed() {
                    let end = regex_end(bytes, at);
                    out.push(at..end, Kind::String);
                    js.prev = Prev::Value;
                    at = end;
                } else {
                    let end = if next == b'=' { at + 2 } else { at + 1 };
                    out.push(at..end, Kind::Operator);
                    js.prev = Prev::Op;
                    js.expect = Expect::Expr;
                    at = end;
                }
            }
            // `?.` is a dot (no operator's color).
            b'?' if next == b'.' && !byte(bytes, at + 2).is_ascii_digit() => {
                js.expect = Expect::Property;
                js.prev = Prev::Op;
                at += 2;
            }
            _ if operator_char(here) => {
                let mut end = at;
                while operator_char(byte(bytes, end)) {
                    end += 1;
                }
                let run = text.get(at..end).unwrap_or("");
                // A generator's `function*` is all keyword.
                let kind = if run == "*" && js.prev == Prev::Keyword("function") { Kind::Keyword } else { Kind::Operator };
                out.push(at..end, kind);
                operator(run, js);
                at = end;
            }
            b'(' => {
                open_paren(bytes, at, js);
                at += 1;
            }
            b')' => {
                js.swallowing = false;
                if js.close(&[Ctx::Params, Ctx::Arrow, Ctx::Paren, Ctx::ForHead, Ctx::TypeParams]) == Some(Ctx::Params)
                    && scan::followed_by(bytes, at + 1, b':')
                {
                    js.returns = true;
                }
                js.prev = Prev::CloseParen;
                js.expect = Expect::Expr;
                js.named = false;
                at += 1;
            }
            b'{' => {
                open_brace(js);
                at += 1;
            }
            b'}' => {
                if js.top() == Some(Ctx::JsxExpr) {
                    js.stack.pop();
                    js.prev = Prev::Value;
                    at += 1;
                    continue;
                }
                if js.top() == Some(Ctx::Template) {
                    js.stack.pop();
                    at = template(bytes, at, at + 1, js, out);
                    js.prev = Prev::Value;
                    continue;
                }
                let closed = js.close(&[Ctx::Block, Ctx::Object, Ctx::Class, Ctx::Interface, Ctx::Enum, Ctx::Pattern, Ctx::Import, Ctx::Export]);
                js.expect = match js.top() {
                    Some(Ctx::Class | Ctx::Interface) => Expect::Member,
                    _ => Expect::Expr,
                };
                if closed == Some(Ctx::Pattern) && js.top() == Some(Ctx::Params) {
                    js.expect = Expect::Expr;
                }
                js.prev = Prev::CloseBrace;
                js.named = false;
                at += 1;
            }
            b'[' => {
                if js.expect == Expect::Def {
                    js.stack.push(Ctx::Pattern);
                } else {
                    js.stack.push(Ctx::Bracket);
                    js.expect = Expect::Expr;
                }
                js.prev = Prev::Open;
                at += 1;
            }
            b']' => {
                js.close(&[Ctx::Bracket, Ctx::Pattern]);
                js.prev = Prev::Value;
                js.expect = Expect::Expr;
                at += 1;
            }
            b',' => {
                if js.prev == Prev::Open && js.top() == Some(Ctx::Paren) {
                    js.swallowing = true;
                }
                comma(js);
                at += 1;
            }
            b';' => {
                js.decl = None;
                if js.types.is_some_and(|(depth, _)| depth >= js.stack.len()) {
                    js.types = None;
                }
                js.import = false;
                js.pending = Pending::None;
                js.expect = match js.top() {
                    Some(Ctx::Class | Ctx::Interface) => Expect::Member,
                    _ => Expect::Expr,
                };
                js.prev = Prev::Semi;
                at += 1;
            }
            b':' => {
                colon(js);
                at += 1;
            }
            _ => at += 1,
        }
        if here != b' ' && here != b'\t' && !word_start(here) {
            js.type_word = false;
            if here != b'<' && here != b'>' {
                js.generic = false;
            }
        }
    }
    end_of_line(js);
}

/// A name: what it is, and what it makes of the names after it.
fn word(text: &str, bytes: &[u8], start: usize, end: usize, js: &mut Js, flavor: Flavor) -> Option<Kind> {
    let word = text.get(start..end).unwrap_or("");
    let after = scan::next_significant(bytes, end).map(|(_, byte)| byte);
    let followed_by_name = after.is_some_and(|byte| word_start(byte) || byte == b'#' || byte == b'[' || byte == b'*');
    let ts = matches!(flavor, Flavor::TypeScript | Flavor::Tsx);
    let arrow_next = scan::next_significant(bytes, end).is_some_and(|(at, _)| scan::at(bytes, at, "=>"));
    let new_word = std::mem::take(&mut js.after_new);
    js.generic = new_word;
    if js.expect == Expect::Property {
        js.expect = Expect::Expr;
        js.prev = Prev::Value;
        js.named = false;
        return Some(Kind::Property);
    }
    if js.expect == Expect::Member {
        js.prev = Prev::Value;
        match js.top() {
            Some(Ctx::Enum) => {
                js.expect = Expect::Expr;
                js.named = false;
                return None;
            }
            Some(Ctx::Class) if std::mem::take(&mut js.signature) && CLASS_MODIFIERS.contains(&word) => return Some(Kind::Variable),
            Some(Ctx::Class) if CLASS_MODIFIERS.contains(&word) && followed_by_name => return Some(Kind::Keyword),
            Some(Ctx::Interface) if word == "readonly" && followed_by_name => return Some(Kind::Keyword),
            Some(Ctx::Object) if matches!(word, "get" | "set" | "async") && followed_by_name => return Some(Kind::Keyword),
            _ => {}
        }
        js.expect = Expect::Expr;
        js.named = true;
        if matches!(after, Some(b'(' | b'<')) {
            js.pending = Pending::Params;
            js.generic = true;
        }
        return Some(Kind::Property);
    }
    if js.in_types() {
        js.prev = Prev::Value;
        js.named = false;
        let after_type = std::mem::take(&mut js.type_word);
        if TYPE_KEYWORDS.contains(&word) {
            return Some(Kind::Keyword);
        }
        // A type predicate's parameter (`x is string`), a function type's parameter, a name
        // right after a type's (`asserts cond`).
        if after_type
            || scan::next_significant(bytes, end).is_some_and(|(next, _)| scan::at(bytes, next, "is "))
            || (js.top() == Some(Ctx::TypeParams) && matches!(after, Some(b':' | b'?')))
        {
            return Some(Kind::Variable);
        }
        if matches!(word, "undefined" | "null" | "true" | "false") {
            return None;
        }
        js.type_word = true;
        return Some(Kind::Property);
    }
    if js.expect == Expect::TypeName {
        js.expect = Expect::Expr;
        js.alias = true;
        js.generic = true;
        js.prev = Prev::Value;
        return Some(Kind::Property);
    }
    if js.import || js.top() == Some(Ctx::Import) {
        if matches!(word, "from" | "as") {
            js.prev = Prev::Keyword("import");
            js.expect = if word == "from" { Expect::Expr } else { Expect::Def };
            return Some(Kind::Keyword);
        }
        if js.expect == Expect::Def || js.top() == Some(Ctx::Import) {
            js.prev = Prev::Keyword("import");
            js.expect = Expect::Expr;
            if js.top() != Some(Ctx::Import) {
                js.import_default = true;
            }
            return None;
        }
    }
    if word == "as" && js.top() == Some(Ctx::Export) {
        js.prev = Prev::Keyword("as");
        return Some(Kind::Keyword);
    }
    if ts
        && js.expect == Expect::Def
        && matches!(js.top(), Some(Ctx::Params))
        && matches!(word, "public" | "private" | "protected" | "readonly" | "override")
        && followed_by_name
    {
        return Some(Kind::Keyword);
    }
    if js.expect == Expect::Def && !KEYWORDS.contains(&word) {
        js.prev = Prev::Value;
        js.named = true;
        if js.top() == Some(Ctx::Pattern) && after == Some(b':') {
            js.named = false;
            return Some(Kind::Property);
        }
        if js.pending != Pending::None {
            js.generic = true;
        }
        js.expect = Expect::Expr;
        return None;
    }
    js.named = false;
    if ATOMS.contains(&word) {
        js.prev = Prev::Value;
        js.expect = Expect::Expr;
        return None;
    }
    if KEYWORDS.contains(&word) {
        js.prev = Prev::Keyword(keyword(word));
        js.expect = Expect::Expr;
        match word {
            "var" | "let" | "const" => {
                js.decl = Some(js.stack.len());
                js.expect = Expect::Def;
            }
            "function" => {
                js.pending = Pending::Params;
                js.expect = Expect::Def;
                js.generic = true;
            }
            "class" => {
                js.pending = Pending::Class;
                js.expect = Expect::Def;
            }
            "catch" => js.pending = Pending::Params,
            "import" if after != Some(b'(') && after != Some(b'.') => {
                js.import = true;
                js.import_default = false;
                js.expect = Expect::Def;
            }
            "new" => js.after_new = true,
            "extends" if ts => js.types = Some((js.stack.len(), true)),
            _ => {}
        }
        return Some(Kind::Keyword);
    }
    let statement = matches!(js.prev, Prev::Start | Prev::Semi | Prev::CloseBrace | Prev::Open | Prev::Keyword("export" | "declare" | "default"));
    if ts && statement && TS_STATEMENTS.contains(&word) && after.is_some_and(word_start) {
        js.prev = Prev::Keyword(keyword(word));
        match word {
            "type" => js.expect = Expect::TypeName,
            "interface" => {
                js.pending = Pending::Interface;
                js.expect = Expect::Def;
            }
            "enum" => {
                js.pending = Pending::Enum;
                js.expect = Expect::Def;
            }
            _ => {}
        }
        return Some(Kind::Keyword);
    }
    if ts && word == "implements" && js.pending == Pending::Class {
        js.types = Some((js.stack.len(), true));
        js.prev = Prev::Keyword("extends");
        return Some(Kind::Keyword);
    }
    if ts && matches!(word, "as" | "satisfies") && matches!(js.prev, Prev::Value | Prev::CloseParen) {
        js.types = Some((js.stack.len(), false));
        js.prev = Prev::Keyword("as");
        return Some(Kind::Keyword);
    }
    if word == "async" && after.is_some_and(|byte| word_start(byte) || byte == b'(' || byte == b'[') {
        js.prev = Prev::Keyword("async");
        return Some(Kind::Keyword);
    }
    if word == "of" && js.top() == Some(Ctx::ForHead) && matches!(js.prev, Prev::Value | Prev::CloseBrace) {
        js.prev = Prev::Keyword("of");
        return Some(Kind::Keyword);
    }
    js.prev = Prev::Value;
    js.expect = Expect::Expr;
    if arrow_next {
        return None;
    }
    if flavor == Flavor::Json {
        return None;
    }
    Some(Kind::Variable)
}

/// `word` as one of the static names the parse remembers.
fn keyword(word: &str) -> &'static str {
    KEYWORDS.iter().chain(TS_STATEMENTS).find(|known| **known == word).copied().unwrap_or("")
}

/// An operator's run (`=`, `=>`, `<`, …) on the parse.
fn operator(run: &str, js: &mut Js) {
    let keep_named = matches!(run, "?" | "!");
    if run == "=>" {
        if js.types.is_some_and(|(_, body)| body) {
            js.types = None;
        }
        js.prev = Prev::Arrow;
        js.expect = Expect::Expr;
        js.named = false;
        return;
    }
    if run == "=" && js.top() != Some(Ctx::TypeArgs) {
        if js.alias {
            js.alias = false;
            js.types = Some((js.stack.len(), false));
        } else if js.types.is_some_and(|(depth, _)| depth >= js.stack.len()) {
            js.types = None;
        }
    }
    for symbol in run.bytes() {
        if symbol == b'<' && (js.generic || js.in_types()) {
            js.stack.push(Ctx::TypeArgs);
        } else if symbol == b'>' && js.top() == Some(Ctx::TypeArgs) {
            js.stack.pop();
        }
    }
    js.generic = false;
    js.prev = Prev::Op;
    // A generator's `*` keeps its name (or a member's) to come.
    if !(run == "*" && matches!(js.expect, Expect::Def | Expect::Member)) {
        js.expect = Expect::Expr;
    }
    if !keep_named {
        js.named = false;
    }
}

fn open_paren(bytes: &[u8], at: usize, js: &mut Js) {
    if js.in_types() && js.pending != Pending::Params {
        js.stack.push(Ctx::TypeParams);
    } else if js.pending == Pending::Params {
        js.pending = Pending::None;
        js.stack.push(Ctx::Params);
        js.expect = Expect::Def;
    } else if js.prev == Prev::Keyword("for") {
        js.stack.push(Ctx::ForHead);
        js.expect = Expect::Expr;
    } else if !js.stack.contains(&Ctx::JsxExpr) && arrow_params(bytes, at) {
        js.stack.push(Ctx::Arrow);
        js.expect = Expect::Def;
    } else {
        js.stack.push(Ctx::Paren);
        js.expect = Expect::Expr;
    }
    js.prev = Prev::Open;
    js.named = false;
}

/// Whether the `(` at `at` opens an arrow function's parameters: its `)` on this line, then
/// `=>` (after a return type).
fn arrow_params(bytes: &[u8], at: usize) -> bool {
    let mut depth = 0usize;
    let mut index = at;
    while let Some(here) = bytes.get(index) {
        match here {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let Some((next, after)) = scan::next_significant(bytes, index + 1) else { return false };
                    if scan::at(bytes, next, "=>") {
                        return true;
                    }
                    if after == b':' {
                        let rest = bytes.get(next..).unwrap_or(&[]);
                        let stop = rest.iter().position(|byte| *byte == b'{' || *byte == b';').unwrap_or(rest.len());
                        return rest.get(..stop).is_some_and(|head| head.windows(2).any(|pair| pair == b"=>"));
                    }
                    return false;
                }
            }
            b'"' | b'\'' | b'`' => match scan::string_end(bytes, index, *here) {
                Some(end) => {
                    index = end;
                    continue;
                }
                None => return false,
            },
            _ => {}
        }
        index += 1;
    }
    false
}

fn open_brace(js: &mut Js) {
    let pending = std::mem::take(&mut js.pending);
    let body = js.types.is_some_and(|(depth, body)| body && depth == js.stack.len());
    if body {
        js.types = None;
    }
    match pending {
        Pending::Class => {
            js.types = None;
            js.stack.push(Ctx::Class);
            js.expect = Expect::Member;
        }
        Pending::Interface => {
            js.types = None;
            js.stack.push(Ctx::Interface);
            js.expect = Expect::Member;
        }
        Pending::Enum => {
            js.stack.push(Ctx::Enum);
            js.expect = Expect::Member;
        }
        // A function's body, after its return type.
        _ if body => {
            js.stack.push(Ctx::Block);
            js.expect = Expect::Expr;
        }
        _ if js.in_types() => {
            js.stack.push(Ctx::Interface);
            js.expect = Expect::Member;
        }
        _ if js.prev == Prev::Keyword("export") => {
            js.stack.push(Ctx::Export);
            js.expect = Expect::Expr;
        }
        // After a default import, a `{` with no `,` before it is an expression's (as the mode
        // reads `import type { … }`).
        _ if js.import && js.import_default && js.prev != Prev::Comma => {
            js.import = false;
            js.stack.push(Ctx::Block);
            js.expect = Expect::Expr;
        }
        _ if js.import => {
            js.stack.push(Ctx::Import);
            js.expect = Expect::Def;
        }
        _ if js.expect == Expect::Def => js.stack.push(Ctx::Pattern),
        _ if js.object_allowed() => {
            js.stack.push(Ctx::Object);
            js.expect = Expect::Member;
        }
        _ => {
            js.stack.push(Ctx::Block);
            js.expect = Expect::Expr;
        }
    }
    js.prev = Prev::Open;
    js.named = false;
}

fn comma(js: &mut Js) {
    if js.types.is_some_and(|(depth, _)| depth >= js.stack.len()) && js.top() != Some(Ctx::TypeArgs) {
        js.types = None;
    }
    match js.top() {
        Some(Ctx::Params | Ctx::Arrow | Ctx::Import | Ctx::Pattern) => js.expect = Expect::Def,
        Some(Ctx::Object | Ctx::Interface | Ctx::Enum) => js.expect = Expect::Member,
        Some(Ctx::TypeArgs) => {}
        _ if js.decl == Some(js.stack.len()) => js.expect = Expect::Def,
        _ => js.expect = Expect::Expr,
    }
    js.prev = Prev::Comma;
    js.named = false;
}

fn colon(js: &mut Js) {
    if js.top() == Some(Ctx::TypeParams) {
        js.named = false;
        js.prev = Prev::Colon;
        return;
    }
    if std::mem::take(&mut js.returns) {
        js.types = Some((js.stack.len(), true));
    } else if js.top() == Some(Ctx::Object) {
        js.expect = Expect::Expr;
    } else if js.top() == Some(Ctx::Pattern) {
        js.expect = Expect::Def;
    } else if js.named {
        js.types = Some((js.stack.len(), false));
    }
    js.prev = Prev::Colon;
    js.named = false;
}

fn end_of_line(js: &mut Js) {
    let open = matches!(js.prev, Prev::Op | Prev::Comma | Prev::Colon);
    if js.types.is_some_and(|(depth, body)| body && depth == js.stack.len()) && js.top() == Some(Ctx::Class) && !open {
        js.signature = true;
    }
    if js.types.is_some_and(|(depth, _)| depth >= js.stack.len()) && !open {
        js.types = None;
    }
    if js.decl.is_some_and(|depth| depth >= js.stack.len()) && !open {
        js.decl = None;
    }
    if js.import && js.top() != Some(Ctx::Import) && js.prev != Prev::Comma {
        js.import = false;
    }
    match js.top() {
        Some(Ctx::Class | Ctx::Interface) if !open => js.expect = Expect::Member,
        Some(Ctx::Object) if js.prev == Prev::Comma || js.prev == Prev::Open => js.expect = Expect::Member,
        _ if js.expect == Expect::Def && matches!(js.prev, Prev::Keyword(_) | Prev::Comma | Prev::Open) => {}
        _ if js.expect == Expect::Member => js.expect = Expect::Expr,
        _ if js.expect != Expect::Def => {}
        _ => js.expect = Expect::Expr,
    }
    // A line that ends a statement lets the next start one (as automatic semicolons do).
    if matches!(js.prev, Prev::Value | Prev::CloseParen | Prev::CloseBrace) && matches!(js.top(), None | Some(Ctx::Block)) && js.types.is_none() {
        js.prev = Prev::Semi;
    }
    js.after_new = false;
    js.generic = false;
    js.named = false;
}

/// A template string's text from `start`, read from `from` (inside it): its text, its backticks,
/// and its `${` and `}` are a string, up to its closing backtick (`js.template` stays set when the
/// line ends first) or a `${`, whose expression follows.
fn template(bytes: &[u8], start: usize, from: usize, js: &mut Js, out: &mut Out) -> usize {
    let mut index = from;
    js.template = false;
    while let Some(here) = bytes.get(index) {
        match here {
            b'\\' => index += 2,
            b'`' => {
                out.push(start..index + 1, Kind::String);
                return index + 1;
            }
            b'$' if byte(bytes, index + 1) == b'{' => {
                out.push(start..index + 2, Kind::String);
                js.stack.push(Ctx::Template);
                js.prev = Prev::Open;
                js.expect = Expect::Expr;
                return index + 2;
            }
            _ => index += 1,
        }
    }
    out.push(start..bytes.len(), Kind::String);
    js.template = true;
    bytes.len()
}

/// The end of a number at `at`: `0x…`, `0o…`, `0b…` (a `n` after), or digits with a fraction and
/// an exponent (`_` between).
fn number(bytes: &[u8], at: usize) -> usize {
    let lower = |index: usize| byte(bytes, index).to_ascii_lowercase();
    if lower(at) == b'0' && matches!(lower(at + 1), b'x' | b'o' | b'b') {
        let mut end = at + 2;
        while lower(end).is_ascii_hexdigit() || lower(end) == b'_' {
            end += 1;
        }
        return if lower(end) == b'n' { end + 1 } else { end };
    }
    let mut end = at;
    while byte(bytes, end).is_ascii_digit() || byte(bytes, end) == b'_' {
        end += 1;
    }
    if byte(bytes, end) == b'n' {
        return end + 1;
    }
    if byte(bytes, end) == b'.' {
        end += 1;
        while byte(bytes, end).is_ascii_digit() || byte(bytes, end) == b'_' {
            end += 1;
        }
    }
    scan::exponent(bytes, end)
}

/// The end of a regular expression opened at `at`: to its closing `/` (outside a `[…]` set), then
/// its flags.
fn regex_end(bytes: &[u8], at: usize) -> usize {
    let mut index = at + 1;
    let mut set = false;
    while let Some(here) = bytes.get(index) {
        match here {
            b'\\' => index += 1,
            b'[' => set = true,
            b']' => set = false,
            b'/' if !set => {
                index += 1;
                while byte(bytes, index).is_ascii_alphabetic() {
                    index += 1;
                }
                return index;
            }
            _ => {}
        }
        index += 1;
    }
    bytes.len()
}

/// A JSX tag's name from `at` (a tag's color).
fn jsx_name(bytes: &[u8], at: usize, out: &mut Out) -> usize {
    let mut end = at;
    while scan::word(byte(bytes, end)) || matches!(byte(bytes, end), b'-' | b'.' | b':' | b'$') {
        end += 1;
    }
    out.push(at..end, Kind::Operator);
    end
}

/// One token in a JSX tag's attributes: a name (an attribute), a string, a `{` expression, or
/// the tag's end (`>`, `/>`), which opens its children or closes the element.
fn jsx_tag(bytes: &[u8], at: usize, closing: bool, js: &mut Js, out: &mut Out) -> usize {
    let here = byte(bytes, at);
    match here {
        b' ' | b'\t' | b'\r' | b'=' => at + 1,
        b'>' => {
            out.push(at..at + 1, Kind::Punctuation);
            js.stack.pop();
            if closing {
                js.close(&[Ctx::JsxChildren]);
            } else {
                js.stack.push(Ctx::JsxChildren);
            }
            jsx_ended(js);
            at + 1
        }
        b'/' if byte(bytes, at + 1) == b'>' => {
            out.push(at..at + 2, Kind::Punctuation);
            js.stack.pop();
            jsx_ended(js);
            at + 2
        }
        b'{' => {
            js.stack.push(Ctx::JsxExpr);
            js.prev = Prev::Open;
            js.expect = Expect::Expr;
            at + 1
        }
        b'"' | b'\'' => {
            let end = scan::string_end(bytes, at, here).unwrap_or(bytes.len());
            out.push(at..end, Kind::String);
            end
        }
        _ if scan::word(here) || here == b'-' => {
            let mut end = at;
            while scan::word(byte(bytes, end)) || matches!(byte(bytes, end), b'-' | b':') {
                end += 1;
            }
            out.push(at..end, Kind::Property);
            end
        }
        _ => at + 1,
    }
}

/// An element's children: text, a `{` expression, a child's tag, or the closing tag's `</`.
fn jsx_children(bytes: &[u8], at: usize, js: &mut Js, out: &mut Out) -> usize {
    match byte(bytes, at) {
        b'{' => {
            js.stack.push(Ctx::JsxExpr);
            js.prev = Prev::Open;
            js.expect = Expect::Expr;
            at + 1
        }
        b'<' if byte(bytes, at + 1) == b'/' => {
            out.push(at..at + 2, Kind::Punctuation);
            js.stack.push(Ctx::JsxTag(true));
            jsx_name(bytes, at + 2, out)
        }
        b'<' => {
            out.push(at..at + 1, Kind::Punctuation);
            js.stack.push(Ctx::JsxTag(false));
            jsx_name(bytes, at + 1, out)
        }
        _ => {
            let mut end = at + 1;
            while end < bytes.len() && !matches!(byte(bytes, end), b'<' | b'{') {
                end += 1;
            }
            end
        }
    }
}

/// After a tag's end: outside any element, the JavaScript around it goes on after a value.
fn jsx_ended(js: &mut Js) {
    if js.top() != Some(Ctx::JsxChildren) {
        js.prev = Prev::Value;
        js.expect = Expect::Expr;
    }
}
