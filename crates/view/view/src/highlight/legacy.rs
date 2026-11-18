//! Data and business languages, as their CodeMirror 5 modes color them, each ported token for
//! token: COBOL and SAS (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Legacy {
    /// COBOL: inside a string.
    string: bool,
    /// SAS: where it is (a data step, a procedure, a macro), what it waits for.
    data_step: bool,
    proc: bool,
    macro_: bool,
    next_word: bool,
    sas_string: Option<u8>,
    comment: bool,
}

fn listed(list: &str, word: &str) -> bool {
    list.split(' ').any(|known| known == word)
}

const COBOL_BUILTINS: &str = "- * ** / + < <= = > >=";

/// COBOL, as CodeMirror 5's cobol mode colors it: the sequence area (columns 1 to 6) plain, the
/// identification area (73 to 80) a header, `*` in column 7 a comment, keywords and `TRUE`,
/// `ZERO`, … atoms (any case), operators builtins, numbers, strings, `.` a link.
pub fn cobol(text: &str, state: &mut Legacy, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if state.string {
            while let Some(here) = stream.next() {
                if (here == b'"' || here == b'\'') && !matches!(stream.peek(), b'"' | b'\'') {
                    state.string = false;
                    break;
                }
            }
            return Some(Kind::String);
        }
        let column = stream.pos;
        let here = stream.next()?;
        if column <= 5 {
            return None;
        }
        if (72..=79).contains(&column) {
            stream.skip_to_end();
            return None;
        }
        if here == b'*' && column == 6 {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if here == b'"' || here == b'\'' {
            state.string = true;
            return Some(Kind::String);
        }
        if here == b'\'' && !(stream.peek().is_ascii_digit() || stream.peek() == b':') {
            return None;
        }
        if here == b'.' {
            return Some(Kind::Link);
        }
        if cobol_number(stream, here) {
            return Some(Kind::Number);
        }
        let symbol = |byte: u8| scan::js_word(byte) || b"*+-".contains(&byte);
        if symbol(here) {
            let mut column = column;
            while column < 71 && stream.eat(symbol).is_some() {
                column += 1;
            }
        }
        let word = stream.current().to_ascii_uppercase();
        if listed(COBOL_KEYWORDS, &word) {
            Some(Kind::Keyword)
        } else if listed(COBOL_BUILTINS, &word) {
            Some(Kind::Property)
        } else {
            // `TRUE`, `ZERO`, … atoms, and other words: plain.
            None
        }
    });
}

/// The mode's number test after `here`: `0x…`, or a sign before a digit, digits, a fraction, an
/// exponent (taking what it reads).
fn cobol_number(stream: &mut Stream, here: u8) -> bool {
    if here == b'0' && stream.eat(|byte| byte == b'x' || byte == b'X').is_some() {
        stream.eat_while(|byte| byte.is_ascii_hexdigit());
        return true;
    }
    let mut first = here;
    if (here == b'+' || here == b'-') && stream.peek().is_ascii_digit() {
        stream.eat(|byte| byte == b'+' || byte == b'-');
        first = stream.next().unwrap_or(0);
    }
    if !first.is_ascii_digit() {
        return false;
    }
    stream.eat(|byte| byte == first);
    stream.eat_while(|byte| byte.is_ascii_digit());
    if stream.peek() == b'.' {
        stream.next();
        stream.eat_while(|byte| byte.is_ascii_digit());
    }
    if stream.eat(|byte| byte == b'e' || byte == b'E').is_some() {
        stream.eat(|byte| byte == b'+' || byte == b'-');
        stream.eat_while(|byte| byte.is_ascii_digit());
    }
    true
}

const DATA: u8 = 1;
const PROC: u8 = 2;
const MACRO: u8 = 4;
const ALL: u8 = 8;

/// The SAS word table's entry for `word` (the last group that has it): its style and states.
fn sas_word(word: &str) -> Option<(&'static str, u8)> {
    SAS_GROUPS.iter().rev().find(|(_, words, _)| listed(words, word)).map(|(style, _, states)| (*style, *states))
}

