//! XQuery, as CodeMirror 5's xquery mode colors it, ported token for token with the stack it
//! keeps of what is open (elements, attributes, strings, comments, brackets): keywords and
//! operators keywords, `xs:` types atoms (plain), axes qualifiers, a call's name a definition,
//! `$variables` and other names variables, numbers plain, strings, `(: :)` comments, and direct
//! XML (its tags, attributes, and text).

use super::Kind;
use super::scan::{self, Out, Stream};

/// What reads the next token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tokenize {
    #[default]
    Base,
    /// A start tag's attributes.
    Attributes,
    /// A string to `quote`, then (when set) the attributes again.
    String(u8, bool),
    Comment,
    XmlComment,
    Cdata,
    Instruction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    Tag,
    CodeBlock,
    Comment,
    Paren,
    Bracket,
    String,
    Attribute,
    XmlConstructor,
}

#[derive(Clone, Debug, Default)]
pub struct XQuery {
    tokenize: Tokenize,
    /// What is open, and what reads after it closes back to the one around it.
    stack: Vec<(Open, Tokenize)>,
}

impl XQuery {
    fn top(&self, open: Open) -> bool {
        self.stack.last().is_some_and(|(kind, _)| *kind == open)
    }
    fn push(&mut self, open: Open, tokenize: Tokenize) {
        self.stack.push((open, tokenize));
    }
    fn pop(&mut self) {
        self.stack.pop();
        self.tokenize = self.stack.last().map_or(Tokenize::Base, |(_, tokenize)| *tokenize);
    }
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

const OPERATORS: &str = "eq ne lt le gt ge := = > >= < <= . | ? and or div idiv mod * / + -";
const AXES: &str = "self:: attribute:: child:: descendant:: descendant-or-self:: parent:: ancestor:: ancestor-or-self:: following:: \
preceding:: following-sibling:: preceding-sibling::";

/// The style the mode's word table gives `word`.
fn table(word: &str) -> Option<&'static str> {
    if word == "," {
        Some("punctuation")
    } else if listed(AXES, word) {
        Some("axis")
    } else if listed(OPERATORS, word) {
        Some("operator")
    } else if listed(TYPES, word) {
        Some("atom")
    } else if listed(KEYWORDS, word) {
        Some("keyword")
    } else {
        None
    }
}

fn name_byte(byte: u8) -> bool {
    scan::js_word(byte) || byte == b'$' || byte == b'-'
}

pub fn line(text: &str, state: &mut XQuery, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        match state.tokenize {
            Tokenize::Base => base(stream, state),
            Tokenize::Attributes => attributes(stream, state),
            Tokenize::String(quote, attributes) => string(stream, state, quote, attributes),
            Tokenize::Comment => comment(stream, state),
            Tokenize::XmlComment => till(stream, state, b"-->", Some(Kind::Comment)),
            Tokenize::Cdata => till(stream, state, b"]]", Some(Kind::Comment)),
            Tokenize::Instruction => till(stream, state, b"?>", Some(Kind::Comment)),
        }
    });
}

/// To `close`: then `style`, else (the line ended first) none.
fn till(stream: &mut Stream, state: &mut XQuery, close: &[u8], style: Option<Kind>) -> Option<Kind> {
    match stream.rest().windows(close.len()).position(|window| window == close) {
        Some(at) => {
            stream.pos += at + close.len();
            state.tokenize = Tokenize::Base;
            style
        }
        None => {
            stream.skip_to_end();
            None
        }
    }
}

fn comment(stream: &mut Stream, state: &mut XQuery) -> Option<Kind> {
    let mut colon = false;
    let mut paren = false;
    let mut depth = 0;
    while let Some(here) = stream.next() {
        if here == b')' && colon {
            if depth == 0 {
                state.pop();
                break;
            }
            depth -= 1;
        } else if here == b':' && paren {
            depth += 1;
        }
        colon = here == b':';
        paren = here == b'(';
    }
    Some(Kind::Comment)
}

