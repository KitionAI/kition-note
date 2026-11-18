//! Sections by heading path or line: read, replace, insert, delete, move, and heading levels.

use std::ops::Range;

use kition_note_markdown::{Heading, Index, sub};

use crate::{Edit, EditError, HeadingRef, Reason, Result, content_end, find_heading, insert_block, invalid, minimal};

/// Where `section.insert` and `section.move` put Markdown relative to a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// Before the heading.
    Before,
    /// After the section (and its subsections).
    After,
    /// Inside, right under the heading.
    Start,
    /// Inside, at the end of the section's own text (before its subsections).
    End,
}

impl Position {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "before" => Some(Self::Before),
            "after" => Some(Self::After),
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

/// The section's body: from the line after the heading to the section's end.
pub fn body(heading: &Heading) -> Range<usize> {
    heading.range.end..heading.section.end
}

/// Where the section's own text ends: before its first subsection.
fn own_end(ix: &Index, heading: &Heading) -> usize {
    ix.headings
        .iter()
        .filter(|other| other.range.start >= heading.range.end && other.range.start < heading.section.end && other.level > heading.level)
        .map(|other| other.range.start)
        .min()
        .unwrap_or(heading.section.end)
}

/// The byte where `position` puts a block for `heading`.
pub fn position_at(text: &str, ix: &Index, heading: &Heading, position: Position) -> usize {
    match position {
        Position::Before => heading.range.start,
        Position::After => heading.section.end,
        Position::Start => heading.range.end,
        Position::End => content_end(text, heading.range.end..own_end(ix, heading)),
    }
}

/// `section.read`: the section's range, with or without its heading.
pub fn read(ix: &Index, at: &HeadingRef, with_heading: bool) -> Result<Range<usize>> {
    let heading = find_heading(ix, at)?;
    Ok(if with_heading { heading.section.clone() } else { body(heading) })
}

/// `section.replace`: the body (or, `with_heading`, heading and body) becomes `markdown`; the
/// blank lines that separate it from the next section stay.
pub fn replace(text: &str, ix: &Index, at: &HeadingRef, markdown: &str, with_heading: bool) -> Result<Vec<Edit>> {
    let heading = find_heading(ix, at)?;
    let start = if with_heading { heading.range.start } else { heading.range.end };
    let end = content_end(text, start..heading.section.end);
    let body = markdown.trim_matches(['\n', '\r']);
    if body.trim().is_empty() {
        return Err(invalid("markdown is empty; use section.delete to remove a section"));
    }
    let old = sub(text, start..end);
    if old.trim().is_empty() {
        return Ok(vec![insert_block(text, heading.range.end, body)]);
    }
    // The blank lines between the heading and its text stay.
    let lead = old.len() - old.trim_start_matches(['\n', '\r']).len();
    let mut new = format!("{}{body}", sub(old, 0..lead));
    if old.ends_with('\n') {
        new.push('\n');
    }
    Ok(minimal(text, start..end, &new))
}

/// `section.insert`: `markdown` as a block at `position` of the section.
pub fn insert(text: &str, ix: &Index, at: &HeadingRef, position: Position, markdown: &str) -> Result<Vec<Edit>> {
    if markdown.trim().is_empty() {
        return Err(invalid("markdown is empty"));
    }
    let heading = find_heading(ix, at)?;
    Ok(vec![insert_block(text, position_at(text, ix, heading, position), markdown)])
}

/// `section.delete`: the heading and everything under it.
pub fn delete(ix: &Index, at: &HeadingRef) -> Result<Vec<Edit>> {
    let heading = find_heading(ix, at)?;
    Ok(vec![Edit::delete(heading.section.clone())])
}

/// `section.move`: cuts the section and puts it at `position` of the target section.
pub fn move_to(text: &str, ix: &Index, at: &HeadingRef, to: &HeadingRef, position: Position) -> Result<Vec<Edit>> {
    let heading = find_heading(ix, at)?;
    let target = find_heading(ix, to)?;
    let place = position_at(text, ix, target, position);
    if heading.section.contains(&target.range.start) {
        return Err(EditError::new(Reason::Refused, "a section cannot move into itself"));
    }
    if place == heading.section.start || place == heading.section.end {
        return Ok(Vec::new());
    }
    let end = content_end(text, heading.section.clone());
    let moving = sub(text, heading.section.start..end);
    let inserted = insert_block(text, place, moving);
    crate::normalize(vec![Edit::delete(heading.section.clone()), inserted])
}

/// `heading.setLevel`: changes only the `#` run; a Setext heading changes its underline for
/// levels 1 and 2 and becomes ATX only with `atx`.
pub fn set_level(text: &str, ix: &Index, at: &HeadingRef, level: u8, atx: bool) -> Result<Vec<Edit>> {
    if !(1..=6).contains(&level) {
        return Err(invalid("level must be 1 to 6"));
    }
    let heading = find_heading(ix, at)?;
    if !heading.setext {
        return Ok(minimal(text, heading.marker.clone(), &"#".repeat(usize::from(level))));
    }
    if level <= 2 && !atx {
        let width = sub(text, heading.marker.clone()).chars().count();
        return Ok(minimal(text, heading.marker.clone(), &(if level == 1 { "=" } else { "-" }).repeat(width)));
    }
    if !atx {
        return Err(EditError::new(
            Reason::Refused,
            "an underlined (Setext) heading has only levels 1 and 2; pass style \"atx\" to rewrite it as # heading",
        ));
    }
    // `Title\n=====\n` becomes `### Title\n`. A heading whose text spans lines keeps every line,
    // joined with one space: Markdown reads a line break inside a heading as a space, so the
    // heading's text stays the same.
    let lines = sub(text, heading.range.start..heading.marker.start);
    let words: Vec<&str> = lines.split('\n').map(str::trim).filter(|line| !line.is_empty()).collect();
    let new = format!("{} {}", "#".repeat(usize::from(level)), words.join(" "));
    Ok(minimal(text, heading.range.start..heading.marker.end, &new))
}

#[cfg(test)]
mod tests {
    use kition_note_markdown::index;

