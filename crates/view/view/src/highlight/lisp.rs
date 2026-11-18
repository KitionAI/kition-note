//! Lisps, as CodeMirror 5's modes color them: Clojure (its special forms and core functions
//! keywords, the name after `(` a builtin, `:keyword` and `nil`/`true`/`false` atoms).

use super::Kind;
use super::scan::{self, Out, Stream};

const CLOJURE_SPECIAL: &str = ". catch def do if monitor-enter monitor-exit new quote recur set! throw try var";
const CLOJURE_CORE: &str = "* *' *1 *2 *3 *agent* *allow-unresolved-vars* *assert* *clojure-version* *command-line-args* *compile-files* \
*compile-path* *compiler-options* *data-readers* *default-data-reader-fn* *e *err* *file* *flush-on-newline* \
*fn-loader* *in* *math-context* *ns* *out* *print-dup* *print-length* *print-level* *print-meta* \
*print-namespace-maps* *print-readably* *read-eval* *reader-resolver* *source-path* *suppress-read* \
*unchecked-math* *use-context-classloader* *verbose-defrecords* *warn-on-reflection* + +' - -' -> ->> \
->ArrayChunk ->Eduction ->Vec ->VecNode ->VecSeq -cache-protocol-fn -reset-methods .. / < <= = == > >= \
EMPTY-NODE Inst StackTraceElement->vec Throwable->map accessor aclone add-classpath add-watch agent agent-error \
agent-errors aget alength alias all-ns alter alter-meta! alter-var-root amap ancestors and any? apply areduce \
array-map as-> aset aset-boolean aset-byte aset-char aset-double aset-float aset-int aset-long aset-short \
assert assoc assoc! assoc-in associative? atom await await-for await1 bases bean bigdec bigint biginteger \
binding bit-and bit-and-not bit-clear bit-flip bit-not bit-or bit-set bit-shift-left bit-shift-right bit-test \
bit-xor boolean boolean-array boolean? booleans bound-fn bound-fn* bound? bounded-count butlast byte byte-array \
bytes bytes? case cast cat char char-array char-escape-string char-name-string char? chars chunk chunk-append \
chunk-buffer chunk-cons chunk-first chunk-next chunk-rest chunked-seq? class class? clear-agent-errors \
clojure-version coll? comment commute comp comparator compare compare-and-set! compile complement completing \
concat cond cond-> cond->> condp conj conj! cons constantly construct-proxy contains? count counted? create-ns \
create-struct cycle dec dec' decimal? declare dedupe default-data-readers definline definterface defmacro \
defmethod defmulti defn defn- defonce defprotocol defrecord defstruct deftype delay delay? deliver denominator \
deref derive descendants destructure disj disj! dissoc dissoc! distinct distinct? doall dorun doseq dosync \
dotimes doto double double-array double? doubles drop drop-last drop-while eduction empty empty? ensure \
ensure-reduced enumeration-seq error-handler error-mode eval even? every-pred every? ex-data ex-info extend \
extend-protocol extend-type extenders extends? false? ffirst file-seq filter filterv find find-keyword find-ns \
find-protocol-impl find-protocol-method find-var first flatten float float-array float? floats flush fn fn? \
fnext fnil for force format frequencies future future-call future-cancel future-cancelled? future-done? future? \
gen-class gen-interface gensym get get-in get-method get-proxy-class get-thread-bindings get-validator group-by \
halt-when hash hash-combine hash-map hash-ordered-coll hash-set hash-unordered-coll ident? identical? identity \
if-let if-not if-some ifn? import in-ns inc inc' indexed? init-proxy inst-ms inst-ms* inst? instance? int \
int-array int? integer? interleave intern interpose into into-array ints io! isa? iterate iterator-seq juxt \
keep keep-indexed key keys keyword keyword? last lazy-cat lazy-seq let letfn line-seq list list* list? load \
load-file load-reader load-string loaded-libs locking long long-array longs loop macroexpand macroexpand-1 \
make-array make-hierarchy map map-entry? map-indexed map? mapcat mapv max max-key memfn memoize merge \
merge-with meta method-sig methods min min-key mix-collection-hash mod munge name namespace namespace-munge \
nat-int? neg-int? neg? newline next nfirst nil? nnext not not-any? not-empty not-every? not= ns ns-aliases \
ns-imports ns-interns ns-map ns-name ns-publics ns-refers ns-resolve ns-unalias ns-unmap nth nthnext nthrest \
num number? numerator object-array odd? or parents partial partition partition-all partition-by pcalls peek \
persistent! pmap pop pop! pop-thread-bindings pos-int? pos? pr pr-str prefer-method prefers \
primitives-classnames print print-ctor print-dup print-method print-simple print-str printf println println-str \
prn prn-str promise proxy proxy-call-with-super proxy-mappings proxy-name proxy-super push-thread-bindings \
pvalues qualified-ident? qualified-keyword? qualified-symbol? quot rand rand-int rand-nth random-sample range \
ratio? rational? rationalize re-find re-groups re-matcher re-matches re-pattern re-seq read read-line \
read-string reader-conditional reader-conditional? realized? record? reduce reduce-kv reduced reduced? \
reductions ref ref-history-count ref-max-history ref-min-history ref-set refer refer-clojure reify \
release-pending-sends rem remove remove-all-methods remove-method remove-ns remove-watch repeat repeatedly \
replace replicate require reset! reset-meta! reset-vals! resolve rest restart-agent resultset-seq reverse \
reversible? rseq rsubseq run! satisfies? second select-keys send send-off send-via seq seq? seqable? seque \
sequence sequential? set set-agent-send-executor! set-agent-send-off-executor! set-error-handler! \
set-error-mode! set-validator! set? short short-array shorts shuffle shutdown-agents simple-ident? \
simple-keyword? simple-symbol? slurp some some-> some->> some-fn some? sort sort-by sorted-map sorted-map-by \
sorted-set sorted-set-by sorted? special-symbol? spit split-at split-with str string? struct struct-map subs \
subseq subvec supers swap! swap-vals! symbol symbol? sync tagged-literal tagged-literal? take take-last \
take-nth take-while test the-ns thread-bound? time to-array to-array-2d trampoline transduce transient tree-seq \
true? type unchecked-add unchecked-add-int unchecked-byte unchecked-char unchecked-dec unchecked-dec-int \
unchecked-divide-int unchecked-double unchecked-float unchecked-inc unchecked-inc-int unchecked-int \
unchecked-long unchecked-multiply unchecked-multiply-int unchecked-negate unchecked-negate-int \
unchecked-remainder-int unchecked-short unchecked-subtract unchecked-subtract-int underive unquote \
unquote-splicing unreduced unsigned-bit-shift-right update update-in update-proxy uri? use uuid? val vals \
var-get var-set var? vary-meta vec vector vector-of vector? volatile! volatile? vreset! vswap! when when-first \
when-let when-not when-some while with-bindings with-bindings* with-in-str with-loading-context with-local-vars \
with-meta with-open with-out-str with-precision with-redefs with-redefs-fn xml-seq zero? zipmap";

