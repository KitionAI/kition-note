//! Tcl and mIRC scripts, as CodeMirror 5's tcl mode and the mirc mode built on it color them,
//! each ported token for token (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Tcl {
    /// What it reads next.
    mode: Mode,
    before_params: bool,
    in_params: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Code,
    /// A `#* *#` comment.
    Comment,
    /// A `# [[ ]]#` block.
    Meta,
    /// A string to its quote.
    String(u8),
}

const COMMANDS: &str = "safe after append array auto_execok auto_import auto_load auto_mkindex auto_mkindex_old auto_qualify \
auto_reset bgerror binary break catch cd close concat continue dde eof encoding error eval exec exit expr fblocked fconfigure \
fcopy file fileevent filename filename flush for foreach format gets glob global history http if incr info interp join lappend \
lindex linsert list llength load lrange lreplace lsearch lset lsort memory msgcat namespace open package parray pid \
pkg::create pkg_mkIndex proc puts pwd re_syntax read regex regexp registry regsub rename resource return scan seek set socket \
source split string subst switch tcl_endOfWord tcl_findLibrary tcl_startOfNextWord tcl_wordBreakAfter tcl_startOfPreviousWord \
tcl_wordBreakBefore tcltest tclvars tell time trace unknown unset update uplevel upvar variable vwait";
const CONTROL: &str = "if elseif else and not or eq ne in ni for foreach while switch";

fn operator(byte: u8) -> bool {
    b"+-*&%=<>!?^/|".contains(&byte)
}

/// Tcl: commands and `if`, `eq`, … keywords (any case), `$variables` builtins, numbers, operators
/// and a lone `"` comments, `#` comments (`#* *#` blocks, `# [[ ]]#` meta), strings only inside a
/// `(…)` after a variable or a control word.
pub fn line(text: &str, state: &mut Tcl, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        match state.mode {
            Mode::Code => code(stream, state),
            Mode::Comment => comment(stream, state),
            Mode::Meta => meta(stream, state),
            Mode::String(quote) => string(stream, state, quote),
        }
    });
}

fn code(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let before_params = std::mem::take(&mut state.before_params);
    let here = stream.next()?;
    if (here == b'"' || here == b'\'') && state.in_params {
        state.mode = Mode::String(here);
        return string(stream, state, here);
    }
    if b"[]{}(),;.".contains(&here) {
        if here == b'(' && before_params {
            state.in_params = true;
        } else if here == b')' {
            state.in_params = false;
        }
        return None;
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
        return Some(Kind::Number);
    }
    if here == b'#' {
        if stream.eat(|byte| byte == b'*').is_some() {
            state.mode = Mode::Comment;
            return comment(stream, state);
        }
        let mut at = stream.pos;
        let mut brackets = 0;
        while brackets < 2 {
            while stream.byte(at) == b' ' {
                at += 1;
            }
            if stream.byte(at) != b'[' {
                break;
            }
            at += 1;
            brackets += 1;
        }
        if brackets == 2 {
            stream.pos = at;
            state.mode = Mode::Meta;
            return meta(stream, state);
        }
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if here == b'"' {
        // The mode's `skipTo(/"/)` looks for the text `/"/` itself.
        if let Some(found) = stream.rest().windows(3).position(|three| three == b"/\"/") {
            stream.pos += found;
        }
        return Some(Kind::Comment);
    }
    if here == b'$' {
        stream.eat_while(|byte| byte.is_ascii_alphanumeric() || b"$_.{:".contains(&byte));
        stream.eat_while(|byte| byte == b'}');
        state.before_params = true;
        return Some(Kind::Property);
    }
    if operator(here) {
        stream.eat_while(operator);
        return Some(Kind::Comment);
    }
    stream.eat_while(|byte| scan::js_word(byte) || b"$_{}".contains(&byte) || byte >= 0x80);
    let word = stream.current().to_ascii_lowercase();
    if COMMANDS.split_ascii_whitespace().any(|known| known == word) {
        return Some(Kind::Keyword);
    }
    if CONTROL.split_ascii_whitespace().any(|known| known == word) {
        state.before_params = true;
        return Some(Kind::Keyword);
    }
    None
}

fn string(stream: &mut Stream, state: &mut Tcl, quote: u8) -> Option<Kind> {
    let mut escaped = false;
    while let Some(here) = stream.next() {
        if here == quote && !escaped {
            state.mode = Mode::Code;
            break;
        }
        escaped = !escaped && here == b'\\';
    }
    Some(Kind::String)
}

fn comment(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if here == b'#' && star {
            state.mode = Mode::Code;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}

fn meta(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let mut closing = 0;
    while let Some(here) = stream.next() {
        if here == b'#' && closing == 2 {
            state.mode = Mode::Code;
            break;
        }
        if here == b']' {
            closing += 1;
        } else if here != b' ' {
            closing = 0;
        }
    }
    Some(Kind::Comment)
}

/// mIRC scripts: commands and control words keywords (any case), known `$identifiers` keywords
/// and other `$names` builtins, `%variables` strings, numbers and `\x` escapes numbers, a `"`
/// a keyword, operator runs, `;` comments (outside a call's parentheses), `/* */` comments,
/// `; (( ))` meta.
pub fn mirc(text: &str, state: &mut Tcl, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        match state.mode {
            Mode::Comment => mirc_comment(stream, state),
            Mode::Meta => mirc_meta(stream, state),
            _ => mirc_code(stream, state),
        }
    });
}

