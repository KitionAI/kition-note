//! Blocks by `^id`, block ids, and task checkboxes.

use std::ops::Range;

use kition_note_markdown::{BlockId, Index, Task, sub};

use crate::{Edit, EditError, HeadingRef, Reason, Result, find_heading, insert_block, invalid, minimal, not_found, section};

/// The block named `id`; `ambiguous` when two blocks carry it.
pub fn find<'a>(ix: &'a Index, id: &str, occurrence: Option<usize>) -> Result<&'a BlockId> {
    let all: Vec<&BlockId> = ix.block_ids.iter().filter(|block| block.id == id).collect();
    match (occurrence, all.as_slice()) {
        (_, []) => Err(not_found(format!("no block has the id ^{id}"))),
        (None, [one]) => Ok(one),
        (None, many) => Err(EditError::new(Reason::Ambiguous, format!("{} blocks end with ^{id}; give occurrence (from 1)", many.len()))),
        (Some(n), all) => n
            .checked_sub(1)
            .and_then(|index| all.get(index))
            .copied()
            .ok_or_else(|| EditError::new(Reason::OutOfRange, format!("^{id} occurs {} times, so there is no occurrence {n}", all.len()))),
    }
}

fn valid_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(invalid("a block id is letters, digits and hyphens, without the caret"));
    }
    Ok(())
}

/// Whether `markdown` ends with a block id of its own.
fn ends_with_id(markdown: &str) -> bool {
    let last = markdown.trim_end().rsplit(char::is_whitespace).next().unwrap_or_default();
    last.len() > 1 && last.starts_with('^') && last.chars().skip(1).all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The list marker a list item's text starts with (`- `, `1. `, with a task's `[ ] ` after it),
/// and the width its continuation lines are indented by (the marker without the checkbox).
fn list_marker(source: &str) -> Option<(&str, usize)> {
    let digits = source.chars().take_while(char::is_ascii_digit).count();
    let bullet = match source.chars().next()? {
        '-' | '*' | '+' => 1,
        _ if (1..=9).contains(&digits) && source.get(digits..).is_some_and(|rest| rest.starts_with(['.', ')'])) => digits + 1,
        _ => return None,
    };
    let rest = source.get(bullet..)?;
    let spaces = rest.chars().take_while(|c| *c == ' ').count();
    if !(1..=4).contains(&spaces) {
        return None;
    }
    let indent = bullet + spaces;
    let rest = source.get(indent..)?;
    let checkbox = ["[ ] ", "[x] ", "[X] "].iter().any(|box_| rest.starts_with(box_));
    let end = if checkbox { indent + 4 } else { indent };
    Some((source.get(..end)?, indent))
}

/// The ids `markdown` sets (`^id` tokens), without the caret.
fn ids_in(markdown: &str) -> Vec<String> {
    kition_note_markdown::index_part(markdown, false).block_ids.into_iter().map(|found| found.id).collect()
}

/// `block.replace`: the block becomes `markdown`, keeping its `^id` unless `markdown` sets one.
/// An id `markdown` sets that another block carries is refused, as `block.setId` refuses it. A
/// list item named by its id keeps its marker (and checkbox) unless `markdown` starts with one of
/// its own; its further lines are indented to stay in the item.
pub fn replace(text: &str, ix: &Index, id: &str, markdown: &str) -> Result<Vec<Edit>> {
    let block = find(ix, id, None)?;
    let body = markdown.trim_matches(['\n', '\r']);
    if body.is_empty() {
        return Err(invalid("markdown is empty"));
    }
    let replaced = block.block.clone();
    for new_id in ids_in(body) {
        let elsewhere =
            ix.block_ids.iter().any(|other| other.id == new_id && !(replaced.start <= other.range.start && other.range.end <= replaced.end));
        if elsewhere {
            return Err(EditError::new(Reason::Refused, format!("another block already ends with ^{new_id}")));
        }
    }
    let new = if ends_with_id(body) {
        body.to_owned()
    } else if block.own_line {
        format!("{body}\n\n^{id}")
    } else {
        format!("{body} ^{id}")
    };
    let source = sub(text, replaced.clone());
    let item = if block.own_line || list_marker(body).is_some() { None } else { list_marker(source) };
    let new = match item {
        Some((marker, width)) => {
            let line_start = sub(text, 0..replaced.start).rfind('\n').map_or(0, |at| at + 1);
            let lead = sub(text, line_start..replaced.start);
            let indent = if lead.trim().is_empty() { lead.len() + width } else { width };
            let pad = " ".repeat(indent);
            let mut lines = new.split('\n');
            let mut out = format!("{marker}{}", lines.next().unwrap_or_default());
            for line in lines {
                out.push('\n');
                if !line.trim().is_empty() {
                    out.push_str(&pad);
                }
                out.push_str(line);
            }
            out
        }
        None => new,
    };
    Ok(minimal(text, replaced, &new))
}

