//! PHP, Swift, Lua, PowerShell, and R, as their CodeMirror 5 modes color them (see each
//! function), with what each carries from one line to the next.

use super::Kind;
use super::scan::{self, Out};

/// What carries from one line to the next.
#[derive(Clone, Debug, Default)]
pub struct Scripting {
    /// A block comment open (`/* */`, `--[[ ]]`, `<# #>`), with its closing text.
    comment: Option<&'static str>,
    /// A string open: its closing text.
    string: Option<&'static str>,
    /// Swift: the string `\(…)` interpolations open (their parentheses' depth).
    interpolation: Vec<usize>,
    /// Swift: the last token was a defining keyword.
    define: bool,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

/// From `at` inside a block comment closed by `close`: to it, or the line's end.
fn comment_rest(text: &str, at: usize, close: &'static str, state: &mut Scripting, out: &mut Out) -> usize {
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

const PHP_KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "array",
    "as",
    "break",
    "callable",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "else",
    "elseif",
    "enddeclare",
    "endfor",
    "endforeach",
    "endif",
    "endswitch",
    "endwhile",
    "enum",
    "extends",
    "final",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "interface",
    "instanceof",
    "insteadof",
    "namespace",
    "new",
    "or",
    "private",
    "protected",
    "public",
    "static",
    "switch",
    "throw",
    "trait",
    "try",
    "use",
    "var",
    "while",
    "xor",
    "die",
    "echo",
    "empty",
    "exit",
    "eval",
    "include",
    "include_once",
    "isset",
    "list",
    "require",
    "require_once",
    "return",
    "print",
    "unset",
    "__halt_compiler",
    "self",
    "parent",
    "yield",
    "finally",
    "readonly",
    "match",
];
const PHP_ATOMS: &[&str] = &[
    "true",
    "false",
    "null",
    "TRUE",
    "FALSE",
    "NULL",
    "__CLASS__",
    "__DIR__",
    "__FILE__",
    "__LINE__",
    "__METHOD__",
    "__FUNCTION__",
    "__NAMESPACE__",
    "__TRAIT__",
];
const PHP_BUILTINS: &str = "strlen array_map count print_r var_dump var_export implode explode str_replace strpos stripos substr sprintf \
printf json_encode json_decode array_filter array_keys array_values array_merge in_array is_array array_key_exists file_get_contents \
file_put_contents date time mktime strtotime trim ltrim rtrim strtolower strtoupper ucfirst ucwords lcfirst preg_match preg_match_all \
preg_replace preg_split array_push array_pop array_shift array_unshift array_slice array_splice array_search array_reverse array_sum \
array_unique array_combine array_flip array_fill range sort rsort usort uasort uksort ksort krsort asort arsort intval floatval strval boolval \
settype gettype is_null is_string is_numeric is_int is_integer is_float is_bool is_object is_callable number_format round floor ceil abs max \
min rand mt_rand random_int md5 sha1 crc32 hash base64_encode base64_decode urlencode urldecode htmlspecialchars htmlentities strip_tags \
nl2br str_pad str_repeat str_split strrev strcmp strcasecmp str_contains str_starts_with str_ends_with mb_strlen mb_substr mb_strtolower \
mb_strtoupper uniqid microtime usleep sleep header setcookie session_start session_destroy define defined constant function_exists \
method_exists property_exists class_exists get_class get_parent_class call_user_func call_user_func_array func_get_args compact extract \
serialize unserialize fopen fclose fread fwrite fgets feof file is_file is_dir mkdir rmdir unlink rename copy dirname basename pathinfo \
realpath glob scandir error_log trigger_error set_error_handler error_reporting ini_set array_walk array_column array_chunk array_pad \
array_diff array_intersect iterator_to_array spl_autoload_register";

/// PHP, as CodeMirror 5's php mode colors it: keywords, `$variables`, its builtin functions,
/// the name after `class`, `function`, or `namespace` a definition, other names variables,
/// `<?` and `?>` operators, `"…$name…"` strings.
pub fn php(text: &str, state: &mut Scripting, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.comment {
        at = comment_rest(text, 0, close, state, out);
    }
    if let Some(close) = state.string {
        at = php_string(bytes, 0, 0, close.as_bytes().first().copied().unwrap_or(b'"'), state, out);
    }
    let mut define = false;
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'#' if byte(bytes, at + 1) != b'[' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'/' if byte(bytes, at + 1) == b'/' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'/' if byte(bytes, at + 1) == b'*' => at = comment_rest(text, at, "*/", state, out),
            b'"' | b'\'' => at = php_string(bytes, at, at + 1, here, state, out),
            b'$' if scan::word(byte(bytes, at + 1)) => {
                at = scan::word_end(bytes, at + 1, b"");
                out.push(start..at, Kind::Variable);
                define = false;
            }
            b'0'..=b'9' => {
                at = scan::c_number(bytes, at, true, &[]).unwrap_or(at + 1);
                out.push(start..at, Kind::Number);
            }
            b'+' | b'-' | b'*' | b'&' | b'%' | b'=' | b'<' | b'>' | b'!' | b'?' | b'|' | b'/' => {
                while matches!(byte(bytes, at), b'+' | b'-' | b'*' | b'&' | b'%' | b'=' | b'<' | b'>' | b'!' | b'?' | b'|' | b'/') {
                    at += 1;
                }
                out.push(start..at, Kind::Operator);
                define = false;
            }
            _ if here.is_ascii_alphabetic() || here == b'_' || here == b'\\' || here >= 0x80 => {
                at = scan::word_end(bytes, at, b"\\");
                let word = text.get(start..at).unwrap_or("");
                let kind = if define {
                    None
                } else if PHP_KEYWORDS.iter().any(|keyword| keyword.eq_ignore_ascii_case(word)) {
                    Some(Kind::Keyword)
                } else if PHP_ATOMS.contains(&word) {
                    None
                } else if PHP_BUILTINS.split_ascii_whitespace().any(|builtin| builtin == word) {
                    Some(Kind::Property)
                } else {
                    Some(Kind::Variable)
                };
                out.mark(start..at, kind);
                define = matches!(word, "class" | "function" | "namespace" | "interface" | "trait" | "enum");
            }
            _ => {
                at += 1;
                define = false;
            }
        }
    }
}

