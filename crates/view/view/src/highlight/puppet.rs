//! Puppet, as CodeMirror 5's puppet mode colors it, ported token for token: keywords, resource
//! types builtins, `present`, `true`, … atoms (plain), `$variables`, `name =>` attributes tags,
//! definitions plain, numbers, strings (a variable inside a double-quoted one apart), `/regex/`
//! a variable, `#` comments, braces brackets.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug, Default)]
pub struct Puppet {
    in_definition: bool,
    in_include: bool,
    continue_string: bool,
    /// The open string's quote.
    pending: u8,
}

const KEYWORDS: &str = "class define site node include import inherits case if else in and elsif default or";
const ATOMS: &str = "false true running present absent directory undef";
const BUILTINS: &str = "action augeas burst chain computer cron destination dport exec file filebucket group host icmp iniface \
interface jump k5login limit log_level log_prefix macauthorization mailalias maillist mcx mount nagios_command nagios_contact \
nagios_contactgroup nagios_host nagios_hostdependency nagios_hostescalation nagios_hostextinfo nagios_hostgroup nagios_service \
nagios_servicedependency nagios_serviceescalation nagios_serviceextinfo nagios_servicegroup nagios_timeperiod name notify \
outiface package proto reject resources router schedule scheduled_task selboolean selmodule service source sport \
ssh_authorized_key sshkey stage state table tidy todest toports tosource user vlan yumrepo zfs zone zpool";

fn listed(list: &str, word: &str) -> bool {
    list.split_ascii_whitespace().any(|known| known == word)
}

fn space(byte: u8) -> bool {
    byte.is_ascii_whitespace()
}

fn name(byte: u8) -> bool {
    scan::js_word(byte) || byte == b':'
}

/// Where a run of `ok` bytes from `at` ends.
fn run_of(stream: &Stream, at: usize, ok: impl Fn(u8) -> bool) -> usize {
    let len = stream.bytes.len();
    (at..len).find(|end| !ok(stream.byte(*end))).unwrap_or(len.max(at))
}

/// The variable after a `$` at `at`, as the mode's
/// `({)?([a-z][a-z0-9_]*)?((::[a-z][a-z0-9_]*)*::)?[a-zA-Z0-9_]+(})?` takes it: where it ends.
fn variable(stream: &Stream, at: usize) -> Option<usize> {
    let start = if stream.byte(at) == b'{' { at + 1 } else { at };
    let lower = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_';
    let segment = |from: usize| stream.byte(from).is_ascii_lowercase().then(|| run_of(stream, from + 1, lower));
    let last = |from: usize| {
        let end = run_of(stream, from, scan::js_word);
        (end > from).then(|| if stream.byte(end) == b'}' { end + 1 } else { end })
    };
    let colons = |from: usize| stream.byte(from) == b':' && stream.byte(from + 1) == b':';
    // The first segment as long as it goes, then shorter; then none.
    let mut firsts: Vec<usize> = match segment(start) {
        Some(end) => (start + 1..=end).rev().collect(),
        None => Vec::new(),
    };
    firsts.push(start);
    for first in firsts {
        // `::segment` repeated as often as it goes, then fewer, each needing `::` and a name.
        let mut ends = vec![first];
        let mut at = first;
        while colons(at)
            && let Some(end) = segment(at + 2)
        {
            at = end;
            ends.push(end);
        }
        for end in ends.iter().rev() {
            if colons(*end)
                && let Some(found) = last(end + 2)
            {
                return Some(found);
            }
        }
        if let Some(found) = last(first) {
            return Some(found);
        }
    }
    None
}

pub fn line(text: &str, state: &mut Puppet, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        token(stream, state)
    });
}

/// `(\s+)?` then a run of `ok` from `at`: where it ends, if the run is not empty.
fn spaced_run(stream: &Stream, at: usize, ok: impl Fn(u8) -> bool) -> Option<usize> {
    let from = run_of(stream, at, space);
    let end = run_of(stream, from, ok);
    (end > from).then_some(end)
}

