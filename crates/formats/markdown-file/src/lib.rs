//! The note's file: Markdown bytes to the text the replica keeps and back, byte for byte.
//!
//! Ported from Kition's `packages/document-editor/src/kdm/documentText.ts` and `markdownFile.ts`,
//! with the design's changes (`docs/design.md` 2.1, 2.3): the line style is two registers (`eol`,
//! `bom`), a file with mixed line endings or a lone CR keeps every `\r` in its text (the
//! TypeScript rewrote such files to one ending), and invalid UTF-8 is refused for editing instead
//! of being replaced by U+FFFD.
//!
//! The rule that makes the round trip exact: [`decode`] then [`encode`] gives back the input
//! bytes for every valid UTF-8 file.

pub mod bench;
pub mod merge;

/// How `\n` in the text is written to the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Eol {
    /// `\n` as it is (also a file without line endings).
    #[default]
    Lf,
    /// `\n` as `\r\n`: the text holds no `\r`.
    Crlf,
    /// As it is: the file mixes endings or has a lone CR, and the text keeps each `\r`.
    Mixed,
}

impl Eol {
    /// The value of the `Note.eol` register (absent means `lf`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Crlf => "crlf",
            Self::Mixed => "mixed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lf" => Some(Self::Lf),
            "crlf" => Some(Self::Crlf),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

/// How a file writes the text the replica keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineStyle {
    pub eol: Eol,
    /// The file starts with a UTF-8 byte order mark.
    pub bom: bool,
}

/// The UTF-8 byte order mark.
pub const BOM: &[u8] = b"\xEF\xBB\xBF";

/// A file that is not valid UTF-8: the note opens read-only with `lossy` text and saves `bytes`
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidUtf8 {
    /// Where the first invalid byte is.
    pub at: usize,
    /// The text with each invalid sequence replaced by U+FFFD, for showing only.
    pub lossy: String,
}

impl std::fmt::Display for InvalidUtf8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the file is not valid UTF-8 (byte {})", self.at)
    }
}

impl std::error::Error for InvalidUtf8 {}

/// Line endings counted in a text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Endings {
    pub lf: usize,
    pub crlf: usize,
    pub cr: usize,
}

impl Endings {
    pub fn count(text: &str) -> Self {
        // Most files have no `\r`: their line breaks counted at once (a large note's took long).
        if !text.as_bytes().contains(&b'\r') {
            return Self { lf: text.bytes().filter(|byte| *byte == b'\n').count(), ..Self::default() };
        }
        let mut endings = Self::default();
        let mut bytes = text.bytes().peekable();
        while let Some(byte) = bytes.next() {
            match byte {
                b'\n' => endings.lf += 1,
                b'\r' if bytes.peek() == Some(&b'\n') => {
                    bytes.next();
                    endings.crlf += 1;
                }
                b'\r' => endings.cr += 1,
                _ => {}
            }
        }
        endings
    }

    /// The file's line ending style: one kind (`lf` when it has none), else mixed.
    pub fn eol(self) -> Eol {
        match (self.lf > 0, self.crlf > 0, self.cr > 0) {
            (_, _, true) | (true, true, _) => Eol::Mixed,
            (false, true, false) => Eol::Crlf,
            _ => Eol::Lf,
        }
    }
}

/// The file's text for the replica and the style to write it back with: one byte order mark is
/// removed; CRLF files are kept as `\n` text; mixed and lone-CR files keep their `\r`.
pub fn decode(bytes: &[u8]) -> Result<(String, LineStyle), InvalidUtf8> {
    let bom = bytes.starts_with(BOM);
    let body = bytes.get(if bom { BOM.len() } else { 0 }..).unwrap_or_default();
    let text = std::str::from_utf8(body).map_err(|error| InvalidUtf8 {
        at: error.valid_up_to() + if bom { BOM.len() } else { 0 },
        lossy: String::from_utf8_lossy(body).into_owned(),
    })?;
    let eol = Endings::count(text).eol();
    let text = if eol == Eol::Crlf { text.replace("\r\n", "\n") } else { text.to_owned() };
    Ok((text, LineStyle { eol, bom }))
}

/// The file bytes of `text` written in `style`: the inverse of [`decode`].
///
/// In a `crlf` note every `\n` is written as `\r\n`, except one a `\r` already precedes (text
/// pasted with its CRLF endings is not doubled).
pub fn encode(text: &str, style: LineStyle) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + if style.bom { BOM.len() } else { 0 });
    if style.bom {
        out.extend_from_slice(BOM);
    }
    match style.eol {
        Eol::Lf | Eol::Mixed => out.extend_from_slice(text.as_bytes()),
        Eol::Crlf => {
            let mut previous = 0u8;
            for &byte in text.as_bytes() {
                if byte == b'\n' && previous != b'\r' {
                    out.push(b'\r');
                }
                out.push(byte);
                previous = byte;
            }
        }
    }
    out
}

/// Text written into a note by a command, in the note's style: in an `lf` or `crlf` note `\r\n`
/// and lone `\r` become `\n` (the note's text holds no `\r`); a `mixed` note keeps them as given.
pub fn normalize_inserted(text: &str, eol: Eol) -> std::borrow::Cow<'_, str> {
    if eol == Eol::Mixed || !text.contains('\r') {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}

/// The byte offset of `text`'s character `n` (from 0), or its end when it has no more.
fn char_offset(text: &str, n: usize) -> usize {
    // An ASCII run is one byte per character (checked at once: a large note's pieces took long).
    match text.as_bytes().get(..n) {
        Some(head) if head.is_ascii() => n,
        None if text.is_ascii() => text.len(),
        _ => text.char_indices().nth(n).map_or(text.len(), |(at, _)| at),
    }
}

/// The text cut into pieces of about `piece` characters, each ending after a line break when
/// there is one within another `piece` characters: a large body is written to the replica in
/// these pieces, last first, so that an edit inside it does not scan the whole body.
pub fn pieces(text: &str, piece: usize) -> Vec<&str> {
    let piece = piece.max(1);
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        // The byte offset after `piece` characters, or the end.
        let cut = char_offset(rest, piece);
        let end = if cut >= rest.len() {
            rest.len()
        } else {
            // Up to the next line break (from the last character of the piece on) within
            // another `piece` characters.
            let back = rest.get(..cut).and_then(|head| head.char_indices().last()).map_or(0, |(at, _)| at);
            let window = rest.get(back..).unwrap_or_default();
            let limit = char_offset(window, piece);
            match window.get(..limit).and_then(|w| w.find('\n')) {
                Some(newline) => back + newline + 1,
                None => cut,
            }
        };
        let Some((head, tail)) = rest.split_at_checked(end) else {
            out.push(rest);
            break;
        };
        out.push(head);
        rest = tail;
    }
    out
}

#[cfg(test)]
mod tests;