/// A PHP string's text from `start`, read from `from`: a double-quoted one's `$name` (and `{$…}`)
/// is a variable inside it; to its closing quote or the line's end (it stays open).
fn php_string(bytes: &[u8], start: usize, from: usize, quote: u8, state: &mut Scripting, out: &mut Out) -> usize {
    let mut piece = start;
    let mut index = from;
    state.string = None;
    while let Some(here) = bytes.get(index) {
        match *here {
            b'\\' => index += 2,
            close if close == quote => {
                out.push(piece..index + 1, Kind::String);
                return index + 1;
            }
            b'{' if quote == b'"' && byte(bytes, index + 1) == b'$' => {
                out.push(piece..index, Kind::String);
                let end = bytes.get(index..).and_then(|rest| rest.iter().position(|byte| *byte == b'}')).map_or(bytes.len(), |found| index + found);
                out.push(index + 1..end, Kind::Variable);
                index = (end + 1).min(bytes.len());
                piece = index;
            }
            b'$' if quote == b'"' && scan::word(byte(bytes, index + 1)) => {
                out.push(piece..index, Kind::String);
                let end = scan::word_end(bytes, index + 1, b"");
                out.push(index..end, Kind::Variable);
                index = end;
                piece = end;
            }
            _ => index += 1,
        }
    }
    out.push(piece..bytes.len(), Kind::String);
    state.string = Some(if quote == b'"' { "\"" } else { "'" });
    bytes.len()
}

const SWIFT_KEYWORDS: &[&str] = &[
    "_",
    "var",
    "let",
    "class",
    "enum",
    "extension",
    "import",
    "protocol",
    "struct",
    "func",
    "typealias",
    "associatedtype",
    "open",
    "public",
    "internal",
    "fileprivate",
    "private",
    "deinit",
    "init",
    "new",
    "override",
    "self",
    "subscript",
    "super",
    "convenience",
    "dynamic",
    "final",
    "indirect",
    "lazy",
    "required",
    "static",
    "unowned",
    "weak",
    "as",
    "is",
    "break",
    "case",
    "continue",
    "default",
    "else",
    "fallthrough",
    "for",
    "guard",
    "if",
    "in",
    "repeat",
    "switch",
    "where",
    "while",
    "defer",
    "return",
    "inout",
    "mutating",
    "nonmutating",
    "catch",
    "do",
    "rethrows",
    "throw",
    "throws",
    "try",
    "didSet",
    "get",
    "set",
    "willSet",
    "assignment",
    "associativity",
    "infix",
    "left",
    "none",
    "operator",
    "postfix",
    "precedence",
    "precedencegroup",
    "prefix",
    "right",
    "Any",
    "AnyObject",
    "Type",
    "dynamicType",
    "Self",
    "Protocol",
    "__COLUMN__",
    "__FILE__",
    "__FUNCTION__",
    "__LINE__",
];
const SWIFT_DEFINING: &[&str] =
    &["var", "let", "class", "enum", "extension", "import", "protocol", "struct", "func", "typealias", "associatedtype", "for"];
