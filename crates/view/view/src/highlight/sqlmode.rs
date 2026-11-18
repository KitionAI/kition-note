//! SQL, as CodeMirror 5's sql mode colors it with each dialect's configuration (standard SQL,
//! MySQL, SQLite, Cassandra's CQL, PL/SQL, PostgreSQL, Esper), ported token for token: keywords,
//! builtins (types and functions), atoms (plain), client commands strings, `@variables` and
//! quoted names variable-2, numbers (as each dialect writes them), strings, operators, brackets,
//! punctuation, and comments (`--`, `#`, `//`, nested `/* */`, as each dialect takes them).

use super::Kind;
use super::scan::{self, Out, Stream};

/// The dialect, by its MIME type's configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// `text/x-sql`.
    Standard,
    MySql,
    Sqlite,
    Cql,
    PlSql,
    PostgreSql,
    Esper,
}

/// What the mode reads next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SqlMode {
    #[default]
    Base,
    /// A string to its quote, and whether a backslash escapes.
    String(u8, bool),
    /// A nested comment, this deep.
    Comment(u32),
    /// After `E` of `E'…'`: the quote comes next, then a string a backslash escapes in.
    EscapeConstant,
}

/// What one dialect configures.
struct Config {
    client: &'static str,
    keywords: &'static str,
    builtins: &'static str,
    atoms: &'static str,
    dates: &'static str,
    operators: &'static [u8],
    backslash_escapes: bool,
    /// The `support` flags.
    odbc_dot_table: bool,
    double_quote: bool,
    binary_number: bool,
    hex_number: bool,
    n_char_cast: bool,
    charset_cast: bool,
    comment_hash: bool,
    comment_space_required: bool,
    comment_slash_slash: bool,
    decimalless_float: bool,
    zeroless_float: bool,
    escape_constant: bool,
    /// The hooks: `@`, `` ` ``, `\`, and SQLite's `:`, `?`, `$`, `"`.
    hook_at: bool,
    hook_backtick: bool,
    hook_backslash: bool,
    hook_sqlite: bool,
}

const NONE: Config = Config {
    client: "",
    keywords: "",
    builtins: "",
    atoms: "",
    dates: "",
    operators: b"",
    backslash_escapes: true,
    odbc_dot_table: false,
    double_quote: false,
    binary_number: false,
    hex_number: false,
    n_char_cast: false,
    charset_cast: false,
    comment_hash: false,
    comment_space_required: false,
    comment_slash_slash: false,
    decimalless_float: false,
    zeroless_float: false,
    escape_constant: false,
    hook_at: false,
    hook_backtick: false,
    hook_backslash: false,
    hook_sqlite: false,
};

