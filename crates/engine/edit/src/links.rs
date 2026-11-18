//! Links: insert one, change one's destination, and retarget every link to a renamed file.

use kition_note_markdown::{Index, Link, LinkKind, sub};

use crate::{Edit, EditError, Reason, Result, invalid, minimal, not_found};

/// How `link.insert` writes a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStyle {
    Wikilink,
    Markdown,
}

fn check_destination(destination: &str) -> Result<()> {
    if destination.trim().is_empty() {
        return Err(invalid("destination is empty"));
    }
    if destination.contains(['\n', '\r']) {
        return Err(invalid("destination is one line"));
    }
    Ok(())
}

/// `text` with a backslash before each of `escaped`, and before a backslash that comes before one
/// of them or ends the text (so it cannot escape what follows it in the link).
fn backslashed(text: &str, escaped: &[char]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        if escaped.contains(&c) || (c == '\\' && next.is_none_or(|next| escaped.contains(&next))) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A destination in angle brackets (CommonMark), its own brackets escaped.
fn angled(destination: &str) -> String {
    format!("<{}>", backslashed(destination, &['<', '>']))
}

/// A Markdown destination as written: in angle brackets when it has spaces, parentheses or angle
/// brackets, which a bare destination cannot hold.
fn markdown_destination(destination: &str) -> String {
    if destination.contains([' ', '(', ')', '<', '>']) { angled(destination) } else { destination.to_owned() }
}

/// A destination written percent-encoded (`my%20plan.md`), in the same style.
fn percent_encoded(destination: &str) -> String {
    let mut out = String::with_capacity(destination.len());
    for c in destination.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            c => out.push(c),
        }
    }
    out
}

/// A Markdown link's text as written: its brackets escaped, so the link ends where it should.
fn markdown_text(text: &str) -> String {
    backslashed(text, &['[', ']'])
}

/// A wikilink cannot escape `]]` or `|` in its destination, nor `]]` in its alias.
fn check_wikilink(destination: &str, alias: Option<&str>) -> Result<()> {
    if destination.contains("]]") || destination.contains('|') {
        return Err(invalid("a wikilink's destination cannot hold ]] or |"));
    }
    if alias.is_some_and(|alias| alias.contains("]]")) {
        return Err(invalid("a wikilink's text cannot hold ]]"));
    }
    Ok(())
}

/// The text of a new link.
pub fn render(destination: &str, text: Option<&str>, style: LinkStyle, embed: bool) -> Result<String> {
    check_destination(destination)?;
    let text = text.filter(|text| !text.is_empty());
    if let Some(text) = text
        && text.contains(['\n', '\r'])
    {
        return Err(invalid("text is one line"));
    }
    let bang = if embed { "!" } else { "" };
    Ok(match style {
        LinkStyle::Wikilink => {
            check_wikilink(destination, text)?;
            match text {
                Some(text) if text != destination => format!("{bang}[[{destination}|{text}]]"),
                _ => format!("{bang}[[{destination}]]"),
            }
        }
        LinkStyle::Markdown => {
            let shown = text.unwrap_or_else(|| destination.rsplit('/').next().unwrap_or(destination));
            format!("{bang}[{}]({})", markdown_text(shown), markdown_destination(destination))
        }
    })
}

/// The links of a line (from 1) in order: what `index` counts.
pub fn on_line(ix: &Index, line: usize) -> Vec<&Link> {
    let Some(range) = line.checked_sub(1).and_then(|line| ix.lines.line(line)) else { return Vec::new() };
    ix.links.iter().filter(|link| range.start <= link.range.start && link.range.start < range.end).collect()
}

/// The `index`-th link (from 0) of `line` (from 1).
pub fn find(ix: &Index, line: usize, index: usize) -> Result<&Link> {
    let links = on_line(ix, line);
    if links.is_empty() {
        return Err(not_found(format!("line {line} has no link")));
    }
    let count = links.len();
    links
        .into_iter()
        .nth(index)
        .ok_or_else(|| EditError::new(Reason::OutOfRange, format!("line {line} has {count} links, so there is no link {index}")))
}

/// `link.update`: the link's destination (and its text when given) change; nothing else does.
pub fn update(text: &str, ix: &Index, line: usize, index: usize, destination: &str, shown: Option<&str>) -> Result<Vec<Edit>> {
    check_destination(destination)?;
    let link = find(ix, line, index)?;
    let range = link
        .destination_range
        .clone()
        .ok_or_else(|| EditError::new(Reason::Refused, "a reference link's destination is in its definition; edit the definition line"))?;
    let new = if link.kind.is_wiki() {
        check_wikilink(destination, shown)?;
        destination.to_owned()
    } else if sub(text, range.clone()).starts_with('<') {
        angled(destination)
    } else {
        markdown_destination(destination)
    };
    let mut edits = minimal(text, range.clone(), &new);
    if let Some(shown) = shown {
        if shown.contains(['\n', '\r']) {
            return Err(invalid("text is one line"));
        }
        match (&link.text_range, link.kind.is_wiki()) {
            (Some(span), true) => edits.extend(minimal(text, span.clone(), shown)),
            (Some(span), false) => edits.extend(minimal(text, span.clone(), &markdown_text(shown))),
            (None, true) => edits.push(Edit::insert(range.end, format!("|{shown}"))),
            (None, false) => {}
        }
    }
    crate::normalize(edits)
}