const SWIFT_ATOMS: &[&str] = &["true", "false", "nil", "self", "super", "_"];
const SWIFT_TYPES: &[&str] = &[
    "Array",
    "Bool",
    "Character",
    "Dictionary",
    "Double",
    "Float",
    "Int",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "Never",
    "Optional",
    "Set",
    "String",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "Void",
];

/// Swift, as CodeMirror 5's swift mode colors it: keywords, the name right after a defining
/// keyword a definition, its standard types and `@attributes` apart, `.member` a property,
/// punctuation grey, string `\(…)` code.
pub fn swift(text: &str, state: &mut Scripting, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.comment {
        at = comment_rest(text, 0, close, state, out);
    }
    if let Some(close) = state.string
        && state.interpolation.is_empty()
    {
        at = swift_string(bytes, 0, 0, close, state, out);
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        let next = byte(bytes, at + 1);
        if matches!(here, b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        let define = std::mem::take(&mut state.define);
        if here == b'/' && next == b'/' {
            out.push(at..bytes.len(), Kind::Comment);
            state.define = define;
            return;
        }
        if here == b'/' && next == b'*' {
            at = comment_rest(text, at, "*/", state, out);
            state.define = define;
            continue;
        }
        if here == b'#' && next.is_ascii_alphabetic() {
            at = scan::word_end(bytes, at + 1, b"");
            out.push(start..at, Kind::Property);
            continue;
        }
        if here == b'@' && (next.is_ascii_alphabetic() || next == b'_') {
            at = scan::word_end(bytes, at + 1, b"$");
            out.push(start..at, Kind::Property);
            continue;
        }
        let digit_at = if here == b'-' { at + 1 } else { at };
        if byte(bytes, digit_at).is_ascii_digit() {
            let end = scan::c_number(bytes, digit_at, true, &[]).unwrap_or(digit_at + 1);
            out.push(start..end, Kind::Number);
            at = end;
            continue;
        }
        if here == b'.' && (next.is_ascii_alphabetic() || next == b'_') {
            at = scan::word_end(bytes, at + 1, b"$");
            out.push(start..at, Kind::Property);
            continue;
        }
        // `$0`: a closure's argument, a variable.
        if here == b'$' && next.is_ascii_digit() {
            at += 1;
            while byte(bytes, at).is_ascii_digit() {
                at += 1;
            }
            out.push(start..at, Kind::Variable);
            continue;
        }
        if b"+-/*%=|&<>~^?!".contains(&here) {
            out.push(at..at + 1, Kind::Operator);
            at += 1;
            continue;
        }
        if b":;,.(){}[]".contains(&here) {
            at += 1;
            if scan::at(bytes, at, "..") {
                at += 2;
            }
            out.push(start..at, Kind::Punctuation);
            match here {
                b'(' => {
                    if let Some(depth) = state.interpolation.last_mut() {
                        *depth += 1;
                    }
                }
                b')' => {
                    if let Some(depth) = state.interpolation.last_mut() {
                        if *depth == 0 {
                            // The interpolation's `)`: the string goes on.
                            state.interpolation.pop();
                            let close = state.string.unwrap_or("\"");
                            at = swift_string(bytes, start, start + 1, close, state, out);
                            continue;
                        }
                        *depth -= 1;
                    }
                }
                _ => {}
            }
            continue;
        }
        if here == b'"' || here == b'\'' {
            let close: &'static str = if scan::at(bytes, at, "\"\"\"") {
                "\"\"\""
            } else if here == b'"' {
                "\""
            } else {
                "'"
            };
            at = swift_string(bytes, at, at + close.len(), close, state, out);
            continue;
        }
        if here.is_ascii_alphabetic() || here == b'_' || here == b'`' {
            at = scan::word_end(bytes, at + usize::from(here == b'`'), b"$");
            if byte(bytes, at) == b'`' {
                at += 1;
            }
            let word = text.get(start..at).unwrap_or("");
            let kind = if SWIFT_TYPES.contains(&word) {
                Some(Kind::Variable)
            } else if SWIFT_ATOMS.contains(&word) {
                None
            } else if SWIFT_KEYWORDS.contains(&word) {
                state.define = SWIFT_DEFINING.contains(&word);
                Some(Kind::Keyword)
            } else if define {
                None
            } else {
                Some(Kind::Variable)
            };
            out.mark(start..at, kind);
            continue;
        }
        at += 1;
    }
}