fn string(stream: &mut Stream, state: &mut XQuery, quote: u8, attributes: bool) -> Option<Kind> {
    let after = if attributes { Tokenize::Attributes } else { Tokenize::Base };
    // A string left open on the stack closes on this quote.
    if state.top(Open::String) && stream.current().as_bytes() == [quote] {
        state.pop();
        if attributes {
            state.tokenize = after;
        }
        return Some(Kind::String);
    }
    state.push(Open::String, Tokenize::String(quote, attributes));
    state.tokenize = Tokenize::String(quote, attributes);
    while let Some(here) = stream.next() {
        if here == quote {
            state.pop();
            if attributes {
                state.tokenize = after;
            }
            break;
        }
    }
    Some(Kind::String)
}

fn attributes(stream: &mut Stream, state: &mut XQuery) -> Option<Kind> {
    let here = stream.next()?;
    if here == b'/' && stream.eat(|byte| byte == b'>').is_some() {
        if state.top(Open::Attribute) {
            state.pop();
        }
        if state.top(Open::Tag) {
            state.pop();
        }
        return Some(Kind::Operator);
    }
    if here == b'>' {
        if state.top(Open::Attribute) {
            state.pop();
        }
        return Some(Kind::Operator);
    }
    if here == b'=' {
        return None;
    }
    if here == b'"' || here == b'\'' {
        state.tokenize = Tokenize::String(here, true);
        return string(stream, state, here, true);
    }
    if !state.top(Open::Attribute) {
        state.push(Open::Attribute, Tokenize::Attributes);
    }
    stream.eat(|byte| byte.is_ascii_alphabetic() || byte == b'_' || byte == b':');
    stream.eat_while(|byte| byte.is_ascii_alphanumeric() || b"-_:.".contains(&byte));
    stream.eat_space();
    if matches!(stream.peek(), b'>' | b'/') {
        state.pop();
        state.tokenize = Tokenize::Base;
    }
    Some(Kind::Property)
}

fn base(stream: &mut Stream, state: &mut XQuery) -> Option<Kind> {
    let here = stream.next()?;
    // A quoted URI before `:` (an `EQName`'s namespace).
    let eq_name = (here == b'"' || here == b'\'') && {
        // `[^"]+` then the quote and `:` (`[^"]+'` may give back what the `'` needs).
        let rest = stream.rest();
        let run = rest.iter().take_while(|byte| **byte != b'"').count();
        (1..=run).any(|at| rest.get(at) == Some(&here) && rest.get(at + 1) == Some(&b':'))
    };
    if here == b'<' {
        if stream.take("!--") {
            state.tokenize = Tokenize::XmlComment;
            return till(stream, state, b"-->", Some(Kind::Comment));
        }
        if stream.at("![CDATA") {
            state.tokenize = Tokenize::Cdata;
            return Some(Kind::Operator);
        }
        if stream.at("?") {
            state.tokenize = Tokenize::Instruction;
            return till(stream, state, b"?>", Some(Kind::Comment));
        }
        let closing = stream.eat(|byte| byte == b'/').is_some();
        stream.eat_space();
        stream.eat_while(|byte| !(byte.is_ascii_whitespace() || b"=<>\"'/?".contains(&byte)));
        stream.eat_space();
        if closing && stream.eat(|byte| byte == b'>').is_some() {
            state.pop();
            state.tokenize = Tokenize::Base;
            return Some(Kind::Operator);
        }
        if stream.eat(|byte| byte == b'/').is_none() {
            state.push(Open::Tag, Tokenize::Base);
        }
        state.tokenize = if stream.eat(|byte| byte == b'>').is_some() { Tokenize::Base } else { Tokenize::Attributes };
        return Some(Kind::Operator);
    }
    if here == b'{' {
        state.push(Open::CodeBlock, Tokenize::Base);
        return None;
    }
    if here == b'}' {
        state.pop();
        return None;
    }
    if state.top(Open::Tag) {
        if here == b'>' {
            return Some(Kind::Operator);
        }
        if here == b'/' && stream.eat(|byte| byte == b'>').is_some() {
            state.pop();
            return Some(Kind::Operator);
        }
        return Some(Kind::Variable);
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| byte.is_ascii_digit());
        if stream.eat(|byte| byte == b'.').is_some() {
            stream.eat_while(|byte| byte.is_ascii_digit());
        }
        if stream.peek() == b'E' {
            let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'+' | b'-'));
            if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
        return None;
    }
    if here == b'(' && stream.eat(|byte| byte == b':').is_some() {
        state.push(Open::Comment, Tokenize::Base);
        state.tokenize = Tokenize::Comment;
        return comment(stream, state);
    }
    if !eq_name && (here == b'"' || here == b'\'') {
        state.tokenize = Tokenize::String(here, false);
        return string(stream, state, here, false);
    }
    if here == b'$' {
        if stream.eat(|byte| byte == b'"').is_some() {
            while stream.next().is_some_and(|byte| byte != b'"') {}
            stream.eat(|byte| byte == b':');
        } else {
            stream.eat_while(name_byte);
            if !stream.at(":=") {
                stream.eat(|byte| byte == b':');
            }
        }
        stream.eat_while(name_byte);
        return Some(Kind::Variable);
    }
    if here == b':' && stream.eat(|byte| byte == b'=').is_some() {
        return Some(Kind::Keyword);
    }
    match here {
        b'(' => {
            state.push(Open::Paren, Tokenize::Base);
            return None;
        }
        b')' => {
            state.pop();
            return None;
        }
        b'[' => {
            state.push(Open::Bracket, Tokenize::Base);
            return None;
        }
        b']' => {
            state.pop();
            return None;
        }
        _ => {}
    }
    let single = table(stream.current());
    if eq_name {
        while stream.next().is_some_and(|byte| byte != here) {}
    }
    if single.is_none() {
        stream.eat_while(name_byte);
    }
    let prefixed = stream.eat(|byte| byte == b':').is_some();
    if stream.eat(|byte| byte == b':').is_none() && prefixed {
        stream.eat_while(name_byte);
    }
    let call = stream.rest().iter().find(|byte| **byte != b' ' && **byte != b'\t') == Some(&b'(');
    let word = stream.current().to_string();
    let mut style = table(&word);
    if call && style.is_none() {
        style = Some("call");
    }
    if state.top(Open::XmlConstructor) {
        state.pop();
        return Some(Kind::Variable);
    }
    if word == "element" || word == "attribute" || style == Some("axis") {
        state.push(Open::XmlConstructor, Tokenize::Base);
    }
    match style {
        Some("keyword" | "operator") => Some(Kind::Keyword),
        Some("axis") => Some(Kind::String),
        Some("call") => Some(Kind::Variable),
        Some(_) => None,
        None => Some(Kind::Variable),
    }
}