/// What carries between lines: an open string or `(comment …)` form, and the last token's text.
#[derive(Clone, Debug, Default)]
pub struct Lisp {
    string: bool,
    comment: bool,
    last: String,
}

/// Clojure's delimiters: what ends a symbol or a number (`#` and `'` end a number too).
fn delimiter(byte: u8, number: bool) -> bool {
    byte == 0 || b"\\[] \t\r\"(),;@^`{}~".contains(&byte) || (number && (byte == b'#' || byte == b'\''))
}

/// Where a Clojure number at `at` ends, if one is there (followed by a delimiter).
fn clojure_number(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let ends = |end: usize| delimiter(byte(end), true).then_some(end);
    let digits = |from: usize| (from..).find(|index| !byte(*index).is_ascii_digit()).unwrap_or(from);
    let exponent = |from: usize| {
        let mut end = from;
        if matches!(byte(end), b'e' | b'E') {
            end += 1;
            if matches!(byte(end), b'+' | b'-') {
                end += 1;
            }
            let after = digits(end);
            if after > end {
                return Some(after);
            }
        }
        None
    };
    let signed = if matches!(byte(at), b'+' | b'-') { at + 1 } else { at };
    let whole = digits(signed);
    if whole == signed {
        return None;
    }
    // The suffixes in the mode's order: `N`, an exponent, a fraction (`M` or an exponent after
    // it), a ratio, hex, a radix; else the digits alone.
    if byte(whole) == b'N'
        && let Some(end) = ends(whole + 1)
    {
        return Some(end);
    }
    if let Some(end) = exponent(whole).and_then(ends) {
        return Some(end);
    }
    let dots: &[usize] = if byte(whole) == b'.' { &[1, 0] } else { &[0] };
    for dot in dots {
        let from = whole + dot;
        let most = digits(from);
        for fraction in (from..=most).rev() {
            if byte(fraction) == b'M'
                && let Some(end) = ends(fraction + 1)
            {
                return Some(end);
            }
            if let Some(end) = exponent(fraction).and_then(ends) {
                return Some(end);
            }
            if let Some(end) = ends(fraction) {
                return Some(end);
            }
        }
    }
    if byte(whole) == b'/' && digits(whole + 1) > whole + 1 {
        return ends(digits(whole + 1));
    }
    let run = |from: usize, ok: fn(u8) -> bool| (from..).find(|index| !ok(byte(*index))).unwrap_or(from);
    if matches!(byte(whole), b'x' | b'X') && run(whole + 1, |byte| byte.is_ascii_hexdigit()) > whole + 1 {
        return ends(run(whole + 1, |byte| byte.is_ascii_hexdigit()));
    }
    if byte(whole) == b'r' && run(whole + 1, |byte| byte.is_ascii_alphanumeric()) > whole + 1 {
        return ends(run(whole + 1, |byte| byte.is_ascii_alphanumeric()));
    }
    None
}