fn sas_kind(style: &str) -> Option<Kind> {
    match style {
        "builtin" => Some(Kind::Property),
        "variable" | "variable-2" => Some(Kind::Variable),
        "operator" => Some(Kind::Operator),
        "comment" => Some(Kind::Comment),
        "string" => Some(Kind::String),
        "number" => Some(Kind::Number),
        _ => None,
    }
}

/// SAS, as CodeMirror 5's sas mode colors it: in a data step, a procedure, or a macro the words
/// each knows (statements plain, `data`, `proc`, `run` builtins), the name after `data` or `proc`
/// a variable, `&macro` variables, operators, numbers, strings, `/* */` and `*` comments.
pub fn sas(text: &str, state: &mut Legacy, out: &mut Out) {
    let indentation = text.bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count();
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        sas_token(stream, state, indentation).and_then(sas_kind)
    });
}

fn sas_token(stream: &mut Stream, state: &mut Legacy, indentation: usize) -> Option<&'static str> {
    let start = stream.pos;
    let here = stream.next()?;
    if here == b'/' && stream.eat(|byte| byte == b'*').is_some() {
        state.comment = true;
        return Some("comment");
    }
    if state.comment {
        if here == b'*' && stream.peek() == b'/' {
            stream.next();
            state.comment = false;
        } else if let Some(star) = stream.rest().iter().position(|byte| *byte == b'*') {
            stream.pos += star + 1;
            if stream.eat(|byte| byte == b'/').is_some() {
                state.comment = false;
            }
        } else {
            stream.skip_to_end();
        }
        return Some("comment");
    }
    if here == b'*' && start == indentation {
        stream.skip_to_end();
        return Some("comment");
    }
    let pair = [here, stream.peek()];
    if (here == b'"' || here == b'\'') && state.sas_string.is_none() {
        state.sas_string = Some(here);
        return Some("string");
    }
    if let Some(quote) = state.sas_string {
        if quote == here {
            state.sas_string = None;
        } else if let Some(close) = stream.rest().iter().position(|byte| *byte == quote) {
            stream.pos += close + 1;
            state.sas_string = None;
        } else {
            stream.skip_to_end();
        }
        return Some("string");
    }
    if here.is_ascii_digit() || here == b'.' {
        sas_number(stream, here);
        return Some("number");
    }
    if [b"<=", b">=", b"!=", b"<>"].iter().any(|operator| **operator == pair) {
        stream.next();
        return Some("operator");
    }
    if [b"eq", b"lt", b"le", b"gt", b"ge", b"in", b"ne", b"or"].iter().any(|operator| **operator == pair) {
        stream.next();
        if stream.peek() == b' ' {
            return Some("operator");
        }
    } else if b"=(:),{}.*<>+-/^[]".contains(&here) {
        return Some("operator");
    }
    // The word: the first byte and the run after the position (`n + match(...)`, which skips a
    // second byte the two-letter operators read).
    let word_byte = |byte: u8| scan::js_word(byte) || b"%&;".contains(&byte);
    let mut word = (here as char).to_string();
    if word_byte(stream.peek()) {
        let from = stream.pos;
        stream.eat_while(word_byte);
        word.push_str(&text_of(stream, from));
        if word.contains('&') {
            return Some("variable");
        }
    }
    if state.next_word {
        stream.eat_while(scan::js_word);
        if stream.peek() == b'.'
            && let Some(space) = stream.rest().iter().position(|byte| *byte == b' ')
        {
            stream.pos += space;
        }
        state.next_word = false;
        return Some("variable-2");
    }
    let word = word.to_ascii_lowercase();
    let entry = sas_word(&word);
    if state.data_step {
        if word == "run;" || run_semicolon(stream) {
            state.data_step = false;
            return Some("builtin");
        }
        if !word.is_empty() && stream.next() == Some(b'.') {
            return Some(if scan::js_word(stream.peek()) { "variable-2" } else { "variable" });
        }
        if let Some((style, _)) = entry.filter(|(_, states)| states & (DATA | ALL) != 0) {
            stream.pos = start + word.len();
            return Some(style);
        }
    }
    if state.proc {
        if word == "run;" || word == "quit;" {
            state.proc = false;
            return Some("builtin");
        }
        if let Some((style, _)) = entry.filter(|(_, states)| states & (PROC | ALL) != 0) {
            stream.eat_while(scan::js_word);
            return Some(style);
        }
    }
    if state.macro_ {
        if word == "%mend" {
            if stream.peek() == b';' {
                stream.next();
            }
            state.macro_ = false;
            return Some("builtin");
        }
        if let Some((style, _)) = entry.filter(|(_, states)| states & (MACRO | ALL) != 0) {
            stream.eat_while(scan::js_word);
            return Some(style);
        }
        return Some("atom");
    }
    let (style, states) = entry?;
    stream.back_up(1);
    stream.eat_while(scan::js_word);
    if word == "data" && stream.peek() != b'=' {
        state.data_step = true;
        state.next_word = true;
        return Some("builtin");
    }
    if word == "proc" {
        state.proc = true;
        state.next_word = true;
        return Some("builtin");
    }
    if word == "%macro" {
        state.macro_ = true;
        state.next_word = true;
        return Some("builtin");
    }
    if title_digit(&word) {
        return Some("def");
    }
    if word == "footnote" {
        stream.eat(|byte| (b'1'..=b'9').contains(&byte));
        return Some("def");
    }
    let applies =
        (state.data_step && states & DATA != 0) || (state.proc && states & PROC != 0) || (state.macro_ && states & MACRO != 0) || states & ALL != 0;
    if applies { Some(style) } else { None }
}

