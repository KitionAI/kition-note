//! The code highlighter for fenced code blocks, colored as Obsidian's live preview colors them:
//! its languages' CodeMirror 5 modes, ported per language (what each mode calls a keyword, a
//! definition, a variable, a property, a type, an operator), line by line, carrying what is open
//! (a block comment, a multi-line string, a bracket) from one line to the next. The fixture
//! `fixtures/code-tokens.json`, recorded from Obsidian, holds it to Obsidian's colors.

use std::ops::Range;

mod apl;
mod assorted;
mod clike;
mod coffee;
mod crystal;
mod css;
mod ebnf;
mod erlang;
mod formats;
mod forth;
mod gherkin;
mod haml;
mod haxe;
mod http;
mod idl;
mod js;
mod julia;
mod legacy;
mod lisp;
mod livescript;
mod markup;
mod mathematica;
mod misc;
mod ml;
mod more;
mod mscgen;
mod oz;
mod perl;
mod pug;
mod puppet;
mod python;
mod rdf;
mod regex;
mod rst;
mod ruby;
mod rust;
mod scan;
mod scripting;
mod shell;
mod simple;
mod simplemode;
mod slim;
mod small;
mod soy;
mod sqlmode;
mod stylus;
mod tcl;
mod templates;
pub mod tex;
mod toml;
mod ttcn;
mod varied;
mod vbscript;
mod verilog;
mod xml;
mod xquery;
mod yaml;
mod z80;

/// What a span is, by the color Obsidian gives it (a CodeMirror token class, or several that
/// share a color). A span of none of these (a definition, an atom, plain text) has the text's
/// own color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `keyword`.
    Keyword,
    /// `string`, `string-2`, `qualifier`.
    String,
    /// `number`.
    Number,
    /// `comment`, `meta`.
    Comment,
    /// `variable`, `variable-2`, `variable-3`.
    Variable,
    /// `operator`, `tag`, `error`.
    Operator,
    /// `property`, `builtin`, `type`, `attribute`.
    Property,
    /// `bracket`, `punctuation`.
    Punctuation,
    /// `link`.
    Link,
}

impl Kind {
    /// The kind's number in a style byte's token bits (1 to 9; 0 is none).
    pub fn code(self) -> u32 {
        match self {
            Kind::Keyword => 1,
            Kind::String => 2,
            Kind::Number => 3,
            Kind::Comment => 4,
            Kind::Variable => 5,
            Kind::Operator => 6,
            Kind::Property => 7,
            Kind::Punctuation => 8,
            Kind::Link => 9,
        }
    }

    /// The kind [`Kind::code`] gave `code`.
    pub fn of_code(code: u32) -> Option<Kind> {
        Some(match code {
            1 => Kind::Keyword,
            2 => Kind::String,
            3 => Kind::Number,
            4 => Kind::Comment,
            5 => Kind::Variable,
            6 => Kind::Operator,
            7 => Kind::Property,
            8 => Kind::Punctuation,
            9 => Kind::Link,
            _ => return None,
        })
    }
}