fn config(dialect: Dialect) -> Config {
    match dialect {
        Dialect::Standard => Config {
            keywords: SQL_KEYWORDS,
            builtins: SQL_BUILTINS,
            atoms: "false true null unknown",
            dates: "date time timestamp",
            operators: b"*+-%<>!=&|~^/",
            odbc_dot_table: true,
            double_quote: true,
            binary_number: true,
            hex_number: true,
            ..NONE
        },
        Dialect::MySql => Config {
            client: MYSQL_CLIENT,
            keywords: MYSQL_KEYWORDS,
            builtins: MYSQL_BUILTINS,
            atoms: MYSQL_ATOMS,
            dates: MYSQL_DATES,
            operators: b"*+-%<>!=&|^",
            odbc_dot_table: true,
            decimalless_float: true,
            zeroless_float: true,
            binary_number: true,
            hex_number: true,
            double_quote: true,
            n_char_cast: true,
            charset_cast: true,
            comment_hash: true,
            comment_space_required: true,
            hook_at: true,
            hook_backtick: true,
            hook_backslash: true,
            ..NONE
        },
        Dialect::Sqlite => Config {
            client: SQLITE_CLIENT,
            keywords: SQLITE_KEYWORDS,
            builtins: SQLITE_BUILTINS,
            atoms: SQLITE_ATOMS,
            dates: SQLITE_DATES,
            operators: b"*+-%<>!=&|/~",
            decimalless_float: true,
            zeroless_float: true,
            hook_at: true,
            hook_backtick: true,
            hook_sqlite: true,
            ..NONE
        },
        Dialect::Cql => Config {
            client: CQL_CLIENT,
            keywords: CQL_KEYWORDS,
            builtins: CQL_BUILTINS,
            atoms: CQL_ATOMS,
            dates: CQL_DATES,
            operators: b"<>=",
            comment_slash_slash: true,
            decimalless_float: true,
            ..NONE
        },
        Dialect::PlSql => Config {
            client: PLSQL_CLIENT,
            keywords: PLSQL_KEYWORDS,
            builtins: PLSQL_BUILTINS,
            atoms: PLSQL_ATOMS,
            dates: PLSQL_DATES,
            operators: b"*/+-%<>!=~",
            double_quote: true,
            n_char_cast: true,
            zeroless_float: true,
            binary_number: true,
            hex_number: true,
            ..NONE
        },
        Dialect::PostgreSql => Config {
            client: PGSQL_CLIENT,
            keywords: PGSQL_KEYWORDS,
            builtins: PGSQL_BUILTINS,
            atoms: PGSQL_ATOMS,
            dates: PGSQL_DATES,
            operators: b"*/+-%<>!=&|^#@?~",
            backslash_escapes: false,
            odbc_dot_table: true,
            decimalless_float: true,
            zeroless_float: true,
            binary_number: true,
            hex_number: true,
            n_char_cast: true,
            charset_cast: true,
            escape_constant: true,
            ..NONE
        },
        Dialect::Esper => Config {
            client: ESPER_CLIENT,
            keywords: ESPER_KEYWORDS,
            builtins: ESPER_BUILTINS,
            atoms: ESPER_ATOMS,
            dates: ESPER_DATES,
            operators: b"*+-%<>!=&|^/#@?~",
            decimalless_float: true,
            zeroless_float: true,
            binary_number: true,
            hex_number: true,
            ..NONE
        },
    }
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

pub fn line(text: &str, state: &mut SqlMode, dialect: Dialect, out: &mut Out) {
    let config = config(dialect);
    scan::run(text, out, |stream| {
        if *state == SqlMode::Base && stream.eat_space() {
            return None;
        }
        match *state {
            SqlMode::Base => base(stream, state, &config),
            SqlMode::String(quote, escapes) => string(stream, state, quote, escapes),
            SqlMode::Comment(depth) => comment(stream, state, depth),
            SqlMode::EscapeConstant => {
                let quote = stream.next().unwrap_or(b'\'');
                string(stream, state, quote, true)
            }
        }
    });
}

fn string(stream: &mut Stream, state: &mut SqlMode, quote: u8, escapes: bool) -> Option<Kind> {
    let mut escaped = false;
    *state = SqlMode::String(quote, escapes);
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            *state = SqlMode::Base;
            break;
        }
        escaped = escapes && !escaped && here == b'\\';
    }
    Some(Kind::String)
}

/// `^.*?(\/\*|\*\/)`: one nested comment's part.
fn comment(stream: &mut Stream, state: &mut SqlMode, depth: u32) -> Option<Kind> {
    let found = stream.rest().windows(2).position(|pair| pair == b"/*" || pair == b"*/");
    match found {
        Some(at) => {
            let opens = stream.rest().get(at..at + 2) == Some(b"/*");
            stream.pos += at + 2;
            *state = if opens {
                SqlMode::Comment(depth + 1)
            } else if depth > 1 {
                SqlMode::Comment(depth - 1)
            } else {
                SqlMode::Base
            };
        }
        None => stream.skip_to_end(),
    }
    Some(Kind::Comment)
}

/// A quoted name (`` ` `` or `"`, the quote doubled inside) to its close, a variable-2; else
/// the word after the quote.
fn quoted_name(stream: &mut Stream, quote: u8) -> Option<Kind> {
    while let Some(here) = stream.next() {
        if here == quote && stream.eat(|byte| byte == quote).is_none() {
            return Some(Kind::Variable);
        }
    }
    stream.pos = stream.start + 1;
    stream.eat_while(scan::js_word).then_some(Kind::Variable)
}