/// `/title[1-9]/` anywhere in the word.
fn title_digit(word: &str) -> bool {
    word.match_indices("title").any(|(at, _)| word.as_bytes().get(at + 5).is_some_and(|byte| (b'1'..=b'9').contains(byte)))
}

fn text_of(stream: &Stream, from: usize) -> String {
    String::from_utf8_lossy(stream.bytes.get(from..stream.pos).unwrap_or_default()).into_owned()
}

/// `run\s;` where the stream stands (taking it).
fn run_semicolon(stream: &mut Stream) -> bool {
    let rest = stream.rest();
    if rest.starts_with(b"run") && rest.get(3).is_some_and(u8::is_ascii_whitespace) && rest.get(4) == Some(&b';') {
        stream.pos += 5;
        return true;
    }
    false
}

fn sas_number(stream: &mut Stream, here: u8) {
    let digits = |stream: &mut Stream| {
        stream.eat_while(|byte| byte.is_ascii_digit());
    };
    let exponent = |stream: &mut Stream| {
        if matches!(stream.peek(), b'e' | b'E') {
            let sign = usize::from(matches!(stream.byte(stream.pos + 1), b'+' | b'-'));
            if stream.byte(stream.pos + 1 + sign).is_ascii_digit() {
                stream.pos += 1 + sign;
                stream.eat_while(|byte| byte.is_ascii_digit());
            }
        }
    };
    match here {
        b'.' => {
            if stream.peek().is_ascii_digit() {
                digits(stream);
                exponent(stream);
            }
        }
        b'0' if matches!(stream.peek(), b'x' | b'X') && stream.byte(stream.pos + 1).is_ascii_hexdigit() => {
            stream.next();
            stream.eat_while(|byte| byte.is_ascii_hexdigit());
        }
        b'0' if stream.peek() == b'0' && (b'0'..=b'7').contains(&stream.byte(stream.pos + 1)) => {
            stream.next();
            stream.eat_while(|byte| (b'0'..=b'7').contains(&byte));
        }
        b'0' => {}
        _ => {
            digits(stream);
            if stream.peek() == b'.' {
                stream.next();
                digits(stream);
            }
            exponent(stream);
        }
    }
}