/// A language the highlighter knows, by its CodeMirror mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    TypeScript,
    Jsx,
    Tsx,
    Json,
    Python,
    Rust,
    Go,
    Clike(clike::Dialect),
    Shell,
    Css,
    Scss,
    Yaml,
    Toml,
    Properties,
    Diff,
    Html,
    Ruby,
    Php,
    Swift,
    Lua,
    PowerShell,
    R,
    Perl,
    Haskell,
    Groovy,
    Nginx,
    Cmake,
    Protobuf,
    Latex,
    Dockerfile,
    Clojure,
    Erlang,
    Julia,
    Octave,
    Vb,
    OCaml,
    FSharp,
    Scheme,
    CommonLisp,
    Fortran,
    CoffeeScript,
    Elm,
    Pascal,
    Smalltalk,
    Vhdl,
    Sparql,
    Cypher,
    D,
    Crystal,
    Verilog,
    Tcl,
    Gherkin,
    Http,
    Mathematica,
    VbScript,
    Puppet,
    Apl,
    Sieve,
    Forth,
    Factor,
    Brainfuck,
    Eiffel,
    Turtle,
    NTriples,
    Mscgen(mscgen::Dialect),
    Ebnf,
    Spreadsheet,
    Z80,
    Solr,
    Troff,
    RpmSpec,
    AsciiArmor,
    LiterateHaskell,
    Hxml,
    Yacas,
    Mumps,
    Pig,
    Fcl,
    Oz,
    Idl,
    WebIdl,
    Mirc,
    LiveScript,
    SqlDialect(sqlmode::Dialect),
    Stylus,
    Pug,
    Sml,
    MlLike,
    Haxe,
    Modelica,
    Ecl,
    Dylan,
    Nsis,
    Wast,
    XQuery,
    Cobol,
    Sas,
    Ttcn,
    TtcnCfg,
    Asterisk,
    Dtd,
    Velocity,
    TiddlyWiki,
    Tiki,
    Pegjs,
    Smarty,
    Haml,
    Soy,
    Slim,
    Rst,
}