/// The `@`, `:`, `?`, `$` hook: a variable's name after it (`@@session.`, quoted, or a word).
fn variable(stream: &mut Stream) -> Option<Kind> {
    if stream.take("@") {
        stream.take("session.");
        stream.take("local.");
        stream.take("global.");
    }
    for quote in *b"'\"`" {
        if stream.eat(|byte| byte == quote).is_some() {
            // `^.*<quote>`: to the line's last such quote.
            if let Some(last) = stream.rest().iter().rposition(|byte| *byte == quote) {
                stream.pos += last + 1;
            }
            return Some(Kind::Variable);
        }
    }
    stream.eat_while(|byte| byte.is_ascii_alphanumeric() || b"$._".contains(&byte)).then_some(Kind::Variable)
}

fn base(stream: &mut Stream, state: &mut SqlMode, config: &Config) -> Option<Kind> {
    let here = stream.next()?;
    let lower = here.to_ascii_lowercase();
    match here {
        b'@' if config.hook_at => return variable(stream),
        b':' | b'?' | b'$' if config.hook_sqlite => return variable(stream),
        b'"' if config.hook_sqlite => return quoted_name(stream, b'"'),
        b'`' if config.hook_backtick => return quoted_name(stream, b'`'),
        b'\\' if config.hook_backslash => {
            if stream.take("N") {
                return None;
            }
            return stream.eat(|byte| byte.is_ascii_alphabetic() || b".#!?".contains(&byte)).map(|_| Kind::Variable);
        }
        _ => {}
    }
    let run_from = |stream: &Stream, from: usize, ok: fn(u8) -> bool| {
        (from..stream.bytes.len()).find(|end| !ok(stream.byte(*end))).unwrap_or(stream.bytes.len().max(from))
    };
    if config.hex_number {
        if here == b'0' && matches!(stream.peek(), b'x' | b'X') && stream.byte(stream.pos + 1).is_ascii_hexdigit() {
            stream.pos = run_from(stream, stream.pos + 1, |byte| byte.is_ascii_hexdigit());
            return Some(Kind::Number);
        }
        if lower == b'x' && stream.peek() == b'\'' {
            let end = run_from(stream, stream.pos + 1, |byte| byte.is_ascii_hexdigit());
            if end > stream.pos + 1 && stream.byte(end) == b'\'' {
                stream.pos = end + 1;
                return Some(Kind::Number);
            }
        }
    }
    if config.binary_number {
        if lower == b'b' && stream.peek() == b'\'' {
            let end = run_from(stream, stream.pos + 1, |byte| byte == b'0' || byte == b'1');
            if end > stream.pos + 1 && stream.byte(end) == b'\'' {
                stream.pos = end + 1;
                return Some(Kind::Number);
            }
        }
        if here == b'0' && stream.peek() == b'b' && matches!(stream.byte(stream.pos + 1), b'0' | b'1') {
            stream.pos = run_from(stream, stream.pos + 1, |byte| byte == b'0' || byte == b'1');
            return Some(Kind::Number);
        }
    }
    if here.is_ascii_digit() {
        // `[0-9]*(\.[0-9]+)?([eE][-+]?[0-9]+)?`, then (where floats may end in a point) `.`
        stream.eat_while(|byte| byte.is_ascii_digit());
        if stream.peek() == b'.' && stream.byte(stream.pos + 1).is_ascii_digit() {
            stream.next();
            stream.eat_while(|byte| byte.is_ascii_digit());
        }
        if matches!(stream.peek(), b'e' | b'E') {
            let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'+' | b'-'));
            if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
        if config.decimalless_float && stream.peek() == b'.' && stream.byte(stream.pos + 1) != b'.' {
            stream.next();
        }
        return Some(Kind::Number);
    }
    if here == b'?' && (stream.eat_space() || stream.eol() || stream.take(";")) {
        return Some(Kind::Variable);
    }
    if here == b'\'' || (here == b'"' && config.double_quote) {
        return string(stream, state, here, config.backslash_escapes);
    }
    // `N'…'` and `_charset'…'`: the prefix a keyword, before its string.
    let cast = (config.n_char_cast && lower == b'n')
        || (config.charset_cast && here == b'_' && {
            let start = stream.pos;
            let matched = stream.eat(|byte| byte.is_ascii_alphabetic()).is_some();
            if matched {
                stream.eat_while(|byte| byte.is_ascii_alphanumeric());
            } else {
                stream.pos = start;
            }
            matched
        });
    if cast && matches!(stream.peek(), b'\'' | b'"') {
        return Some(Kind::Keyword);
    }
    if config.escape_constant && lower == b'e' && (stream.peek() == b'\'' || (stream.peek() == b'"' && config.double_quote)) {
        *state = SqlMode::EscapeConstant;
        return Some(Kind::Keyword);
    }
    if config.comment_slash_slash && here == b'/' && stream.take("/") {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if (config.comment_hash && here == b'#') || (here == b'-' && stream.take("-") && (!config.comment_space_required || stream.take(" "))) {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here == b'/' && stream.take("*") {
        *state = SqlMode::Comment(1);
        return comment(stream, state, 1);
    }
    if here == b'.' {
        if config.zeroless_float && stream.peek().is_ascii_digit() {
            stream.eat_while(|byte| byte.is_ascii_digit());
            if matches!(stream.peek(), b'e' | b'E') {
                let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'+' | b'-'));
                if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                    stream.pos += 1 + sign;
                    stream.eat_while(|byte| byte.is_ascii_digit());
                }
            }
            return Some(Kind::Number);
        }
        if stream.eat_while(|byte| byte == b'.') {
            return None;
        }
        if config.odbc_dot_table && stream.eat_while(|byte| scan::js_word(byte) || byte == b'$' || byte == b'#') {
            return Some(Kind::Variable);
        }
        return None;
    }
    if config.operators.contains(&here) {
        stream.eat_while(|byte| config.operators.contains(&byte));
        return Some(Kind::Operator);
    }
    if b"{}()[]".contains(&here) {
        return Some(Kind::Punctuation);
    }
    if b";.,:".contains(&here) {
        stream.eat_while(|byte| b";.,:".contains(&byte));
        return Some(Kind::Punctuation);
    }
    stream.eat_while(scan::js_word);
    let word = stream.current().to_ascii_lowercase();
    if listed(config.dates, &word) {
        let spaces = run_from(stream, stream.pos, |byte| byte == b' ');
        let quote = stream.byte(spaces);
        if spaces > stream.pos
            && (quote == b'\'' || quote == b'"')
            && let Some(close) = stream.bytes.get(spaces + 1..).and_then(|rest| rest.iter().position(|byte| *byte == quote))
        {
            stream.pos = spaces + 1 + close + 1;
            return Some(Kind::Number);
        }
    }
    if listed(config.atoms, &word) {
        None
    } else if listed(config.builtins, &word) {
        Some(Kind::Property)
    } else if listed(config.keywords, &word) {
        Some(Kind::Keyword)
    } else if listed(config.client, &word) {
        Some(Kind::String)
    } else {
        None
    }
}