const COBOL_KEYWORDS: &str = "ACCEPT ACCESS ACQUIRE ADD ADDRESS ADVANCING AFTER ALIAS ALL ALPHABET ALPHABETIC ALPHABETIC-LOWER ALPHABETIC-UPPER \
ALPHANUMERIC ALPHANUMERIC-EDITED ALSO ALTER ALTERNATE AND ANY ARE AREA AREAS ARITHMETIC ASCENDING ASSIGN AT ATTRIBUTE AUTHOR \
AUTO AUTO-SKIP AUTOMATIC B-AND B-EXOR B-LESS B-NOT B-OR BACKGROUND-COLOR BACKGROUND-COLOUR BEEP BEFORE BELL BINARY BIT BITS \
BLANK BLINK BLOCK BOOLEAN BOTTOM BY CALL CANCEL CD CF CH CHARACTER CHARACTERS CLASS CLOCK-UNITS CLOSE COBOL CODE CODE-SET \
COL COLLATING COLUMN COMMA COMMIT COMMITMENT COMMON COMMUNICATION COMP COMP-0 COMP-1 COMP-2 COMP-3 COMP-4 COMP-5 COMP-6 \
COMP-7 COMP-8 COMP-9 COMPUTATIONAL COMPUTATIONAL-0 COMPUTATIONAL-1 COMPUTATIONAL-2 COMPUTATIONAL-3 COMPUTATIONAL-4 \
COMPUTATIONAL-5 COMPUTATIONAL-6 COMPUTATIONAL-7 COMPUTATIONAL-8 COMPUTATIONAL-9 COMPUTE CONFIGURATION CONNECT CONSOLE \
CONTAINED CONTAINS CONTENT CONTINUE CONTROL CONTROL-AREA CONTROLS CONVERTING COPY CORR CORRESPONDING COUNT CRT CRT-UNDER \
CURRENCY CURRENT CURSOR DATA DATE DATE-COMPILED DATE-WRITTEN DAY DAY-OF-WEEK DB DB-ACCESS-CONTROL-KEY DB-DATA-NAME \
DB-EXCEPTION DB-FORMAT-NAME DB-RECORD-NAME DB-SET-NAME DB-STATUS DBCS DBCS-EDITED DE DEBUG-CONTENTS DEBUG-ITEM DEBUG-LINE \
DEBUG-NAME DEBUG-SUB-1 DEBUG-SUB-2 DEBUG-SUB-3 DEBUGGING DECIMAL-POINT DECLARATIVES DEFAULT DELETE DELIMITED DELIMITER \
DEPENDING DESCENDING DESCRIBED DESTINATION DETAIL DISABLE DISCONNECT DISPLAY DISPLAY-1 DISPLAY-2 DISPLAY-3 DISPLAY-4 \
DISPLAY-5 DISPLAY-6 DISPLAY-7 DISPLAY-8 DISPLAY-9 DIVIDE DIVISION DOWN DROP DUPLICATE DUPLICATES DYNAMIC EBCDIC EGI EJECT \
ELSE EMI EMPTY EMPTY-CHECK ENABLE END END. END-ACCEPT END-ACCEPT. END-ADD END-CALL END-COMPUTE END-DELETE END-DISPLAY \
END-DIVIDE END-EVALUATE END-IF END-INVOKE END-MULTIPLY END-OF-PAGE END-PERFORM END-READ END-RECEIVE END-RETURN END-REWRITE \
END-SEARCH END-START END-STRING END-SUBTRACT END-UNSTRING END-WRITE END-XML ENTER ENTRY ENVIRONMENT EOP EQUAL EQUALS ERASE \
ERROR ESI EVALUATE EVERY EXCEEDS EXCEPTION EXCLUSIVE EXIT EXTEND EXTERNAL EXTERNALLY-DESCRIBED-KEY FD FETCH FILE \
FILE-CONTROL FILE-STREAM FILES FILLER FINAL FIND FINISH FIRST FOOTING FOR FOREGROUND-COLOR FOREGROUND-COLOUR FORMAT FREE \
FROM FULL FUNCTION GENERATE GET GIVING GLOBAL GO GOBACK GREATER GROUP HEADING HIGH-VALUE HIGH-VALUES HIGHLIGHT I-O \
I-O-CONTROL ID IDENTIFICATION IF IN INDEX INDEX-1 INDEX-2 INDEX-3 INDEX-4 INDEX-5 INDEX-6 INDEX-7 INDEX-8 INDEX-9 INDEXED \
INDIC INDICATE INDICATOR INDICATORS INITIAL INITIALIZE INITIATE INPUT INPUT-OUTPUT INSPECT INSTALLATION INTO INVALID INVOKE \
IS JUST JUSTIFIED KANJI KEEP KEY LABEL LAST LD LEADING LEFT LEFT-JUSTIFY LENGTH LENGTH-CHECK LESS LIBRARY LIKE LIMIT LIMITS \
LINAGE LINAGE-COUNTER LINE LINE-COUNTER LINES LINKAGE LOCAL-STORAGE LOCALE LOCALLY LOCK MEMBER MEMORY MERGE MESSAGE \
METACLASS MODE MODIFIED MODIFY MODULES MOVE MULTIPLE MULTIPLY NATIONAL NATIVE NEGATIVE NEXT NO NO-ECHO NONE NOT NULL \
NULL-KEY-MAP NULL-MAP NULLS NUMBER NUMERIC NUMERIC-EDITED OBJECT OBJECT-COMPUTER OCCURS OF OFF OMITTED ON ONLY OPEN OPTIONAL \
OR ORDER ORGANIZATION OTHER OUTPUT OVERFLOW OWNER PACKED-DECIMAL PADDING PAGE PAGE-COUNTER PARSE PERFORM PF PH PIC PICTURE \
PLUS POINTER POSITION POSITIVE PREFIX PRESENT PRINTING PRIOR PROCEDURE PROCEDURE-POINTER PROCEDURES PROCEED PROCESS \
PROCESSING PROGRAM PROGRAM-ID PROMPT PROTECTED PURGE QUEUE QUOTE QUOTES RANDOM RD READ READY REALM RECEIVE RECONNECT RECORD \
RECORD-NAME RECORDS RECURSIVE REDEFINES REEL REFERENCE REFERENCE-MONITOR REFERENCES RELATION RELATIVE RELEASE REMAINDER \
REMOVAL RENAMES REPEATED REPLACE REPLACING REPORT REPORTING REPORTS REPOSITORY REQUIRED RERUN RESERVE RESET RETAINING \
RETRIEVAL RETURN RETURN-CODE RETURNING REVERSE-VIDEO REVERSED REWIND REWRITE RF RH RIGHT RIGHT-JUSTIFY ROLLBACK ROLLING \
ROUNDED RUN SAME SCREEN SD SEARCH SECTION SECURE SECURITY SEGMENT SEGMENT-LIMIT SELECT SEND SENTENCE SEPARATE SEQUENCE \
SEQUENTIAL SET SHARED SIGN SIZE SKIP1 SKIP2 SKIP3 SORT SORT-MERGE SORT-RETURN SOURCE SOURCE-COMPUTER SPACE-FILL \
SPECIAL-NAMES STANDARD STANDARD-1 STANDARD-2 START STARTING STATUS STOP STORE STRING SUB-QUEUE-1 SUB-QUEUE-2 SUB-QUEUE-3 \
SUB-SCHEMA SUBFILE SUBSTITUTE SUBTRACT SUM SUPPRESS SYMBOLIC SYNC SYNCHRONIZED SYSIN SYSOUT TABLE TALLYING TAPE TENANT \
TERMINAL TERMINATE TEST TEXT THAN THEN THROUGH THRU TIME TIMES TITLE TO TOP TRAILING TRAILING-SIGN TRANSACTION TYPE TYPEDEF \
UNDERLINE UNEQUAL UNIT UNSTRING UNTIL UP UPDATE UPON USAGE USAGE-MODE USE USING VALID VALIDATE VALUE VALUES VARYING VLR WAIT \
WHEN WHEN-COMPILED WITH WITHIN WORDS WORKING-STORAGE WRITE XML XML-CODE XML-EVENT XML-NTEXT XML-TEXT ZERO ZERO-FILL";