    use super::*;
    use crate::apply;

    const NOTE: &str = "# Plan\n\nIntro\n\n## Risks\n\nMany\n\n### Detail\n\nx\n\n## Owners\n\nAnn\n";

    fn path(parts: &[&str]) -> HeadingRef {
        HeadingRef::Path(parts.iter().map(|part| (*part).to_owned()).collect())
    }

    #[test]
    fn replace_keeps_the_separation() {
        let ix = index(NOTE);
        assert_eq!(apply(NOTE, &replace(NOTE, &ix, &path(&["Plan", "Owners"]), "Bo\n", false).unwrap()), NOTE.replace("Ann", "Bo"));
        assert_eq!(
            apply(NOTE, &replace(NOTE, &ix, &path(&["Risks"]), "Few", false).unwrap()),
            "# Plan\n\nIntro\n\n## Risks\n\nFew\n\n## Owners\n\nAnn\n"
        );
        assert_eq!(
            apply(NOTE, &replace(NOTE, &ix, &HeadingRef::Line(13), "## People\n\nAnn", true).unwrap()),
            NOTE.replace("## Owners", "## People")
        );
        let empty = "# A\n# B\n";
        assert_eq!(apply(empty, &replace(empty, &index(empty), &path(&["A"]), "x", false).unwrap()), "# A\n\nx\n\n# B\n");
    }

    #[test]
    fn insert_adds_blank_lines_only_where_needed() {
        let ix = index(NOTE);
        assert_eq!(apply(NOTE, &insert(NOTE, &ix, &path(&["Owners"]), Position::End, "- Bo").unwrap()), format!("{NOTE}\n- Bo\n"));
        assert_eq!(
            apply(NOTE, &insert(NOTE, &ix, &path(&["Risks"]), Position::End, "More").unwrap()),
            "# Plan\n\nIntro\n\n## Risks\n\nMany\n\nMore\n\n### Detail\n\nx\n\n## Owners\n\nAnn\n"
        );
        assert_eq!(
            apply(NOTE, &insert(NOTE, &ix, &path(&["Risks"]), Position::Before, "## New").unwrap()),
            "# Plan\n\nIntro\n\n## New\n\n## Risks\n\nMany\n\n### Detail\n\nx\n\n## Owners\n\nAnn\n"
        );
        assert_eq!(
            apply(NOTE, &insert(NOTE, &ix, &path(&["Risks"]), Position::Start, "First").unwrap()),
            "# Plan\n\nIntro\n\n## Risks\n\nFirst\n\nMany\n\n### Detail\n\nx\n\n## Owners\n\nAnn\n"
        );
    }

    #[test]
    fn delete_and_move_whole_sections() {
        let ix = index(NOTE);
        assert_eq!(apply(NOTE, &delete(&ix, &path(&["Risks"])).unwrap()), "# Plan\n\nIntro\n\n## Owners\n\nAnn\n");
        assert_eq!(
            apply(NOTE, &move_to(NOTE, &ix, &path(&["Owners"]), &path(&["Risks"]), Position::Before).unwrap()),
            "# Plan\n\nIntro\n\n## Owners\n\nAnn\n\n## Risks\n\nMany\n\n### Detail\n\nx\n\n"
        );
        assert!(move_to(NOTE, &ix, &path(&["Risks"]), &path(&["Detail"]), Position::After).is_err());
    }

    #[test]
    fn paths_are_checked() {
        let ix = index("# A\n## N\n# B\n## N\n");
        assert_eq!(find_heading(&ix, &path(&["N"])).unwrap_err().reason, Reason::Ambiguous);
        assert_eq!(find_heading(&ix, &path(&["B", "N"])).unwrap().range.start, 13);
        assert_eq!(find_heading(&ix, &path(&["Z"])).unwrap_err().reason, Reason::NotFound);
        assert_eq!(find_heading(&ix, &HeadingRef::Line(2)).unwrap().text, "N");
        assert_eq!(find_heading(&ix, &HeadingRef::Line(5)).unwrap_err().reason, Reason::NotFound);
    }

    #[test]
    fn set_level_changes_only_the_marker() {
        let text = "# T\n\n### D\n\nSub\n---\n";
        let ix = index(text);
        assert_eq!(apply(text, &set_level(text, &ix, &HeadingRef::Line(3), 2, false).unwrap()), "# T\n\n## D\n\nSub\n---\n");
        assert_eq!(apply(text, &set_level(text, &ix, &path(&["T", "Sub"]), 1, false).unwrap()), "# T\n\n### D\n\nSub\n===\n");
        assert!(set_level(text, &ix, &path(&["T", "Sub"]), 3, false).is_err());
        assert_eq!(apply(text, &set_level(text, &ix, &path(&["T", "Sub"]), 3, true).unwrap()), "# T\n\n### D\n\n### Sub\n");
    }
}