const MYSQL_CLIENT: &str =
    "charset clear connect edit ego exit go help nopager notee nowarning pager print prompt quit rehash source status system tee";

const MYSQL_KEYWORDS: &str = "alter and as asc between by count create delete desc distinct drop from group having in insert into is join like not on or \
order select set table union update values where limit accessible action add after algorithm all analyze asensitive at \
authors auto_increment autocommit avg avg_row_length before binary binlog both btree cache call cascade cascaded case \
catalog_name chain change changed character check checkpoint checksum class_origin client_statistics close coalesce code \
collate collation collations column columns comment commit committed completion concurrent condition connection consistent \
constraint contains continue contributors convert cross current current_date current_time current_timestamp current_user \
cursor data database databases day_hour day_microsecond day_minute day_second deallocate dec declare default delay_key_write \
delayed delimiter des_key_file describe deterministic dev_pop dev_samp deviance diagnostics directory disable discard \
distinctrow div dual dumpfile each elseif enable enclosed end ends engine engines enum errors escape escaped even event \
events every execute exists exit explain extended fast fetch field fields first flush for force foreign found_rows full \
fulltext function general get global grant grants group group_concat handler hash help high_priority hosts hour_microsecond \
hour_minute hour_second if ignore ignore_server_ids import index index_statistics infile inner innodb inout insensitive \
insert_method install interval invoker isolation iterate key keys kill language last leading leave left level limit linear \
lines list load local localtime localtimestamp lock logs low_priority master master_heartbeat_period \
master_ssl_verify_server_cert masters match max max_rows maxvalue message_text middleint migrate min min_rows \
minute_microsecond minute_second mod mode modifies modify mutex mysql_errno natural next no no_write_to_binlog offline \
offset one online open optimize option optionally out outer outfile pack_keys parser partition partitions password phase \
plugin plugins prepare preserve prev primary privileges procedure processlist profile profiles purge query quick range read \
read_write reads real rebuild recover references regexp relaylog release remove rename reorganize repair repeatable replace \
require resignal restrict resume return returns revoke right rlike rollback rollup row row_format rtree savepoint schedule \
schema schema_name schemas second_microsecond security sensitive separator serializable server session share show signal \
slave slow smallint snapshot soname spatial specific sql sql_big_result sql_buffer_result sql_cache sql_calc_found_rows \
sql_no_cache sql_small_result sqlexception sqlstate sqlwarning ssl start starting starts status std stddev stddev_pop \
stddev_samp storage straight_join subclass_origin sum suspend table_name table_statistics tables tablespace temporary \
terminated to trailing transaction trigger triggers truncate uncommitted undo uninstall unique unlock upgrade usage use \
use_frm user user_resources user_statistics using utc_date utc_time utc_timestamp value variables varying view views \
warnings when while with work write xa xor year_month zerofill begin do then else loop repeat";

