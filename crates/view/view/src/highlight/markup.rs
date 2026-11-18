//! Configuration and wiki languages, as their CodeMirror 5 modes color them, each ported token
//! for token: Asterisk dialplans, DTDs, Velocity templates, TiddlyWiki, and Tiki wiki (see each
//! function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Markup {
    comment: bool,
    /// Asterisk: where in an `exten =>` line it is.
    exten_start: bool,
    exten_same: bool,
    exten_include: bool,
    exten_exten: bool,
    exten_priority: bool,
    exten_application: bool,
    /// DTD and Velocity: an open string's quote; DTD: a `<?` instruction.
    string: Option<u8>,
    instruction: bool,
    /// Velocity.
    before_params: bool,
    in_params: bool,
    in_string: bool,
    last_builtin: bool,
    meta: bool,
    /// TiddlyWiki and Tiki: what reads the next token.
    wiki: Wiki,
    block: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Wiki {
    #[default]
    Base,
    Code,
    /// To a closing pair: its bytes and its style.
    Pair([u8; 2], Option<Kind>),
    Macro,
    /// Tiki: to `close`, then (when `back`) the base again at the line's end.
    Until(&'static str, Option<Kind>, bool),
    /// Tiki: a plugin's attributes, an attribute's value.
    Plugin,
    Value,
    Quoted(u8),
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

/// Asterisk, as CodeMirror 5's asterisk mode colors it: `exten`, `same`, `include` atoms (plain)
/// and the parts after them (the extension a tag, the priority a number, a known application
/// plain, as is its `def strong`), `[contexts]` headers, `"strings"` to their quote, `${variables}`, `#include` lines,
/// `;` and `;-- --;` comments.
pub fn asterisk(text: &str, state: &mut Markup, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.exten_start {
            stream.eat_while(|byte| !byte.is_ascii_whitespace());
            let current = stream.current();
            state.exten_start = false;
            if current == "=" || current == "=>" {
                state.exten_exten = true;
                return None;
            }
            stream.skip_to_end();
            return Some(Kind::Operator);
        }
        if state.exten_exten {
            state.exten_exten = false;
            state.exten_priority = true;
            stream.eat_while(|byte| byte != b',');
            if state.exten_include {
                stream.skip_to_end();
                state.exten_priority = false;
                state.exten_include = false;
            }
            if state.exten_same {
                state.exten_priority = false;
                state.exten_same = false;
                state.exten_application = true;
            }
            return Some(Kind::Operator);
        }
        if state.exten_priority {
            state.exten_priority = false;
            state.exten_application = true;
            stream.next();
            if state.exten_same {
                return None;
            }
            stream.eat_while(|byte| byte != b',');
            return Some(Kind::Number);
        }
        if state.exten_application {
            stream.eat_while(|byte| byte == b',');
            if stream.current() == "," {
                return None;
            }
            stream.eat_while(scan::js_word);
            state.exten_application = false;
            return None;
        }
        asterisk_base(stream, state)
    });
}

fn asterisk_base(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let here = stream.next()?;
    let skip_to = |stream: &mut Stream, byte: u8| {
        if let Some(found) = stream.rest().iter().position(|next| *next == byte) {
            stream.pos += found;
            true
        } else {
            false
        }
    };
    if state.comment {
        if here == b'-' && stream.take("-;") {
            state.comment = false;
        } else if let Some(found) = stream.rest().windows(3).position(|three| three == b"--;") {
            stream.pos += found + 3;
            state.comment = false;
        } else {
            stream.skip_to_end();
        }
        return Some(Kind::Comment);
    }
    if here == b';' {
        if stream.take("--") && stream.peek() != b'-' {
            state.comment = true;
        } else {
            stream.skip_to_end();
        }
        return Some(Kind::Comment);
    }
    if here == b'[' {
        skip_to(stream, b']');
        stream.eat(|byte| byte == b']');
        return None;
    }
    if here == b'"' {
        skip_to(stream, b'"');
        return Some(Kind::String);
    }
    if here == b'\'' {
        skip_to(stream, b'\'');
        return Some(Kind::String);
    }
    if here == b'#' {
        stream.eat_while(scan::js_word);
        if matches!(stream.current(), "#include" | "#exec") {
            stream.skip_to_end();
            return None;
        }
    }
    if here == b'$' && stream.peek() == b'{' {
        skip_to(stream, b'}');
        stream.eat(|byte| byte == b'}');
        return Some(Kind::Variable);
    }
    stream.eat_while(scan::js_word);
    match stream.current() {
        "exten" | "same" | "include" | "ignorepat" | "switch" => {
            state.exten_start = true;
            match stream.current() {
                "same" => state.exten_same = true,
                "include" | "switch" | "ignorepat" => state.exten_include = true,
                _ => {}
            }
            None
        }
        _ => None,
    }
}