/// Where a character literal (`\a`, `\newline`) at `at` ends, if one is there.
fn clojure_character(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let ends = |end: usize| delimiter(byte(end), false).then_some(end);
    let rest = bytes.get(at + 1..).unwrap_or_default();
    for name in ["backspace", "formfeed", "newline", "return", "space", "tab"] {
        if rest.starts_with(name.as_bytes())
            && let Some(end) = ends(at + 1 + name.len())
        {
            return Some(end);
        }
    }
    let run = |ok: fn(u8) -> bool, count: usize| rest.get(1..=count).is_some_and(|run| run.iter().all(|byte| ok(*byte)));
    if byte(at + 1) == b'o'
        && run(|byte| (b'0'..=b'7').contains(&byte), 3)
        && let Some(end) = ends(at + 5)
    {
        return Some(end);
    }
    if matches!(byte(at + 1), b'u' | b'x')
        && run(|byte| byte.is_ascii_hexdigit(), 4)
        && let Some(end) = ends(at + 6)
    {
        return Some(end);
    }
    // Any one character (a whole UTF-8 one), or none.
    let mut one = at + 1;
    if one < bytes.len() {
        one += 1;
        while (0x80..0xC0).contains(&byte(one)) {
            one += 1;
        }
        if let Some(end) = ends(one) {
            return Some(end);
        }
    }
    ends(at + 1)
}

/// Clojure, as CodeMirror 5's clojure mode colors it.
pub fn clojure(text: &str, state: &mut Lisp, out: &mut Out) {
    scan::run(text, out, |stream| {
        let (space, style) = if state.string {
            (false, clojure_string(stream, state))
        } else if state.comment {
            (true, clojure_comment_form(stream, state))
        } else {
            clojure_token(stream, state)
        };
        // Spaces, commas, and comments are not the last token.
        if !space {
            state.last = stream.current().to_string();
        }
        style
    });
}

fn clojure_string(stream: &mut Stream, state: &mut Lisp) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            state.string = false;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

/// A `(comment …)` form, to its closing parenthesis (counted from this line's start).
fn clojure_comment_form(stream: &mut Stream, state: &mut Lisp) -> Option<Kind> {
    let mut depth = 1;
    while let Some(here) = stream.next() {
        if here == b')' {
            depth -= 1;
        }
        if here == b'(' {
            depth += 1;
        }
        if depth == 0 {
            stream.back_up(1);
            state.comment = false;
            break;
        }
    }
    Some(Kind::Comment)
}

