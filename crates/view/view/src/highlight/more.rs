//! Haskell, Groovy, nginx, CMake, and Protocol Buffers, as their CodeMirror 5 modes color them
//! (see each function), with what each carries from one line to the next.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct More {
    /// A block comment open (Haskell's `{-`, nginx's `/*`), with its closing text.
    comment: Option<&'static str>,
    /// Groovy: what it reads next (innermost last; none, its code), and the last token's type.
    groovy: Vec<Groovy>,
    last: &'static str,
    /// nginx: its blocks open (`{`) and the directive being read (`r`, a rule), innermost last.
    nginx: Vec<u8>,
    /// nginx: an open string (its quote).
    nginx_string: Option<u8>,
    /// CMake: an open string (its quote).
    cmake_string: Option<u8>,
    /// Dockerfile: where its reader is.
    docker: Docker,
    /// Octave: a `%{` comment open, and whether the last token was a value (a `'` after it transposes).
    octave_comment: bool,
    octave_value: bool,
    /// VB and Fortran: an open string (its quote).
    vb_string: Option<u8>,
    fortran_string: Option<u8>,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn comment_rest(text: &str, at: usize, close: &'static str, state: &mut More, out: &mut Out) -> usize {
    match text.get(at..).and_then(|rest| rest.find(close)) {
        Some(found) => {
            out.push(at..at + found + close.len(), Kind::Comment);
            state.comment = None;
            at + found + close.len()
        }
        None => {
            out.push(at..text.len(), Kind::Comment);
            state.comment = Some(close);
            text.len()
        }
    }
}

const HASKELL_KEYWORDS: &[&str] = &[
    "case", "class", "data", "default", "deriving", "do", "else", "foreign", "if", "import", "in", "infix", "infixl", "infixr", "instance", "let",
    "module", "newtype", "of", "then", "type", "where", "_",
];
const HASKELL_RESERVED_OPS: &[&str] = &["..", ":", "::", "=", "\\", "<-", "->", "@", "~", "=>"];
const HASKELL_BUILTINS: &str = "! !! $ $! && * ** + ++ - . / /= < <= =<< == > >= >> >>= ^ ^^ || Bool Bounded Char Double EQ Either Enum Eq \
False FilePath Float Floating Fractional Functor GT IO IOError Int Integer Integral Just LT Left Maybe Monad Nothing Num Ord Ordering \
Rational Read ReadS Real RealFloat RealFrac Right Show ShowS String True abs acos acosh all and any appendFile asTypeOf asin asinh atan \
atan2 atanh break catch ceiling compare concat concatMap const cos cosh curry cycle decodeFloat div divMod drop dropWhile either elem \
encodeFloat enumFrom enumFromThen enumFromThenTo enumFromTo error even exp exponent fail filter flip floatDigits floatRadix floatRange \
floor fmap foldl foldl1 foldr foldr1 fromEnum fromInteger fromIntegral fromRational fst gcd getChar getContents getLine head id init \
interact ioError isDenormalized isIEEE isInfinite isNaN isNegativeZero iterate last lcm length lex lines log logBase lookup map mapM \
mapM_ max maxBound maximum maybe min minBound minimum mod negate not notElem null odd or otherwise pi pred print product properFraction \
putChar putStr putStrLn quot quotRem read readFile readIO readList readLn readParen reads readsPrec realToFrac recip rem repeat \
replicate return reverse round scaleFloat scanl scanl1 scanr scanr1 seq sequence sequence_ show showChar showList showParen showString \
shows showsPrec significand signum sin sinh snd span splitAt sqrt subtract succ sum tail take takeWhile tan tanh toEnum toInteger \
toRational truncate uncurry undefined unlines until unwords unzip unzip3 userError words writeFile zip zip3 zipWith zipWith3";

/// Haskell, as CodeMirror 5's haskell mode colors it: keywords and reserved operators keywords, a
/// capitalized name a variable-2, a module's qualifier green, the Prelude's names and operators
/// builtins, other names and operators variables, `--` and nested `{- -}` comments.
pub fn haskell(text: &str, state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.comment {
        at = comment_rest(text, 0, close, state, out);
    }
    let builtin = |word: &str| HASKELL_BUILTINS.split_ascii_whitespace().any(|known| known == word);
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        if here == b'{' && byte(bytes, at + 1) == b'-' {
            at = comment_rest(text, at, "-}", state, out);
            continue;
        }
        if here == b'-' && byte(bytes, at + 1) == b'-' && !b"!#$%&*+./<=>?@\\^|~:".contains(&byte(bytes, at + 2)) {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'"' => {
                at = scan::string_end(bytes, at, b'"').unwrap_or(bytes.len());
                out.push(start..at, Kind::String);
            }
            b'\'' if byte(bytes, at + 2) == b'\'' || (byte(bytes, at + 1) == b'\\' && byte(bytes, at + 3) == b'\'') => {
                at = scan::string_end(bytes, at, b'\'').unwrap_or(bytes.len());
                out.push(start..at, Kind::String);
            }
            b'0'..=b'9' => {
                at = haskell_number(bytes, at);
                out.push(start..at, Kind::Number);
            }
            _ if here.is_ascii_uppercase() => {
                at = scan::word_end(bytes, at, b"'");
                // `Data.` before a name: the module's qualifier.
                if byte(bytes, at) == b'.' && byte(bytes, at + 1).is_ascii_alphabetic() {
                    at += 1;
                    out.push(start..at, Kind::String);
                    continue;
                }
                let word = text.get(start..at).unwrap_or("");
                out.push(start..at, if builtin(word) { Kind::Property } else { Kind::Variable });
            }
            _ if here.is_ascii_lowercase() || here == b'_' => {
                at = scan::word_end(bytes, at, b"'");
                let word = text.get(start..at).unwrap_or("");
                let kind = if HASKELL_KEYWORDS.contains(&word) {
                    Kind::Keyword
                } else if builtin(word) {
                    Kind::Property
                } else {
                    Kind::Variable
                };
                out.push(start..at, kind);
            }
            _ if b"!#$%&*+./<=>?@\\^|-~:".contains(&here) => {
                while b"!#$%&*+./<=>?@\\^|-~:".contains(&byte(bytes, at)) && at < bytes.len() {
                    at += 1;
                }
                let op = text.get(start..at).unwrap_or("");
                let kind = if HASKELL_RESERVED_OPS.contains(&op) {
                    Kind::Keyword
                } else if builtin(op) {
                    Kind::Property
                } else {
                    Kind::Variable
                };
                out.push(start..at, kind);
            }
            _ => at += 1,
        }
    }
}

const GROOVY_KEYWORDS: &str = "abstract as assert boolean break byte case catch char class const continue def default do double else enum \
extends final finally float for goto if implements import in instanceof int interface long native new package private protected public \
return short static strictfp super switch synchronized threadsafe throw throws trait transient try void volatile while";
const GROOVY_BLOCKS: &[&str] =
    &["catch", "class", "def", "do", "else", "enum", "finally", "for", "if", "interface", "switch", "trait", "try", "while"];

