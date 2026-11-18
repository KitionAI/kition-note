//! The C family, as CodeMirror 5's clike mode colors it, per dialect: keywords, types (a type's
//! `*` too), builtins, atoms, names variables, a name after a definition keyword (or a type,
//! at the top, before `(`) a definition, preprocessor lines and annotations meta.

use super::Kind;
use super::scan::{self, Out};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    C,
    Cpp,
    Java,
    Kotlin,
    CSharp,
    Scala,
    Dart,
    ObjectiveC,
    ObjectiveCpp,
    Squirrel,
}

/// A dialect's words and habits, as the mode's configuration for its MIME type.
struct Config {
    keywords: &'static [&'static str],
    /// More keywords (a dialect that joins two lists).
    extra_keywords: &'static [&'static str],
    types: &'static [&'static str],
    builtins: &'static [&'static str],
    atoms: &'static [&'static str],
    def_keywords: &'static [&'static str],
    /// A name with a type before it, at the top, before `(`, is a definition.
    type_first: bool,
    /// `#` lines are the preprocessor's (meta).
    preprocessor: bool,
    /// `@name` is meta.
    annotations: bool,
    /// A type's `*` is a type too.
    pointers: bool,
    /// `_t` names are types.
    t_types: bool,
    suffixes: &'static [&'static str],
    underscores: bool,
}

const C_KEYWORDS: &[&str] = &[
    "auto", "if", "break", "case", "register", "continue", "return", "default", "do", "sizeof", "static", "else", "struct", "switch", "extern",
    "typedef", "union", "for", "goto", "while", "enum", "const", "volatile", "inline", "restrict", "asm", "fortran",
];
const SQUIRREL_KEYWORDS: &[&str] = &[
    "base",
    "break",
    "clone",
    "continue",
    "const",
    "default",
    "delete",
    "enum",
    "extends",
    "function",
    "in",
    "class",
    "foreach",
    "local",
    "resume",
    "return",
    "this",
    "throw",
    "typeof",
    "yield",
    "constructor",
    "instanceof",
    "static",
];
const C_TYPES: &[&str] = &["int", "long", "char", "short", "double", "float", "unsigned", "signed", "void", "bool"];
const CPP_KEYWORDS: &[&str] = &[
    "auto",
    "if",
    "break",
    "case",
    "register",
    "continue",
    "return",
    "default",
    "do",
    "sizeof",
    "static",
    "else",
    "struct",
    "switch",
    "extern",
    "typedef",
    "union",
    "for",
    "goto",
    "while",
    "enum",
    "const",
    "volatile",
    "inline",
    "restrict",
    "asm",
    "fortran",
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "audit",
    "axiom",
    "bitand",
    "bitor",
    "catch",
    "class",
    "compl",
    "concept",
    "constexpr",
    "const_cast",
    "decltype",
    "delete",
    "dynamic_cast",
    "explicit",
    "export",
    "final",
    "friend",
    "import",
    "module",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "operator",
    "or",
    "or_eq",
    "override",
    "private",
    "protected",
    "public",
    "reinterpret_cast",
    "requires",
    "static_assert",
    "static_cast",
    "template",
    "this",
    "thread_local",
    "throw",
    "try",
    "typeid",
    "typename",
    "using",
    "virtual",
    "xor",
    "xor_eq",
];
const CPP_TYPES: &[&str] =
    &["int", "long", "char", "short", "double", "float", "unsigned", "signed", "void", "bool", "char8_t", "char16_t", "char32_t", "wchar_t"];