fn clojure_token(stream: &mut Stream, state: &mut Lisp) -> (bool, Option<Kind>) {
    if stream.eat_space() || stream.eat(|byte| byte == b',').is_some() {
        return (true, None);
    }
    let bytes = stream.bytes;
    if let Some(end) = clojure_number(bytes, stream.pos) {
        stream.pos = end;
        return (false, Some(Kind::Number));
    }
    if stream.peek() == b'\\'
        && let Some(end) = clojure_character(bytes, stream.pos)
    {
        stream.pos = end;
        return (false, Some(Kind::String));
    }
    let here = stream.peek();
    if here == b'"' {
        stream.pos += 1;
        state.string = true;
        return (false, clojure_string(stream, state));
    }
    if b"([{".contains(&here) || b")]}".contains(&here) {
        stream.pos += 1;
        return (false, Some(Kind::Punctuation));
    }
    if here == b';' {
        stream.skip_to_end();
        return (true, Some(Kind::Comment));
    }
    if b"#'@^`~".contains(&here) {
        stream.pos += 1;
        return (false, Some(Kind::Comment));
    }
    // A symbol: to a delimiter, not starting with a digit, `#`, or `'`.
    if !here.is_ascii_digit() && here != b'#' && here != b'\'' && !delimiter(here, false) {
        stream.eat_while(|byte| !delimiter(byte, false));
        let symbol = stream.current();
        let after_open = state.last == "(";
        if symbol == "comment" && after_open {
            state.comment = true;
            return (true, clojure_comment_form(stream, state));
        }
        let known = |list: &str| list.split_ascii_whitespace().any(|known| known == symbol);
        let style = if matches!(symbol, "false" | "nil" | "true") || symbol.starts_with(':') {
            None
        } else if known(CLOJURE_SPECIAL) || known(CLOJURE_CORE) {
            Some(Kind::Keyword)
        } else if after_open {
            Some(Kind::Property)
        } else {
            Some(Kind::Variable)
        };
        return (false, style);
    }
    // The mode's error: to the next delimiter.
    stream.pos += 1;
    stream.eat_while(|byte| !delimiter(byte, false));
    (false, Some(Kind::Operator))
}

const SCHEME_BUILTINS: &str = "case-lambda call/cc class cond-expand define-class define-values exit-handler field import inherit init-field \
interface let*-values let-values let/ec mixin opt-lambda override protect provide public rename require \
require-for-syntax syntax syntax-case syntax-error unit/sig unless when with-syntax and begin \
call-with-current-continuation call-with-input-file call-with-output-file case cond define define-syntax \
define-macro defmacro delay do dynamic-wind else for-each if lambda let let* let-syntax letrec letrec-syntax \
map or syntax-rules abs acos angle append apply asin assoc assq assv atan boolean? caar cadr \
call-with-input-file call-with-output-file call-with-values car cdddar cddddr cdr ceiling char->integer \
char-alphabetic? char-ci<=? char-ci<? char-ci=? char-ci>=? char-ci>? char-downcase char-lower-case? \
char-numeric? char-ready? char-upcase char-upper-case? char-whitespace? char<=? char<? char=? char>=? char>? \
char? close-input-port close-output-port complex? cons cos current-input-port current-output-port denominator \
display eof-object? eq? equal? eqv? eval even? exact->inexact exact? exp expt #f floor force gcd imag-part \
inexact->exact inexact? input-port? integer->char integer? interaction-environment lcm length list list->string \
list->vector list-ref list-tail list? load log magnitude make-polar make-rectangular make-string make-vector \
max member memq memv min modulo negative? newline not null-environment null? number->string number? numerator \
odd? open-input-file open-output-file output-port? pair? peek-char port? positive? procedure? quasiquote quote \
quotient rational? rationalize read read-char real-part real? remainder reverse round scheme-report-environment \
set! set-car! set-cdr! sin sqrt string string->list string->number string->symbol string-append string-ci<=? \
string-ci<? string-ci=? string-ci>=? string-ci>? string-copy string-fill! string-length string-ref string-set! \
string<=? string<? string=? string>=? string>? string? substring symbol->string symbol? #t tan transcript-off \
transcript-on truncate values vector vector->list vector-fill! vector-length vector-ref vector-set! \
with-input-from-file with-output-to-file write write-char zero?";
/// Scheme's `λ`, a builtin too (a Greek letter, kept out of the word list).
const SCHEME_LAMBDA: &str = "\u{3bb}";

/// What Scheme reads next: its code, a string, a `|symbol|`, a `#| |#` comment, or the form after `#;`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SchemeMode {
    #[default]
    Code,
    String,
    Symbol,
    Comment,
    FormComment,
}

#[derive(Clone, Debug, Default)]
pub struct Scheme {
    mode: SchemeMode,
    /// The open brackets.
    brackets: Vec<u8>,
    /// Inside a `#;` commented form, or a `'` quoted one: its open brackets.
    form_comment: Option<usize>,
    form_quote: Option<usize>,
}