/// What Groovy reads next, innermost last: its code, a string (its quote, and whether it is
/// tripled), a `${…}` in a string (its open braces), a block comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Groovy {
    String(u8, bool),
    Interpolation(usize),
    Comment,
}

/// Groovy, as CodeMirror 5's groovy mode colors it: keywords, `null`, `true`, `false`, and `this`
/// atoms, an annotation meta, a name after `.` or a map's `key:` a property, other names
/// variables, strings (tripled across lines) with `${…}` code, a `/` a slashy string where an
/// expression starts.
pub fn groovy(text: &str, state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if matches!(byte(bytes, at), b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        let start = at;
        // The token's type (the mode's `type`, else its style), for the next token.
        let mut kind: &'static str = "";
        let style = match state.groovy.last().copied() {
            Some(Groovy::String(quote, triple)) => {
                at = groovy_string(bytes, at, quote, triple, state);
                Some(Kind::String)
            }
            Some(Groovy::Comment) => {
                at = groovy_comment(bytes, at, state);
                Some(Kind::Comment)
            }
            Some(Groovy::Interpolation(depth)) if byte(bytes, at) == b'}' && depth == 1 => {
                state.groovy.pop();
                // The string goes on from the `}`.
                match state.groovy.last().copied() {
                    Some(Groovy::String(quote, triple)) => {
                        at = groovy_string(bytes, at + 1, quote, triple, state);
                        Some(Kind::String)
                    }
                    _ => groovy_token(bytes, &mut at, state, &mut kind),
                }
            }
            Some(Groovy::Interpolation(depth)) => {
                let here = byte(bytes, at);
                if let Some(Groovy::Interpolation(open)) = state.groovy.last_mut() {
                    if here == b'}' {
                        *open = depth - 1;
                    } else if here == b'{' {
                        *open = depth + 1;
                    }
                }
                groovy_token(bytes, &mut at, state, &mut kind)
            }
            None => groovy_token(bytes, &mut at, state, &mut kind),
        };
        if at == start {
            at += 1;
        }
        out.mark(start..at, style);
        if style == Some(Kind::Comment) {
            continue;
        }
        state.last = if !kind.is_empty() {
            kind
        } else {
            match style {
                Some(Kind::Operator) => "operator",
                Some(Kind::Keyword) => "keyword",
                Some(Kind::Property) => "property",
                Some(Kind::String) => "string",
                Some(Kind::Number) => "number",
                Some(Kind::Variable) => "variable",
                _ => "atom",
            }
        };
    }
}

/// One token from Groovy's code (`tokenBase`).
fn groovy_token(bytes: &[u8], at: &mut usize, state: &mut More, kind: &mut &'static str) -> Option<Kind> {
    let start = *at;
    let here = byte(bytes, *at);
    *at += 1;
    if here == b'"' || here == b'\'' {
        return groovy_open(bytes, at, here, state);
    }
    let punctuation = match here {
        b'[' => "[",
        b']' => "]",
        b'{' => "{",
        b'}' => "}",
        b'(' => "(",
        b')' => ")",
        b',' => ",",
        b';' => ";",
        b':' => ":",
        b'.' => ".",
        _ => "",
    };
    if !punctuation.is_empty() {
        *kind = punctuation;
        return None;
    }
    if here.is_ascii_digit() {
        while scan::word(byte(bytes, *at)) || byte(bytes, *at) == b'.' {
            *at += 1;
        }
        return Some(Kind::Number);
    }
    if here == b'/' {
        if byte(bytes, *at) == b'*' {
            *at += 1;
            state.groovy.push(Groovy::Comment);
            *at = groovy_comment(bytes, *at, state);
            return Some(Kind::Comment);
        }
        if byte(bytes, *at) == b'/' {
            *at = bytes.len();
            return Some(Kind::Comment);
        }
        if groovy_expression_starts(state.last, false) {
            return groovy_open(bytes, at, here, state);
        }
    }
    if here == b'-' && byte(bytes, *at) == b'>' {
        *at += 1;
        *kind = "->";
        return None;
    }
    if b"+-*&%=<>!?|/~".contains(&here) {
        while b"+-*&%=<>|~".contains(&byte(bytes, *at)) && *at < bytes.len() {
            *at += 1;
        }
        return Some(Kind::Operator);
    }
    while scan::word(byte(bytes, *at)) || byte(bytes, *at) == b'$' {
        *at += 1;
    }
    if here == b'@' {
        while scan::word(byte(bytes, *at)) || matches!(byte(bytes, *at), b'$' | b'.') {
            *at += 1;
        }
        return Some(Kind::Comment);
    }
    if state.last == "." {
        return Some(Kind::Property);
    }
    if byte(bytes, *at) == b':' {
        *at += 1;
        *kind = "proplabel";
        return Some(Kind::Property);
    }
    let word = bytes.get(start..*at).and_then(|word| std::str::from_utf8(word).ok()).unwrap_or("");
    if matches!(word, "null" | "true" | "false" | "this") {
        return None;
    }
    if GROOVY_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
        if GROOVY_BLOCKS.contains(&word) {
            *kind = "newstatement";
        } else if matches!(word, "return" | "break" | "continue") {
            *kind = "standalone";
        }
        return Some(Kind::Keyword);
    }
    Some(Kind::Variable)
}

/// Whether an expression starts after a token of type `last` (where a `/` opens a string).
fn groovy_expression_starts(last: &str, after_line: bool) -> bool {
    last.is_empty()
        || last == "operator"
        || last == "->"
        || last.contains(['.', '[', '{', '(', ',', ';', ':'])
        || matches!(last, "newstatement" | "keyword" | "proplabel")
        || (last == "standalone" && !after_line)
}

/// A string opened by `quote` (read up to `at`): `''` empty, a third quote tripled.
fn groovy_open(bytes: &[u8], at: &mut usize, quote: u8, state: &mut More) -> Option<Kind> {
    let mut triple = false;
    if quote != b'/' && byte(bytes, *at) == quote {
        *at += 1;
        if byte(bytes, *at) != quote {
            return Some(Kind::String);
        }
        *at += 1;
        triple = true;
    }
    state.groovy.push(Groovy::String(quote, triple));
    *at = groovy_string(bytes, *at, quote, triple, state);
    Some(Kind::String)
}

/// A string's text read from `at`: to its quote (three, tripled), a `${` (its code read next),
/// or the line's end (a tripled string goes on).
fn groovy_string(bytes: &[u8], at: usize, quote: u8, triple: bool, state: &mut More) -> usize {
    let mut index = at;
    let mut escaped = false;
    let mut end = !triple;
    while let Some(here) = bytes.get(index).copied() {
        index += 1;
        if here == quote && !escaped {
            if !triple {
                break;
            }
            if byte(bytes, index) == quote && byte(bytes, index + 1) == quote {
                index += 2;
                end = true;
                break;
            }
        }
        if quote == b'"' && here == b'$' && !escaped && byte(bytes, index) == b'{' {
            state.groovy.push(Groovy::Interpolation(1));
            return index + 1;
        }
        escaped = !escaped && here == b'\\';
    }
    if end {
        state.groovy.pop();
    }
    index
}