/// DTDs, as CodeMirror 5's dtd mode colors them: `<!ELEMENT` keywords, names tags, `#PCDATA`
/// atoms, `|` keywords, `%` and `*` numbers, strings, `<!-- -->` comments, `<? ?>` meta.
pub fn dtd(text: &str, state: &mut Markup, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            return dtd_comment(stream, state);
        }
        if let Some(quote) = state.string {
            return dtd_string(stream, state, quote);
        }
        if state.instruction {
            while !stream.eol() {
                if stream.take("?>") {
                    state.instruction = false;
                    break;
                }
                stream.next();
            }
            return Some(Kind::Comment);
        }
        let here = stream.next()?;
        if here == b'<' && stream.eat(|byte| byte == b'!').is_some() {
            if stream.eat_while(|byte| byte == b'-') {
                state.comment = true;
                return dtd_comment(stream, state);
            }
            if stream.eat_while(scan::js_word) {
                return Some(Kind::Keyword);
            }
            return None;
        }
        if here == b'<' && stream.eat(|byte| byte == b'?').is_some() {
            state.instruction = true;
            return Some(Kind::Comment);
        }
        if here == b'#' && stream.eat_while(scan::js_word) {
            return None;
        }
        if here == b'|' {
            return Some(Kind::Keyword);
        }
        if b"()[]-.,+?>".contains(&here) {
            return None;
        }
        if here == b'"' || here == b'\'' {
            return dtd_string(stream, state, here);
        }
        if stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'?' || byte == b'+') {
            if stream.current().ends_with(['?', '+']) {
                stream.back_up(1);
            }
            return Some(Kind::Operator);
        }
        if here == b'%' || here == b'*' {
            return Some(Kind::Number);
        }
        stream.eat_while(|byte| scan::js_word(byte) || b"\\-%.{,".contains(&byte));
        None
    });
}

fn dtd_comment(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let mut dashes = 0;
    while let Some(here) = stream.next() {
        if dashes >= 2 && here == b'>' {
            state.comment = false;
            break;
        }
        dashes = if here == b'-' { dashes + 1 } else { 0 };
    }
    Some(Kind::Comment)
}

fn dtd_string(stream: &mut Stream, state: &mut Markup, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    state.string = Some(quote);
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.string = None;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

const VELOCITY_KEYWORDS: &str = "#end #else #break #stop #[[ #]] #{end} #{else} #{break} #{stop}";
const VELOCITY_FUNCTIONS: &str = "#if #elseif #foreach #set #include #parse #macro #define #evaluate #{if} #{elseif} #{foreach} \
#{set} #{include} #{parse} #{macro} #{define} #{evaluate}";
const VELOCITY_SPECIALS: &str = "$foreach.count $foreach.hasNext $foreach.first $foreach.last $foreach.topmost $foreach.parent.count \
$foreach.parent.hasNext $foreach.parent.first $foreach.parent.last $foreach.parent $velocityCount $!bodyContent $bodyContent";

/// Velocity templates, as CodeMirror 5's velocity mode colors them: directives keywords,
/// `$references` builtins (a known one a keyword), a member after a reference builtin, numbers,
/// strings (in parentheses; a `$` inside one its own token), operators, `##`, `#* *#` comments,
/// `#[[ ]]#` meta.
pub fn velocity(text: &str, state: &mut Markup, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.comment {
            let mut star = false;
            while let Some(here) = stream.next() {
                if here == b'#' && star {
                    state.comment = false;
                    break;
                }
                star = here == b'*';
            }
            return Some(Kind::Comment);
        }
        if state.meta {
            let mut closing = 0;
            while let Some(here) = stream.next() {
                if here == b'#' && closing == 2 {
                    state.meta = false;
                    break;
                }
                if here == b']' {
                    closing += 1;
                } else if here != b' ' {
                    closing = 0;
                }
            }
            return Some(Kind::Comment);
        }
        if let Some(quote) = state.string {
            return velocity_string(stream, state, quote);
        }
        velocity_token(stream, state)
    });
}