fn scheme_symbol(byte: u8) -> bool {
    scan::word(byte) || b"_-!$%&*+./:<=>?@^~".contains(&byte)
}

fn scheme_ends(bytes: &[u8], at: usize) -> bool {
    bytes.get(at).is_none_or(|byte| b"() \t\r;\"".contains(byte))
}

/// The ends a real number without its sign can have at `at` (the mode's alternatives, in order).
fn scheme_ureal(bytes: &[u8], at: usize) -> Vec<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let run = |from: usize, ok: fn(u8) -> bool| (from..).find(|index| !ok(byte(*index))).unwrap_or(from);
    let digits = |from: usize| run(from, |byte| byte.is_ascii_digit());
    let hashes = |from: usize| run(from, |byte| byte == b'#');
    let mut decimals = Vec::new();
    let whole = digits(at);
    if whole > at && byte(whole) == b'#' {
        let mut end = hashes(whole);
        if byte(end) == b'.' {
            end = hashes(end + 1);
        }
        decimals.push(end);
    }
    if whole > at && byte(whole) == b'.' {
        decimals.push(hashes(digits(whole + 1)));
    }
    if byte(at) == b'.' && digits(at + 1) > at + 1 {
        decimals.push(hashes(digits(at + 1)));
    }
    if whole > at {
        decimals.push(whole);
    }
    let mut ends = Vec::new();
    for end in decimals {
        // An exponent: `[esfdl][-+]?\d+`.
        if b"esfdlESFDL".contains(&byte(end)) {
            let sign = if matches!(byte(end + 1), b'-' | b'+') { end + 2 } else { end + 1 };
            if digits(sign) > sign {
                ends.push(digits(sign));
            }
        }
        ends.push(end);
    }
    if whole > at {
        let over = hashes(whole);
        if byte(over) == b'/' && digits(over + 1) > over + 1 {
            ends.push(hashes(digits(over + 1)));
        }
    }
    ends
}

/// The ends an unsigned integer in a radix can have at `at` (`D+#*`, a ratio `/D+#*`).
fn scheme_uinteger(bytes: &[u8], at: usize, digit: fn(u8) -> bool) -> Vec<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let run = |from: usize, ok: &dyn Fn(u8) -> bool| (from..).find(|index| !ok(byte(*index))).unwrap_or(from);
    let whole = run(at, &|byte| digit(byte));
    if whole == at {
        return Vec::new();
    }
    let over = run(whole, &|byte| byte == b'#');
    let mut ends = Vec::new();
    if byte(over) == b'/' && run(over + 1, &|byte| digit(byte)) > over + 1 {
        ends.push(run(run(over + 1, &|byte| digit(byte)), &|byte| byte == b'#'));
    }
    ends.push(over);
    ends
}

/// A number at `at` in the mode's complex forms (`1`, `1/2`, `+i`, `1+2i`, `1@2`), read with
/// `real` for its parts; `signed` lets the plain form take a sign. Its end when one is there.
fn scheme_number(bytes: &[u8], at: usize, real: &dyn Fn(usize) -> Vec<usize>, signed: bool) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let sign = |at: usize| matches!(byte(at), b'+' | b'-');
    let i = |end: usize| (byte(end) == b'i' || byte(end) == b'I').then_some(end + 1).filter(|end| scheme_ends(bytes, *end));
    // `[-+]i`
    if sign(at)
        && let Some(end) = i(at + 1)
    {
        return Some(end);
    }
    // `[-+]real i`
    if sign(at) {
        for end in real(at + 1) {
            if let Some(end) = i(end) {
                return Some(end);
            }
        }
    }
    let from = if sign(at) { at + 1 } else { at };
    // `[-+]?real @ [-+]?real`
    for end in real(from) {
        if byte(end) == b'@' {
            let next = if sign(end + 1) { end + 2 } else { end + 1 };
            if let Some(found) = real(next).into_iter().find(|end| scheme_ends(bytes, *end)) {
                return Some(found);
            }
        }
    }
    // `[-+]?real [-+] real? i`
    for end in real(from) {
        if sign(end) {
            for imaginary in real(end + 1) {
                if let Some(found) = i(imaginary) {
                    return Some(found);
                }
            }
            if let Some(found) = i(end + 1) {
                return Some(found);
            }
        }
    }
    let plain = if signed { from } else { at };
    real(plain).into_iter().find(|end| scheme_ends(bytes, *end))
}