const MYSQL_BUILTINS: &str = "bool boolean bit blob decimal double float long longblob longtext medium mediumblob mediumint mediumtext time timestamp \
tinyblob tinyint tinytext text bigint int int1 int2 int3 int4 int8 integer float float4 float8 double char varbinary varchar \
varcharacter precision date datetime year unsigned signed numeric";

const MYSQL_ATOMS: &str = "false true null unknown";

const MYSQL_DATES: &str = "date time timestamp";

const SQLITE_CLIENT: &str = "auth backup bail binary changes check clone databases dbinfo dump echo eqp exit explain fullschema headers help import \
imposter indexes iotrace limit lint load log mode nullvalue once open output print prompt quit read restore save scanstats \
schema separator session shell show stats system tables testcase timeout timer trace vfsinfo vfslist vfsname width";

const SQLITE_KEYWORDS: &str = "alter and as asc between by count create delete desc distinct drop from group having in insert into is join like not on or \
order select set table union update values where limit abort action add after all analyze attach autoincrement before begin \
cascade case cast check collate column commit conflict constraint cross current_date current_time current_timestamp database \
default deferrable deferred detach each else end escape except exclusive exists explain fail for foreign full glob if ignore \
immediate index indexed initially inner instead intersect isnull key left limit match natural no notnull null of offset \
outer plan pragma primary query raise recursive references regexp reindex release rename replace restrict right rollback row \
savepoint temp temporary then to transaction trigger unique using vacuum view virtual when with without";

const SQLITE_BUILTINS: &str = "bool boolean bit blob decimal double float long longblob longtext medium mediumblob mediumint mediumtext time timestamp \
tinyblob tinyint tinytext text clob bigint int int2 int8 integer float double char varchar date datetime year unsigned \
signed numeric real";

const SQLITE_ATOMS: &str = "null current_date current_time current_timestamp";

const SQLITE_DATES: &str = "date time timestamp datetime";