/// `block.append`: `markdown` as a new block at the end of a section, or of the note.
pub fn append(text: &str, ix: &Index, at: Option<&HeadingRef>, markdown: &str) -> Result<Vec<Edit>> {
    if markdown.trim().is_empty() {
        return Err(invalid("markdown is empty"));
    }
    let place = match at {
        Some(at) => {
            let heading = find_heading(ix, at)?;
            section::position_at(text, ix, heading, section::Position::After).min(crate::content_end(text, heading.section.clone()))
        }
        None => text.len(),
    };
    Ok(vec![insert_block(text, place, markdown)])
}

/// `block.setId` on a line: the line's id changes, or the line gets ` ^id`. A line of the front
/// matter, of a code block or of a table is no Markdown block an id can mark, and is refused; the
/// id the line already has is no change.
pub fn set_id_on_line(text: &str, ix: &Index, line: usize, id: &str) -> Result<Vec<Edit>> {
    valid_id(id)?;
    let content = line
        .checked_sub(1)
        .and_then(|line| ix.lines.content(text, line))
        .ok_or_else(|| EditError::new(Reason::OutOfRange, format!("the note has {} lines", ix.lines.count())))?;
    let within = |range: &Range<usize>| range.start <= content.start && content.start < range.end;
    if ix.frontmatter.as_ref().is_some_and(|found| within(&found.range)) {
        return Err(invalid(format!("line {line} is in the front matter, which holds properties, not blocks")));
    }
    if ix.code.iter().any(|code| within(&code.range)) {
        return Err(invalid(format!("line {line} is in a code block; an id there would be code")));
    }
    if ix.tables.iter().any(|table| within(&table.range)) {
        return Err(invalid(format!("line {line} is a table row; a table's id stands on its own line after the table")));
    }
    let source = sub(text, content.clone()).trim_end_matches('\r');
    if source.trim().is_empty() {
        return Err(invalid(format!("line {line} is blank")));
    }
    let existing = ix.block_ids.iter().find(|block| content.start <= block.range.start && block.range.end <= content.end);
    if existing.is_some_and(|block| block.id == id) {
        return Ok(Vec::new());
    }
    if ix.block_ids.iter().any(|block| block.id == id) {
        return Err(EditError::new(Reason::Refused, format!("another block already ends with ^{id}")));
    }
    if let Some(existing) = existing {
        return Ok(minimal(text, existing.range.clone(), &format!("^{id}")));
    }
    Ok(vec![Edit::insert(content.start + source.trim_end().len(), format!(" ^{id}"))])
}

/// `block.setId` by id: the `occurrence`-th block with `old` gets `id`.
pub fn set_id(text: &str, ix: &Index, old: &str, occurrence: Option<usize>, id: &str) -> Result<Vec<Edit>> {
    valid_id(id)?;
    let block = find(ix, old, occurrence)?;
    if id != old && ix.block_ids.iter().any(|other| other.id == id) {
        return Err(EditError::new(Reason::Refused, format!("another block already ends with ^{id}")));
    }
    Ok(minimal(text, block.range.clone(), &format!("^{id}")))
}