fn groovy_comment(bytes: &[u8], at: usize, state: &mut More) -> usize {
    let mut index = at;
    let mut star = false;
    while let Some(here) = bytes.get(index).copied() {
        index += 1;
        if here == b'/' && star {
            state.groovy.pop();
            return index;
        }
        star = here == b'*';
    }
    index
}

/// A Haskell number from `at`: digits, a fraction (`.` and digits, so `1..10` is a range), an
/// exponent; `0x` hex and `0o` octal.
fn haskell_number(bytes: &[u8], at: usize) -> usize {
    let mut end = at + 1;
    if byte(bytes, at) == b'0' && matches!(byte(bytes, end), b'x' | b'X') {
        end += 1;
        while byte(bytes, end).is_ascii_hexdigit() {
            end += 1;
        }
        return end;
    }
    if byte(bytes, at) == b'0' && matches!(byte(bytes, end), b'o' | b'O') {
        end += 1;
        while matches!(byte(bytes, end), b'0'..=b'7') {
            end += 1;
        }
        return end;
    }
    while byte(bytes, end).is_ascii_digit() {
        end += 1;
    }
    if byte(bytes, end) == b'.' && byte(bytes, end + 1).is_ascii_digit() {
        end += 1;
        while byte(bytes, end).is_ascii_digit() {
            end += 1;
        }
    }
    if matches!(byte(bytes, end), b'e' | b'E') {
        end += 1;
        if matches!(byte(bytes, end), b'-' | b'+') {
            end += 1;
        }
        while byte(bytes, end).is_ascii_digit() {
            end += 1;
        }
    }
    end
}

/// CMake, as CodeMirror 5's cmake mode colors it, a character at a time: a command's name a
/// definition (plain), its parentheses brackets, a digit a number (only the first of `3.16`: the
/// `.` takes the rest as a word), strings broken at each `${…}` variable, comments.
pub fn cmake(text: &str, state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        if matches!(here, b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        at += 1;
        if here == b'$' {
            let mut end = at;
            if byte(bytes, end) == b'{' {
                end += 1;
            }
            let name = end;
            while scan::word(byte(bytes, end)) {
                end += 1;
            }
            if end > name {
                if byte(bytes, end) == b'}' {
                    end += 1;
                }
                at = end;
            }
            out.push(start..at, Kind::Variable);
        } else if let Some(quote) = state.cmake_string {
            at = cmake_string(bytes, start, quote, state);
            out.push(start..at, Kind::String);
        } else if cmake_command(bytes, at) {
            at = scan::word_end(bytes, at, b"");
        } else if here == b'#' {
            out.push(start..bytes.len(), Kind::Comment);
            return;
        } else if here == b'"' || here == b'\'' {
            at = cmake_string(bytes, at, here, state);
            out.push(start..at, Kind::String);
        } else if here == b'(' || here == b')' {
            out.push(start..at, Kind::Punctuation);
        } else if here.is_ascii_digit() {
            out.push(start..at, Kind::Number);
        } else {
            while scan::word(byte(bytes, at)) || byte(bytes, at) == b'-' {
                at += 1;
            }
        }
    }
}

/// Whether a command's name goes on from `at` (spaces, a name, then `(` or ` (`), as the mode
/// matches it after a token's first character.
fn cmake_command(bytes: &[u8], at: usize) -> bool {
    let mut index = at;
    while byte(bytes, index).is_ascii_whitespace() && index < bytes.len() {
        index += 1;
    }
    let name = index;
    while scan::word(byte(bytes, index)) {
        index += 1;
    }
    index > name && (byte(bytes, index) == b'(' || (byte(bytes, index) == b' ' && byte(bytes, index + 1) == b'('))
}

/// A CMake string read from `at` to its quote; a `"` string stops before a `$` (a variable,
/// read next) and goes on to the next line when the line ends first.
fn cmake_string(bytes: &[u8], at: usize, quote: u8, state: &mut More) -> usize {
    let mut index = at;
    let mut previous = 0;
    while let Some(here) = bytes.get(index).copied() {
        index += 1;
        if here == quote {
            state.cmake_string = None;
            return index;
        }
        if here == b'$' && previous != b'\\' && quote == b'"' {
            state.cmake_string = Some(quote);
            return index - 1;
        }
        previous = here;
    }
    state.cmake_string = Some(quote);
    bytes.len()
}

const PROTOBUF_KEYWORDS: &str = "package message import syntax required optional repeated reserved default extensions packed bool bytes \
double enum float string int32 int64 uint32 uint64 sint32 sint64 fixed32 fixed64 sfixed32 sfixed64 option service rpc returns";