/// Scheme, as CodeMirror 5's scheme mode colors it: its builtins (a form's name or not),
/// variables, numbers in every radix and form, strings, `#t` and `'quoted` atoms, `;`, `#| |#`, and
/// `#;` comments.
pub fn scheme(text: &str, state: &mut Scheme, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        let style = scheme_token(stream, state);
        if state.form_comment.is_some() {
            Some(Kind::Comment)
        } else if state.form_quote.is_some() {
            None
        } else {
            style
        }
    });
}

fn scheme_token(stream: &mut Stream, state: &mut Scheme) -> Option<Kind> {
    let bytes = stream.bytes;
    let read_to = |stream: &mut Stream, state: &mut Scheme, close: u8| {
        let mut escaped = false;
        while let Some(here) = stream.next() {
            if here == close && !escaped {
                state.mode = SchemeMode::Code;
                break;
            }
            escaped = !escaped && here == b'\\';
        }
    };
    match state.mode {
        SchemeMode::String => {
            read_to(stream, state, b'"');
            return Some(Kind::String);
        }
        SchemeMode::Symbol => {
            read_to(stream, state, b'|');
            return None;
        }
        SchemeMode::Comment => {
            let mut bar = false;
            while let Some(here) = stream.next() {
                if here == b'#' && bar {
                    state.mode = SchemeMode::Code;
                    break;
                }
                bar = here == b'|';
            }
            return Some(Kind::Comment);
        }
        SchemeMode::FormComment => {
            state.mode = SchemeMode::Code;
            if stream.peek() != b'(' && stream.peek() != b'[' {
                stream.eat_while(|byte| !b" \t\r()[]".contains(&byte));
                return Some(Kind::Comment);
            }
            state.form_comment = Some(0);
        }
        SchemeMode::Code => {}
    }
    let here = stream.next()?;
    let decimal = |at: usize| scheme_ureal(bytes, at);
    match here {
        b'"' => {
            state.mode = SchemeMode::String;
            Some(Kind::String)
        }
        b'\'' => {
            if stream.peek() == b'(' || stream.peek() == b'[' {
                if state.form_quote.is_none() {
                    state.form_quote = Some(0);
                }
            } else {
                stream.eat_while(scheme_symbol);
            }
            None
        }
        b'|' => {
            state.mode = SchemeMode::Symbol;
            None
        }
        b'#' => {
            if stream.eat(|byte| byte == b'|').is_some() {
                state.mode = SchemeMode::Comment;
                return Some(Kind::Comment);
            }
            if stream.eat(|byte| matches!(byte, b't' | b'f' | b'T' | b'F')).is_some() {
                return None;
            }
            if stream.eat(|byte| byte == b';').is_some() {
                state.mode = SchemeMode::FormComment;
                return Some(Kind::Comment);
            }
            let exactness = stream.eat(|byte| matches!(byte, b'e' | b'i' | b'E' | b'I')).is_some();
            if !exactness {
                stream.back_up(1);
            }
            let radix: Option<fn(u8) -> bool> = if stream.at("#b") || stream.at("#B") {
                Some(|byte| byte == b'0' || byte == b'1')
            } else if stream.at("#o") || stream.at("#O") {
                Some(|byte| (b'0'..=b'7').contains(&byte))
            } else if stream.at("#x") || stream.at("#X") {
                Some(|byte| byte.is_ascii_hexdigit())
            } else {
                None
            };
            let decimal_prefix = stream.at("#d") || stream.at("#D");
            let mut read = false;
            if radix.is_some() || decimal_prefix {
                stream.pos += 2;
                read = true;
            }
            let mut prefixed = true;
            if !read && matches!(stream.peek(), b'-' | b'+' | b'0'..=b'9' | b'.') {
                prefixed = false;
                read = true;
            } else if !read && !exactness {
                stream.eat(|byte| byte == b'#');
            }
            if !read {
                return None;
            }
            if prefixed && !exactness && stream.peek() == b'#' && matches!(stream.look(1), b'e' | b'i' | b'E' | b'I') {
                stream.pos += 2;
            }
            let end = match radix {
                Some(digit) => scheme_number(bytes, stream.pos, &|at| scheme_uinteger(bytes, at, digit), true),
                None => scheme_number(bytes, stream.pos, &decimal, false),
            };
            if let Some(end) = end {
                stream.pos = end;
                return Some(Kind::Number);
            }
            None
        }
        b'-' | b'+' | b'0'..=b'9' | b'.' if scheme_number(bytes, stream.pos - 1, &decimal, false).is_some() => {
            stream.pos = scheme_number(bytes, stream.pos - 1, &decimal, false).unwrap_or(stream.pos);
            Some(Kind::Number)
        }
        b';' => {
            stream.skip_to_end();
            Some(Kind::Comment)
        }
        b'(' | b'[' => {
            state.brackets.push(here);
            if let Some(depth) = state.form_comment.as_mut() {
                *depth += 1;
            }
            if let Some(depth) = state.form_quote.as_mut() {
                *depth += 1;
            }
            Some(Kind::Punctuation)
        }
        b')' | b']' => {
            let open = if here == b')' { b'(' } else { b'[' };
            if state.brackets.last() == Some(&open) {
                state.brackets.pop();
                if let Some(depth) = state.form_comment.as_mut() {
                    *depth -= 1;
                    if *depth == 0 {
                        state.form_comment = None;
                        return Some(Kind::Comment);
                    }
                }
                if let Some(depth) = state.form_quote.as_mut() {
                    *depth -= 1;
                    if *depth == 0 {
                        state.form_quote = None;
                        return None;
                    }
                }
            }
            Some(Kind::Punctuation)
        }
        _ => {
            stream.eat_while(scheme_symbol);
            let symbol = stream.current();
            if symbol == SCHEME_LAMBDA || SCHEME_BUILTINS.split_ascii_whitespace().any(|known| known == symbol) {
                Some(Kind::Property)
            } else {
                Some(Kind::Variable)
            }
        }
    }
}