/// A Swift string's text from `start`, read from `from`: to `close`, or a `\(` (its code to read
/// next), or the line's end (a multi-line one stays open).
fn swift_string(bytes: &[u8], start: usize, from: usize, close: &'static str, state: &mut Scripting, out: &mut Out) -> usize {
    let mut index = from;
    state.string = None;
    while index < bytes.len() {
        if byte(bytes, index) == b'\\' {
            if byte(bytes, index + 1) == b'(' {
                out.push(start..index + 2, Kind::String);
                state.interpolation.push(0);
                state.string = Some(close);
                return index + 2;
            }
            index += 2;
            continue;
        }
        if scan::at(bytes, index, close) {
            out.push(start..index + close.len(), Kind::String);
            return index + close.len();
        }
        index += 1;
    }
    out.push(start..bytes.len(), Kind::String);
    if close.len() == 3 {
        state.string = Some(close);
    }
    bytes.len()
}

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "elseif", "false", "nil", "not", "or", "return", "true", "function", "end", "if", "then", "else", "do", "while", "repeat",
    "until", "for", "in", "local",
];
const LUA_BUILTINS: &str = "_G _VERSION assert collectgarbage dofile error getfenv getmetatable ipairs load loadfile loadstring module next \
pairs pcall print rawequal rawget rawset require select setfenv setmetatable tonumber tostring type unpack xpcall coroutine.create \
coroutine.resume coroutine.running coroutine.status coroutine.wrap coroutine.yield debug.debug debug.getfenv debug.gethook debug.getinfo \
debug.getlocal debug.getmetatable debug.getregistry debug.getupvalue debug.setfenv debug.sethook debug.setlocal debug.setmetatable \
debug.setupvalue debug.traceback close flush lines read seek setvbuf write io.close io.flush io.input io.lines io.open io.output io.popen \
io.read io.stderr io.stdin io.stdout io.tmpfile io.type io.write math.abs math.acos math.asin math.atan math.atan2 math.ceil math.cos \
math.cosh math.deg math.exp math.floor math.fmod math.frexp math.huge math.ldexp math.log math.log10 math.max math.min math.modf math.pi \
math.pow math.rad math.random math.randomseed math.sin math.sinh math.sqrt math.tan math.tanh os.clock os.date os.difftime os.execute \
os.exit os.getenv os.remove os.rename os.setlocale os.time os.tmpname package.cpath package.loaded package.loaders package.loadlib \
package.path package.preload package.seeall string.byte string.char string.dump string.find string.format string.gmatch string.gsub \
string.len string.lower string.match string.rep string.reverse string.sub string.upper table.concat table.insert table.maxn table.remove \
table.sort";

/// Lua, as CodeMirror 5's lua mode colors it: keywords, its builtins (dotted ones too), every
/// other name a variable (dots and dashes in it), numbers, strings and `[[…]]` long strings,
/// `--` and `--[[…]]` comments; operators and brackets plain.
pub fn lua(text: &str, state: &mut Scripting, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.comment {
        at = comment_rest(text, 0, close, state, out);
    }
    if let Some(close) = state.string {
        at = lua_long(text, 0, 0, close, state, out);
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        if here == b'-' && byte(bytes, at + 1) == b'-' {
            if let Some(close) = long_bracket(bytes, at + 2) {
                at = comment_rest(text, at, close, state, out);
                continue;
            }
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        if here == b'"' || here == b'\'' {
            at = scan::string_end(bytes, at, here).unwrap_or(bytes.len());
            out.push(start..at, Kind::String);
            continue;
        }
        if here == b'['
            && let Some(close) = long_bracket(bytes, at)
        {
            at = lua_long(text, at, at, close, state, out);
            continue;
        }
        if here.is_ascii_digit() {
            at += 1;
            while scan::word(byte(bytes, at)) || matches!(byte(bytes, at), b'.' | b'%') {
                at += 1;
            }
            out.push(start..at, Kind::Number);
            continue;
        }
        if scan::word(here) {
            at += 1;
            while scan::word(byte(bytes, at)) || matches!(byte(bytes, at), b'\\' | b'-' | b'.') {
                at += 1;
            }
            let word = text.get(start..at).unwrap_or("");
            let kind = if LUA_KEYWORDS.contains(&word) {
                Kind::Keyword
            } else if LUA_BUILTINS.split_ascii_whitespace().any(|builtin| builtin == word) {
                Kind::Property
            } else {
                Kind::Variable
            };
            out.push(start..at, kind);
            continue;
        }
        at += 1;
    }
}

/// The closing text of a long bracket (`[[`, `[==[`) at `at`, if one opens there.
fn long_bracket(bytes: &[u8], at: usize) -> Option<&'static str> {
    if byte(bytes, at) != b'[' {
        return None;
    }
    let level = bytes.get(at + 1..).map_or(0, |rest| rest.iter().take_while(|byte| **byte == b'=').count());
    (byte(bytes, at + 1 + level) == b'[').then_some(match level {
        0 => "]]",
        1 => "]=]",
        2 => "]==]",
        _ => "]===]",
    })
}