fn velocity_string(stream: &mut Stream, state: &mut Markup, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    state.string = Some(quote);
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.string = None;
            break;
        }
        if quote == b'"' && stream.peek() == b'$' && !escaped {
            state.string = None;
            state.in_string = true;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn velocity_token(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let before_params = std::mem::take(&mut state.before_params);
    let here = stream.next()?;
    if here == b'\'' && !state.in_string && state.in_params {
        state.last_builtin = false;
        return velocity_string(stream, state, here);
    }
    if here == b'"' {
        state.last_builtin = false;
        if state.in_string {
            state.in_string = false;
            return Some(Kind::String);
        }
        if state.in_params {
            return velocity_string(stream, state, here);
        }
        return None;
    }
    if b"[]{}(),;.".contains(&here) {
        if here == b'(' && before_params {
            state.in_params = true;
        } else if here == b')' {
            state.in_params = false;
            state.last_builtin = true;
        }
        return None;
    }
    if here.is_ascii_digit() {
        state.last_builtin = false;
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
        return Some(Kind::Number);
    }
    if here == b'#' && stream.eat(|byte| byte == b'*').is_some() {
        state.last_builtin = false;
        state.comment = true;
        let mut star = false;
        while let Some(next) = stream.next() {
            if next == b'#' && star {
                state.comment = false;
                break;
            }
            star = next == b'*';
        }
        return Some(Kind::Comment);
    }
    if here == b'#' {
        let at = stream.pos;
        let mut scan_at = at;
        let mut opened = 0;
        while opened < 2 {
            while stream.byte(scan_at) == b' ' {
                scan_at += 1;
            }
            if stream.byte(scan_at) != b'[' {
                break;
            }
            scan_at += 1;
            opened += 1;
        }
        if opened == 2 {
            stream.pos = scan_at;
            state.last_builtin = false;
            state.meta = true;
            let mut closing = 0;
            while let Some(next) = stream.next() {
                if next == b'#' && closing == 2 {
                    state.meta = false;
                    break;
                }
                if next == b']' {
                    closing += 1;
                } else if next != b' ' {
                    closing = 0;
                }
            }
            return Some(Kind::Comment);
        }
        if stream.eat(|byte| byte == b'#').is_some() {
            state.last_builtin = false;
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
    }
    if here == b'$' {
        stream.eat(|byte| byte == b'!');
        stream.eat_while(|byte| scan::js_word(byte) || b"$.{}-".contains(&byte));
        if listed(VELOCITY_SPECIALS, stream.current()) {
            return Some(Kind::Keyword);
        }
        state.last_builtin = true;
        state.before_params = true;
        return Some(Kind::Property);
    }
    let operator = |byte: u8| b"+-*&%=<>!?:/|".contains(&byte);
    if operator(here) {
        state.last_builtin = false;
        stream.eat_while(operator);
        return Some(Kind::Operator);
    }
    let start = stream.start;
    stream.eat_while(|byte| scan::js_word(byte) || b"$_{}@".contains(&byte));
    let word = stream.current();
    if listed(VELOCITY_KEYWORDS, word) {
        return Some(Kind::Keyword);
    }
    let directive = word.starts_with('#') && {
        let rest = word.strip_prefix('#').unwrap_or("");
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    };
    if listed(VELOCITY_FUNCTIONS, word) || (directive && stream.peek() == b'(' && !listed(VELOCITY_FUNCTIONS, &word.to_ascii_lowercase())) {
        state.before_params = true;
        state.last_builtin = false;
        return Some(Kind::Keyword);
    }
    if state.in_string {
        state.last_builtin = false;
        return Some(Kind::String);
    }
    if start > 0 && stream.byte(start - 1) == b'.' && state.last_builtin {
        return Some(Kind::Property);
    }
    state.last_builtin = false;
    None
}

/// TiddlyWiki, as CodeMirror 5's tiddlywiki mode colors it: list and heading marks, `/***/`
/// and `{{{ }}}` blocks comments, links, `----` rules punctuation, `<<macros>>` names keywords, numbers, `"` strings;
/// emphasis plain.
pub fn tiddlywiki(text: &str, state: &mut Markup, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        match state.wiki {
            Wiki::Code => tiddly_code(stream, state),
            Wiki::Pair(pair, style) => {
                let mut previous = false;
                while let Some(here) = stream.next() {
                    if here == pair[1] && previous {
                        state.wiki = Wiki::Base;
                        break;
                    }
                    previous = here == pair[0];
                }
                style
            }
            Wiki::Macro => {
                let Some(here) = stream.next() else {
                    state.wiki = Wiki::Base;
                    return None;
                };
                if here == b'>' && stream.peek() == b'>' {
                    stream.next();
                    state.wiki = Wiki::Base;
                    return None;
                }
                stream.eat_while(|byte| scan::js_word(byte) || byte == b'$');
                listed(TIDDLY_MACROS, stream.current()).then_some(Kind::Keyword)
            }
            _ => tiddly_base(stream, state),
        }
    });
}