/// Protocol Buffers, as CodeMirror 5's protobuf mode colors it: keywords (its types too, in any
/// case), names variables, signed numbers, strings (to their quote, no escapes), `//` comments.
pub fn protobuf(text: &str, _state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    let digits = |from: usize| (from..).find(|index| !byte(bytes, *index).is_ascii_digit()).unwrap_or(from);
    let exponent = |from: usize| {
        let mut end = from;
        if matches!(byte(bytes, end), b'E' | b'e' | b'D' | b'd') {
            end += 1;
            if matches!(byte(bytes, end), b'+' | b'-') {
                end += 1;
            }
            let after = digits(end);
            if after > end {
                return after;
            }
        }
        from
    };
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        if matches!(here, b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        if here == b'/' && byte(bytes, at + 1) == b'/' {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        if matches!(here, b'0'..=b'9' | b'.' | b'+' | b'-') {
            let signed = if matches!(here, b'+' | b'-') { at + 1 } else { at };
            let hex = byte(bytes, signed) == b'0' && byte(bytes, signed + 1) == b'x' && byte(bytes, signed + 2).is_ascii_hexdigit();
            let whole = digits(signed);
            let end = if hex {
                (signed + 2..).find(|index| !byte(bytes, *index).is_ascii_hexdigit()).unwrap_or(signed + 2)
            } else if byte(bytes, whole) == b'.' && byte(bytes, whole + 1).is_ascii_digit() {
                exponent(digits(whole + 1))
            } else if whole > signed {
                exponent(whole)
            } else {
                start
            };
            if end > start {
                out.push(start..end, Kind::Number);
                at = end;
                continue;
            }
        }
        if here == b'"' || here == b'\'' {
            // `"…"`, a doubled quote inside it.
            let mut index = at + 1;
            let mut closed = None;
            while index < bytes.len() {
                if byte(bytes, index) == here {
                    if byte(bytes, index + 1) == here {
                        index += 2;
                        continue;
                    }
                    closed = Some(index + 1);
                    break;
                }
                index += 1;
            }
            if let Some(end) = closed {
                out.push(start..end, Kind::String);
                at = end;
                continue;
            }
        }
        if here.is_ascii_alphabetic() || here == b'_' || here >= 0x80 {
            while byte(bytes, at).is_ascii_alphanumeric() || byte(bytes, at) == b'_' || byte(bytes, at) >= 0x80 {
                at += 1;
            }
            let word = text.get(start..at).unwrap_or("");
            let keyword = PROTOBUF_KEYWORDS.split_ascii_whitespace().find(|known| {
                word.get(..known.len()).is_some_and(|head| head.eq_ignore_ascii_case(known)) && !scan::word(byte(bytes, start + known.len()))
            });
            match keyword {
                Some(known) => {
                    out.push(start..start + known.len(), Kind::Keyword);
                    at = start + known.len();
                }
                None => out.push(start..at, Kind::Variable),
            }
            continue;
        }
        at += 1;
    }
}

const NGINX_KEYWORDS: &str = "reak return rewrite set accept_mutex accept_mutex_delay access_log add_after_body add_before_body add_header \
addition_types aio alias allow ancient_browser ancient_browser_value auth_basic auth_basic_user_file auth_http \
auth_http_header auth_http_timeout autoindex autoindex_exact_size autoindex_localtime charset charset_types \
client_body_buffer_size client_body_in_file_only client_body_in_single_buffer client_body_temp_path \
client_body_timeout client_header_buffer_size client_header_timeout client_max_body_size connection_pool_size \
create_full_put_path daemon dav_access dav_methods debug_connection debug_points default_type degradation \
degrade deny devpoll_changes devpoll_events directio directio_alignment empty_gif env epoll_events error_log \
eventport_events expires fastcgi_bind fastcgi_buffer_size fastcgi_buffers fastcgi_busy_buffers_size \
fastcgi_cache fastcgi_cache_key fastcgi_cache_methods fastcgi_cache_min_uses fastcgi_cache_path \
fastcgi_cache_use_stale fastcgi_cache_valid fastcgi_catch_stderr fastcgi_connect_timeout fastcgi_hide_header \
fastcgi_ignore_client_abort fastcgi_ignore_headers fastcgi_index fastcgi_intercept_errors \
fastcgi_max_temp_file_size fastcgi_next_upstream fastcgi_param fastcgi_pass_header fastcgi_pass_request_body \
fastcgi_pass_request_headers fastcgi_read_timeout fastcgi_send_lowat fastcgi_send_timeout \
fastcgi_split_path_info fastcgi_store fastcgi_store_access fastcgi_temp_file_write_size fastcgi_temp_path \
fastcgi_upstream_fail_timeout fastcgi_upstream_max_fails flv geoip_city geoip_country \
google_perftools_profiles gzip gzip_buffers gzip_comp_level gzip_disable gzip_hash gzip_http_version \
gzip_min_length gzip_no_buffer gzip_proxied gzip_static gzip_types gzip_vary gzip_window if_modified_since \
ignore_invalid_headers image_filter image_filter_buffer image_filter_jpeg_quality image_filter_transparency \
imap_auth imap_capabilities imap_client_buffer index ip_hash keepalive_requests keepalive_timeout \
kqueue_changes kqueue_events large_client_header_buffers limit_conn limit_conn_log_level limit_rate \
limit_rate_after limit_req limit_req_log_level limit_req_zone limit_zone lingering_time lingering_timeout \
lock_file log_format log_not_found log_subrequest map_hash_bucket_size map_hash_max_size master_process \
memcached_bind memcached_buffer_size memcached_connect_timeout memcached_next_upstream memcached_read_timeout \
memcached_send_timeout memcached_upstream_fail_timeout memcached_upstream_max_fails merge_slashes \
min_delete_depth modern_browser modern_browser_value msie_padding msie_refresh multi_accept open_file_cache \
open_file_cache_errors open_file_cache_events open_file_cache_min_uses open_file_cache_valid \
open_log_file_cache output_buffers override_charset perl perl_modules perl_require perl_set pid pop3_auth \
pop3_capabilities port_in_redirect postpone_gzipping postpone_output protocol proxy proxy_bind proxy_buffer \
proxy_buffer_size proxy_buffering proxy_buffers proxy_busy_buffers_size proxy_cache proxy_cache_key \
proxy_cache_methods proxy_cache_min_uses proxy_cache_path proxy_cache_use_stale proxy_cache_valid \
proxy_connect_timeout proxy_headers_hash_bucket_size proxy_headers_hash_max_size proxy_hide_header \
proxy_ignore_client_abort proxy_ignore_headers proxy_intercept_errors proxy_max_temp_file_size proxy_method \
proxy_next_upstream proxy_pass_error_message proxy_pass_header proxy_pass_request_body \
proxy_pass_request_headers proxy_read_timeout proxy_redirect proxy_send_lowat proxy_send_timeout \
proxy_set_body proxy_set_header proxy_ssl_session_reuse proxy_store proxy_store_access \
proxy_temp_file_write_size proxy_temp_path proxy_timeout proxy_upstream_fail_timeout proxy_upstream_max_fails \
random_index read_ahead real_ip_header recursive_error_pages request_pool_size reset_timedout_connection \
resolver resolver_timeout rewrite_log rtsig_overflow_events rtsig_overflow_test rtsig_overflow_threshold \
rtsig_signo satisfy secure_link_secret send_lowat send_timeout sendfile sendfile_max_chunk \
server_name_in_redirect server_names_hash_bucket_size server_names_hash_max_size server_tokens \
set_real_ip_from smtp_auth smtp_capabilities smtp_client_buffer smtp_greeting_delay so_keepalive \
source_charset ssi ssi_ignore_recycled_buffers ssi_min_file_chunk ssi_silent_errors ssi_types ssi_value_length \
ssl ssl_certificate ssl_certificate_key ssl_ciphers ssl_client_certificate ssl_crl ssl_dhparam ssl_engine \
ssl_prefer_server_ciphers ssl_protocols ssl_session_cache ssl_session_timeout ssl_verify_client \
ssl_verify_depth starttls stub_status sub_filter sub_filter_once sub_filter_types tcp_nodelay tcp_nopush \
thread_stack_size timeout timer_resolution types_hash_bucket_size types_hash_max_size underscores_in_headers \
uninitialized_variable_warn use user userid userid_domain userid_expires userid_mark userid_name userid_p3p \
userid_path userid_service valid_referers variables_hash_bucket_size variables_hash_max_size \
worker_connections worker_cpu_affinity worker_priority worker_processes worker_rlimit_core \
worker_rlimit_nofile worker_rlimit_sigpending worker_threads working_directory xclient xml_entities \
xslt_stylesheet xslt_types";
const NGINX_BLOCKS: &[&str] =
    &["http", "mail", "events", "server", "types", "location", "upstream", "charset_map", "limit_except", "if", "geo", "map"];
const NGINX_IMPORTANT: &[&str] =
    &["include", "root", "server", "server_name", "listen", "internal", "proxy_pass", "memcached_pass", "fastcgi_pass", "try_files"];

/// nginx, as CodeMirror 5's nginx mode colors it, token by token as it reads: a directive a
/// keyword (or `string-2`, the important ones), a block's name `variable-2`, and the rest of a
/// directive's line a number, a token running from its first word to the next word (`80
/// default_server`), so a name before `.`, `:`, or `;` is plain.
pub fn nginx(text: &str, state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if matches!(byte(bytes, at), b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        let start = at;
        // What the token is (the mode's `type`) and its style.
        let mut kind: &[u8] = b"";
        let mut style: Option<Kind>;
        let mut variable = false;
        if let Some(quote) = state.nginx_string {
            at = nginx_string_end(bytes, at, quote, state);
            kind = b"string";
            style = Some(Kind::String);
        } else if state.comment.is_some() {
            let close = state.comment.unwrap_or("*/");
            match text.get(at..).and_then(|rest| rest.find(close)) {
                Some(found) => {
                    at += found + close.len();
                    state.comment = None;
                }
                None => at = bytes.len(),
            }
            kind = b"comment";
            style = Some(Kind::Comment);
        } else {
            while scan::word(byte(bytes, at)) || byte(bytes, at) == b'$' {
                at += 1;
            }
            let word = text.get(start..at).unwrap_or("");
            if NGINX_KEYWORDS.split_ascii_whitespace().any(|known| known == word) {
                style = Some(Kind::Keyword);
            } else if NGINX_BLOCKS.contains(&word) {
                style = Some(Kind::Variable);
            } else if NGINX_IMPORTANT.contains(&word) {
                style = Some(Kind::String);
            } else {
                let here = byte(bytes, at);
                at = (at + 1).min(bytes.len());
                style = None;
                match here {
                    b'@' => {
                        while scan::word(byte(bytes, at)) || matches!(byte(bytes, at), b'\\' | b'-') {
                            at += 1;
                        }
                        style = Some(Kind::Comment);
                        kind = b"@media";
                    }
                    b'/' if byte(bytes, at) == b'*' => {
                        state.comment = Some("*/");
                        match text.get(at + 1..).and_then(|rest| rest.find("*/")) {
                            Some(found) => {
                                at += 1 + found + 2;
                                state.comment = None;
                            }
                            None => at = bytes.len(),
                        }
                        kind = b"comment";
                        style = Some(Kind::Comment);
                    }
                    b'=' => kind = b"compare",
                    b'~' | b'|' if byte(bytes, at) == b'=' => {
                        at += 1;
                        kind = b"compare";
                    }
                    b'"' | b'\'' => {
                        at = nginx_string_end(bytes, at, here, state);
                        kind = b"string";
                        style = Some(Kind::String);
                    }
                    b'#' => {
                        at = bytes.len();
                        kind = b"comment";
                        style = Some(Kind::Comment);
                    }
                    b'!' => {
                        while matches!(byte(bytes, at), b' ' | b'\t') {
                            at += 1;
                        }
                        while scan::word(byte(bytes, at)) {
                            at += 1;
                        }
                        kind = b"important";
                        style = Some(Kind::Keyword);
                    }
                    b'0'..=b'9' => {
                        while scan::word(byte(bytes, at)) || matches!(byte(bytes, at), b'.' | b'%') {
                            at += 1;
                        }
                        kind = b"unit";
                        style = Some(Kind::Number);
                    }
                    b',' | b'.' | b'+' | b'>' | b'*' | b'/' => kind = b"select-op",
                    b';' => kind = b";",
                    b'{' => kind = b"{",
                    b'}' => kind = b"}",
                    b':' | b'[' | b']' => kind = b":",
                    _ => {
                        while scan::word(byte(bytes, at)) || matches!(byte(bytes, at), b'\\' | b'-') {
                            at += 1;
                        }
                        variable = true;
                    }
                }
            }
        }
        let current = state.nginx.last().copied();
        if variable {
            style = match current {
                Some(b'r') => Some(Kind::Number),
                None | Some(b'M') => Some(Kind::Operator),
                _ => None,
            };
            kind = b"variable";
        }
        out.mark(start..at, style);
        if current == Some(b'r') && matches!(kind, b"{" | b"}" | b";") {
            state.nginx.pop();
        }
        match kind {
            b"{" if current == Some(b'm') => {
                if let Some(last) = state.nginx.last_mut() {
                    *last = b'M';
                }
            }
            b"{" => state.nginx.push(b'{'),
            b"}" => {
                state.nginx.pop();
            }
            b"@media" => state.nginx.push(b'm'),
            _ if current == Some(b'{') && kind != b"comment" => state.nginx.push(b'r'),
            _ => {}
        }
    }
}

/// The end of an nginx string read from `at` (after its quote), or the line's end (still open).
fn nginx_string_end(bytes: &[u8], at: usize, quote: u8, state: &mut More) -> usize {
    let mut index = at;
    let mut escaped = false;
    while let Some(here) = bytes.get(index) {
        index += 1;
        if *here == quote && !escaped {
            state.nginx_string = None;
            return index;
        }
        escaped = !escaped && *here == b'\\';
    }
    // The string ends with the line unless the line ends in a backslash.
    state.nginx_string = if escaped { Some(quote) } else { None };
    bytes.len()
}

const DOCKER_INSTRUCTIONS: &[&str] = &[
    "from",
    "expose",
    "run",
    "cmd",
    "entrypoint",
    "shell",
    "arg",
    "maintainer",
    "label",
    "env",
    "add",
    "copy",
    "volume",
    "user",
    "workdir",
    "onbuild",
    "stopsignal",
    "healthcheck",
];

/// Where a Dockerfile's reader is: a line's start, after `FROM`, in a JSON array, in `EXPOSE`'s
/// ports, in an instruction's arguments, or in a quoted string inside them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Docker {
    #[default]
    Start,
    From,
    Array,
    Expose,
    Arguments,
    Quoted(u8),
}