const LISP_SPECIAL: &[&str] = &[
    "block",
    "return-from",
    "catch",
    "load-time-value",
    "setq",
    "eval-when",
    "locally",
    "symbol-macrolet",
    "flet",
    "macrolet",
    "tagbody",
    "function",
    "multiple-value-call",
    "the",
    "go",
    "multiple-value-prog1",
    "throw",
    "if",
    "progn",
    "unwind-protect",
    "labels",
    "progv",
    "let",
    "quote",
];

/// Whether a symbol after `(` is a keyword: a special form, or a name like `defun`, `with-…`,
/// `…-case`, `when` (the mode's patterns; its `let*` reads as `le` and any `t`s).
fn lisp_keyword(symbol: &str) -> bool {
    let let_star = symbol.strip_prefix("le").is_some_and(|rest| rest.bytes().all(|byte| byte == b't'));
    LISP_SPECIAL.contains(&symbol)
        || let_star
        || ["with", "def", "do", "prog"].iter().any(|prefix| symbol.starts_with(prefix))
        || ["case", "bind", "when", "unless"].iter().any(|suffix| symbol.ends_with(suffix))
        || symbol == "cond"
}

#[derive(Clone, Debug, Default)]
pub struct CommonLisp {
    string: bool,
    comment: bool,
    /// The last token's type: `open`, `close`, `symbol`, or none.
    last: &'static str,
}

/// Common Lisp, as CodeMirror 5's commonlisp mode colors it: a form's special operator or
/// definer a keyword, `nil`, `t`, and `:keywords` atoms, `&optional` a variable-2, numbers,
/// strings, `#\c` characters, `;` and `#| |#` comments.
pub fn common_lisp(text: &str, state: &mut CommonLisp, out: &mut Out) {
    scan::run(text, out, |stream| {
        let mut kind = "";
        let style = if state.string {
            lisp_string(stream, state)
        } else if state.comment {
            kind = "ws";
            lisp_comment(stream, state)
        } else {
            lisp_token(stream, state, &mut kind)
        };
        if kind != "ws" {
            state.last = kind;
        }
        style
    });
}