/// A long string's text from `start`, read from `from`: to `close`, or the line's end.
fn lua_long(text: &str, start: usize, from: usize, close: &'static str, state: &mut Scripting, out: &mut Out) -> usize {
    match text.get(from..).and_then(|rest| rest.find(close)).map(|found| from + found) {
        Some(found) => {
            out.push(start..found + close.len(), Kind::String);
            state.string = None;
            found + close.len()
        }
        None => {
            out.push(start..text.len(), Kind::String);
            state.string = Some(close);
            text.len()
        }
    }
}

const PS_KEYWORDS: &[&str] = &[
    "begin",
    "break",
    "catch",
    "continue",
    "data",
    "default",
    "do",
    "dynamicparam",
    "else",
    "elseif",
    "end",
    "exit",
    "filter",
    "finally",
    "for",
    "foreach",
    "from",
    "function",
    "if",
    "in",
    "param",
    "process",
    "return",
    "switch",
    "throw",
    "trap",
    "try",
    "until",
    "where",
    "while",
];
const PS_WORD_OPERATORS: &[&str] = &[
    "f",
    "not",
    "bnot",
    "split",
    "isplit",
    "csplit",
    "join",
    "is",
    "isnot",
    "as",
    "eq",
    "ne",
    "gt",
    "ge",
    "lt",
    "le",
    "ieq",
    "ine",
    "igt",
    "ige",
    "ilt",
    "ile",
    "ceq",
    "cne",
    "cgt",
    "cge",
    "clt",
    "cle",
    "like",
    "notlike",
    "match",
    "notmatch",
    "contains",
    "notcontains",
    "ilike",
    "inotlike",
    "imatch",
    "inotmatch",
    "icontains",
    "inotcontains",
    "clike",
    "cnotlike",
    "cmatch",
    "cnotmatch",
    "ccontains",
    "cnotcontains",
    "replace",
    "ireplace",
    "creplace",
    "and",
    "or",
    "xor",
    "band",
    "bor",
    "bxor",
];
const PS_VERBS: &str = "Add Clear Close Copy Enter Exit Find Format Get Hide Join Lock Move New Open Optimize Pop Push Redo Remove Rename \
Reset Resize Search Select Set Show Skip Split Step Switch Undo Unlock Watch Connect Disconnect Read Receive Send Write Backup Checkpoint \
Compare Compress Convert ConvertFrom ConvertTo Dismount Edit Expand Export Group Import Initialize Limit Merge Mount Out Publish Restore \
Save Sync Unpublish Update Approve Assert Complete Confirm Deny Disable Enable Install Invoke Register Request Restart Resume Start Stop \
Submit Suspend Uninstall Unregister Wait Debug Measure Ping Repair Resolve Test Trace Block Grant Protect Revoke Unblock Unprotect Use \
ForEach Sort Tee Where";
const PS_VARIABLES: &[&str] = &[
    "$",
    "?",
    "^",
    "_",
    "args",
    "true",
    "false",
    "null",
    "this",
    "home",
    "host",
    "input",
    "pid",
    "profile",
    "pwd",
    "matches",
    "error",
    "psitem",
    "pscmdlet",
    "psboundparameters",
    "psscriptroot",
    "pscommandpath",
    "psversiontable",
    "lastexitcode",
    "event",
    "sender",
    "ofs",
    "foreach",
];