/// The mode's word table, group by group in its order (a later group's style wins for a word in two).
const SAS_GROUPS: &[(&str, &str, u8)] = &[
    ("def", SAS_WORDS_0, DATA),
    ("def", SAS_WORDS_1, DATA),
    ("def", SAS_WORDS_2, DATA),
    ("def", SAS_WORDS_3, DATA),
    ("def", SAS_WORDS_4, DATA),
    ("def", SAS_WORDS_5, DATA),
    ("def", SAS_WORDS_6, DATA),
    ("def", SAS_WORDS_7, DATA),
    ("builtin", SAS_WORDS_8, DATA),
    ("def", SAS_WORDS_9, PROC),
    ("def", SAS_WORDS_10, MACRO),
    ("builtin", SAS_WORDS_11, ALL),
    ("def", SAS_WORDS_12, ALL),
    ("def", SAS_WORDS_13, ALL),
    ("variable", SAS_WORDS_14, ALL),
    ("def", SAS_WORDS_15, ALL),
    ("def", SAS_WORDS_16, DATA | PROC),
    ("operator", SAS_WORDS_17, DATA | PROC),
];

const SAS_WORDS_0: &str = "stack pgm view source debug nesting nolist";

const SAS_WORDS_1: &str = "if while until for do do; end end; then else cancel";

