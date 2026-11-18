//! The Obsidian layer pulldown-cmark does not know: `#tags`, `^block-ids`, `==highlights==`,
//! `%%comments%%`, and bare URLs. One byte-driven pass outside the property block and code; only
//! the bytes `% = # ^ h` can start anything.

use std::ops::Range;

use crate::{BlockId, Index, Link, LinkKind, Tag, sub};

/// Ranges the scan skips (property block, code, math, links), sorted by start.
struct Skipped {
    ranges: Vec<Range<usize>>,
    cursor: usize,
}

impl Skipped {
    fn new(ix: &Index) -> Self {
        let mut ranges: Vec<Range<usize>> = ix.frontmatter.iter().map(|found| found.range.clone()).collect();
        ranges.extend(ix.code.iter().map(|code| code.range.clone()));
        ranges.extend(ix.inline_code.iter().cloned());
        ranges.extend(
            ix.links
                .iter()
                .filter(|link| !matches!(link.kind, LinkKind::Reference))
                .map(|link| link.destination_range.clone().unwrap_or(link.range.clone())),
        );
        ranges.extend(ix.links.iter().filter(|link| link.kind.is_wiki() || link.kind == LinkKind::Autolink).map(|link| link.range.clone()));
        ranges.sort_by_key(|range| range.start);
        Self { ranges, cursor: 0 }
    }

    /// The end of the skipped range holding `at`, if any (`at` only grows).
    fn skip(&mut self, at: usize) -> Option<usize> {
        while self.ranges.get(self.cursor).is_some_and(|range| range.end <= at) {
            self.cursor += 1;
        }
        self.ranges.get(self.cursor..)?.iter().take_while(|range| range.start <= at).filter(|range| range.contains(&at)).map(|range| range.end).max()
    }
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

pub fn scan(text: &str, ix: &mut Index) {
    let bytes = text.as_bytes();
    let mut skipped = Skipped::new(ix);
    let mut comment_open: Option<usize> = None;
    let mut urls: Vec<Link> = Vec::new();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if !matches!(byte, b'%' | b'=' | b'#' | b'^' | b'h' | b'w') {
            at += 1;
            continue;
        }
        // An `h` or a `w` matters only when an address starts there (most are letters of words).
        if (byte == b'h' && !bytes.get(at..).is_some_and(|rest| rest.starts_with(b"http")))
            || (byte == b'w' && !bytes.get(at..).is_some_and(|rest| rest.starts_with(b"www.")))
        {
            at += 1;
            continue;
        }
        if comment_open.is_none()
            && let Some(end) = skipped.skip(at)
        {
            at = end.max(at + 1);
            while !text.is_char_boundary(at) && at < text.len() {
                at += 1;
            }
            continue;
        }
        let rest = sub(text, at..text.len());
        if rest.starts_with("%%") {
            match comment_open.take() {
                Some(start) => ix.comments.push(start..at + 2),
                None => comment_open = Some(at),
            }
            at += 2;
            continue;
        }
        if comment_open.is_some() {
            at += 1;
            continue;
        }
        let line = || sub(rest, 0..rest.find('\n').unwrap_or(rest.len()));
        if rest.starts_with("==")
            && let Some(end) = sub(line(), 2..line().len()).find("==")
            && end > 0
        {
            ix.highlights.push(at..at + 4 + end);
            at += 4 + end;
            continue;
        }
        let prev = sub(text, 0..at).chars().next_back();
        if byte == b'#' && prev.is_none_or(char::is_whitespace) {
            let name: String = sub(rest, 1..rest.len()).chars().take_while(|c| is_tag_char(*c)).collect();
            // A tag needs a character that is not a digit; `# ` and `##` are heading markers.
            if !name.is_empty() && name.chars().any(|c| !c.is_ascii_digit()) {
                ix.tags.push(Tag { range: at..at + 1 + name.len(), name: name.clone() });
                at += 1 + name.len();
                continue;
            }
        }
        if byte == b'^' && prev.is_none_or(char::is_whitespace) {
            let name: String = sub(rest, 1..rest.len()).chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            let after = sub(rest, 1 + name.len()..rest.len());
            if !name.is_empty()
                && (after.is_empty()
                    || after.starts_with('\n')
                    || after.starts_with("\r\n")
                    || after.trim_start_matches([' ', '\t']).starts_with('\n'))
            {
                ix.block_ids.push(BlockId { range: at..at + 1 + name.len(), id: name.clone(), block: at..at, own_line: false });
            }
        }
        // `www.` starts one too (a GFM autolink literal), its destination over `http://`.
        let www = byte == b'w' && rest.starts_with("www.");
        if (www || (byte == b'h' && (rest.starts_with("https://") || rest.starts_with("http://"))))
            && prev.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '*' | '_'))
        {
            let len = rest.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`')).unwrap_or(rest.len());
            // Trailing punctuation ends a sentence, not the URL (GFM autolink literals).
            let url = sub(rest, 0..len).trim_end_matches(['.', ',', ':', ';', '!', '?', ')', '*', '_', '\'']);
            let line_start = sub(text, 0..at).rfind('\n').map_or(0, |offset| offset + 1);
            let before = sub(text, line_start..at).trim_start();
            // `[label]: https://...` defines a reference; it is not a link in the text.
            let definition = before.starts_with('[') && before.trim_end().ends_with("]:");
            let long_enough = if www { url.len() > 4 && sub(url, 4..url.len()).contains('.') } else { url.len() > 8 };
            if long_enough && !definition {
                let destination = if www { format!("http://{url}") } else { url.to_owned() };
                urls.push(Link {
                    kind: LinkKind::BareUrl,
                    range: at..at + url.len(),
                    destination: destination.clone(),
                    destination_range: Some(at..at + url.len()),
                    file: destination,
                    anchor: None,
                    text: None,
                    text_range: None,
                });
                at += url.len();
                continue;
            }
        }
        at += 1;
    }
    ix.links.extend(urls);
    ix.links.sort_by_key(|link| link.range.start);
}