fn lisp_string(stream: &mut Stream, state: &mut CommonLisp) -> Option<Kind> {
    state.string = true;
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == b'"' && !escaped {
            state.string = false;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn lisp_comment(stream: &mut Stream, state: &mut CommonLisp) -> Option<Kind> {
    state.comment = true;
    let mut previous = 0;
    while let Some(here) = stream.next() {
        if here == b'#' && previous == b'|' {
            state.comment = false;
            break;
        }
        previous = here;
    }
    Some(Kind::Comment)
}

/// A symbol's rest (a backslash escaping the next byte).
fn lisp_symbol(stream: &mut Stream) {
    while let Some(here) = stream.next() {
        if here == b'\\' {
            stream.next();
        } else if b" \t\r'`,@()[]\";".contains(&here) {
            stream.back_up(1);
            break;
        }
    }
}

/// A number at `at`: `[+-]?(\d+|\d*\.\d+)([efd][+-]?\d+)?`, a ratio, or `#b`, `#o`, `#x` digits.
fn lisp_number(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = |index: usize| bytes.get(index).copied().unwrap_or(0);
    let run = |from: usize, ok: fn(u8) -> bool| (from..).find(|index| !ok(byte(*index))).unwrap_or(from);
    let digits = |from: usize| run(from, |byte| byte.is_ascii_digit());
    let signed = if matches!(byte(at), b'+' | b'-') { at + 1 } else { at };
    let exponent = |end: usize| {
        if matches!(byte(end), b'e' | b'f' | b'd') {
            let sign = if matches!(byte(end + 1), b'+' | b'-') { end + 2 } else { end + 1 };
            if digits(sign) > sign {
                return digits(sign);
            }
        }
        end
    };
    if digits(signed) > signed {
        return Some(exponent(digits(signed)));
    }
    if byte(signed) == b'.' && digits(signed + 1) > signed + 1 {
        return Some(exponent(digits(signed + 1)));
    }
    if byte(at) == b'#' {
        let digit: Option<fn(u8) -> bool> = match byte(at + 1) {
            b'b' => Some(|byte| byte == b'0' || byte == b'1'),
            b'o' => Some(|byte| (b'0'..=b'7').contains(&byte)),
            b'x' => Some(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            _ => None,
        };
        if let Some(digit) = digit {
            let from = if matches!(byte(at + 2), b'+' | b'-') { at + 3 } else { at + 2 };
            let end = run(from, digit);
            if end > from {
                return Some(end);
            }
        }
    }
    None
}

fn lisp_token(stream: &mut Stream, state: &mut CommonLisp, kind: &mut &'static str) -> Option<Kind> {
    if stream.eat_space() {
        *kind = "ws";
        return None;
    }
    if let Some(end) = lisp_number(stream.bytes, stream.pos) {
        stream.pos = end;
        return Some(Kind::Number);
    }
    let mut here = stream.next()?;
    if here == b'\\' {
        here = stream.next().unwrap_or(0);
    }
    match here {
        b'"' => return lisp_string(stream, state),
        b'(' => {
            *kind = "open";
            return Some(Kind::Punctuation);
        }
        b')' | b']' => {
            *kind = "close";
            return Some(Kind::Punctuation);
        }
        b';' => {
            stream.skip_to_end();
            *kind = "ws";
            return Some(Kind::Comment);
        }
        b'\'' | b'`' | b',' | b'@' => return None,
        b'|' => {
            let rest = stream.bytes.get(stream.pos..).unwrap_or_default();
            return match rest.iter().position(|byte| *byte == b'|') {
                Some(found) => {
                    stream.pos += found + 1;
                    None
                }
                None => {
                    stream.skip_to_end();
                    Some(Kind::Operator)
                }
            };
        }
        b'#' => {
            let next = stream.next().unwrap_or(0);
            return match next {
                b'(' => {
                    *kind = "open";
                    Some(Kind::Punctuation)
                }
                b'+' | b'-' | b'=' | b'.' | b'\'' => None,
                b'0'..=b'9' if stream.bytes.get(stream.pos..).is_some_and(|rest| rest.iter().find(|byte| !byte.is_ascii_digit()) == Some(&b'#')) => {
                    stream.eat_while(|byte| byte.is_ascii_digit());
                    stream.pos += 1;
                    None
                }
                b'|' => {
                    *kind = "ws";
                    lisp_comment(stream, state)
                }
                b':' => {
                    lisp_symbol(stream);
                    Some(Kind::Comment)
                }
                b'\\' => {
                    stream.next();
                    lisp_symbol(stream);
                    Some(Kind::String)
                }
                _ => Some(Kind::Operator),
            };
        }
        _ => {}
    }
    lisp_symbol(stream);
    let symbol = stream.current();
    if symbol == "." {
        return None;
    }
    *kind = "symbol";
    if symbol == "nil" || symbol == "t" || symbol.starts_with(':') {
        None
    } else if state.last == "open" && lisp_keyword(symbol) {
        Some(Kind::Keyword)
    } else {
        Some(Kind::Variable)
    }
}
