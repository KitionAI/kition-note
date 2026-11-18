//! Perl, as CodeMirror 5's perl mode colors it, ported token for token: its table of words
//! (keywords, `my` a definition, `eq` an operator, `STDIN` a variable), `$`, `@`, `%`, and `&`
//! variables, quote-like operators (`q`, `qq`, `qw`, `qr`, `m`, `s`, `tr`, `y`) to their
//! delimiters, a `/` a regular expression only after `~`, heredocs, other names meta.

use super::Kind;
use super::scan::{Out, Stream};

/// The mode's keywords (its builtin functions too).
const KEYWORDS: &str = r##"if elsif else while unless for foreach abs accept alarm atan2 bind binmode bless bootstrap break
caller chdir chmod chomp chop chown chr chroot close closedir connect continue cos crypt dbmclose
dbmopen default defined delete die do dump each endgrent endhostent endnetent endprotoent endpwent
endservent eof eval exec exists exit exp fcntl fileno flock fork format formline getc getgrent
getgrgid getgrnam gethostbyaddr gethostbyname gethostent getlogin getnetbyaddr getnetbyname
getnetent getpeername getpgrp getppid getpriority getprotobyname getprotobynumber getprotoent
getpwent getpwnam getpwuid getservbyname getservbyport getservent getsockname getsockopt given glob
gmtime goto grep hex import index int ioctl join keys kill last lc lcfirst length link listen
localtime lock log lstat map mkdir msgctl msgget msgrcv msgsnd new next no oct open opendir ord pack
package pipe pop pos print printf prototype push rand read readdir readline readlink readpipe recv
redo ref rename require reset return reverse rewinddir rindex rmdir say scalar seek seekdir select
semctl semget semop send setgrent sethostent setnetent setpgrp setpriority setprotoent setpwent
setservent setsockopt shift shmctl shmget shmread shmwrite shutdown sin sleep socket socketpair sort
splice split sprintf sqrt srand stat state study sub substr symlink syscall sysopen sysread sysseek
system syswrite tell telldir tie tied time times truncate uc ucfirst umask undef unlink unpack
unshift untie use utime values vec wait waitpid wantarray warn when write"##;
/// Declarations: definitions, in the text's color.
const DEFS: &str = r##"local my our"##;
/// Operators, words among them.
const OPERATORS: &str = r##"-> ++ -- ** =~ !~ * / % x + - . << >> < > <= >= lt gt le ge == != <=> eq ne cmp ~~ & | ^ && || // ..
... ? : = += -= *= , => :: not and or xor"##;
/// Perl's own variables and handles (`variable-2`).
const VARIABLES: &str = r##"BEGIN END PRINT PRINTF GETC READ READLINE DESTROY TIE TIEHANDLE UNTIE STDIN STDIN_TOP STDOUT
STDOUT_TOP STDERR STDERR_TOP $ARG $_ @ARG @_ $LIST_SEPARATOR $" $PROCESS_ID $PID $$ $REAL_GROUP_ID
$GID $( $EFFECTIVE_GROUP_ID $EGID $) $PROGRAM_NAME $0 $SUBSCRIPT_SEPARATOR $SUBSEP $; $REAL_USER_ID
$UID $< $EFFECTIVE_USER_ID $EUID $> $a $b $COMPILING $^C $DEBUGGING $^D ${^ENCODING} $ENV %ENV
$SYSTEM_FD_MAX $^F @F ${^GLOBAL_PHASE} $^H %^H @INC %INC $INPLACE_EDIT $^I $^M $OSNAME $^O ${^OPEN}
$PERLDB $^P $SIG %SIG $BASETIME $^T ${^TAINT} ${^UNICODE} ${^UTF8CACHE} ${^UTF8LOCALE} $PERL_VERSION
$^V ${^WIN32_SLOPPY_STAT} $EXECUTABLE_NAME $^X $1 $MATCH $& ${^MATCH} $PREMATCH $` ${^PREMATCH}
$POSTMATCH $' ${^POSTMATCH} $LAST_PAREN_MATCH $+ $LAST_SUBMATCH_RESULT $^N @LAST_MATCH_END @+
%LAST_PAREN_MATCH %+ @LAST_MATCH_START @- %LAST_MATCH_START %- $LAST_REGEXP_CODE_RESULT $^R
${^RE_DEBUG_FLAGS} ${^RE_TRIE_MAXBUF} $ARGV @ARGV ARGV ARGVOUT $OUTPUT_FIELD_SEPARATOR $OFS $,
$INPUT_LINE_NUMBER $NR $. $INPUT_RECORD_SEPARATOR $RS $/ $OUTPUT_RECORD_SEPARATOR $ORS $\
$OUTPUT_AUTOFLUSH $| $ACCUMULATOR $^A $FORMAT_FORMFEED $^L $FORMAT_PAGE_NUMBER $% $FORMAT_LINES_LEFT
$- $FORMAT_LINE_BREAK_CHARACTERS $: $FORMAT_LINES_PER_PAGE $= $FORMAT_TOP_NAME $^ $FORMAT_NAME $~
${^CHILD_ERROR_NATIVE} $EXTENDED_OS_ERROR $^E $EXCEPTIONS_BEING_CAUGHT $^S $WARNING $^W
${^WARNING_BITS} $OS_ERROR $ERRNO $! %OS_ERROR %ERRNO %! $CHILD_ERROR $? $EVAL_ERROR $@ $OFMT $# $*
$ARRAY_BASE $[ $OLD_PERL_VERSION $]"##;

/// What the mode reads next: Perl, the rest of a quote-like (its closing delimiters, its style,
/// whether its flags follow), or a heredoc (to the line that ends it).
#[derive(Clone, Debug, Default)]
enum Mode {
    #[default]
    Perl,
    Chain(Vec<u8>, Option<Kind>, bool),
    Heredoc(String),
}

#[derive(Clone, Debug, Default)]
pub struct Perl {
    mode: Mode,
    /// A substitution's second part (`s{…}{…}`), read as the next token.
    pending: Option<(u8, Option<Kind>, bool)>,
}

fn class(word: &str) -> Option<u8> {
    let has = |list: &str| list.split_ascii_whitespace().any(|known| known == word);
    if has(KEYWORDS) {
        Some(1)
    } else if has(DEFS) {
        Some(2)
    } else if has(OPERATORS) {
        Some(4)
    } else if has(VARIABLES) {
        Some(5)
    } else {
        None
    }
}

/// The style for a word's class (none: meta).
fn style(class: Option<u8>) -> Option<Kind> {
    match class {
        Some(1) => Some(Kind::Keyword),
        Some(2) | Some(3) => None,
        Some(4) => Some(Kind::Operator),
        Some(5) => Some(Kind::Variable),
        _ => Some(Kind::Comment),
    }
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn delimiter(byte: u8) -> bool {
    b"([{<^'\"!~/".contains(&byte)
}

fn flags(byte: u8) -> bool {
    b"goseximacplud".contains(&byte)
}

fn closing(open: u8) -> u8 {
    match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        other => other,
    }
}

/// Reads a quote-like to its closing delimiters (`tokenChain`): the first closes this token; a
/// second (a substitution's) is read as the next one; at the line's end it goes on.
fn chain(stream: &mut Stream, perl: &mut Perl, closers: Vec<u8>, style: Option<Kind>, tail: bool) -> Option<Kind> {
    perl.pending = None;
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if Some(&here) == closers.first() && !escaped {
            if let Some(second) = closers.get(1) {
                perl.pending = Some((*second, style, tail));
            } else if tail {
                stream.eat_while(flags);
            }
            perl.mode = Mode::Perl;
            return style;
        }
        escaped = !escaped && here == b'\\';
    }
    perl.mode = Mode::Chain(closers, style, tail);
    style
}

fn heredoc(stream: &mut Stream, perl: &mut Perl, end: String) -> Option<Kind> {
    perl.mode = Mode::Heredoc(end);
    stream.pos = stream.bytes.len();
    Some(Kind::String)
}

pub fn line(text: &str, perl: &mut Perl, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut stream = Stream { bytes, start: 0, pos: 0 };
    if let Mode::Heredoc(end) = &perl.mode {
        if text == end {
            perl.mode = Mode::Perl;
        }
        out.push(0..bytes.len(), Kind::String);
        return;
    }
    while stream.pos < bytes.len() {
        stream.start = stream.pos;
        let style = match std::mem::take(&mut perl.mode) {
            Mode::Chain(closers, style, tail) => chain(&mut stream, perl, closers, style, tail),
            Mode::Heredoc(end) => {
                perl.mode = Mode::Heredoc(end);
                stream.pos = bytes.len();
                Some(Kind::String)
            }
            Mode::Perl => token(&mut stream, perl),
        };
        if stream.pos == stream.start {
            stream.pos += 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

fn number_end(bytes: &[u8], at: usize) -> Option<usize> {
    let digits = |from: usize| {
        let mut end = from;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        end
    };
    let exponent = |from: usize| {
        let mut end = from;
        if matches!(bytes.get(end), Some(b'e')) {
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
    // `-?(\d*\.\d+(e[+-]?\d+)?|\d+\.\d*)`
    let signed = if bytes.get(at) == Some(&b'-') { at + 1 } else { at };
    let whole = digits(signed);
    if bytes.get(whole) == Some(&b'.') {
        let fraction = digits(whole + 1);
        if fraction > whole + 1 {
            return Some(exponent(fraction));
        }
        if whole > signed {
            return Some(whole + 1);
        }
    }
    if bytes.get(at) == Some(&b'0') && bytes.get(at + 1) == Some(&b'x') {
        let end = (at + 2..).find(|index| !bytes.get(*index).is_some_and(u8::is_ascii_hexdigit)).unwrap_or(at + 2);
        if end > at + 2 {
            return Some(end);
        }
    }
    if bytes.get(at) == Some(&b'0') && bytes.get(at + 1) == Some(&b'b') {
        let end = (at + 2..).find(|index| !matches!(bytes.get(*index), Some(b'0' | b'1'))).unwrap_or(at + 2);
        if end > at + 2 {
            return Some(end);
        }
    }
    let end = digits(at);
    (end > at).then(|| exponent(end))
}

/// One token from Perl's own text (`tokenPerl`).
fn token(stream: &mut Stream, perl: &mut Perl) -> Option<Kind> {
    let bytes = stream.bytes;
    if matches!(stream.look(0), b' ' | b'\t' | b'\r') {
        stream.eat_while(|byte| matches!(byte, b' ' | b'\t' | b'\r'));
        return None;
    }
    if let Some((closer, style, tail)) = perl.pending.take() {
        return chain(stream, perl, vec![closer], style, tail);
    }
    let first = stream.look(0);
    if ((first == b'-' && (stream.look(1).is_ascii_digit() || stream.look(1) == b'.')) || first.is_ascii_digit() || first == b'.')
        && let Some(end) = number_end(bytes, stream.pos)
    {
        stream.pos = end;
        return Some(Kind::Number);
    }
    if first == b'<' && stream.look(1) == b'<' && (stream.look(2).is_ascii_alphabetic() || stream.look(2) == b'_') {
        stream.pos += 2;
        stream.eat_while(word);
        let end = stream.current()[2..].to_string();
        return heredoc(stream, perl, end);
    }
    if stream.pos == 0 && bytes.starts_with(b"=item") && !bytes.get(5).copied().is_some_and(word) {
        return heredoc(stream, perl, "=cut".to_string());
    }
    let here = stream.next()?;
    let before = stream.look(-2);
    if here == b'"' || here == b'\'' {
        if stream.pos >= 3 && bytes.get(stream.pos - 3..stream.pos - 1) == Some(b"<<".as_slice()) {
            let back = stream.pos;
            stream.eat_while(word);
            let end = String::from_utf8_lossy(bytes.get(back..stream.pos).unwrap_or_default()).to_string();
            if !end.is_empty() && stream.eat(|byte| byte == here).is_some() {
                return heredoc(stream, perl, end);
            }
            stream.pos = back;
        }
        return chain(stream, perl, vec![here], Some(Kind::String), false);
    }
    if here == b'q' && !word(before) {
        let kind = stream.look(0);
        let (style, tail, at) = match kind {
            b'x' | b'r' => (Some(Kind::String), true, 1),
            b'q' => (Some(Kind::String), false, 1),
            b'w' => (Some(Kind::Punctuation), false, 1),
            _ => (Some(Kind::String), false, 0),
        };
        let open = stream.look(at);
        if matches!(open, b'(' | b'[' | b'{' | b'<') {
            stream.skip(at as usize + 1);
            return chain(stream, perl, vec![closing(open)], style, tail);
        }
        if b"^'\"!~/".contains(&open) {
            if at == 1 {
                stream.skip(1);
            }
            if let Some(open) = stream.eat(|byte| byte == open) {
                return chain(stream, perl, vec![open], style, tail);
            }
        }
    }
    if here == b'm'
        && !word(before)
        && let Some(open) = stream.eat(delimiter)
    {
        return chain(stream, perl, vec![closing(open)], Some(Kind::String), true);
    }
    if matches!(here, b's' | b'y' | b't') && !b"/>]})".contains(&before) && !word(before) {
        let ready = here != b't' || stream.eat(|byte| byte == b'r').is_some();
        if ready && let Some(open) = stream.eat(delimiter) {
            let close = closing(open);
            return chain(stream, perl, vec![close, close], Some(Kind::String), true);
        }
    }
    if here == b'`' {
        return chain(stream, perl, vec![here], Some(Kind::Variable), false);
    }
    if here == b'/' {
        let prefix = bytes.get(..stream.pos - 1).and_then(|text| std::str::from_utf8(text).ok()).unwrap_or("").trim_end();
        if prefix.ends_with('~') {
            return chain(stream, perl, vec![here], Some(Kind::String), true);
        }
        return Some(Kind::Operator);
    }
    if here == b'$' {
        let back = stream.pos;
        if stream.eat_while(|byte| byte.is_ascii_digit())
            || (stream.eat(|byte| byte == b'{').is_some()
                && stream.eat_while(|byte| byte.is_ascii_digit())
                && stream.eat(|byte| byte == b'}').is_some())
        {
            return Some(Kind::Variable);
        }
        stream.pos = back;
    }
    if matches!(here, b'$' | b'@' | b'%') {
        let back = stream.pos;
        let caret = stream.eat(|byte| byte == b'^').is_some() && stream.eat(|byte| byte.is_ascii_uppercase()).is_some();
        if (caret || (!b"@$%&".contains(&before) && stream.eat(|byte| b"=|\\-#?@;:&`~^![]*'\"$+.,/<>()".contains(&byte)).is_some()))
            && class(stream.current()).is_some()
        {
            return Some(Kind::Variable);
        }
        stream.pos = back;
    }
    if matches!(here, b'$' | b'@' | b'%' | b'&') {
        let back = stream.pos;
        let named = stream.eat_while(|byte| word(byte) || byte == b'$')
            || (stream.eat(|byte| byte == b'{').is_some()
                && stream.eat_while(|byte| word(byte) || byte == b'$')
                && stream.eat(|byte| byte == b'}').is_some());
        if named {
            return Some(Kind::Variable);
        }
        stream.pos = back;
    }
    if here == b'#' && before != b'$' {
        stream.pos = bytes.len();
        return Some(Kind::Comment);
    }
    let operator = |byte: u8| b":+-^*$&%@=<>!?|/~.".contains(&byte);
    if operator(here) {
        let back = stream.pos;
        stream.eat_while(operator);
        if class(stream.current()) == Some(4) {
            return Some(Kind::Operator);
        }
        stream.pos = back;
    }
    if here == b'_' && stream.pos == 1 {
        let rest = bytes.get(1..).unwrap_or_default();
        if rest.starts_with(b"_END__") {
            return chain(stream, perl, vec![0], Some(Kind::Comment), false);
        }
        if rest.starts_with(b"_DATA__") {
            return chain(stream, perl, vec![0], Some(Kind::Variable), false);
        }
    }
    if word(here) {
        let back = stream.pos;
        if before == b'{' && (stream.look(0) == b'}' || (stream.eat_while(word) && stream.look(0) == b'}')) {
            return Some(Kind::String);
        }
        stream.pos = back;
    }
    if here.is_ascii_uppercase() {
        let back = stream.pos;
        stream.eat_while(|byte| byte.is_ascii_uppercase() || byte == b'_');
        let after = stream.look(0);
        if !(after.is_ascii_digit() || after.is_ascii_lowercase()) {
            return if before == b':' { Some(Kind::Comment) } else { style(class(stream.current())) };
        }
        stream.pos = back;
    }
    if here.is_ascii_alphabetic() || here == b'_' {
        stream.eat_while(word);
        return if before == b':' { Some(Kind::Comment) } else { style(class(stream.current())) };
    }
    None
}