fn token(stream: &mut Stream, state: &mut Puppet) -> Option<Kind> {
    let at = stream.pos;
    let word_end = run_of(stream, at, scan::js_word);
    let word = std::str::from_utf8(stream.bytes.get(at..word_end).unwrap_or_default()).unwrap_or("").to_string();
    // `(\s+)?\w+\s+=>`, `(\s+)?[\w:_]+(\s+)?{`, `(\s+)?@{1,2}[\w:_]+(\s+)?{`, from the token's start.
    let attribute = spaced_run(stream, at, scan::js_word).is_some_and(|end| {
        let arrow = run_of(stream, end, space);
        arrow > end && stream.byte(arrow) == b'=' && stream.byte(arrow + 1) == b'>'
    });
    let block = spaced_run(stream, at, name).is_some_and(|end| stream.byte(run_of(stream, end, space)) == b'{');
    let collector = {
        let from = run_of(stream, at, space);
        let ats = run_of(stream, from, |byte| byte == b'@');
        ats > from && ats - from <= 2 && {
            let end = run_of(stream, ats, name);
            end > ats && stream.byte(run_of(stream, end, space)) == b'{'
        }
    };
    let here = stream.next()?;
    if here == b'$' {
        return match variable(stream, stream.pos) {
            Some(end) => {
                stream.pos = end;
                Some(Kind::Variable)
            }
            None => Some(Kind::Operator),
        };
    }
    if state.continue_string {
        stream.back_up(1);
        return string(stream, state);
    }
    if state.in_definition {
        if let Some(end) = spaced_run(stream, stream.pos, name) {
            stream.pos = run_of(stream, end, space);
            return None;
        }
        let spaces = run_of(stream, stream.pos, space);
        if spaces > stream.pos && stream.byte(spaces) == b'{' {
            stream.pos = spaces + 1;
        }
        state.in_definition = false;
    }
    if state.in_include {
        if let Some(end) = spaced_run(stream, stream.pos, |byte| !space(byte)) {
            stream.pos = run_of(stream, end, space);
        }
        state.in_include = false;
        return None;
    }
    if let Some(end) = spaced_run(stream, stream.pos, scan::js_word)
        && stream.byte(end) == b'('
    {
        // A call: its name a definition.
        stream.pos = end;
        return None;
    }
    if attribute {
        if let Some(end) = spaced_run(stream, stream.pos, scan::js_word) {
            stream.pos = end;
        }
        return Some(Kind::Operator);
    }
    let known = listed(KEYWORDS, &word) || listed(ATOMS, &word) || listed(BUILTINS, &word);
    if !word.is_empty() && known {
        stream.pos = word_end;
        // `\s+\S+\s+{` ahead: a definition's name follows.
        let gap = run_of(stream, word_end, space);
        let title = run_of(stream, gap, |byte| !space(byte));
        let after = run_of(stream, title, space);
        if gap > word_end && title > gap && after > title && stream.byte(after) == b'{' {
            state.in_definition = true;
        }
        if word == "include" {
            state.in_include = true;
        }
        return if listed(BUILTINS, &word) {
            Some(Kind::Property)
        } else if listed(KEYWORDS, &word) {
            Some(Kind::Keyword)
        } else {
            None
        };
    }
    if word.len() >= 2 && word.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
        stream.pos = run_of(stream, at + 1, name);
        return None;
    }
    if block {
        if let Some(end) = spaced_run(stream, stream.pos, name) {
            stream.pos = end;
        }
        return None;
    }
    if collector {
        let from = run_of(stream, stream.pos, space);
        let ats = run_of(stream, from, |byte| byte == b'@');
        stream.pos = ats.min(from + 2).max(from);
        return None;
    }
    match here {
        b'#' => {
            stream.skip_to_end();
            Some(Kind::Comment)
        }
        b'\'' | b'"' => {
            state.pending = here;
            string(stream, state)
        }
        b'{' | b'}' => Some(Kind::Punctuation),
        b'/' => {
            if let Some(slash) = stream.rest().iter().position(|byte| *byte == b'/') {
                stream.pos += slash + 1;
            }
            Some(Kind::Variable)
        }
        b'0'..=b'9' => {
            stream.eat_while(|byte| byte.is_ascii_digit());
            Some(Kind::Number)
        }
        b'=' => {
            stream.eat(|byte| byte == b'>');
            Some(Kind::Operator)
        }
        _ => {
            stream.eat_while(|byte| scan::js_word(byte) || byte == b'-');
            None
        }
    }
}

/// A string to its quote; a `$` in a double-quoted one stops it (the variable is its own token).
fn string(stream: &mut Stream, state: &mut Puppet) -> Option<Kind> {
    let mut previous = 0;
    let mut last = 0;
    let mut variable = false;
    while !stream.eol() {
        let Some(here) = stream.next() else { break };
        last = here;
        if here == state.pending {
            break;
        }
        if here == b'$' && previous != b'\\' && state.pending == b'"' {
            variable = true;
            break;
        }
        previous = here;
    }
    if variable {
        stream.back_up(1);
    }
    state.continue_string = last != state.pending;
    Some(Kind::String)
}