/// The task on `line` (from 1): the list item whose first line it is.
pub fn task_on_line(ix: &Index, line: usize) -> Result<&Task> {
    let range: Range<usize> = line
        .checked_sub(1)
        .and_then(|line| ix.lines.line(line))
        .ok_or_else(|| EditError::new(Reason::OutOfRange, format!("the note has {} lines", ix.lines.count())))?;
    ix.tasks
        .iter()
        .find(|task| range.start <= task.marker.start && task.marker.start < range.end)
        .ok_or_else(|| not_found(format!("line {line} holds no task")))
}

/// The task the block id `found` marks: the innermost task item holding the id, or, for an id on
/// its own line after a list, the list's one task.
pub fn task_of<'a>(ix: &'a Index, found: &BlockId) -> Result<&'a Task> {
    let holding =
        ix.tasks.iter().filter(|task| task.item.start <= found.range.start && found.range.end <= task.item.end).max_by_key(|task| task.item.start);
    if let Some(task) = holding {
        return Ok(task);
    }
    let inside: Vec<&Task> = ix.tasks.iter().filter(|task| found.block.start <= task.marker.start && task.marker.end <= found.block.end).collect();
    match inside.as_slice() {
        [] => Err(not_found(format!("the block ^{} marks holds no task", found.id))),
        [one] => Ok(one),
        many => Err(EditError::new(Reason::Ambiguous, format!("^{} marks a list of {} tasks; toggle one by its line", found.id, many.len()))),
    }
}

/// `task.toggle`: changes the one character of `[ ]` or `[x]`.
pub fn toggle(text: &str, task: &Task, done: Option<bool>) -> Vec<Edit> {
    let done = done.unwrap_or(!task.done);
    if done == task.done {
        return Vec::new();
    }
    let inner = task.marker.start + 1..task.marker.end.saturating_sub(1);
    minimal(text, inner, if done { "x" } else { " " })
}

#[cfg(test)]
mod tests {
    use kition_note_markdown::index;

    use super::*;
    use crate::apply;

    #[test]
    fn replace_keeps_the_id() {
        let text = "First ^a1\n\nSecond\n";
        let ix = index(text);
        assert_eq!(apply(text, &replace(text, &ix, "a1", "New text").unwrap()), "New text ^a1\n\nSecond\n");
        assert_eq!(apply(text, &replace(text, &ix, "a1", "New ^b2").unwrap()), "New ^b2\n\nSecond\n");
        assert_eq!(replace(text, &ix, "zz", "x").unwrap_err().reason, Reason::NotFound);
    }

    #[test]
    fn duplicate_ids_get_fixed_by_occurrence() {
        let text = "First ^a1\n\nSecond ^a1\n";
        let ix = index(text);
        assert_eq!(find(&ix, "a1", None).unwrap_err().reason, Reason::Ambiguous);
        assert_eq!(apply(text, &set_id(text, &ix, "a1", Some(2), "a1-2").unwrap()), "First ^a1\n\nSecond ^a1-2\n");
        let text = "Plain line\n";
        assert_eq!(apply(text, &set_id_on_line(text, &index(text), 1, "p").unwrap()), "Plain line ^p\n");
        assert!(set_id_on_line(text, &index(text), 1, "bad id").is_err());
    }

    #[test]
    fn toggle_changes_one_character() {
        let text = "- [ ] one\n- [x] two\n";
        let ix = index(text);
        let task = task_on_line(&ix, 1).unwrap();
        assert_eq!(apply(text, &toggle(text, task, None)), "- [x] one\n- [x] two\n");
        let task = task_on_line(&ix, 2).unwrap();
        assert_eq!(apply(text, &toggle(text, task, None)), "- [ ] one\n- [ ] two\n");
        assert!(toggle(text, task, Some(true)).is_empty());
        assert!(task_on_line(&ix, 3).is_err());
    }

    #[test]
    fn append_at_the_end_of_a_section_or_note() {
        let text = "# A\n\nx\n\n# B\n\ny";
        let ix = index(text);
        assert_eq!(apply(text, &append(text, &ix, Some(&HeadingRef::Path(vec!["A".into()])), "- z").unwrap()), "# A\n\nx\n\n- z\n\n# B\n\ny");
        assert_eq!(apply(text, &append(text, &ix, None, "end").unwrap()), "# A\n\nx\n\n# B\n\ny\n\nend\n");
    }
}