const SAS_WORDS_2: &str = "label format _n_ _error_";

const SAS_WORDS_3: &str = "ALTER BUFNO BUFSIZE CNTLLEV COMPRESS DLDMGACTION ENCRYPT ENCRYPTKEY EXTENDOBSCOUNTER GENMAX GENNUM INDEX LABEL OBSBUF OUTREP \
PW PWREQ READ REPEMPTY REPLACE REUSE ROLE SORTEDBY SPILL TOBSNO TYPE WRITE FILECLOSE FIRSTOBS IN OBS POINTOBS WHERE WHEREUP \
IDXNAME IDXWHERE DROP KEEP RENAME";

const SAS_WORDS_4: &str = "filevar finfo finv fipname fipnamel fipstate first firstobs floor";

const SAS_WORDS_5: &str = "varfmt varinfmt varlabel varlen varname varnum varray varrayx vartype verify vformat vformatd vformatdx vformatn vformatnx \
vformatw vformatwx vformatx vinarray vinarrayx vinformat vinformatd vinformatdx vinformatn vinformatnx vinformatw \
vinformatwx vinformatx vlabel vlabelx vlength vlengthx vname vnamex vnferr vtype vtypex weekday";

const SAS_WORDS_6: &str = "zipfips zipname zipnamel zipstate";

const SAS_WORDS_7: &str = "put putc putn";

const SAS_WORDS_8: &str = "data run";

const SAS_WORDS_9: &str = "data";

const SAS_WORDS_10: &str = "%if %end %end; %else %else; %do %do; %then";

const SAS_WORDS_11: &str = "proc run; quit; libname filename %macro %mend option options";

const SAS_WORDS_12: &str = "footnote title libname ods";

const SAS_WORDS_13: &str = "%let %put %global %sysfunc %eval";

const SAS_WORDS_14: &str = "&sysbuffr &syscc &syscharwidth &syscmd &sysdate &sysdate9 &sysday &sysdevic &sysdmg &sysdsn &sysencoding &sysenv &syserr \
&syserrortext &sysfilrc &syshostname &sysindex &sysinfo &sysjobid &syslast &syslckrc &syslibrc &syslogapplname &sysmacroname \
&sysmenv &sysmsg &sysncpu &sysodspath &sysparm &syspbuff &sysprocessid &sysprocessname &sysprocname &sysrc &sysscp &sysscpl \
&sysscpl &syssite &sysstartid &sysstartname &systcpiphostname &systime &sysuserid &sysver &sysvlong &sysvlong4 \
&syswarningtext";

const SAS_WORDS_15: &str = "source2 nosource2 page pageno pagesize";