const CQL_CLIENT: &str = "";

const CQL_KEYWORDS: &str = "add all allow alter and any apply as asc authorize batch begin by clustering columnfamily compact consistency count create \
custom delete desc distinct drop each_quorum exists filtering from grant if in index insert into key keyspace keyspaces \
level limit local_one local_quorum modify nan norecursive nosuperuser not of on one order password permission permissions \
primary quorum rename revoke schema select set storage superuser table three to token truncate ttl two type unlogged update \
use user users using values where with writetime";

const CQL_BUILTINS: &str = "ascii bigint blob boolean counter decimal double float frozen inet int list map static text timestamp timeuuid tuple uuid \
varchar varint";

const CQL_ATOMS: &str = "false true infinity NaN";

const CQL_DATES: &str = "";

const PLSQL_CLIENT: &str = "appinfo arraysize autocommit autoprint autorecovery autotrace blockterminator break btitle cmdsep colsep compatibility \
compute concat copycommit copytypecheck define describe echo editfile embedded escape exec execute feedback flagger flush \
heading headsep instance linesize lno loboffset logsource long longchunksize markup native newpage numformat numwidth \
pagesize pause pno recsep recsepchar release repfooter repheader serveroutput shiftinout show showmode size spool \
sqlblanklines sqlcase sqlcode sqlcontinue sqlnumber sqlpluscompatibility sqlprefix sqlprompt sqlterminator suffix tab term \
termout time timing trimout trimspool ttitle underline verify version wrap";

const PLSQL_KEYWORDS: &str = "abort accept access add all alter and any array arraylen as asc assert assign at attributes audit authorization avg \
base_table begin between binary_integer body boolean by case cast char char_base check close cluster clusters colauth column \
comment commit compress connect connected constant constraint crash create current currval cursor data_base database date \
dba deallocate debugoff debugon decimal declare default definition delay delete desc digits dispose distinct do drop else \
elseif elsif enable end entry escape exception exception_init exchange exclusive exists exit external fast fetch file for \
force form from function generic goto grant group having identified if immediate in increment index indexes indicator \
initial initrans insert interface intersect into is key level library like limited local lock log logging long loop master \
maxextents maxtrans member minextents minus mislabel mode modify multiset new next no noaudit nocompress nologging \
noparallel not nowait number_base object of off offline on online only open option or order out package parallel partition \
pctfree pctincrease pctused pls_integer positive positiven pragma primary prior private privileges procedure public raise \
range raw read rebuild record ref references refresh release rename replace resource restrict return returning returns \
reverse revoke rollback row rowid rowlabel rownum rows run savepoint schema segment select separate session set share \
snapshot some space split sql start statement storage subtype successful synonym tabauth table tables tablespace task \
terminate then to trigger truncate type union unique unlimited unrecoverable unusable update use using validate value values \
variable view views when whenever where while with work";

const PLSQL_BUILTINS: &str = "abs acos add_months ascii asin atan atan2 average bfile bfilename bigserial bit blob ceil character chartorowid chr clob \
concat convert cos cosh count dec decode deref dual dump dup_val_on_index empty error exp false float floor found glb \
greatest hextoraw initcap instr instrb int integer isopen last_day least length lengthb ln lower lpad ltrim lub make_ref max \
min mlslabel mod months_between natural naturaln nchar nclob new_time next_day nextval nls_charset_decl_len nls_charset_id \
nls_charset_name nls_initcap nls_lower nls_sort nls_upper nlssort no_data_found notfound null number numeric nvarchar2 nvl \
others power rawtohex real reftohex round rowcount rowidtochar rowtype rpad rtrim serial sign signtype sin sinh smallint \
soundex sqlcode sqlerrm sqrt stddev string substr substrb sum sysdate tan tanh to_char text to_date to_label to_multi_byte \
to_number to_single_byte translate true trunc uid unlogged upper user userenv varchar varchar2 variance varying vsize xml";

const PLSQL_ATOMS: &str = "false true null";

const PLSQL_DATES: &str = "date time timestamp";