const TIDDLY_MACROS: &str = "allTags closeAll list newJournal newTiddler permaview saveChanges search slider tabs tag tagging tags \
tiddler timeline today version option with filter";

fn tiddly_code(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    if state.block && !stream.current().is_empty() {
        return Some(Kind::Comment);
    }
    let rest = stream.rest();
    if !state.block
        && let Some(found) = rest.windows(3).position(|three| three == b"}}}")
    {
        stream.pos += found + 3;
        state.wiki = Wiki::Base;
    } else if state.block && stream.sol() && rest == b"}}}" {
        stream.pos += 3;
        state.wiki = Wiki::Base;
    } else {
        stream.next();
    }
    Some(Kind::Comment)
}

fn tiddly_base(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let sol = stream.sol();
    let here = stream.peek();
    state.block = false;
    if sol && b"</*{}-".contains(&here) {
        let line = stream.rest();
        if line == b"{{{" {
            state.block = true;
            stream.pos += 3;
            state.wiki = Wiki::Code;
            return tiddly_code(stream, state);
        }
        if line == b"<<<" {
            stream.skip_to_end();
            return None;
        }
        if [b"/***".as_slice(), b"***/", b"//{{{", b"//}}}", b"<!--{{{-->", b"<!--}}}-->"].contains(&line) {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if line.len() >= 4 && line.iter().all(|byte| *byte == b'-') {
            // A rule, which Obsidian colors as punctuation.
            stream.skip_to_end();
            return Some(Kind::Punctuation);
        }
    }
    stream.next();
    if sol && b"/*!#;:>|".contains(&here) {
        match here {
            b'!' => {
                stream.skip_to_end();
                return None;
            }
            b'*' | b'#' | b';' | b':' => {
                stream.eat_while(|byte| byte == here);
                return Some(Kind::Comment);
            }
            b'>' => {
                stream.eat_while(|byte| byte == b'>');
                return None;
            }
            b'|' => return None,
            _ => {}
        }
    }
    if here == b'{' && stream.take("{{") {
        state.wiki = Wiki::Code;
        return tiddly_code(stream, state);
    }
    if matches!(here.to_ascii_lowercase(), b'h' | b'f')
        && matches!(stream.peek().to_ascii_lowercase(), b't' | b'i')
        && let Some(end) = tiddly_url(stream)
    {
        stream.pos = end;
        return Some(Kind::Link);
    }
    match here {
        b'"' => return Some(Kind::String),
        b'~' => return None,
        b'[' | b']' if stream.eat(|byte| byte == here).is_some() => return None,
        b'@' => {
            stream.eat_while(|byte| scan::js_word(byte) || byte == b'-');
            return Some(Kind::Link);
        }
        b'0'..=b'9' => {
            stream.eat_while(|byte| byte.is_ascii_digit());
            return Some(Kind::Number);
        }
        b'/' if stream.eat(|byte| byte == b'%').is_some() => return tiddly_pair(stream, state, *b"%/", Some(Kind::Comment)),
        b'/' if stream.eat(|byte| byte == b'/').is_some() => return tiddly_pair(stream, state, *b"//", None),
        b'_' if stream.eat(|byte| byte == b'_').is_some() => return tiddly_pair(stream, state, *b"__", None),
        b'-' if stream.eat(|byte| byte == b'-').is_some() => {
            if stream.peek() != b' ' {
                return tiddly_pair(stream, state, *b"--", None);
            }
            return None;
        }
        b'\'' if stream.eat(|byte| byte == b'\'').is_some() => return tiddly_pair(stream, state, *b"''", None),
        b'<' if stream.eat(|byte| byte == b'<').is_some() => {
            state.wiki = Wiki::Macro;
            return None;
        }
        _ => {}
    }
    stream.eat_while(|byte| scan::js_word(byte) || byte == b'$');
    None
}

/// `\b(ttps?|tp|ile)://[-A-Z0-9+&@#/%?=~_|$!:,.;]*[A-Z0-9+&@#/%=~_|$]` (any case) where the
/// stream stands: where it ends.
fn tiddly_url(stream: &Stream) -> Option<usize> {
    let rest = stream.rest();
    let scheme = ["ttps", "ttp", "tp", "ile"].iter().find(|scheme| {
        rest.get(..scheme.len()).is_some_and(|text| text.eq_ignore_ascii_case(scheme.as_bytes()))
            && rest.get(scheme.len()..scheme.len() + 3) == Some(b"://")
    })?;
    let from = scheme.len() + 3;
    let body = |byte: u8| byte.is_ascii_alphanumeric() || b"-+&@#/%?=~_|$!:,.;".contains(&byte);
    let last = |byte: u8| byte.is_ascii_alphanumeric() || b"+&@#/%=~_|$".contains(&byte);
    let run = rest.get(from..)?.iter().take_while(|byte| body(**byte)).count();
    let end = (from..from + run).rev().find(|at| rest.get(*at).is_some_and(|byte| last(*byte)))?;
    Some(stream.pos + end + 1)
}

fn tiddly_pair(stream: &mut Stream, state: &mut Markup, pair: [u8; 2], style: Option<Kind>) -> Option<Kind> {
    state.wiki = Wiki::Pair(pair, style);
    let mut previous = false;
    while let Some(here) = stream.next() {
        if here == pair[1] && previous {
            state.wiki = Wiki::Base;
            break;
        }
        previous = here == pair[0];
    }
    style
}

/// Tiki wiki, as CodeMirror 5's tiki mode colors it: `{PLUGIN}` tags with their attributes,
/// `((wiki links))` and `[links]` variables, `||` and `::` comments, `-=titles=-` and `!headings`
/// strings, `--deleted--` errors, `~np~` meta, list marks brackets; emphasis plain.
pub fn tiki(text: &str, state: &mut Markup, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        match state.wiki {
            Wiki::Until(close, style, back) => tiki_until(stream, state, close, style, back),
            Wiki::Plugin => tiki_plugin(stream, state),
            Wiki::Value => {
                while !stream.eol() {
                    let here = stream.next().unwrap_or(0);
                    if here == b' ' || here == b',' || b" )}".contains(&stream.peek()) {
                        state.wiki = Wiki::Plugin;
                        break;
                    }
                }
                Some(Kind::String)
            }
            Wiki::Quoted(quote) => {
                while !stream.eol() {
                    if stream.next() == Some(quote) {
                        state.wiki = Wiki::Plugin;
                        break;
                    }
                }
                Some(Kind::String)
            }
            _ => tiki_base(stream, state),
        }
    });
}