/// A file name as links compare it: `./` and a `.md` extension dropped, case folded.
fn comparable(file: &str) -> String {
    let file = file.trim().trim_start_matches("./");
    let file = file.strip_suffix(".md").unwrap_or(file);
    file.to_lowercase()
}

/// `links.retarget`: every wikilink, Markdown link and embed to `from` points at `to`, keeping its
/// anchor and text. Answers the edits and how many links changed.
pub fn retarget(text: &str, ix: &Index, from: &str, to: &str) -> Result<(Vec<Edit>, usize)> {
    check_destination(from)?;
    check_destination(to)?;
    let wanted = comparable(from);
    let mut edits = Vec::new();
    let mut count = 0;
    for link in ix.links.iter().filter(|link| {
        !link.is_external()
            && !link.file.is_empty()
            && matches!(link.kind, LinkKind::Markdown | LinkKind::Image | LinkKind::Wikilink | LinkKind::Embed)
    }) {
        if comparable(&link.file) != wanted {
            continue;
        }
        let Some(range) = link.destination_range.clone() else { continue };
        let written = sub(text, range.clone());
        let anchor = written.find('#').map_or("", |at| sub(written, at..written.len()).trim_end_matches('>'));
        let new_file = if link.kind.is_wiki() {
            check_wikilink(to, None)?;
            if link.file.ends_with(".md") || !to.ends_with(".md") { to.to_owned() } else { to.trim_end_matches(".md").to_owned() }
        } else if written.contains('%') {
            percent_encoded(to)
        } else {
            to.to_owned()
        };
        let destination = format!("{new_file}{anchor}");
        let new = if link.kind.is_wiki() {
            destination
        } else if written.starts_with('<') {
            angled(&destination)
        } else {
            markdown_destination(&destination)
        };
        let changed = minimal(text, range, &new);
        if !changed.is_empty() {
            count += 1;
            edits.extend(changed);
        }
    }
    Ok((crate::normalize(edits)?, count))
}

#[cfg(test)]
mod tests {
    use kition_note_markdown::index;

    use super::*;
    use crate::apply;

    #[test]
    fn render_writes_both_styles() {
        assert_eq!(render("Plan", None, LinkStyle::Wikilink, false).unwrap(), "[[Plan]]");
        assert_eq!(render("Plan#Risks", Some("risks"), LinkStyle::Wikilink, false).unwrap(), "[[Plan#Risks|risks]]");
        assert_eq!(render("pic.png", None, LinkStyle::Wikilink, true).unwrap(), "![[pic.png]]");
        assert_eq!(render("docs/my plan.md", None, LinkStyle::Markdown, false).unwrap(), "[my plan.md](<docs/my plan.md>)");
        assert!(render("", None, LinkStyle::Markdown, false).is_err());
    }

    #[test]
    fn update_changes_one_destination() {
        let text = "See [[Old#H|alias]] and [x](old.md).\n";
        let ix = index(text);
        assert_eq!(apply(text, &update(text, &ix, 1, 0, "New#H", None).unwrap()), "See [[New#H|alias]] and [x](old.md).\n");
        assert_eq!(apply(text, &update(text, &ix, 1, 1, "new.md", Some("y")).unwrap()), "See [[Old#H|alias]] and [y](new.md).\n");
        assert_eq!(update(text, &ix, 1, 2, "z", None).unwrap_err().reason, Reason::OutOfRange);
        assert_eq!(update(text, &ix, 2, 0, "z", None).unwrap_err().reason, Reason::NotFound);
    }

    #[test]
    fn retarget_keeps_anchors_and_aliases() {
        let text = "[[Plan]] [[plan#Risks|r]] ![[Plan]] [p](Plan.md#^b) [q](other.md) [[Planning]] [w](https://x.org/Plan.md)\n";
        let ix = index(text);
        let (edits, count) = retarget(text, &ix, "Plan.md", "Road map.md").unwrap();
        assert_eq!(count, 4);
        assert_eq!(
            apply(text, &edits),
            "[[Road map]] [[Road map#Risks|r]] ![[Road map]] [p](<Road map.md#^b>) [q](other.md) [[Planning]] [w](https://x.org/Plan.md)\n"
        );
    }
}
