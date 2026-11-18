//! What every language's tokenizer shares: the spans it collects, and reading a line's bytes.

use std::ops::Range;

use super::Kind;

/// The spans a line collects, in order.
#[derive(Default)]
pub struct Out {
    spans: Vec<(Range<usize>, Kind)>,
}

impl Out {
    /// Marks `range` as `kind` (an empty range marks nothing).
    pub fn push(&mut self, range: Range<usize>, kind: Kind) {
        if range.start < range.end {
            self.spans.push((range, kind));
        }
    }

    /// Marks `range` as `kind` when there is one.
    pub fn mark(&mut self, range: Range<usize>, kind: Option<Kind>) {
        if let Some(kind) = kind {
            self.push(range, kind);
        }
    }

    /// The kind of the span that ends last, if any.
    pub fn last_kind(&self) -> Option<Kind> {
        self.spans.iter().max_by_key(|(range, _)| range.end).map(|(_, kind)| *kind)
    }

    /// Another line part's spans, `offset` bytes into this line.
    pub fn extend_shifted(&mut self, other: Out, offset: usize) {
        self.spans.extend(other.spans.into_iter().map(|(range, kind)| (range.start + offset..range.end + offset, kind)));
    }

    /// The spans, sorted and apart (where two overlap, the later one starts where it starts).
    pub fn finish(mut self) -> Vec<(Range<usize>, Kind)> {
        self.spans.sort_by_key(|(range, _)| range.start);
        let mut out: Vec<(Range<usize>, Kind)> = Vec::with_capacity(self.spans.len());
        for (range, kind) in self.spans {
            if let Some((last, _)) = out.last_mut()
                && last.end > range.start
            {
                last.end = range.start;
                if last.start >= last.end {
                    out.pop();
                }
            }
            out.push((range, kind));
        }
        out
    }
}

/// A word character as CodeMirror's `\w` takes it, or any non-ASCII byte (a word in another
/// script; never splits a character, as every byte of one is non-ASCII).
pub fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// A word character as a JavaScript regular expression's `\w` and `\b` take it (ASCII only).
pub fn js_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Where the word starting at `at` ends (`extra` bytes count as word bytes too).
pub fn word_end(bytes: &[u8], at: usize, extra: &[u8]) -> usize {
    let mut end = at;
    while bytes.get(end).is_some_and(|byte| word(*byte) || extra.contains(byte)) {
        end += 1;
    }
    end
}

/// The first byte at or after `at` that is not a space or a tab, and where it is.
pub fn next_significant(bytes: &[u8], at: usize) -> Option<(usize, u8)> {
    let mut at = at;
    while let Some(byte) = bytes.get(at) {
        if *byte != b' ' && *byte != b'\t' {
            return Some((at, *byte));
        }
        at += 1;
    }
    None
}

/// Where a quoted string opened at `at` (its quote there) closes: past the closing quote, or
/// `None` when it runs past the line's end. A backslash escapes the byte after it.
pub fn string_end(bytes: &[u8], at: usize, quote: u8) -> Option<usize> {
    let mut index = at + 1;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'\\' {
            index += 2;
            continue;
        }
        if *byte == quote {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

/// Whether the text at `at` starts with `prefix`.
pub fn at(bytes: &[u8], at: usize, prefix: &str) -> bool {
    bytes.get(at..at + prefix.len()).is_some_and(|slice| slice == prefix.as_bytes())
}

/// Whether the word `word` is followed (after spaces) by `byte`.
pub fn followed_by(bytes: &[u8], end: usize, byte: u8) -> bool {
    next_significant(bytes, end).is_some_and(|(_, next)| next == byte)
}

/// The end of a number in the C family's shape at `at`: `0x…`, `0b…`, or digits with a fraction
/// and an exponent (`_` allowed when `underscores`), then one of `suffixes` (case-insensitive).
pub fn c_number(bytes: &[u8], at: usize, underscores: bool, suffixes: &[&str]) -> Option<usize> {
    let lower = |index: usize| bytes.get(index).map(u8::to_ascii_lowercase);
    let digit = |byte: Option<u8>| byte.is_some_and(|byte| byte.is_ascii_digit() || (underscores && byte == b'_'));
    let mut end = at;
    if lower(at) == Some(b'0') && lower(at + 1) == Some(b'x') && lower(at + 2).is_some_and(|byte| byte.is_ascii_hexdigit()) {
        end = at + 2;
        while lower(end).is_some_and(|byte| byte.is_ascii_hexdigit() || (underscores && byte == b'_')) {
            end += 1;
        }
    } else if lower(at) == Some(b'0') && lower(at + 1) == Some(b'b') && lower(at + 2).is_some_and(|byte| byte == b'0' || byte == b'1') {
        end = at + 2;
        while lower(end).is_some_and(|byte| byte == b'0' || byte == b'1' || (underscores && byte == b'_')) {
            end += 1;
        }
    } else if digit(lower(at)) {
        while digit(lower(end)) {
            end += 1;
        }
        if lower(end) == Some(b'.') {
            end += 1;
            while digit(lower(end)) {
                end += 1;
            }
        }
        end = exponent(bytes, end);
    } else if lower(at) == Some(b'.') && digit(lower(at + 1)) {
        end = at + 1;
        while digit(lower(end)) {
            end += 1;
        }
        end = exponent(bytes, end);
    } else {
        return None;
    }
    for suffix in suffixes {
        if bytes.get(end..end + suffix.len()).is_some_and(|slice| slice.eq_ignore_ascii_case(suffix.as_bytes())) {
            return Some(end + suffix.len());
        }
    }
    Some(end)
}

/// Past an exponent (`e`, an optional sign, digits) at `at`, or `at` when there is none.
pub fn exponent(bytes: &[u8], at: usize) -> usize {
    if bytes.get(at).is_some_and(|byte| *byte == b'e' || *byte == b'E') {
        let mut end = at + 1;
        if bytes.get(end).is_some_and(|byte| *byte == b'+' || *byte == b'-') {
            end += 1;
        }
        if bytes.get(end).is_some_and(u8::is_ascii_digit) {
            while bytes.get(end).is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_') {
                end += 1;
            }
            return end;
        }
    }
    at
}

/// A CodeMirror stream over a line: its text, where the token began, and where it is.
pub struct Stream<'a> {
    pub bytes: &'a [u8],
    pub start: usize,
    pub pos: usize,
}