const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "interface",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "volatile",
    "while",
    "@interface",
];
const JAVA_TYPES: &[&str] = &[
    "var",
    "byte",
    "short",
    "int",
    "long",
    "float",
    "double",
    "boolean",
    "char",
    "void",
    "Boolean",
    "Byte",
    "Character",
    "Double",
    "Float",
    "Integer",
    "Long",
    "Number",
    "Object",
    "Short",
    "String",
    "StringBuffer",
    "StringBuilder",
    "Void",
];
const KOTLIN_KEYWORDS: &[&str] = &[
    "package",
    "as",
    "typealias",
    "class",
    "interface",
    "this",
    "super",
    "val",
    "operator",
    "var",
    "fun",
    "for",
    "is",
    "in",
    "This",
    "throw",
    "return",
    "annotation",
    "break",
    "continue",
    "object",
    "if",
    "else",
    "while",
    "do",
    "try",
    "when",
    "file",
    "import",
    "where",
    "by",
    "get",
    "set",
    "abstract",
    "enum",
    "open",
    "inner",
    "override",
    "private",
    "public",
    "internal",
    "protected",
    "catch",
    "finally",
    "out",
    "final",
    "vararg",
    "reified",
    "dynamic",
    "companion",
    "constructor",
    "init",
    "sealed",
    "field",
    "property",
    "receiver",
    "param",
    "sparam",
    "lateinit",
    "data",
    "inline",
    "noinline",
    "tailrec",
    "external",
    "crossinline",
    "const",
    "infix",
    "suspend",
    "actual",
    "expect",
    "setparam",
    "value",
];
const KOTLIN_TYPES: &[&str] = &[
    "Boolean",
    "Byte",
    "Character",
    "CharSequence",
    "Class",
    "ClassLoader",
    "Cloneable",
    "Comparable",
    "Compiler",
    "Double",
    "Exception",
    "Float",
    "Integer",
    "Long",
    "Math",
    "Number",
    "Object",
    "Package",
    "Pair",
    "Process",
    "Runtime",
    "Runnable",
    "SecurityManager",
    "Short",
    "StackTraceElement",
    "StrictMath",
    "String",
    "StringBuffer",
    "System",
    "Thread",
    "ThreadGroup",
    "ThreadLocal",
    "Throwable",
    "Triple",
    "Void",
    "Annotation",
    "Any",
    "BooleanArray",
    "ByteArray",
    "Char",
    "CharArray",
    "DeprecationLevel",
    "DoubleArray",
    "Enum",
    "FloatArray",
    "Function",
    "Int",
    "IntArray",
    "Lazy",
    "LazyThreadSafetyMode",
    "LongArray",
    "Nothing",
    "ShortArray",
    "Unit",
];
const CSHARP_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "base",
    "break",
    "case",
    "catch",
    "checked",
    "class",
    "const",
    "continue",
    "default",
    "delegate",
    "do",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "finally",
    "fixed",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "init",
    "interface",
    "internal",
    "is",
    "lock",
    "namespace",
    "new",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "record",
    "ref",
    "required",
    "return",
    "sealed",
    "sizeof",
    "stackalloc",
    "static",
    "struct",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "unchecked",
    "unsafe",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
    "add",
    "alias",
    "ascending",
    "descending",
    "dynamic",
    "from",
    "get",
    "global",
    "group",
    "into",
    "join",
    "let",
    "orderby",
    "partial",
    "remove",
    "select",
    "set",
    "value",
    "var",
    "yield",
];
const CSHARP_TYPES: &[&str] = &[
    "Action",
    "Boolean",
    "Byte",
    "Char",
    "DateTime",
    "DateTimeOffset",
    "Decimal",
    "Double",
    "Func",
    "Guid",
    "Int16",
    "Int32",
    "Int64",
    "Object",
    "SByte",
    "Single",
    "String",
    "Task",
    "TimeSpan",
    "UInt16",
    "UInt32",
    "UInt64",
    "bool",
    "byte",
    "char",
    "decimal",
    "double",
    "short",
    "int",
    "long",
    "object",
    "sbyte",
    "float",
    "string",
    "ushort",
    "uint",
    "ulong",
];
const SCALA_KEYWORDS: &[&str] = &[
    "abstract",
    "case",
    "catch",
    "class",
    "def",
    "do",
    "else",
    "extends",
    "final",
    "finally",
    "for",
    "forSome",
    "if",
    "implicit",
    "import",
    "lazy",
    "match",
    "new",
    "null",
    "object",
    "override",
    "package",
    "private",
    "protected",
    "return",
    "sealed",
    "super",
    "this",
    "throw",
    "trait",
    "try",
    "type",
    "val",
    "var",
    "while",
    "with",
    "yield",
    "_",
    "assert",
    "assume",
    "require",
    "print",
    "println",
    "printf",
    "readLine",
    "readBoolean",
    "readByte",
    "readChar",
    "readDouble",
    "readFloat",
    "readInt",
    "readLong",
    "readShort",
];
const SCALA_TYPES: &[&str] = &[
    "AnyVal",
    "App",
    "Application",
    "Array",
    "BufferedIterator",
    "BigDecimal",
    "BigInt",
    "Char",
    "Console",
    "Either",
    "Enumeration",
    "Equiv",
    "Error",
    "Exception",
    "Fractional",
    "Function",
    "IndexedSeq",
    "Int",
    "Integral",
    "Iterable",
    "Iterator",
    "List",
    "Map",
    "Numeric",
    "Nil",
    "NotNull",
    "Option",
    "Ordered",
    "Ordering",
    "PartialFunction",
    "PartialOrdering",
    "Product",
    "Proxy",
    "Range",
    "Responder",
    "Seq",
    "Serializable",
    "Set",
    "Specializable",
    "Stream",
    "StringBuilder",
    "StringContext",
    "Symbol",
    "Throwable",
    "Traversable",
    "TraversableOnce",
    "Tuple",
    "Unit",
    "Vector",
    "Boolean",
    "Byte",
    "Character",
    "CharSequence",
    "Class",
    "ClassLoader",
    "Cloneable",
    "Comparable",
    "Compiler",
    "Double",
    "Float",
    "Integer",
    "Long",
    "Math",
    "Number",
    "Object",
    "Package",
    "Process",
    "Runtime",
    "Runnable",
    "SecurityManager",
    "Short",
    "StackTraceElement",
    "StrictMath",
    "String",
    "StringBuffer",
    "System",
    "Thread",
    "ThreadGroup",
    "ThreadLocal",
    "Void",
];
const DART_KEYWORDS: &[&str] = &[
    "this",
    "super",
    "static",
    "final",
    "const",
    "abstract",
    "class",
    "extends",
    "external",
    "factory",
    "implements",
    "mixin",
    "get",
    "native",
    "set",
    "typedef",
    "with",
    "enum",
    "throw",
    "rethrow",
    "assert",
    "break",
    "case",
    "continue",
    "default",
    "in",
    "return",
    "new",
    "deferred",
    "async",
    "await",
    "covariant",
    "try",
    "catch",
    "finally",
    "do",
    "else",
    "for",
    "if",
    "switch",
    "while",
    "import",
    "library",
    "export",
    "part",
    "of",
    "show",
    "hide",
    "is",
    "as",
    "extension",
    "on",
    "yield",
    "late",
    "required",
    "sealed",
    "base",
    "interface",
    "when",
    "inline",
];
const DART_TYPES: &[&str] = &["Never", "Null", "bool", "double", "dynamic", "int", "num", "Object", "String", "var", "void"];
const OBJC_KEYWORDS: &[&str] = &[
    "auto",
    "if",
    "break",
    "case",
    "register",
    "continue",
    "return",
    "default",
    "do",
    "sizeof",
    "static",
    "else",
    "struct",
    "switch",
    "extern",
    "typedef",
    "union",
    "for",
    "goto",
    "while",
    "enum",
    "const",
    "volatile",
    "inline",
    "restrict",
    "asm",
    "fortran",
    "bycopy",
    "byref",
    "in",
    "inout",
    "oneway",
    "out",
    "self",
    "super",
    "atomic",
    "nonatomic",
    "retain",
    "copy",
    "readwrite",
    "readonly",
    "strong",
    "weak",
    "assign",
    "typeof",
    "nullable",
    "nonnull",
    "null_resettable",
    "_cmd",
    "@interface",
    "@implementation",
    "@end",
    "@protocol",
    "@encode",
    "@property",
    "@synthesize",
    "@dynamic",
    "@class",
    "@public",
    "@package",
    "@private",
    "@protected",
    "@required",
    "@optional",
    "@try",
    "@catch",
    "@finally",
    "@import",
    "@selector",
    "@encode",
    "@defs",
    "@synchronized",
    "@autoreleasepool",
    "@compatibility_alias",
    "@available",
];
const OBJC_TYPES: &[&str] = &[
    "int",
    "long",
    "char",
    "short",
    "double",
    "float",
    "unsigned",
    "signed",
    "void",
    "bool",
    "id",
    "SEL",
    "instancetype",
    "Class",
    "Protocol",
    "BOOL",
];