/// The instruction at the line's start (after spaces) and where it ends, in any case.
fn docker_instruction(bytes: &[u8]) -> Option<(usize, usize, &'static str)> {
    let start = bytes.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let end = (start..=bytes.len()).find(|index| !bytes.get(*index).is_some_and(|byte| scan::word(*byte))).unwrap_or(bytes.len());
    let word = std::str::from_utf8(bytes.get(start..end)?).ok()?.to_ascii_lowercase();
    DOCKER_INSTRUCTIONS.iter().find(|known| **known == word).map(|known| (start, end, *known))
}

/// Dockerfile, as CodeMirror 5's dockerfile mode colors it: an instruction a keyword (`AS` too),
/// `EXPOSE`'s ports numbers, strings in a JSON array or the arguments, comments.
pub fn dockerfile(text: &str, state: &mut More, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut stream = Stream { bytes, start: 0, pos: 0 };
    let blank = |byte: u8| byte == b' ' || byte == b'\t' || byte == b'\r';
    while !stream.eol() {
        stream.start = stream.pos;
        let style = match state.docker {
            Docker::Start | Docker::Arguments if stream.sol() && text.trim_start().starts_with('#') => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            Docker::Start if stream.sol() && docker_instruction(bytes).is_some() => {
                let Some((start, end, word)) = docker_instruction(bytes) else { break };
                let rest = bytes.get(end..).unwrap_or_default();
                let after = rest.iter().position(|byte| !blank(*byte)).unwrap_or(rest.len());
                let mut matched = true;
                if word == "from" {
                    stream.pos = end;
                    state.docker = Docker::From;
                } else if after == rest.len() || rest.get(after) == Some(&b'#') {
                    // The instruction alone: a comment after it is the mode's error.
                    out.push(end + after..bytes.len(), Kind::Operator);
                    stream.pos = bytes.len();
                } else if matches!(word, "run" | "cmd" | "entrypoint" | "shell") && rest.get(after) == Some(&b'[') && after > 0 {
                    stream.pos = end + after + 1;
                    state.docker = Docker::Array;
                } else if after > 0 {
                    stream.pos = end + after;
                    state.docker = if word == "expose" { Docker::Expose } else { Docker::Arguments };
                } else {
                    matched = false;
                    stream.pos += 1;
                }
                if matched {
                    out.push(start..end, Kind::Keyword);
                }
                None
            }
            Docker::Start => {
                stream.pos += 1;
                None
            }
            Docker::From => {
                state.docker = Docker::Start;
                let rest = bytes.get(stream.pos..).unwrap_or_default();
                let lead = rest.iter().position(|byte| !blank(*byte)).unwrap_or(rest.len());
                if lead == rest.len() {
                    stream.skip_to_end();
                    None
                } else if rest.get(lead) == Some(&b'#') {
                    out.push(stream.pos + lead..bytes.len(), Kind::Operator);
                    stream.skip_to_end();
                    None
                } else {
                    // `(\s*\S+\s+)(as)`: the image, then `AS`.
                    let image =
                        (lead..rest.len()).find(|index| rest.get(*index).is_some_and(|byte| byte.is_ascii_whitespace())).unwrap_or(rest.len());
                    let gap =
                        (image..rest.len()).find(|index| !rest.get(*index).is_some_and(|byte| byte.is_ascii_whitespace())).unwrap_or(rest.len());
                    if image > lead && gap > image && rest.get(gap..gap + 2).is_some_and(|word| word.eq_ignore_ascii_case(b"as")) {
                        out.push(stream.pos + gap..stream.pos + gap + 2, Kind::Keyword);
                        stream.pos += gap + 2;
                    }
                    None
                }
            }
            Docker::Array => {
                if stream.take("]") {
                    state.docker = Docker::Start;
                    None
                } else if stream.peek() == b'"' {
                    stream.pos = docker_string(bytes, stream.pos + 1, b'"').0;
                    Some(Kind::String)
                } else {
                    stream.pos += 1;
                    None
                }
            }
            Docker::Expose => {
                let digit = stream.peek().is_ascii_digit();
                stream.eat_while(|byte| byte.is_ascii_digit() == digit);
                if stream.eol() {
                    state.docker = Docker::Start;
                }
                digit.then_some(Kind::Number)
            }
            Docker::Arguments => {
                let here = stream.peek();
                if here == b'"' || here == b'\'' {
                    let (end, _) = docker_string(bytes, stream.pos + 1, here);
                    if end == bytes.len() {
                        // To the line's end, closed or not.
                        stream.pos = end;
                        state.docker = Docker::Start;
                    } else {
                        stream.pos += 1;
                        state.docker = Docker::Quoted(here);
                    }
                    Some(Kind::String)
                } else if here == b'#' {
                    state.docker = Docker::Start;
                    None
                } else {
                    stream.eat_while(|byte| !b"#\"'".contains(&byte));
                    if stream.eol() && !matches!(bytes.last(), Some(b'\\' | b'`')) {
                        state.docker = Docker::Start;
                    }
                    None
                }
            }
            Docker::Quoted(quote) => {
                let here = stream.next().unwrap_or(0);
                if here == b'\\' {
                    stream.next();
                } else if here == quote {
                    state.docker = Docker::Arguments;
                }
                Some(Kind::String)
            }
        };
        if stream.pos <= stream.start && !matches!(state.docker, Docker::Start | Docker::Arguments) {
            continue;
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// A quoted string's end from `at` (after its quote), and whether it closed before the line's end.
fn docker_string(bytes: &[u8], at: usize, quote: u8) -> (usize, bool) {
    let mut index = at;
    while let Some(here) = bytes.get(index).copied() {
        if here == b'\\' {
            index += 2;
        } else if here == quote {
            return (index + 1, true);
        } else {
            index += 1;
        }
    }
    (bytes.len(), false)
}

const OCTAVE_BUILTINS: &str = "error eval function abs acos atan asin cos cosh exp log prod sum log10 max min sign sin sinh sqrt tan \
reshape break zeros default margin round ones rand syn ceil floor size clear eye mean std cov det eig inv norm rank trace expm logm sqrtm \
linspace plot title xlabel ylabel legend text grid meshgrid mesh num2str fft ifft arrayfun cellfun input fliplr flipud ismember";
const OCTAVE_KEYWORDS: &str = "return case switch else elseif end endif endfunction if otherwise do for while try catch classdef properties \
events methods global persistent endfor endwhile printf sprintf disp until continue pkg";

/// Octave (MATLAB), as CodeMirror 5's octave mode colors it: keywords, builtin functions,
/// variables, numbers (a sign with them), strings, a `'` after a value a transpose operator,
/// `%` and `#` comments (`%{ … %}` blocks).
pub fn octave(text: &str, state: &mut More, out: &mut Out) {
    scan::run(text, out, |stream| {
        if state.octave_comment {
            if let Some(found) = text.get(stream.pos..).and_then(|rest| rest.find("%}")) {
                stream.pos += found + 2;
                state.octave_comment = false;
            } else {
                stream.skip_to_end();
            }
            return Some(Kind::Comment);
        }
        // After a value, a `'` (not at the line's start) transposes it.
        let transposable = std::mem::take(&mut state.octave_value);
        if transposable && !stream.sol() && stream.peek() == b'\'' {
            stream.pos += 1;
            return Some(Kind::Operator);
        }
        let style = octave_token(stream, state);
        if matches!(style, Some(Kind::Number | Kind::Variable)) {
            state.octave_value = true;
        }
        style
    });
}

fn octave_token(stream: &mut Stream, state: &mut More) -> Option<Kind> {
    if stream.eat_space() {
        return None;
    }
    if stream.take("%{") {
        state.octave_comment = true;
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if matches!(stream.peek(), b'%' | b'#') {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    let bytes = stream.bytes;
    let start = stream.pos;
    if matches!(stream.peek(), b'0'..=b'9' | b'.' | b'+' | b'-') {
        let signed = if matches!(stream.peek(), b'+' | b'-') { start + 1 } else { start };
        let digits = |from: usize| (from..).find(|index| !bytes.get(*index).is_some_and(u8::is_ascii_digit)).unwrap_or(from);
        let imaginary = |end: usize| if matches!(bytes.get(end), Some(b'i' | b'j')) { end + 1 } else { end };
        let exponent = |from: usize| {
            let mut end = from;
            if matches!(bytes.get(end), Some(b'E' | b'e' | b'D' | b'd')) {
                end += 1;
                if matches!(bytes.get(end), Some(b'+' | b'-')) {
                    end += 1;
                }
                let after = digits(end);
                if after > end {
                    return after;
                }
            }
            from
        };
        let hex_digits = (signed + 2..).find(|index| !bytes.get(*index).is_some_and(u8::is_ascii_hexdigit)).unwrap_or(signed + 2);
        let whole = digits(signed);
        let end = if bytes.get(signed) == Some(&b'0') && bytes.get(signed + 1) == Some(&b'x') && hex_digits > signed + 2 {
            imaginary(hex_digits)
        } else if bytes.get(whole) == Some(&b'.') && digits(whole + 1) > whole + 1 {
            imaginary(exponent(digits(whole + 1)))
        } else if whole > signed {
            imaginary(exponent(whole))
        } else {
            start
        };
        if end > start {
            stream.pos = end;
            return Some(Kind::Number);
        }
    }
    let word_end = (start..).find(|index| !bytes.get(*index).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')).unwrap_or(start);
    let word = std::str::from_utf8(bytes.get(start..word_end).unwrap_or_default()).unwrap_or("");
    if matches!(word, "nan" | "NaN" | "inf" | "Inf") {
        stream.pos = word_end;
        return Some(Kind::Number);
    }
    let here = stream.peek();
    if here == b'"' || here == b'\'' {
        // To its quote (a doubled quote inside it) or the line's end.
        let mut index = start + 1;
        while index < bytes.len() {
            if bytes.get(index) == Some(&here) {
                if bytes.get(index + 1) == Some(&here) {
                    index += 2;
                    continue;
                }
                index += 1;
                break;
            }
            index += 1;
        }
        stream.pos = index.min(bytes.len());
        return Some(Kind::String);
    }
    let has = |list: &str| !word.is_empty() && list.split_ascii_whitespace().any(|known| known == word);
    if has(OCTAVE_KEYWORDS) {
        stream.pos = word_end;
        return Some(Kind::Keyword);
    }
    if has(OCTAVE_BUILTINS) {
        stream.pos = word_end;
        return Some(Kind::Property);
    }
    if here.is_ascii_alphabetic() || here == b'_' || here >= 0x80 {
        stream.eat_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80);
        return Some(Kind::Variable);
    }
    if b"+-*/&|^~<>!@'\\".contains(&here) {
        stream.pos += 1;
        return Some(Kind::Operator);
    }
    for operator in ["==", "~=", "<=", ">=", "<<", ">>", ".+", ".-", ".*", "./", ".^", ".\\"] {
        if stream.take(operator) {
            return Some(Kind::Operator);
        }
    }
    stream.pos += 1;
    if b"([{},:=;.".contains(&here) {
        return None;
    }
    if here == b']' || here == b')' {
        // A closed bracket is a value too.
        state.octave_value = true;
        return None;
    }
    Some(Kind::Operator)
}

const VB_OPERATOR_WORDS: &str = "and andalso or orelse xor in not is isnot like";
const VB_KEYWORDS: &str = "class module sub enum select while if function get set property try structure synclock using with else elseif \
case catch finally next loop end do object boolean char string byte sbyte short ushort int16 uint16 integer uinteger int32 uint32 long \
ulong int64 uint64 decimal single double float date datetime intptr uintptr addhandler addressof alias as byref byval cbool cbyte cchar \
cdate cdbl cdec cint clng cobj compare const continue csbyte cshort csng cstr cuint culng cushort declare default delegate dim directcast \
each erase error event exit explicit false for friend gettype goto handles implements imports infer inherits interface isfalse istrue \
lib me mod mustinherit mustoverride my mybase myclass namespace narrowing new nothing notinheritable notoverridable of off on operator \
option optional out overloads overridable overrides paramarray partial private protected public raiseevent readonly redim removehandler \
resume return shadows shared static step stop strict then throw to true trycast typeof until when widening withevents writeonly";
const VB_DIRECTIVES: &[&str] = &["#const", "#else", "#elseif", "#end", "#if", "#region"];

/// VB.NET, as CodeMirror 5's vb mode colors it: keywords (types among them, any case), word
/// operators (`AndAlso`), numbers, strings, `'` comments; a name after `.` takes the dot.
pub fn vb(text: &str, state: &mut More, out: &mut Out) {
    scan::run(text, out, |stream| {
        let style = vb_token(stream, state);
        if stream.current() == "." {
            // A member: the dot and its name together, a variable (else the mode's error).
            return if vb_token(stream, state) == Some(Kind::Variable) { Some(Kind::Variable) } else { Some(Kind::Operator) };
        }
        style
    });
}

fn vb_token(stream: &mut Stream, state: &mut More) -> Option<Kind> {
    if let Some(quote) = state.vb_string {
        return vb_string(stream, state, quote);
    }
    if stream.eat_space() {
        return None;
    }
    if stream.peek() == b'\'' {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    let bytes = stream.bytes;
    let at = stream.pos;
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let digits = |from: usize| (from..).find(|index| !byte(*index).is_ascii_digit()).unwrap_or(from);
    let suffix = |end: usize, letter: u8| if byte(end).eq_ignore_ascii_case(&letter) { end + 1 } else { end };
    // Floats: `\d*\.\d+F?`, `\d+\.\d*F?`, `\.\d+F?` (then `J`); integers: `&H…`, `&O…`, `[1-9]\d*F?`
    // (then `J`), `0` (then `L`).
    let whole = digits(at);
    let float = if byte(whole) == b'.' && digits(whole + 1) > whole + 1 {
        Some(suffix(digits(whole + 1), b'f'))
    } else if whole > at && byte(whole) == b'.' {
        // `\d+\.\d*F?` takes an upper-case `F` only.
        Some(if byte(whole + 1) == b'F' { whole + 2 } else { whole + 1 })
    } else {
        None
    };
    if let Some(end) = float {
        stream.pos = suffix(end, b'j');
        return Some(Kind::Number);
    }
    let hex = (at + 2..).find(|index| !byte(*index).is_ascii_hexdigit()).unwrap_or(at + 2);
    let octal = (at + 2..).find(|index| !(b'0'..=b'7').contains(&byte(*index))).unwrap_or(at + 2);
    let integer = if byte(at) == b'&' && byte(at + 1).eq_ignore_ascii_case(&b'h') && hex > at + 2 {
        Some(hex)
    } else if byte(at) == b'&' && byte(at + 1).eq_ignore_ascii_case(&b'o') && octal > at + 2 {
        Some(octal)
    } else if (b'1'..=b'9').contains(&byte(at)) {
        Some(suffix(if byte(whole) == b'F' { whole + 1 } else { whole }, b'j'))
    } else if byte(at) == b'0' && !byte(at + 1).is_ascii_digit() && !byte(at + 1).eq_ignore_ascii_case(&b'x') {
        Some(at + 1)
    } else {
        None
    };
    if let Some(end) = integer {
        stream.pos = suffix(end, b'l');
        return Some(Kind::Number);
    }
    if stream.take("\"") {
        return vb_string(stream, state, b'"');
    }
    for operator in ["//=", ">>=", "<<=", "**=", "+=", "-=", "*=", "%=", "/=", "&=", "|=", "^="] {
        if stream.take(operator) {
            return None;
        }
    }
    for operator in ["==", "<>", "<=", ">=", "<<", ">>", "//", "**"] {
        if stream.take(operator) {
            return Some(Kind::Operator);
        }
    }
    let here = stream.peek();
    if b"+-*/%&\\|^~<>!".contains(&here) {
        stream.pos += 1;
        return Some(Kind::Operator);
    }
    let start = if here == b'#' { at + 1 } else { at };
    let end = (start..).find(|index| !(byte(*index).is_ascii_alphanumeric() || byte(*index) == b'_')).unwrap_or(start);
    let word = std::str::from_utf8(bytes.get(at..end).unwrap_or_default()).unwrap_or("").to_ascii_lowercase();
    if here != b'#' && end > at && VB_OPERATOR_WORDS.split_ascii_whitespace().any(|known| known == word) {
        stream.pos = end;
        return Some(Kind::Operator);
    }
    if b"()[]{}@,:`=;.".contains(&here) {
        stream.pos += 1;
        return None;
    }
    if (here != b'#' && end > at && VB_KEYWORDS.split_ascii_whitespace().any(|known| known == word)) || VB_DIRECTIVES.contains(&word.as_str()) {
        stream.pos = end;
        return Some(Kind::Keyword);
    }
    if here.is_ascii_alphabetic() || here == b'_' {
        stream.pos = end;
        return Some(Kind::Variable);
    }
    stream.next();
    Some(Kind::Operator)
}

/// A string to its closing quote (which ends it at the line's end too).
fn vb_string(stream: &mut Stream, state: &mut More, quote: u8) -> Option<Kind> {
    state.vb_string = None;
    while !stream.eol() {
        stream.eat_while(|byte| byte != b'\'' && byte != b'"');
        if stream.eat(|byte| byte == quote).is_some() {
            return Some(Kind::String);
        }
        stream.eat(|byte| byte == b'\'' || byte == b'"');
    }
    Some(Kind::String)
}

const FORTRAN_KEYWORDS: &str = "abstract accept allocatable allocate array assign asynchronous backspace bind block byte call case class \
close common contains continue cycle data deallocate decode deferred dimension do elemental else encode end endif entry enumerator \
equivalence exit external extrinsic final forall format function generic go goto if implicit import include inquire intent interface \
intrinsic module namelist non_intrinsic non_overridable none nopass nullify open optional options parameter pass pause pointer print \
private program protected public pure read recursive result return rewind save select sequence stop subroutine target then to type use \
value volatile where while write";
const FORTRAN_BUILTINS: &str = "abort abs access achar acos adjustl adjustr aimag aint alarm all allocated alog amax amin amod and anint \
any asin associated atan besj besjn besy besyn bit_size btest cabs ccos ceiling cexp char chdir chmod clog cmplx command_argument_count \
complex conjg cos cosh count cpu_time cshift csin csqrt ctime c_funloc c_loc c_associated c_null_ptr c_null_funptr c_f_pointer \
c_null_char c_alert c_backspace c_form_feed c_new_line c_carriage_return c_horizontal_tab c_vertical_tab dabs dacos dasin datan \
date_and_time dbesj dbesjn dbesy dbesyn dble dcos dcosh ddim derf derfc dexp digits dim dint dlog dmax dmin dmod dnint dot_product \
dprod dsign dsinh dsin dsqrt dtanh dtan dtime eoshift epsilon erf erfc etime exit exp exponent extends_type_of fdate fget fgetc float \
floor flush fnum fputc fput fraction fseek fstat ftell gerror getarg get_command get_command_argument get_environment_variable getcwd \
getenv getgid getlog getpid getuid gmtime hostnm huge iabs iachar iand iargc ibclr ibits ibset ichar idate idim idint idnint ieor \
ierrno ifix imag imagpart index int ior irand isatty ishft ishftc isign iso_c_binding is_iostat_end is_iostat_eor itime kill kind \
lbound len len_trim lge lgt link lle llt lnblnk loc log logical long lshift lstat ltime matmul max maxexponent maxloc maxval mclock \
merge move_alloc min minexponent minloc minval mod modulo mvbits nearest new_line nint not or pack perror precision present product \
radix rand random_number random_seed range real realpart rename repeat reshape rrspacing rshift same_type_as scale scan second \
selected_int_kind selected_real_kind set_exponent shape short sign signal sinh sin sleep sngl spacing spread sqrt srand stat sum \
symlnk system system_clock tan tanh time tiny transfer transpose trim ttynam ubound umask unlink unpack verify xor zabs zcos zexp zlog \
zsin zsqrt c_bool c_char c_double c_double_complex c_float c_float_complex c_funptr c_int c_int16_t c_int32_t c_int64_t c_int8_t \
c_int_fast16_t c_int_fast32_t c_int_fast64_t c_int_fast8_t c_int_least16_t c_int_least32_t c_int_least64_t c_int_least8_t c_intmax_t \
c_intptr_t c_long c_long_double c_long_double_complex c_long_long c_ptr c_short c_signed_char c_size_t character double integer";

/// Fortran, as CodeMirror 5's fortran mode colors it: keywords and intrinsics (any case), `.and.`
/// operators (any character around the word, as the mode's pattern takes them), numbers,
/// strings, `!` comments.
pub fn fortran(text: &str, state: &mut More, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if let Some(quote) = state.fortran_string {
            return fortran_string(stream, state, quote);
        }
        let bytes = stream.bytes;
        let at = stream.pos;
        // `(.and.|.or.|…)`: a `.` matching any character.
        for word in ["and", "or", "eq", "lt", "le", "gt", "ge", "ne", "not", "eqv", "neqv"] {
            let end = at + word.len() + 2;
            if bytes.get(at + 1..end - 1).is_some_and(|middle| middle.eq_ignore_ascii_case(word.as_bytes())) && end <= bytes.len() {
                stream.pos = end;
                return Some(Kind::Operator);
            }
        }
        let here = stream.next()?;
        match here {
            b'!' => {
                stream.skip_to_end();
                Some(Kind::Comment)
            }
            b'"' | b'\'' => fortran_string(stream, state, here),
            b'[' | b']' | b'(' | b')' | b',' => None,
            b'0'..=b'9' => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'.');
                Some(Kind::Number)
            }
            b'+' | b'-' | b'*' | b'&' | b'=' | b'<' | b'>' | b'/' | b':' => {
                stream.eat_while(|byte| b"+-*&=<>/:".contains(&byte));
                Some(Kind::Operator)
            }
            _ => {
                stream.eat_while(|byte| scan::word(byte) || byte == b'$');
                let word = stream.current().to_ascii_lowercase();
                let has = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
                if has(FORTRAN_KEYWORDS) {
                    Some(Kind::Keyword)
                } else if has(FORTRAN_BUILTINS) || matches!(word.as_str(), "real" | "complex" | "logical") {
                    Some(Kind::Property)
                } else {
                    Some(Kind::Variable)
                }
            }
        }
    });
}

/// A string to its quote; one left open by a backslash at the line's end goes on.
fn fortran_string(stream: &mut Stream, state: &mut More, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    let mut closed = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            closed = true;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    state.fortran_string = (!closed && escaped).then_some(quote);
    Some(Kind::String)
}