const PGSQL_CLIENT: &str = "source";

const PGSQL_KEYWORDS: &str = "alter and as asc between by count create delete desc distinct drop from group having in insert into is join like not on or \
order select set table union update values where limit a abort abs absent absolute access according action ada add admin \
after aggregate alias all allocate also alter always analyse analyze and any are array array_agg array_max_cardinality as \
asc asensitive assert assertion assignment asymmetric at atomic attach attribute attributes authorization avg backward \
base64 before begin begin_frame begin_partition bernoulli between bigint binary bit bit_length blob blocked bom boolean both \
breadth by c cache call called cardinality cascade cascaded case cast catalog catalog_name ceil ceiling chain char \
char_length character character_length character_set_catalog character_set_name character_set_schema characteristics \
characters check checkpoint class class_origin clob close cluster coalesce cobol collate collation collation_catalog \
collation_name collation_schema collect column column_name columns command_function command_function_code comment comments \
commit committed concurrently condition condition_number configuration conflict connect connection connection_name constant \
constraint constraint_catalog constraint_name constraint_schema constraints constructor contains content continue control \
conversion convert copy corr corresponding cost count covar_pop covar_samp create cross csv cube cume_dist current \
current_catalog current_date current_default_transform_group current_path current_role current_row current_schema \
current_time current_timestamp current_transform_group_for_type current_user cursor cursor_name cycle data database datalink \
datatype date datetime_interval_code datetime_interval_precision day db deallocate debug dec decimal declare default \
defaults deferrable deferred defined definer degree delete delimiter delimiters dense_rank depends depth deref derived desc \
describe descriptor detach detail deterministic diagnostics dictionary disable discard disconnect dispatch distinct \
dlnewcopy dlpreviouscopy dlurlcomplete dlurlcompleteonly dlurlcompletewrite dlurlpath dlurlpathonly dlurlpathwrite \
dlurlscheme dlurlserver dlvalue do document domain double drop dump dynamic dynamic_function dynamic_function_code each \
element else elseif elsif empty enable encoding encrypted end end_frame end_partition endexec enforced enum equals errcode \
error escape event every except exception exclude excluding exclusive exec execute exists exit exp explain expression \
extension external extract false family fetch file filter final first first_value flag float floor following for force \
foreach foreign fortran forward found frame_row free freeze from fs full function functions fusion g general generated get \
global go goto grant granted greatest group grouping groups handler having header hex hierarchy hint hold hour id identity \
if ignore ilike immediate immediately immutable implementation implicit import in include including increment indent index \
indexes indicator info inherit inherits initially inline inner inout input insensitive insert instance instantiable instead \
int integer integrity intersect intersection interval into invoker is isnull isolation join k key key_member key_type label \
lag language large last last_value lateral lead leading leakproof least left length level library like like_regex limit link \
listen ln load local localtime localtimestamp location locator lock locked log logged loop lower m map mapping match matched \
materialized max max_cardinality maxvalue member merge message message_length message_octet_length message_text method min \
minute minvalue mod mode modifies module month more move multiset mumps name names namespace national natural nchar nclob \
nesting new next nfc nfd nfkc nfkd nil no none normalize normalized not nothing notice notify notnull nowait nth_value ntile \
null nullable nullif nulls number numeric object occurrences_regex octet_length octets of off offset oids old on only open \
operator option options or order ordering ordinality others out outer output over overlaps overlay overriding owned owner p \
pad parallel parameter parameter_mode parameter_name parameter_ordinal_position parameter_specific_catalog \
parameter_specific_name parameter_specific_schema parser partial partition pascal passing passthrough password path percent \
percent_rank percentile_cont percentile_disc perform period permission pg_context pg_datatype_name pg_exception_context \
pg_exception_detail pg_exception_hint placing plans pli policy portion position position_regex power precedes preceding \
precision prepare prepared preserve primary print_strict_params prior privileges procedural procedure procedures program \
public publication query quote raise range rank read reads real reassign recheck recovery recursive ref references \
referencing refresh regr_avgx regr_avgy regr_count regr_intercept regr_r2 regr_slope regr_sxx regr_sxy regr_syy reindex \
relative release rename repeatable replace replica requiring reset respect restart restore restrict result result_oid return \
returned_cardinality returned_length returned_octet_length returned_sqlstate returning returns reverse revoke right role \
rollback rollup routine routine_catalog routine_name routine_schema routines row row_count row_number rows rowtype rule \
savepoint scale schema schema_name schemas scope scope_catalog scope_name scope_schema scroll search second section security \
select selective self sensitive sequence sequences serializable server server_name session session_user set setof sets share \
show similar simple size skip slice smallint snapshot some source space specific specific_name specifictype sql sqlcode \
sqlerror sqlexception sqlstate sqlwarning sqrt stable stacked standalone start state statement static statistics stddev_pop \
stddev_samp stdin stdout storage strict strip structure style subclass_origin submultiset subscription substring \
substring_regex succeeds sum symmetric sysid system system_time system_user t table table_name tables tablesample tablespace \
temp template temporary text then ties time timestamp timezone_hour timezone_minute to token top_level_count trailing \
transaction transaction_active transactions_committed transactions_rolled_back transform transforms translate \
translate_regex translation treat trigger trigger_catalog trigger_name trigger_schema trim trim_array true truncate trusted \
type types uescape unbounded uncommitted under unencrypted union unique unknown unlink unlisten unlogged unnamed unnest \
until untyped update upper uri usage use_column use_variable user user_defined_type_catalog user_defined_type_code \
user_defined_type_name user_defined_type_schema using vacuum valid validate validator value value_of values var_pop var_samp \
varbinary varchar variable_conflict variadic varying verbose version versioning view views volatile warning when whenever \
where while whitespace width_bucket window with within without work wrapper write xml xmlagg xmlattributes xmlbinary xmlcast \
xmlcomment xmlconcat xmldeclaration xmldocument xmlelement xmlexists xmlforest xmliterate xmlnamespaces xmlparse xmlpi \
xmlquery xmlroot xmlschema xmlserialize xmltable xmltext xmlvalidate year yes zone";