impl Stream<'_> {
    /// The byte `offset` from the position (the mode's `look`), 0 past either end.
    pub fn look(&self, offset: isize) -> u8 {
        let at = self.pos as isize + offset;
        if at < 0 { 0 } else { self.bytes.get(at as usize).copied().unwrap_or(0) }
    }
    pub fn next(&mut self) -> Option<u8> {
        let here = self.bytes.get(self.pos).copied()?;
        self.pos += 1;
        Some(here)
    }
    pub fn eat(&mut self, ok: impl Fn(u8) -> bool) -> Option<u8> {
        let here = self.bytes.get(self.pos).copied().filter(|here| ok(*here))?;
        self.pos += 1;
        Some(here)
    }
    pub fn eat_while(&mut self, ok: impl Fn(u8) -> bool) -> bool {
        let start = self.pos;
        while self.eat(&ok).is_some() {}
        self.pos > start
    }
    pub fn current(&self) -> &str {
        self.bytes.get(self.start..self.pos).and_then(|text| std::str::from_utf8(text).ok()).unwrap_or("")
    }
    /// The mode's `eatSuffix`: moves `count` on, short of the line's last byte.
    pub fn skip(&mut self, count: usize) {
        self.pos = (self.pos + count).min(self.bytes.len().saturating_sub(1));
    }
    pub fn peek(&self) -> u8 {
        self.look(0)
    }
    /// Whether the text at the position starts with `text`.
    pub fn at(&self, text: &str) -> bool {
        self.bytes.get(self.pos..).is_some_and(|rest| rest.starts_with(text.as_bytes()))
    }
    /// Takes `text` when the position starts with it (the mode's `match` of a string).
    pub fn take(&mut self, text: &str) -> bool {
        let found = self.at(text);
        if found {
            self.pos += text.len();
        }
        found
    }
    /// The line from the position on.
    pub fn rest(&self) -> &[u8] {
        self.bytes.get(self.pos..).unwrap_or_default()
    }
    /// The byte at `at`, 0 past the line's end.
    pub fn byte(&self, at: usize) -> u8 {
        self.bytes.get(at).copied().unwrap_or(0)
    }
    /// Takes the first of `words` (`|` between them) the position starts with that a word
    /// boundary follows (a mode's `^((a)|(b)|…)\b`, any case when `ignore_case`).
    pub fn take_word_of(&mut self, words: &str, ignore_case: bool) -> bool {
        let found = words.split('|').find(|word| {
            let end = self.pos + word.len();
            let Some(text) = self.bytes.get(self.pos..end) else { return false };
            let same = if ignore_case { text.eq_ignore_ascii_case(word.as_bytes()) } else { text == word.as_bytes() };
            same && word.bytes().last().is_some_and(js_word) != js_word(self.byte(end))
        });
        if let Some(word) = found {
            self.pos += word.len();
        }
        found.is_some()
    }
    pub fn back_up(&mut self, count: usize) {
        self.pos = self.pos.saturating_sub(count).max(self.start);
    }
    pub fn sol(&self) -> bool {
        self.pos == 0
    }
    pub fn eol(&self) -> bool {
        self.pos >= self.bytes.len()
    }
    pub fn skip_to_end(&mut self) {
        self.pos = self.bytes.len();
    }
    pub fn eat_space(&mut self) -> bool {
        self.eat_while(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
    }
}

/// Reads a line token by token, as CodeMirror runs a mode: each token's span marked with its
/// style (a token that reads nothing takes a byte, so the line always ends).
pub fn run(text: &str, out: &mut Out, mut token: impl FnMut(&mut Stream) -> Option<Kind>) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        let style = token(&mut stream);
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
        }
        out.mark(stream.start..stream.pos, style);
    }
}

/// Reads a line as [`run`] does, but a token that reads nothing is read again (its state may
/// have moved on), up to ten times, as CodeMirror does; then it takes a byte unstyled.
pub fn run_again(text: &str, out: &mut Out, mut token: impl FnMut(&mut Stream) -> Option<Kind>) {
    let mut stream = Stream { bytes: text.as_bytes(), start: 0, pos: 0 };
    while !stream.eol() {
        stream.start = stream.pos;
        let mut style = None;
        for _ in 0..10 {
            style = token(&mut stream);
            if stream.pos > stream.start {
                break;
            }
            stream.pos = stream.start;
        }
        if stream.pos <= stream.start {
            stream.pos = stream.start + 1;
            style = None;
        }
        out.mark(stream.start..stream.pos, style);
    }
}