fn config(dialect: Dialect) -> Config {
    let base = Config {
        keywords: C_KEYWORDS,
        extra_keywords: &[],
        types: C_TYPES,
        builtins: &[],
        atoms: &["NULL", "true", "false"],
        def_keywords: &["struct", "enum", "union"],
        type_first: true,
        preprocessor: true,
        annotations: false,
        pointers: true,
        t_types: true,
        suffixes: &["ll", "u", "l", "f"],
        underscores: false,
    };
    match dialect {
        Dialect::C => base,
        Dialect::Cpp => Config {
            keywords: CPP_KEYWORDS,
            types: CPP_TYPES,
            atoms: &["true", "false", "NULL", "nullptr"],
            def_keywords: &["class", "namespace", "struct", "enum", "union"],
            ..base
        },
        Dialect::ObjectiveC => {
            Config { keywords: OBJC_KEYWORDS, types: OBJC_TYPES, atoms: &["YES", "NO", "NULL", "Nil", "nil", "true", "false", "nullptr"], ..base }
        }
        Dialect::ObjectiveCpp => Config {
            keywords: OBJC_KEYWORDS,
            extra_keywords: CPP_KEYWORDS,
            types: OBJC_TYPES,
            atoms: &["YES", "NO", "NULL", "Nil", "nil", "true", "false", "nullptr"],
            def_keywords: &["struct", "enum", "union", "@interface", "@implementation", "@protocol", "@class", "class", "namespace"],
            ..base
        },
        Dialect::Squirrel => Config {
            keywords: SQUIRREL_KEYWORDS,
            atoms: &["true", "false", "null"],
            def_keywords: &["function", "local", "class"],
            pointers: false,
            ..base
        },
        Dialect::Java => Config {
            keywords: JAVA_KEYWORDS,
            types: JAVA_TYPES,
            atoms: &["true", "false", "null"],
            def_keywords: &["class", "interface", "enum", "@interface"],
            preprocessor: false,
            annotations: true,
            pointers: false,
            t_types: false,
            underscores: true,
            ..base
        },
        Dialect::Kotlin => Config {
            keywords: KOTLIN_KEYWORDS,
            types: KOTLIN_TYPES,
            atoms: &["true", "false", "null", "this"],
            def_keywords: &["class", "val", "var", "object", "interface", "fun"],
            type_first: false,
            preprocessor: false,
            annotations: true,
            pointers: false,
            t_types: false,
            underscores: true,
            ..base
        },
        Dialect::CSharp => Config {
            keywords: CSHARP_KEYWORDS,
            types: CSHARP_TYPES,
            atoms: &["true", "false", "null"],
            def_keywords: &["class", "interface", "namespace", "record", "struct", "var"],
            preprocessor: true,
            annotations: false,
            pointers: false,
            t_types: false,
            ..base
        },
        Dialect::Scala => Config {
            keywords: SCALA_KEYWORDS,
            types: SCALA_TYPES,
            atoms: &["true", "false", "null"],
            def_keywords: &["class", "enum", "def", "object", "package", "trait", "type", "val", "var"],
            type_first: false,
            preprocessor: false,
            annotations: true,
            pointers: false,
            t_types: false,
            ..base
        },
        Dialect::Dart => Config {
            keywords: DART_KEYWORDS,
            types: DART_TYPES,
            builtins: &["Function", "Type", "Iterable", "Map", "List", "Set", "Future", "Stream"],
            atoms: &["true", "false", "null"],
            def_keywords: &["class", "enum", "extension", "mixin"],
            type_first: false,
            preprocessor: false,
            annotations: true,
            pointers: false,
            t_types: false,
            ..base
        },
    }
}