const SAS_WORDS_16: &str = "_all_ _character_ _cmd_ _freq_ _i_ _infile_ _last_ _msg_ _null_ _numeric_ _temporary_ _type_ abort abs addr adjrsq airy \
alpha alter altlog altprint and arcos array arsin as atan attrc attrib attrn authserver autoexec awscontrol awsdef awsmenu \
awsmenumerge awstitle backward band base betainv between blocksize blshift bnot bor brshift bufno bufsize bxor by byerr \
byline byte calculated call cards cards4 catcache cbufno cdf ceil center cexist change chisq cinv class cleanup close cnonct \
cntllev coalesce codegen col collate collin column comamid comaux1 comaux2 comdef compbl compound compress config continue \
convert cos cosh cpuid create cross crosstab css curobs cv daccdb daccdbsl daccsl daccsyd dacctab dairy datalines datalines4 \
datejul datepart datetime day dbcslang dbcstype dclose ddfm ddm delete delimiter depdb depdbsl depsl depsyd deptab dequote \
descending descript design= device dflang dhms dif digamma dim dinfo display distinct dkricond dkrocond dlm dnum do dopen \
doptname doptnum dread drop dropnote dsname dsnferr echo else emaildlg emailid emailpw emailserver emailsys encrypt end \
endsas engine eof eov erf erfc error errorcheck errors exist exp fappend fclose fcol fdelete feedback fetch fetchobs fexist \
fget file fileclose fileexist filefmt filename fileref fmterr fmtsearch fnonct fnote font fontalias fopen foptname foptnum \
force formatted formchar formdelim formdlim forward fpoint fpos fput fread frewind frlen from fsep fuzz fwrite gaminv gamma \
getoption getvarc getvarn go goto group gwindow hbar hbound helpenv helploc hms honorappearance hosthelp hostprint hour hpct \
html hvar ibessel ibr id if index indexc indexw initcmd initstmt inner input inputc inputn inr insert int intck intnx into \
intrr invaliddata irr is jbessel join juldate keep kentb kurtosis label lag last lbound leave left length levels lgamma lib \
library libref line linesize link list log log10 log2 logpdf logpmf logsdf lostcard lowcase lrecl ls macro macrogen maps \
mautosource max maxdec maxr mdy mean measures median memtype merge merror min minute missing missover mlogic mod mode model \
modify month mopen mort mprint mrecall msglevel msymtabmax mvarsize myy n nest netpv new news nmiss no nobatch nobs nocaps \
nocardimage nocenter nocharcode nocmdmac nocol nocum nodate nodbcs nodetails nodmr nodms nodmsbatch nodup nodupkey \
noduplicates noechoauto noequals noerrorabend noexitwindows nofullstimer noicon noimplmac noint nolist noloadlist nomiss \
nomlogic nomprint nomrecall nomsgcase nomstored nomultenvappl nonotes nonumber noobs noovp nopad nopercent noprint \
noprintinit normal norow norsasuser nosetinit nosplash nosymbolgen note notes notitle notitles notsorted noverbose noxsync \
noxwait npv null number numkeys nummousekeys nway obs on open order ordinal otherwise out outer outp= output over ovp p(1 5 \
10 25 50 75 90 95 99) pad pad2 paired parm parmcards path pathdll pathname pdf peek peekc pfkey pmf point poisson poke \
position printer probbeta probbnml probchi probf probgam probhypr probit probnegb probnorm probsig probt procleave prt ps pw \
pwreq qtr quote r ranbin rancau random ranexp rangam range ranks rannor ranpoi rantbl rantri ranuni rcorr read recfm \
register regr remote remove rename repeat repeated replace resolve retain return reuse reverse rewind right round rsquare \
rtf rtrace rtraceloc s s2 samploc sasautos sascontrol sasfrscr sasmsg sasmstore sasscript sasuser saving scan sdf second \
select selection separated seq serror set setcomm setot sign simple sin sinh siteinfo skewness skip sle sls sortedby sortpgm \
sortseq sortsize soundex spedis splashlocation split spool sqrt start std stderr stdin stfips stimer stname stnamel stop \
stopover sub subgroup subpopn substr sum sumwgt symbol symbolgen symget symput sysget sysin sysleave sysmsg sysparm sysprint \
sysprintfont sysprod sysrc system t table tables tan tanh tapeclose tbufsize terminal test then timepart tinv tnonct to \
today tol tooldef totper transformout translate trantab tranwrd trigamma trim trimn trunc truncover type unformatted uniform \
union until upcase update user usericon uss validate value var weight when where while wincharset window work workinit \
workterm write wsum xsync xwait yearcutoff yes yyq min max";

const SAS_WORDS_17: &str = "and not";