const KEYWORDS: &str = "after all allowing ancestor ancestor-or-self any array as ascending at attribute base-uri before boundary-space by case cast \
castable catch child collation comment construction contains content context copy copy-namespaces count decimal-format \
declare default delete descendant descendant-or-self descending diacritics different distance document document-node element \
else empty empty-sequence encoding end entire every exactly except external first following following-sibling for from ftand \
ftnot ft-option ftor function fuzzy greatest group if import in inherit insensitive insert instance intersect into invoke is \
item language last lax least let levels lowercase map modify module most namespace next no node nodes no-inherit no-preserve \
not occurs of only option order ordered ordering paragraph paragraphs parent phrase preceding preceding-sibling preserve \
previous processing-instruction relationship rename replace return revalidation same satisfies schema schema-attribute \
schema-element score self sensitive sentence sentences sequence skip sliding some stable start stemming stop strict strip \
switch text then thesaurus times to transform treat try tumbling type typeswitch union unordered update updating uppercase \
using validate value variable version weight when where wildcards window with without word words xquery";

const TYPES: &str = "xs:anyAtomicType xs:anySimpleType xs:anyType xs:anyURI xs:base64Binary xs:boolean xs:byte xs:date xs:dateTime \
xs:dateTimeStamp xs:dayTimeDuration xs:decimal xs:double xs:duration xs:ENTITIES xs:ENTITY xs:float xs:gDay xs:gMonth \
xs:gMonthDay xs:gYear xs:gYearMonth xs:hexBinary xs:ID xs:IDREF xs:IDREFS xs:int xs:integer xs:item xs:java xs:language \
xs:long xs:Name xs:NCName xs:negativeInteger xs:NMTOKEN xs:NMTOKENS xs:nonNegativeInteger xs:nonPositiveInteger \
xs:normalizedString xs:NOTATION xs:numeric xs:positiveInteger xs:precisionDecimal xs:QName xs:short xs:string xs:time \
xs:token xs:unsignedByte xs:unsignedInt xs:unsignedLong xs:unsignedShort xs:untyped xs:untypedAtomic xs:yearMonthDuration";