/// The language for a fence's info string (its first word), if the highlighter knows it: the
/// names Obsidian highlights (measured; it does not take `py`, `rs`, `h`, `kt`, …).
pub fn grammar(info: &str) -> Option<Language> {
    let language = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    Some(match language.as_str() {
        "js" | "javascript" | "node" => Language::JavaScript,
        "jsx" => Language::Jsx,
        "ts" | "typescript" => Language::TypeScript,
        "tsx" => Language::Tsx,
        "json" | "json5" => Language::Json,
        "python" => Language::Python,
        "rust" => Language::Rust,
        "go" => Language::Go,
        "c" => Language::Clike(clike::Dialect::C),
        "cpp" | "c++" => Language::Clike(clike::Dialect::Cpp),
        "java" => Language::Clike(clike::Dialect::Java),
        "kotlin" => Language::Clike(clike::Dialect::Kotlin),
        "cs" | "csharp" | "c#" => Language::Clike(clike::Dialect::CSharp),
        "scala" => Language::Clike(clike::Dialect::Scala),
        "dart" => Language::Clike(clike::Dialect::Dart),
        "objc" | "objective-c" => Language::Clike(clike::Dialect::ObjectiveC),
        "sh" | "bash" | "zsh" | "shell" => Language::Shell,
        "sql" => Language::SqlDialect(sqlmode::Dialect::Standard),
        "css" => Language::Css,
        "scss" | "less" => Language::Scss,
        "yaml" | "yml" => Language::Yaml,
        "toml" => Language::Toml,
        "ini" | "properties" => Language::Properties,
        "diff" => Language::Diff,
        "html" | "xhtml" | "xml" | "vue" => Language::Html,
        "ruby" | "rb" => Language::Ruby,
        "php" => Language::Php,
        "swift" => Language::Swift,
        "lua" => Language::Lua,
        "powershell" => Language::PowerShell,
        "r" => Language::R,
        "perl" => Language::Perl,
        "haskell" => Language::Haskell,
        "groovy" => Language::Groovy,
        "nginx" => Language::Nginx,
        "cmake" => Language::Cmake,
        "protobuf" => Language::Protobuf,
        "latex" | "tex" | "stex" => Language::Latex,
        "dockerfile" => Language::Dockerfile,
        "clojure" | "clojurescript" | "edn" => Language::Clojure,
        "erlang" => Language::Erlang,
        "julia" | "jl" => Language::Julia,
        "octave" => Language::Octave,
        "vb" => Language::Vb,
        "ocaml" => Language::OCaml,
        "fsharp" => Language::FSharp,
        "scheme" => Language::Scheme,
        "lisp" | "commonlisp" => Language::CommonLisp,
        "fortran" => Language::Fortran,
        "coffeescript" | "coffee" | "coffee-script" => Language::CoffeeScript,
        "elm" => Language::Elm,
        "pascal" => Language::Pascal,
        "smalltalk" => Language::Smalltalk,
        "vhdl" => Language::Vhdl,
        "sparql" => Language::Sparql,
        "cypher" => Language::Cypher,
        "d" => Language::D,
        "crystal" => Language::Crystal,
        "verilog" | "systemverilog" => Language::Verilog,
        "tcl" => Language::Tcl,
        "gherkin" => Language::Gherkin,
        "http" => Language::Http,
        "mathematica" => Language::Mathematica,
        "vbscript" => Language::VbScript,
        "puppet" => Language::Puppet,
        "apl" => Language::Apl,
        "sieve" => Language::Sieve,
        "forth" => Language::Forth,
        "factor" => Language::Factor,
        "brainfuck" => Language::Brainfuck,
        "eiffel" => Language::Eiffel,
        "turtle" => Language::Turtle,
        "ntriples" => Language::NTriples,
        "mscgen" => Language::Mscgen(mscgen::Dialect::Mscgen),
        "xu" => Language::Mscgen(mscgen::Dialect::Xu),
        "msgenny" => Language::Mscgen(mscgen::Dialect::Msgenny),
        "ebnf" => Language::Ebnf,
        "spreadsheet" => Language::Spreadsheet,
        "z80" => Language::Z80,
        "solr" => Language::Solr,
        "troff" => Language::Troff,
        "rpm-spec" => Language::RpmSpec,
        "asciiarmor" | "pgp" => Language::AsciiArmor,
        "haskell-literate" => Language::LiterateHaskell,
        "hxml" => Language::Hxml,
        "yacas" => Language::Yacas,
        "mumps" => Language::Mumps,
        "pig" => Language::Pig,
        "fcl" => Language::Fcl,
        "oz" => Language::Oz,
        "idl" => Language::Idl,
        "webidl" => Language::WebIdl,
        "mirc" => Language::Mirc,
        "livescript" | "ls" => Language::LiveScript,
        "mysql" => Language::SqlDialect(sqlmode::Dialect::MySql),
        "sqlite" => Language::SqlDialect(sqlmode::Dialect::Sqlite),
        "cql" => Language::SqlDialect(sqlmode::Dialect::Cql),
        "plsql" => Language::SqlDialect(sqlmode::Dialect::PlSql),
        "postgresql" => Language::SqlDialect(sqlmode::Dialect::PostgreSql),
        "esper" => Language::SqlDialect(sqlmode::Dialect::Esper),
        "stylus" => Language::Stylus,
        "pug" | "jade" => Language::Pug,
        "sml" => Language::Sml,
        "mllike" => Language::MlLike,
        "squirrel" => Language::Clike(clike::Dialect::Squirrel),
        "objective-c++" | "objc++" => Language::Clike(clike::Dialect::ObjectiveCpp),
        "haxe" => Language::Haxe,
        "modelica" => Language::Modelica,
        "ecl" => Language::Ecl,
        "dylan" => Language::Dylan,
        "nsis" => Language::Nsis,
        "wast" => Language::Wast,
        "xquery" => Language::XQuery,
        "cobol" => Language::Cobol,
        "sas" => Language::Sas,
        "ttcn" => Language::Ttcn,
        "ttcn-cfg" => Language::TtcnCfg,
        "asterisk" => Language::Asterisk,
        "dtd" => Language::Dtd,
        "velocity" => Language::Velocity,
        "tiddlywiki" => Language::TiddlyWiki,
        "tiki" => Language::Tiki,
        "pegjs" => Language::Pegjs,
        "smarty" => Language::Smarty,
        "haml" => Language::Haml,
        "soy" => Language::Soy,
        "slim" => Language::Slim,
        "rst" => Language::Rst,
        _ => return None,
    })
}