/// PowerShell, as CodeMirror 5's powershell mode colors it: keywords (in any case), `$variables`
/// (its automatic ones builtins), cmdlets of its verbs builtins, `-eq`-style and symbol operators,
/// numbers with their size suffix, punctuation grey, other names plain, strings with `$names`.
pub fn powershell(text: &str, state: &mut Scripting, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.comment {
        at = comment_rest(text, 0, close, state, out);
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        let next = byte(bytes, at + 1);
        if matches!(here, b' ' | b'\t' | b'\r') {
            at += 1;
            continue;
        }
        if here == b'(' || here == b')' {
            out.push(at..at + 1, Kind::Punctuation);
            at += 1;
            continue;
        }
        let word_end = |from: usize| {
            let mut end = from;
            while byte(bytes, end).is_ascii_alphanumeric() || matches!(byte(bytes, end), b'_' | b'-') {
                end += 1;
            }
            end
        };
        let boundary = |end: usize| !(byte(bytes, end).is_ascii_alphanumeric() || matches!(byte(bytes, end), b'_' | b'-'));
        // A keyword, in any case, not part of a longer name.
        if here.is_ascii_alphabetic() {
            let end = word_end(at);
            let word = text.get(at..end).unwrap_or("");
            if PS_KEYWORDS.iter().any(|keyword| keyword.eq_ignore_ascii_case(word)) && boundary(end) {
                out.push(at..end, Kind::Keyword);
                at = end;
                continue;
            }
        }
        if let Some(end) = ps_number(bytes, at) {
            out.push(at..end, Kind::Number);
            at = end;
            continue;
        }
        if here == b'-' && next.is_ascii_alphabetic() {
            let mut end = at + 1;
            while byte(bytes, end).is_ascii_alphabetic() {
                end += 1;
            }
            let word = text.get(at + 1..end).unwrap_or("");
            if PS_WORD_OPERATORS.iter().any(|operator| operator.eq_ignore_ascii_case(word)) && !scan::word(byte(bytes, end)) {
                out.push(at..end, Kind::Operator);
                at = end;
                continue;
            }
        }
        if let Some(end) = ps_symbol_operator(bytes, at) {
            out.push(at..end, Kind::Operator);
            at = end;
            continue;
        }
        // A drive (`C:`), a cmdlet, an automatic variable: builtins.
        if here.is_ascii_alphabetic() && next == b':' {
            out.push(at..at + 2, Kind::Property);
            at += 2;
            continue;
        }
        if here.is_ascii_alphabetic() {
            let end = word_end(at);
            let word = text.get(at..end).unwrap_or("");
            if let Some((verb, noun)) = word.split_once('-')
                && !noun.is_empty()
                && PS_VERBS.split_ascii_whitespace().any(|known| known.eq_ignore_ascii_case(verb))
                && ps_known_cmdlet(word)
            {
                out.push(at..end, Kind::Property);
                at = end;
                continue;
            }
            // Any other name: the mode's identifier, in the text's color.
            at = end;
            continue;
        }
        if here == b'$' {
            let end = if matches!(next, b'$' | b'?' | b'^' | b'_') && !scan::word(byte(bytes, at + 2)) {
                at + 2
            } else {
                scan::word_end(bytes, at + 1, b"")
            };
            let name = text.get(at + 1..end).unwrap_or("").to_ascii_lowercase();
            let kind = if PS_VARIABLES.contains(&name.as_str()) { Kind::Property } else { Kind::Variable };
            out.push(at..end.max(at + 1), kind);
            at = end.max(at + 1);
            continue;
        }
        if matches!(here, b'[' | b']' | b'{' | b'}' | b',' | b';' | b'`' | b'\\' | b'.') {
            out.push(at..at + 1, Kind::Punctuation);
            at += 1;
            continue;
        }
        if here == b'@' && matches!(next, b'(' | b'{') {
            out.push(at..at + 2, Kind::Punctuation);
            at += 2;
            continue;
        }
        if here == b'\'' {
            at = scan::string_end(bytes, at, b'\'').unwrap_or(bytes.len());
            out.push(start..at, Kind::String);
            continue;
        }
        if here == b'"' {
            at = ps_string(text, at, state, out);
            continue;
        }
        if here == b'<' && next == b'#' {
            at = comment_rest(text, at, "#>", state, out);
            continue;
        }
        if here == b'#' {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        at += 1;
    }
}

/// The cmdlets the mode knows, by their verb and noun (the common ones).
fn ps_known_cmdlet(word: &str) -> bool {
    const NOUNS: &str = "Output Host Error Warning Verbose Debug Information Progress ChildItem Content Item ItemProperty Location Process \
Service Command Help Member Date Variable Alias Module Object Path Csv Json Xml Html Clixml String Table List Wide Custom File Null \
GridView Printer Default WebRequest RestMethod Expression History Credential Culture Random Unique Event Job PSDrive PSSession \
ComputerInfo NetIPAddress EventLog Acl Clipboard Sleep Computer WmiObject CimInstance Transcript TimeSpan Tee Hash FileHash";
    let Some((_, noun)) = word.split_once('-') else { return false };
    matches!(
        word.to_ascii_lowercase().as_str(),
        "foreach-object" | "where-object" | "sort-object" | "select-object" | "group-object" | "measure-object" | "tee-object" | "start-sleep"
    ) || NOUNS.split_ascii_whitespace().any(|known| known.eq_ignore_ascii_case(noun))
}

/// The end of a PowerShell number at `at`: hex, or digits with a fraction (a lone trailing `.`
/// too), an exponent, an `l`/`d`, and a `kb`…`pb` size.
fn ps_number(bytes: &[u8], at: usize) -> Option<usize> {
    let lower = |index: usize| byte(bytes, index).to_ascii_lowercase();
    let mut end;
    if lower(at) == b'0' && lower(at + 1) == b'x' && lower(at + 2).is_ascii_hexdigit() {
        end = at + 2;
        while lower(end).is_ascii_hexdigit() {
            end += 1;
        }
    } else if lower(at).is_ascii_digit() || (lower(at) == b'.' && lower(at + 1).is_ascii_digit()) {
        end = at;
        while lower(end).is_ascii_digit() {
            end += 1;
        }
        if lower(end) == b'.' {
            end += 1;
            while lower(end).is_ascii_digit() {
                end += 1;
            }
        }
        end = scan::exponent(bytes, end);
    } else {
        return None;
    }
    if matches!(lower(end), b'l' | b'd') {
        end += 1;
    }
    if matches!(lower(end), b'k' | b'm' | b'g' | b't' | b'p') && lower(end + 1) == b'b' {
        end += 2;
    }
    Some(end)
}

/// The end of a PowerShell symbol operator at `at`, if one is there.
fn ps_symbol_operator(bytes: &[u8], at: usize) -> Option<usize> {
    let here = byte(bytes, at);
    let next = byte(bytes, at + 1);
    if matches!(here, b'+' | b'-' | b'*' | b'/' | b'%') && next == b'=' {
        return Some(at + 2);
    }
    if (here == b'+' && next == b'+') || (here == b'-' && next == b'-') || (here == b'.' && next == b'.') {
        return Some(at + 2);
    }
    if matches!(here, b'+' | b'-' | b'*' | b'&' | b'^' | b'%' | b':' | b'=' | b'!' | b'|' | b'/' | b'>') || (here == b'<' && next != b'#') {
        return Some(at + 1);
    }
    None
}

/// A double-quoted PowerShell string at `at`: its `$names` variables inside it, its `$(…)` code.
fn ps_string(text: &str, at: usize, state: &mut Scripting, out: &mut Out) -> usize {
    let bytes = text.as_bytes();
    let mut piece = at;
    let mut index = at + 1;
    while let Some(here) = bytes.get(index) {
        match *here {
            b'`' => index += 2,
            b'$' if byte(bytes, index + 1) == b'(' => {
                out.push(piece..index, Kind::String);
                out.push(index..index + 2, Kind::Punctuation);
                let mut depth = 0usize;
                let mut close = index + 2;
                while let Some(next) = bytes.get(close) {
                    match next {
                        b'(' => depth += 1,
                        b')' if depth == 0 => break,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    close += 1;
                }
                let mut inner = Out::default();
                powershell(text.get(index + 2..close).unwrap_or(""), state, &mut inner);
                out.extend_shifted(inner, index + 2);
                if close < bytes.len() {
                    out.push(close..close + 1, Kind::Punctuation);
                }
                index = (close + 1).min(bytes.len());
                piece = index;
            }
            b'"' => {
                out.push(piece..index + 1, Kind::String);
                return index + 1;
            }
            b'$' if scan::word(byte(bytes, index + 1)) => {
                out.push(piece..index, Kind::String);
                let end = scan::word_end(bytes, index + 1, b"");
                out.push(index..end, Kind::Variable);
                index = end;
                piece = end;
            }
            _ => index += 1,
        }
    }
    out.push(piece..bytes.len(), Kind::String);
    bytes.len()
}

const R_ATOMS: &[&str] = &["NULL", "NA", "Inf", "NaN", "NA_integer_", "NA_real_", "NA_complex_", "NA_character_", "TRUE", "FALSE"];
const R_BUILTINS: &[&str] = &["list", "quote", "bquote", "eval", "return", "call", "parse", "deparse"];
const R_KEYWORDS: &[&str] = &["if", "else", "repeat", "while", "function", "for", "in", "next", "break"];

/// R, as CodeMirror 5's r mode colors it: keywords, a few builtins, atoms (`TRUE`, `NA`, …),
/// every other name (dots in it) a variable, `<-` and `$` and other operators, numbers (`3L`).
pub fn r(text: &str, state: &mut Scripting, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if let Some(close) = state.string {
        let quote = close.as_bytes().first().copied().unwrap_or(b'"');
        let mut index = 0;
        let mut found = None;
        while let Some(here) = bytes.get(index) {
            if *here == b'\\' {
                index += 2;
                continue;
            }
            if *here == quote {
                found = Some(index + 1);
                break;
            }
            index += 1;
        }
        match found {
            Some(end) => {
                out.push(0..end, Kind::String);
                state.string = None;
                at = end;
            }
            None => {
                out.push(0..bytes.len(), Kind::String);
                return;
            }
        }
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        let next = byte(bytes, at + 1);
        if here == b'#' {
            out.push(at..bytes.len(), Kind::Comment);
            return;
        }
        if here == b'0' && next == b'x' {
            at += 2;
            while byte(bytes, at).is_ascii_hexdigit() {
                at += 1;
            }
            out.push(start..at, Kind::Number);
            continue;
        }
        if here.is_ascii_digit() || (here == b'.' && next.is_ascii_digit()) {
            at += 1;
            while byte(bytes, at).is_ascii_digit() {
                at += 1;
            }
            if byte(bytes, at) == b'.' && byte(bytes, at + 1).is_ascii_digit() {
                at += 1;
                while byte(bytes, at).is_ascii_digit() {
                    at += 1;
                }
            }
            at = scan::exponent(bytes, at);
            if byte(bytes, at) == b'L' {
                at += 1;
            }
            out.push(start..at, Kind::Number);
            continue;
        }
        if here == b'"' || here == b'\'' {
            match scan::string_end(bytes, at, here) {
                Some(end) => {
                    out.push(start..end, Kind::String);
                    at = end;
                }
                None => {
                    out.push(start..bytes.len(), Kind::String);
                    state.string = Some(if here == b'"' { "\"" } else { "'" });
                    return;
                }
            }
            continue;
        }
        if here == b'`' {
            at = bytes.get(at + 1..).and_then(|rest| rest.iter().position(|byte| *byte == b'`')).map_or(bytes.len(), |found| at + found + 2);
            out.push(start..at, Kind::Variable);
            continue;
        }
        if here.is_ascii_alphabetic() || here == b'.' {
            at += 1;
            while scan::word(byte(bytes, at)) || byte(bytes, at) == b'.' {
                at += 1;
            }
            let word = text.get(start..at).unwrap_or("");
            let kind = if R_ATOMS.contains(&word) {
                None
            } else if R_KEYWORDS.contains(&word) {
                Some(Kind::Keyword)
            } else if R_BUILTINS.contains(&word) {
                Some(Kind::Property)
            } else {
                Some(Kind::Variable)
            };
            out.mark(start..at, kind);
            continue;
        }
        if here == b'%' {
            // `%>%`, `%in%`: the mode's operator variable-2, drawn as a variable.
            at = bytes.get(at + 1..).and_then(|rest| rest.iter().position(|byte| *byte == b'%')).map_or(bytes.len(), |found| at + found + 2);
            out.push(start..at, Kind::Variable);
            continue;
        }
        if (here == b'<' && next == b'-') || (here == b'-' && next == b'>') {
            at += 2;
            if here == b'<' && byte(bytes, at) == b'-' {
                at += 1;
            }
            if here == b'-' && byte(bytes, at) == b'>' {
                at += 1;
            }
            out.push(start..at, Kind::Operator);
            continue;
        }
        if b"+-*/^<>=!&|~$:".contains(&here) {
            at += 1;
            if here != b'$' {
                while b"+-*/^<>=!&|~$:".contains(&byte(bytes, at)) && byte(bytes, at) != 0 {
                    at += 1;
                }
            }
            out.push(start..at, Kind::Operator);
            continue;
        }
        at += 1;
    }
}