fn tiki_until(stream: &mut Stream, state: &mut Markup, close: &'static str, style: Option<Kind>, back: bool) -> Option<Kind> {
    state.wiki = Wiki::Until(close, style, back);
    while !stream.eol() {
        if stream.take(close) {
            state.wiki = Wiki::Base;
            break;
        }
        stream.next();
    }
    if back {
        state.wiki = Wiki::Base;
    }
    style
}

fn tiki_base(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let sol = stream.sol();
    let here = stream.next()?;
    match here {
        b'{' => {
            stream.eat(|byte| byte == b'/');
            stream.eat_space();
            stream.eat_while(|byte| !(byte.is_ascii_whitespace() || b"=\"'/?(}".contains(&byte)));
            state.wiki = Wiki::Plugin;
            return Some(Kind::Operator);
        }
        b'_' if stream.eat(|byte| byte == b'_').is_some() => return tiki_until(stream, state, "__", None, true),
        b'\'' if stream.eat(|byte| byte == b'\'').is_some() => return tiki_until(stream, state, "''", None, true),
        b'(' if stream.eat(|byte| byte == b'(').is_some() => return tiki_until(stream, state, "))", Some(Kind::Variable), true),
        b'[' => return tiki_until(stream, state, "]", Some(Kind::Variable), true),
        b'|' if stream.eat(|byte| byte == b'|').is_some() => return tiki_until(stream, state, "||", Some(Kind::Comment), false),
        b'-' if stream.eat(|byte| byte == b'=').is_some() => return tiki_until(stream, state, "=-", Some(Kind::String), true),
        b'-' if stream.eat(|byte| byte == b'-').is_some() => return tiki_until(stream, state, "--", Some(Kind::Operator), true),
        b'=' if stream.take("==") => return tiki_until(stream, state, "===", None, true),
        b':' if stream.eat(|byte| byte == b':').is_some() => return tiki_until(stream, state, "::", Some(Kind::Comment), false),
        b'^' => return tiki_until(stream, state, "^", None, false),
        b'~' if stream.take("np~") => return tiki_until(stream, state, "~/np~", Some(Kind::Comment), false),
        _ => {}
    }
    if sol {
        match here {
            b'!' => {
                let _ = stream.take("!!!!!") || stream.take("!!!!") || stream.take("!!!") || stream.take("!!");
                stream.skip_to_end();
                return Some(Kind::String);
            }
            b'*' | b'#' | b'+' => {
                stream.skip_to_end();
                return Some(Kind::Punctuation);
            }
            _ => {}
        }
    }
    None
}

fn tiki_plugin(stream: &mut Stream, state: &mut Markup) -> Option<Kind> {
    let here = stream.next()?;
    match here {
        b'}' => {
            state.wiki = Wiki::Base;
            Some(Kind::Operator)
        }
        b'(' | b')' => Some(Kind::Punctuation),
        b'=' => {
            if stream.peek() == b'>' {
                stream.next();
            }
            if !matches!(stream.peek(), b'\'' | b'"') {
                state.wiki = Wiki::Value;
            }
            Some(Kind::Operator)
        }
        b'\'' | b'"' => {
            state.wiki = Wiki::Quoted(here);
            while !stream.eol() {
                if stream.next() == Some(here) {
                    state.wiki = Wiki::Plugin;
                    break;
                }
            }
            Some(Kind::String)
        }
        _ => {
            stream.eat_while(|byte| !(byte.is_ascii_whitespace() || b"=\"'/?".contains(&byte)));
            Some(Kind::Keyword)
        }
    }
}