/// What carries from one line to the next.
#[derive(Clone, Debug, Default)]
pub struct State {
    js: js::Js,
    python: python::Python,
    simple: simple::Simple,
    clike: clike::Clike,
    shell: shell::Shell,
    css: css::Css,
    yaml: yaml::Yaml,
    toml: toml::Toml,
    xml: xml::Xml,
    ruby: ruby::Ruby,
    scripting: scripting::Scripting,
    more: more::More,
    perl: perl::Perl,
    latex: tex::Latex,
    lisp: lisp::Lisp,
    erlang: erlang::Erlang,
    julia: julia::Julia,
    ml: ml::Ml,
    scheme: lisp::Scheme,
    common_lisp: lisp::CommonLisp,
    coffee: coffee::Coffee,
    misc: misc::Misc,
    crystal: crystal::Crystal,
    verilog: verilog::Verilog,
    tcl: tcl::Tcl,
    gherkin: gherkin::Gherkin,
    http: http::Http,
    mathematica: mathematica::Mathematica,
    puppet: puppet::Puppet,
    forth: forth::Forth,
    factor: forth::Factor,
    small: small::Small,
    turtle: rdf::Turtle,
    mscgen: mscgen::Mscgen,
    ebnf: ebnf::Ebnf,
    z80: z80::Z80,
    formats: formats::Formats,
    assorted: assorted::Assorted,
    oz: oz::Oz,
    webidl: idl::WebIdl,
    livescript: livescript::LiveScript,
    sqlmode: sqlmode::SqlMode,
    stylus: stylus::Stylus,
    pug: Box<pug::Pug>,
    haxe: haxe::Haxe,
    varied: varied::Varied,
    simplemode: simplemode::SimpleMode,
    xquery: xquery::XQuery,
    legacy: legacy::Legacy,
    ttcn: ttcn::Ttcn,
    markup: markup::Markup,
    templates: templates::Templates,
    haml: haml::Haml,
    soy: Box<soy::Soy>,
    slim: Box<slim::Slim>,
    rst: Box<rst::Rst>,
}