const PGSQL_BUILTINS: &str = "bigint int8 bigserial serial8 bit varying varbit boolean bool box bytea character char varchar cidr circle date double \
precision float8 inet integer int int4 interval json jsonb line lseg macaddr macaddr8 money numeric decimal path pg_lsn \
point polygon real float4 smallint int2 smallserial serial2 serial serial4 text time without zone with timetz timestamp \
timestamptz tsquery tsvector txid_snapshot uuid xml";

const PGSQL_ATOMS: &str = "false true null unknown";

const PGSQL_DATES: &str = "date time timestamp";

const ESPER_CLIENT: &str = "source";

const ESPER_KEYWORDS: &str = "alter and as asc between by count create delete desc distinct drop from group having in insert into is join like not on or \
order select set table union update values where limit after all and as at asc avedev avg between by case cast coalesce \
count create current_timestamp day days delete define desc distinct else end escape events every exists false first from \
full group having hour hours in inner insert instanceof into irstream is istream join last lastweekday left limit like max \
match_recognize matches median measures metadatasql min minute minutes msec millisecond milliseconds not null offset on or \
order outer output partition pattern prev prior regexp retain-union retain-intersection right rstream sec second seconds \
select set some snapshot sql stddev sum then true unidirectional until update variable weekday when where window";

const ESPER_BUILTINS: &str = "";

const ESPER_ATOMS: &str = "false true null";

const ESPER_DATES: &str = "time";

const SQL_KEYWORDS: &str = "alter and as asc between by count create delete desc distinct drop from group having in insert into is join like not on or \
order select set table union update values where limit begin";

const SQL_BUILTINS: &str = "bool boolean bit blob enum long longblob longtext medium mediumblob mediumint mediumtext time timestamp tinyblob tinyint \
tinytext text bigint int int1 int2 int3 int4 int8 integer float float4 float8 double char varbinary varchar varcharacter \
precision real date datetime year unsigned signed decimal numeric";