fn mirc_code(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let before_params = std::mem::take(&mut state.before_params);
    let here = stream.next()?;
    if b"[]{}(),.".contains(&here) {
        if here == b'(' && before_params {
            state.in_params = true;
        } else if here == b')' {
            state.in_params = false;
        }
        return None;
    }
    if here.is_ascii_digit() {
        stream.eat_while(|byte| scan::js_word(byte) || byte == b'.');
        return Some(Kind::Number);
    }
    if here == b'\\' {
        stream.eat(|byte| byte == b'\\');
        stream.next();
        return Some(Kind::Number);
    }
    if here == b'/' && stream.eat(|byte| byte == b'*').is_some() {
        state.mode = Mode::Comment;
        return mirc_comment(stream, state);
    }
    if here == b';' {
        // `; *\( *\(`
        let mut at = stream.pos;
        let mut opened = 0;
        while opened < 2 {
            while stream.byte(at) == b' ' {
                at += 1;
            }
            if stream.byte(at) != b'(' {
                break;
            }
            at += 1;
            opened += 1;
        }
        if opened == 2 {
            stream.pos = at;
            state.mode = Mode::Meta;
            return mirc_meta(stream, state);
        }
        if !state.in_params {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
    }
    if here == b'"' {
        stream.eat(|byte| byte == b'"');
        return Some(Kind::Keyword);
    }
    if here == b'$' {
        stream.eat_while(|byte| byte.is_ascii_alphanumeric() || b"$_.:".contains(&byte));
        let word = stream.current().to_ascii_lowercase();
        if MIRC_IDENTIFIERS.split_ascii_whitespace().any(|known| known == word) {
            return Some(Kind::Keyword);
        }
        state.before_params = true;
        return Some(Kind::Property);
    }
    if here == b'%' {
        stream.eat_while(|byte| !(byte == b',' || byte == b'(' || byte == b')' || byte.is_ascii_whitespace()));
        state.before_params = true;
        return Some(Kind::String);
    }
    if operator(here) {
        stream.eat_while(operator);
        return Some(Kind::Operator);
    }
    stream.eat_while(|byte| scan::js_word(byte) || b"$_{}".contains(&byte));
    let word = stream.current().to_ascii_lowercase();
    if MIRC_COMMANDS.split_ascii_whitespace().any(|known| known == word) {
        return Some(Kind::Keyword);
    }
    if CONTROL.split_ascii_whitespace().any(|known| known == word) {
        state.before_params = true;
        return Some(Kind::Keyword);
    }
    None
}

fn mirc_comment(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let mut star = false;
    while let Some(here) = stream.next() {
        if here == b'/' && star {
            state.mode = Mode::Code;
            break;
        }
        star = here == b'*';
    }
    Some(Kind::Comment)
}

fn mirc_meta(stream: &mut Stream, state: &mut Tcl) -> Option<Kind> {
    let mut closing = 0;
    while let Some(here) = stream.next() {
        if here == b';' && closing == 2 {
            state.mode = Mode::Code;
            break;
        }
        if here == b')' {
            closing += 1;
        } else if here != b' ' {
            closing = 0;
        }
    }
    Some(Kind::Comment)
}

/// mIRC's `$identifiers`.
const MIRC_IDENTIFIERS: &str = "$! $$ $& $? $+ $abook $abs $active $activecid $activewid $address $addtok $agent $agentname $agentstat $agentver $alias $and \
$anick $ansi2mirc $aop $appactive $appstate $asc $asctime $asin $atan $avoice $away $awaymsg $awaytime $banmask $base $bfind \
$binoff $biton $bnick $bvar $bytes $calc $cb $cd $ceil $chan $chanmodes $chantypes $chat $chr $cid $clevel $click $cmdbox \
$cmdline $cnick $color $com $comcall $comchan $comerr $compact $compress $comval $cos $count $cr $crc $creq $crlf $ctime \
$ctimer $ctrlenter $date $day $daylight $dbuh $dbuw $dccignore $dccport $dde $ddename $debug $decode $decompress $deltok \
$devent $dialog $did $didreg $didtok $didwm $disk $dlevel $dll $dllcall $dname $dns $duration $ebeeps $editbox $emailaddr \
$encode $error $eval $event $exist $feof $ferr $fgetc $file $filename $filtered $finddir $finddirn $findfile $findfilen \
$findtok $fline $floor $fopen $fread $fserve $fulladdress $fulldate $fullname $fullscreen $get $getdir $getdot $gettok $gmt \
$group $halted $hash $height $hfind $hget $highlight $hnick $hotline $hotlinepos $ial $ialchan $ibl $idle $iel $ifmatch \
$ignore $iif $iil $inelipse $ini $inmidi $inpaste $inpoly $input $inrect $inroundrect $insong $instok $int $inwave $ip \
$isalias $isbit $isdde $isdir $isfile $isid $islower $istok $isupper $keychar $keyrpt $keyval $knick $lactive $lactivecid \
$lactivewid $left $len $level $lf $line $lines $link $lock $lock $locked $log $logstamp $logstampfmt $longfn $longip $lower \
$ltimer $maddress $mask $matchkey $matchtok $md5 $me $menu $menubar $menucontext $menutype $mid $middir $mircdir $mircexe \
$mircini $mklogfn $mnick $mode $modefirst $modelast $modespl $mouse $msfile $network $newnick $nick $nofile $nopath $noqt \
$not $notags $notify $null $numeric $numok $oline $onpoly $opnick $or $ord $os $passivedcc $pic $play $pnick $port $portable \
$portfree $pos $prefix $prop $protect $puttok $qt $query $rand $r $rawmsg $read $readomo $readn $regex $regml $regsub \
$regsubex $remove $remtok $replace $replacex $reptok $result $rgb $right $round $scid $scon $script $scriptdir $scriptline \
$sdir $send $server $serverip $sfile $sha1 $shortfn $show $signal $sin $site $sline $snick $snicks $snotify $sock $sockbr \
$sockerr $sockname $sorttok $sound $sqrt $ssl $sreq $sslready $status $strip $str $stripped $syle $submenu $switchbar $tan \
$target $ticks $time $timer $timestamp $timestampfmt $timezone $tip $titlebar $toolbar $treebar $trust $ulevel $ulist $upper \
$uptime $url $usermode $v1 $v2 $var $vcmd $vcmdstat $vcmdver $version $vnick $vol $wid $width $wildsite $wildtok $window \
$wrap $xor";

const MIRC_COMMANDS: &str = "abook ajinvite alias aline ame amsg anick aop auser autojoin avoice away background ban bcopy beep bread break breplace bset \
btrunc bunset bwrite channel clear clearall cline clipboard close cnick color comclose comopen comreg continue copy creq \
ctcpreply ctcps dcc dccserver dde ddeserver debug dec describe dialog did didtok disable disconnect dlevel dline dll dns \
dqwindow drawcopy drawdot drawfill drawline drawpic drawrect drawreplace drawrot drawsave drawscroll drawtext ebeeps echo \
editbox emailaddr enable events exit fclose filter findtext finger firewall flash flist flood flush flushini font fopen \
fseek fsend fserve fullname fwrite ghide gload gmove gopts goto gplay gpoint gqreq groups gshow gsize gstop gtalk gunload \
hadd halt haltdef hdec hdel help hfree hinc hload hmake hop hsave ial ialclear ialmark identd if ignore iline inc invite \
iuser join kick linesep links list load loadbuf localinfo log mdi me menubar mkdir mnick mode msg nick noop notice notify \
omsg onotice part partall pdcc perform play playctrl pop protect pvoice qme qmsg query queryn quit raw reload remini remote \
remove rename renwin reseterror resetidle return rlevel rline rmdir run ruser save savebuf saveini say scid scon server set \
showmirc signam sline sockaccept sockclose socklist socklisten sockmark sockopen sockpause sockread sockrename sockudp \
sockwrite sound speak splay sreq strip switchbar timer timestamp titlebar tnick tokenize toolbar topic tray treebar ulist \
unload unset unsetall updatenl url uwho var vcadd vcmd vcrem vol while whois window winhelp write writeint if isalnum \
isalpha isaop isavoice isban ischan ishop isignore isin isincs isletter islower isnotify isnum ison isop isprotect isreg \
isupper isvoice iswm iswmcs elseif else goto menu nicklist status title icon size option text edit button check radio box \
scroll list combo link tab item";