/// The spans of one line (byte ranges in `line`), in order and apart.
pub fn line(language: Language, line: &str, state: &mut State) -> Vec<(Range<usize>, Kind)> {
    let mut out = scan::Out::default();
    match language {
        Language::JavaScript => js::line(line, &mut state.js, js::Flavor::JavaScript, &mut out),
        Language::TypeScript => js::line(line, &mut state.js, js::Flavor::TypeScript, &mut out),
        Language::Json => js::line(line, &mut state.js, js::Flavor::Json, &mut out),
        Language::Jsx => js::line(line, &mut state.js, js::Flavor::Jsx, &mut out),
        Language::Tsx => js::line(line, &mut state.js, js::Flavor::Tsx, &mut out),
        Language::Python => python::line(line, &mut state.python, &mut out),
        Language::Rust => rust::line(line, &mut state.simple, &mut out),
        Language::Go => simple::go(line, &mut state.simple, &mut out),
        Language::Clike(dialect) => clike::line(line, &mut state.clike, dialect, &mut out),
        Language::Shell => shell::line(line, &mut state.shell, &mut out),
        Language::Css => css::line(line, &mut state.css, false, &mut out),
        Language::Scss => css::line(line, &mut state.css, true, &mut out),
        Language::Toml => toml::line(line, &mut state.toml, &mut out),
        Language::Properties => toml::properties(line, &mut out),
        Language::Diff => toml::diff(line, &mut out),
        Language::Yaml => yaml::line(line, &mut state.yaml, &mut out),
        Language::Html => xml::line(line, state, &mut out),
        Language::Ruby => ruby::line(line, &mut state.ruby, &mut out),
        Language::Php => scripting::php(line, &mut state.scripting, &mut out),
        Language::Swift => scripting::swift(line, &mut state.scripting, &mut out),
        Language::Lua => scripting::lua(line, &mut state.scripting, &mut out),
        Language::PowerShell => scripting::powershell(line, &mut state.scripting, &mut out),
        Language::R => scripting::r(line, &mut state.scripting, &mut out),
        Language::Perl => perl::line(line, &mut state.perl, &mut out),
        Language::Haskell => more::haskell(line, &mut state.more, &mut out),
        Language::Groovy => more::groovy(line, &mut state.more, &mut out),
        Language::Nginx => more::nginx(line, &mut state.more, &mut out),
        Language::Cmake => more::cmake(line, &mut state.more, &mut out),
        Language::Protobuf => more::protobuf(line, &mut state.more, &mut out),
        Language::Latex => tex::latex(line, &mut state.latex, &mut out),
        Language::Dockerfile => more::dockerfile(line, &mut state.more, &mut out),
        Language::Clojure => lisp::clojure(line, &mut state.lisp, &mut out),
        Language::Erlang => erlang::line(line, &mut state.erlang, &mut out),
        Language::Julia => julia::line(line, &mut state.julia, &mut out),
        Language::Octave => more::octave(line, &mut state.more, &mut out),
        Language::Vb => more::vb(line, &mut state.more, &mut out),
        Language::OCaml => ml::line(line, &mut state.ml, ml::Dialect::OCaml, &mut out),
        Language::FSharp => ml::line(line, &mut state.ml, ml::Dialect::FSharp, &mut out),
        Language::Scheme => lisp::scheme(line, &mut state.scheme, &mut out),
        Language::CommonLisp => lisp::common_lisp(line, &mut state.common_lisp, &mut out),
        Language::Fortran => more::fortran(line, &mut state.more, &mut out),
        Language::CoffeeScript => coffee::line(line, &mut state.coffee, &mut out),
        Language::Elm => misc::elm(line, &mut state.misc, &mut out),
        Language::Pascal => misc::pascal(line, &mut state.misc, &mut out),
        Language::Smalltalk => misc::smalltalk(line, &mut state.misc, &mut out),
        Language::Vhdl => misc::vhdl(line, &mut out),
        Language::Sparql => misc::sparql(line, &mut state.misc, &mut out),
        Language::Cypher => misc::cypher(line, &mut out),
        Language::D => misc::d(line, &mut state.misc, &mut out),
        Language::Crystal => crystal::line(line, &mut state.crystal, &mut out),
        Language::Verilog => verilog::line(line, &mut state.verilog, &mut out),
        Language::Tcl => tcl::line(line, &mut state.tcl, &mut out),
        Language::Gherkin => gherkin::line(line, &mut state.gherkin, &mut out),
        Language::Http => http::line(line, &mut state.http, &mut out),
        Language::Mathematica => mathematica::line(line, &mut state.mathematica, &mut out),
        Language::VbScript => vbscript::line(line, &mut out),
        Language::Puppet => puppet::line(line, &mut state.puppet, &mut out),
        Language::Apl => apl::line(line, &mut out),
        Language::Sieve => small::sieve(line, &mut state.small, &mut out),
        Language::Forth => forth::forth(line, &mut state.forth, &mut out),
        Language::Factor => forth::factor(line, &mut state.factor, &mut out),
        Language::Brainfuck => small::brainfuck(line, &mut state.small, &mut out),
        Language::Eiffel => small::eiffel(line, &mut state.small, &mut out),
        Language::Turtle => rdf::turtle(line, &mut state.turtle, &mut out),
        Language::NTriples => rdf::ntriples(line, &mut out),
        Language::Mscgen(dialect) => mscgen::line(line, &mut state.mscgen, dialect, &mut out),
        Language::Ebnf => ebnf::ebnf(line, &mut state.ebnf, &mut out),
        Language::Spreadsheet => ebnf::spreadsheet(line, &mut state.ebnf, &mut out),
        Language::Z80 => z80::line(line, &mut state.z80, &mut out),
        Language::Solr => formats::solr(line, &mut state.formats, &mut out),
        Language::Troff => formats::troff(line, &mut out),
        Language::RpmSpec => formats::rpm_spec(line, &mut state.formats, &mut out),
        Language::AsciiArmor => formats::armor(line, &mut state.formats, &mut out),
        Language::LiterateHaskell => formats::literate_haskell(line, &mut state.formats, &mut out),
        Language::Hxml => formats::hxml(line, &mut state.formats, &mut out),
        Language::Yacas => assorted::yacas(line, &mut state.assorted, &mut out),
        Language::Mumps => assorted::mumps(line, &mut state.assorted, &mut out),
        Language::Pig => assorted::pig(line, &mut state.assorted, &mut out),
        Language::Fcl => assorted::fcl(line, &mut state.assorted, &mut out),
        Language::Oz => oz::line(line, &mut state.oz, &mut out),
        Language::Idl => idl::idl(line, &mut out),
        Language::WebIdl => idl::webidl(line, &mut state.webidl, &mut out),
        Language::Mirc => tcl::mirc(line, &mut state.tcl, &mut out),
        Language::LiveScript => livescript::line(line, &mut state.livescript, &mut out),
        Language::SqlDialect(dialect) => sqlmode::line(line, &mut state.sqlmode, dialect, &mut out),
        Language::Stylus => stylus::line(line, &mut state.stylus, &mut out),
        Language::Pug => pug::line(line, &mut state.pug, &mut out),
        Language::Sml => ml::line(line, &mut state.ml, ml::Dialect::Sml, &mut out),
        Language::MlLike => ml::line(line, &mut state.ml, ml::Dialect::Plain, &mut out),
        Language::Haxe => haxe::line(line, &mut state.haxe, &mut out),
        Language::Modelica => varied::modelica(line, &mut state.varied, &mut out),
        Language::Ecl => varied::ecl(line, &mut state.varied, &mut out),
        Language::Dylan => varied::dylan(line, &mut state.varied, &mut out),
        Language::Nsis => simplemode::line(line, &mut state.simplemode, simplemode::Table::Nsis, &mut out),
        Language::Wast => simplemode::line(line, &mut state.simplemode, simplemode::Table::Wast, &mut out),
        Language::XQuery => xquery::line(line, &mut state.xquery, &mut out),
        Language::Cobol => legacy::cobol(line, &mut state.legacy, &mut out),
        Language::Sas => legacy::sas(line, &mut state.legacy, &mut out),
        Language::Ttcn => ttcn::ttcn(line, &mut state.ttcn, &mut out),
        Language::TtcnCfg => ttcn::ttcn_cfg(line, &mut state.ttcn, &mut out),
        Language::Asterisk => markup::asterisk(line, &mut state.markup, &mut out),
        Language::Dtd => markup::dtd(line, &mut state.markup, &mut out),
        Language::Velocity => markup::velocity(line, &mut state.markup, &mut out),
        Language::TiddlyWiki => markup::tiddlywiki(line, &mut state.markup, &mut out),
        Language::Tiki => markup::tiki(line, &mut state.markup, &mut out),
        Language::Pegjs => templates::pegjs(line, &mut state.templates, &mut out),
        Language::Smarty => templates::smarty(line, &mut state.templates, &mut out),
        Language::Haml => haml::line(line, &mut state.haml, &mut out),
        Language::Soy => soy::line(line, &mut state.soy, &mut out),
        Language::Slim => slim::line(line, &mut state.slim, &mut out),
        Language::Rst => rst::line(line, &mut state.rst, &mut out),
    }
    on_characters(line, out.finish())
}

/// The spans moved onto character boundaries (a port reads bytes; a span may begin or end inside
/// a character past ASCII): a span takes a character when it holds its first byte, as it colors
/// it.
fn on_characters(text: &str, spans: Vec<(Range<usize>, Kind)>) -> Vec<(Range<usize>, Kind)> {
    let up = |mut at: usize| {
        while at < text.len() && !text.is_char_boundary(at) {
            at += 1;
        }
        at.min(text.len())
    };
    let mut out: Vec<(Range<usize>, Kind)> = Vec::with_capacity(spans.len());
    for (range, kind) in spans {
        let start = up(range.start).max(out.last().map_or(0, |(last, _)| last.end));
        let end = up(range.end);
        if start < end {
            out.push((start..end, kind));
        }
    }
    out
}

#[cfg(test)]
mod fuzz;
#[cfg(test)]
mod tests;