/// What carries between lines: an open block comment or text block, a preprocessor line's `\`,
/// the braces open (a definition is only at the top), and the last token.
#[derive(Clone, Debug, Default)]
pub struct Clike {
    comment: bool,
    text_block: bool,
    preprocessor: bool,
    braces: usize,
    prev: Prev,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Prev {
    #[default]
    Other,
    Def,
    Type,
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn operator_char(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'&' | b'%' | b'=' | b'<' | b'>' | b'!' | b'?' | b'|' | b'/')
}

pub fn line(text: &str, clike: &mut Clike, dialect: Dialect, out: &mut Out) {
    let config = config(dialect);
    let bytes = text.as_bytes();
    let mut at = 0;
    if clike.preprocessor {
        out.push(0..text.len(), Kind::Comment);
        clike.preprocessor = text.trim_end().ends_with('\\');
        return;
    }
    if clike.comment {
        match text.find("*/") {
            Some(found) => {
                out.push(0..found + 2, Kind::Comment);
                clike.comment = false;
                at = found + 2;
            }
            None => {
                out.push(0..text.len(), Kind::Comment);
                return;
            }
        }
    }
    if clike.text_block {
        match text.find("\"\"\"") {
            Some(found) => {
                out.push(0..found + 3, Kind::String);
                clike.text_block = false;
                at = found + 3;
            }
            None => {
                out.push(0..text.len(), Kind::String);
                return;
            }
        }
    }
    if config.preprocessor && text.trim_start().starts_with('#') {
        out.push(0..text.len(), Kind::Comment);
        clike.preprocessor = text.trim_end().ends_with('\\');
        return;
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        let style: Option<Kind>;
        let mut is_type = false;
        let mut def_keyword = false;
        match here {
            b' ' | b'\t' | b'\r' => {
                at += 1;
                continue;
            }
            b'*' if config.pointers && clike.prev == Prev::Type => {
                at += 1;
                out.push(start..at, Kind::Property);
                continue;
            }
            b'@' if config.annotations && word_char(byte(bytes, at + 1)) && !scan::at(bytes, at, "@interface") => {
                at = scan::word_end(bytes, at + 1, b"$");
                out.push(start..at, Kind::Comment);
                clike.prev = Prev::Other;
                continue;
            }
            b'"' if dialect == Dialect::Java && scan::at(bytes, at, "\"\"\"") => {
                match text.get(at + 3..).and_then(|rest| rest.find("\"\"\"")) {
                    Some(found) => at += found + 6,
                    None => {
                        clike.text_block = true;
                        at = bytes.len();
                    }
                }
                style = Some(Kind::String);
            }
            b'"' | b'\'' => {
                at = scan::string_end(bytes, at, here).unwrap_or(bytes.len());
                style = Some(Kind::String);
            }
            b'0'..=b'9' | b'.' if let Some(end) = scan::c_number(bytes, at, config.underscores, config.suffixes) => {
                at = end;
                style = Some(Kind::Number);
            }
            b'[' | b']' | b'(' | b')' | b',' | b';' | b':' | b'.' => {
                at += 1;
                clike.prev = Prev::Other;
                continue;
            }
            b'{' => {
                clike.braces += 1;
                clike.prev = Prev::Other;
                at += 1;
                continue;
            }
            b'}' => {
                clike.braces = clike.braces.saturating_sub(1);
                clike.prev = Prev::Other;
                at += 1;
                continue;
            }
            b'/' if byte(bytes, at + 1) == b'/' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'/' if byte(bytes, at + 1) == b'*' => match text.get(at + 2..).and_then(|rest| rest.find("*/")) {
                Some(found) => {
                    out.push(at..at + found + 4, Kind::Comment);
                    at += found + 4;
                    continue;
                }
                None => {
                    out.push(at..bytes.len(), Kind::Comment);
                    clike.comment = true;
                    return;
                }
            },
            _ if operator_char(here) => {
                at += 1;
                while operator_char(byte(bytes, at)) && !(byte(bytes, at) == b'/' && matches!(byte(bytes, at + 1), b'/' | b'*')) {
                    at += 1;
                }
                style = Some(Kind::Operator);
            }
            _ if word_char(here) || here == b'@' => {
                at = scan::word_end(bytes, at + usize::from(here == b'@'), b"$");
                if matches!(dialect, Dialect::Cpp | Dialect::ObjectiveCpp) {
                    while scan::at(bytes, at, "::") && word_char(byte(bytes, at + 2)) {
                        at = scan::word_end(bytes, at + 2, b"$");
                    }
                }
                let word = text.get(start..at).unwrap_or("");
                if config.keywords.contains(&word) || config.extra_keywords.contains(&word) {
                    style = Some(Kind::Keyword);
                    def_keyword = config.def_keywords.contains(&word);
                } else if config.types.contains(&word) || (config.t_types && word.len() > 2 && word.ends_with("_t")) {
                    style = Some(Kind::Property);
                    is_type = true;
                } else if config.builtins.contains(&word) {
                    style = Some(Kind::Property);
                } else if config.atoms.contains(&word) {
                    style = None;
                } else {
                    let defined = clike.prev == Prev::Def
                        || (config.type_first && type_before(text, start, clike.prev) && clike.braces == 0 && scan::followed_by(bytes, at, b'('));
                    style = if defined { None } else { Some(Kind::Variable) };
                }
            }
            _ => {
                at += 1;
                clike.prev = Prev::Other;
                continue;
            }
        }
        out.mark(start..at, style);
        clike.prev = if def_keyword {
            Prev::Def
        } else if is_type {
            Prev::Type
        } else {
            Prev::Other
        };
    }
}

fn word_char(byte: u8) -> bool {
    scan::word(byte) || byte == b'$'
}

/// Whether a type comes before `at`: the last token was one, or the text before ends in a
/// type's shape (`Foo<Bar> `, `int* `, `T[] `).
fn type_before(text: &str, at: usize, prev: Prev) -> bool {
    if prev == Prev::Type {
        return true;
    }
    let before = text.get(..at).unwrap_or("").trim_end();
    let bytes = before.as_bytes();
    let last = bytes.last().copied().unwrap_or(0);
    let second = bytes.len().checked_sub(2).and_then(|index| bytes.get(index)).copied().unwrap_or(b' ');
    (last == b'>' && second != b'-' && second != b' ' && bytes.len() > 1) || last == b'*' || last == b']'
}
