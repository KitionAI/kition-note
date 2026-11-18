//! An edit's update: when the text changed inside one chunk whose lines shape nothing beyond
//! themselves, only that chunk is parsed, hidden, and laid out again, and the lines after it
//! move (their offsets, line numbers, and heights); a keystroke in a large note costs what its
//! chunk costs, as Obsidian's editor costs what its line does. Anything that can shape other
//! lines (a fence, a table, a callout, the front matter, a reference definition, a picture, a
//! diagram, an HTML block, a fold) takes the whole update instead.

use std::ops::Range;

use crate::View;
use crate::chunk::{Summary, chunks_closed};
use crate::layout::{self, Placed};
use crate::style::{self, Block, Styles};

fn moved(range: &Range<usize>, delta: isize) -> Range<usize> {
    range.start.saturating_add_signed(delta)..range.end.saturating_add_signed(delta)
}

/// How many bytes `a` and `b` share at their start: whole blocks compare as slices (memcmp).
fn shared_start(a: &[u8], b: &[u8]) -> usize {
    const BLOCK: usize = 256;
    let blocks = a.as_chunks::<BLOCK>().0.iter().zip(b.as_chunks::<BLOCK>().0).take_while(|(x, y)| x == y).count() * BLOCK;
    blocks + a.iter().skip(blocks).zip(b.iter().skip(blocks)).take_while(|(x, y)| x == y).count()
}

/// How many bytes `a` and `b` share at their end, at most `room`.
fn shared_end(a: &[u8], b: &[u8], room: usize) -> usize {
    const BLOCK: usize = 256;
    let blocks = a.rchunks_exact(BLOCK).zip(b.rchunks_exact(BLOCK)).take(room / BLOCK).take_while(|(x, y)| x == y).count() * BLOCK;
    blocks + a.iter().rev().skip(blocks).zip(b.iter().rev().skip(blocks)).take(room - blocks).take_while(|(x, y)| x == y).count()
}

/// Bytes `text` and `old` share at their start and at their end (not overlapping), on character
/// boundaries of both.
fn common(old: &str, text: &str) -> (usize, usize) {
    let (a, b) = (old.as_bytes(), text.as_bytes());
    let mut prefix = shared_start(a, b);
    while prefix > 0 && !(old.is_char_boundary(prefix) && text.is_char_boundary(prefix)) {
        prefix -= 1;
    }
    let room = a.len().min(b.len()) - prefix;
    let mut suffix = shared_end(a, b, room);
    while suffix > 0 && !(old.is_char_boundary(old.len() - suffix) && text.is_char_boundary(text.len() - suffix)) {
        suffix -= 1;
    }
    (prefix, suffix)
}

/// Whether a chunk's parse shapes only its own lines.
fn self_contained(parsed: &crate::chunk::Parsed, part: &str) -> bool {
    parsed.tables.is_empty()
        && parsed.callouts.is_empty()
        && !parsed.links.iter().any(|link| matches!(link.kind, kition_note_markdown::LinkKind::Image | kition_note_markdown::LinkKind::Embed))
        && !parsed.styles.lines.iter().any(|line| {
            matches!(line.block, Block::Frontmatter | Block::Property | Block::Math | Block::Html | Block::Diagram | Block::Table { .. }) || line.callout > 0
        })
        // A reference definition shapes the links of other chunks; a Mermaid block draws as a diagram.
        && !part.contains("]:")
        && !part.to_ascii_lowercase().contains("mermaid")
}

/// How many edits took this way (for the tests).
#[cfg(test)]
pub(crate) static TAKEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// How many of those cut the chunks otherwise (for the tests).
#[cfg(test)]
pub(crate) static RECUT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl View {
    /// Updates for `text` after an edit to the last text, when the edit stays in one chunk that
    /// shapes only its own lines (see the module). Answers whether it did; `false` leaves the
    /// view as it was, for the whole update.
    pub(crate) fn update_edit(&mut self, text: &str, active: &Range<usize>, selection: &Range<usize>, folded: &[usize]) -> bool {
        let Some(laid) = self.laid.as_ref() else { return false };
        if !folded.is_empty() || !laid.folded.is_empty() || laid.width != self.theme.width || self.chunk_ranges.len() < 3 {
            return false;
        }
        let old = self.text.as_str();
        let (prefix, suffix) = common(old, text);
        let old_end = old.len() - suffix;
        let delta = text.len() as isize - old.len() as isize;
        // A line that closes a metadata block (`---`, `...`) made or unmade can open or close one
        // that starts in any chunk before: the whole update reads the note again.
        let lines_around = |source: &str, from: usize, to: usize| {
            let start = source.get(..from).and_then(|before| before.rfind('\n')).map_or(0, |at| at + 1);
            let end = source.get(to..).and_then(|after| after.find('\n')).map_or(source.len(), |at| to + at);
            source.get(start..end).unwrap_or("").split('\n').any(crate::chunk::closes_metadata)
        };
        if lines_around(old, prefix, old_end) || lines_around(text, prefix, old_end.saturating_add_signed(delta)) {
            return false;
        }
        // The chunk the edit is in (never the first: the front matter and the title's room).
        let Some(mut index) = self.chunk_ranges.iter().position(|range| range.start <= prefix && prefix < range.end) else { return false };
        // An edit right at a chunk's start that ends by the chunk before's end (a line break typed
        // after a heading's line: its bytes match either way) is the chunk before's, which the
        // line break leaves cut as it was.
        if index > 0
            && self.chunk_ranges.get(index).is_some_and(|range| range.start == prefix)
            && self.chunk_ranges.get(index - 1).is_some_and(|range| old_end <= range.end)
        {
            index -= 1;
        }
        let Some(mut range) = self.chunk_ranges.get(index).cloned() else { return false };
        // The first chunk only without front matter (its properties shape the whole block).
        let front_matter = |value: &str| value.starts_with("---\n") || value.starts_with("---\r\n");
        if (index == 0 && (front_matter(old) || front_matter(text))) || old_end > range.end || range.end >= old.len() {
            return false;
        }
        // The chunks around it keep their bounds: the one before, the edited one, the one after.
        let before = index.checked_sub(1).and_then(|at| self.chunk_ranges.get(at)).cloned();
        let region_start = before.as_ref().map_or(0, |chunk| chunk.start);
        let after_end = self.chunk_ranges.get(index + 1).map_or(old.len(), |next| next.end);
        let mut new_end = range.end.saturating_add_signed(delta);
        let Some(region) = text.get(region_start..after_end.saturating_add_signed(delta)) else { return false };
        // A part that ends in an open fence (a line break typed before backticks) cuts the rest
        // of the note otherwise.
        let (local, closed) = chunks_closed(region, region_start == 0);
        if !closed {
            return false;
        }
        let local: Vec<Range<usize>> = local.into_iter().map(|part| part.start + region_start..part.end + region_start).collect();
        let mut expected: Vec<Range<usize>> = before.iter().cloned().collect();
        expected.extend([range.start..new_end, new_end..after_end.saturating_add_signed(delta)]);
        // An edit that cuts the edited chunk and the next otherwise (a line break that makes a
        // blank line, a line joined) takes them as one part, its new chunks in their place; the
        // chunk before must keep its bounds.
        let recut = local != expected;
        let mut replaced: Vec<Range<usize>> = Vec::new();
        if recut {
            if before.as_ref().is_some_and(|chunk| local.first() != Some(chunk)) || index + 1 >= self.chunk_ranges.len() {
                return false;
            }
            replaced = local.iter().skip(usize::from(before.is_some())).cloned().collect();
            range = range.start..after_end;
            new_end = after_end.saturating_add_signed(delta);
        }
        let (Some(new_part), Some(old_part)) = (text.get(range.start..new_end), old.get(range.clone())) else { return false };
        let mut used = Vec::new();
        let labels = self.labels.clone();
        // A part cut into several chunks parses as they do, one by one.
        let (parsed_new, parsed_old) = if recut {
            let old_ranges: Vec<Range<usize>> = self.chunk_ranges.get(index..index + 2).unwrap_or(&[]).to_vec();
            (
                std::sync::Arc::new(self.chunks.joined(text, &replaced, &labels, &mut used)),
                std::sync::Arc::new(self.chunks.joined(old, &old_ranges, &labels, &mut used)),
            )
        } else {
            (self.chunks.chunk(new_part, index == 0, &labels, &mut used), self.chunks.chunk(old_part, index == 0, &labels, &mut used))
        };
        if !self_contained(&parsed_new, new_part) || !self_contained(&parsed_old, old_part) {
            return false;
        }
        // The caret's lines before and after, when outside the chunk, must be plain too.
        let base = &laid.base;
        let outside_plain = |styles: &Styles, at: &Range<usize>, limit: usize| {
            if at.start > limit {
                return true;
            }
            let lines = styles.line_of(at.start)..styles.line_of(at.end.min(limit)) + 1;
            lines.into_iter().all(|line| {
                styles.lines.get(line).is_some_and(|style| {
                    !matches!(style.block, Block::Frontmatter | Block::Property | Block::Math | Block::Html | Block::Diagram | Block::Table { .. })
                        && style.callout == 0
                })
            })
        };
        if !outside_plain(base, &laid.active, old.len()) || self.mermaid.iter().any(|block| block.range.start < old_end && prefix <= block.range.end)
        {
            return false;
        }
        if self.picture_lines.iter().any(|(line, _)| base.starts.get(*line).is_some_and(|start| (range.start..range.end).contains(start))) {
            return false;
        }

        // The base styles (before the hiding), the chunk's spliced in and the rest moved.
        let first_line = base.line_of(range.start);
        let next_first = base.line_of(range.end);
        let take = parsed_new.styles.lines.len().saturating_sub(usize::from(new_part.ends_with('\n')));
        let new_lines: Vec<style::LineStyle> = parsed_new.styles.lines.iter().take(take).copied().collect();
        let new_starts: Vec<usize> = parsed_new.styles.starts.iter().take(take).map(|start| start + range.start).collect();
        let line_delta = take as isize - (next_first - first_line) as isize;
        let splice = |styles: &mut Styles, bytes: &[style::flag::Flags]| {
            styles.bytes.splice(range.clone(), bytes.iter().copied());
            styles.lines.splice(first_line..next_first, new_lines.iter().copied());
            styles.starts.splice(first_line..next_first, new_starts.iter().copied());
            for start in styles.starts.iter_mut().skip(first_line + take) {
                *start = start.saturating_add_signed(delta);
            }
        };
        let old_laid_active = laid.active.clone();
        let Some(mut laid) = self.laid.take() else { return false };
        let mut base = std::mem::take(&mut laid.base);
        splice(&mut base, &parsed_new.styles.bytes);
        let mut styles = std::mem::take(&mut self.styles);
        splice(&mut styles, &parsed_new.styles.bytes);
        // The hiding again: the chunk's lines, and the caret's lines before and after.
        let old_active = old_laid_active;
        let mapped = |at: usize| if at >= old_end { at.saturating_add_signed(delta) } else { at.min(prefix) };
        let old_active = mapped(old_active.start)..mapped(old_active.end);
        let lines_of = |styles: &Styles, at: &Range<usize>| -> Range<usize> {
            if at.start > text.len() {
                return 0..0;
            }
            styles.line_of(at.start)..styles.line_of(at.end.min(text.len())) + 1
        };
        let mut redo: Vec<usize> = (first_line..first_line + take).chain(lines_of(&base, &old_active)).chain(lines_of(&base, active)).collect();
        // A code block's fences show while the caret is anywhere in it: the caret's whole blocks.
        let code = |line: usize| base.lines.get(line).is_some_and(|style| matches!(style.block, Block::Code { .. }));
        let mut blocks = Vec::new();
        for &line in redo.iter().skip(take) {
            if code(line) {
                let first = (0..line).rev().take_while(|above| code(*above)).last().unwrap_or(line);
                let last = (line + 1..).take_while(|below| code(*below)).last().unwrap_or(line);
                blocks.extend(first..=last);
            }
        }
        redo.extend(blocks);
        redo.sort_unstable();
        redo.dedup();
        // A caret line above the chunk would move the lines between it and the chunk.
        if redo.first().is_some_and(|line| *line < first_line) {
            // The styles are spliced already: the whole update makes everything again.
            self.styles = styles;
            self.laid = None;
            return false;
        }
        // The part's code blocks (not highlighted) in place of the old part's; those of the lines
        // laid out again highlighted.
        self.lights.edit(&range, delta, range.start, &parsed_new.code);
        for &line in &redo {
            let start = base.starts.get(line).copied().unwrap_or(0);
            let end = base.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            self.lights.light(text, start..end, &mut styles, Some(&mut base));
        }
        for &line in &redo {
            let start = base.starts.get(line).copied().unwrap_or(0);
            let end = base.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            if let (Some(target), Some(source)) = (styles.bytes.get_mut(start..end), base.bytes.get(start..end)) {
                target.copy_from_slice(source);
            }
            if let (Some(target), Some(source)) = (styles.lines.get_mut(line), base.lines.get(line)) {
                *target = *source;
            }
            styles.conceal_line(text, line, active, selection);
        }

        // The layout: the chunk's lines and the caret's laid out again, the lines after moved.
        let width = self.theme.content_width();
        let theme = self.theme.clone();
        let mut placed = std::mem::take(&mut self.placed);
        // From the line before, or the first line's own top (below the inline title's room).
        let mut y = match first_line.checked_sub(1).and_then(|line| placed.get(line)) {
            Some(last) => last.y + last.boxed.height + last.pad_bottom,
            None => placed.first().map_or(self.theme.top, |first| first.y - first.space - first.pad_top),
        };
        let mut fresh: Vec<Placed> = Vec::with_capacity(take);
        let no_hidden: Vec<bool> = Vec::new();
        let place = |cache: &mut layout::Cache, styles: &Styles, line: usize| {
            let start = styles.starts.get(line).copied().unwrap_or(0);
            let end = styles.starts.get(line + 1).map_or(text.len(), |next| next.saturating_sub(1));
            let style = styles.lines.get(line).copied().unwrap_or_default();
            let content = text.get(start..end).unwrap_or("");
            let flags = styles.bytes.get(start..end).unwrap_or(&[]);
            (start..end, style, cache.line(&theme, style, content, flags, width))
        };
        for line in first_line..first_line + take {
            let (line_range, style, boxed) = place(&mut self.cache, &styles, line);
            let (space, pad_top, pad_bottom) = layout::spacing(&styles, text, line, boxed.above, &no_hidden);
            y += space + pad_top;
            let fold = (matches!(style.block, Block::Heading(_)) || (style.list > 0 && layout::item_children(text, &styles, line).is_some()))
                .then_some(false);
            fresh.push(Placed { range: line_range, y, style, boxed: boxed.clone(), fold, space, pad_top, pad_bottom });
            y += boxed.height + pad_bottom;
        }
        let tail_start = first_line + take;
        placed.splice(first_line..next_first.min(placed.len()), fresh);
        let redo_after: Vec<usize> = redo.iter().copied().filter(|line| *line >= first_line + take).collect();
        for (offset, line) in placed.iter_mut().skip(tail_start).enumerate() {
            let index = tail_start + offset;
            line.range = moved(&line.range, delta);
            // The caret's lines (shown or hidden again) and the next two (their spacing looks back).
            if redo_after.contains(&index) {
                let (line_range, style, boxed) = place(&mut self.cache, &styles, index);
                line.range = line_range;
                line.style = style;
                line.boxed = boxed;
            }
            if offset < 2 || redo_after.contains(&index) {
                let (space, pad_top, pad_bottom) = layout::spacing(&styles, text, index, line.boxed.above, &no_hidden);
                line.space = space;
                line.pad_top = pad_top;
                line.pad_bottom = pad_bottom;
            }
            y += line.space + line.pad_top;
            line.y = y;
            y += line.boxed.height + line.pad_bottom;
        }
        let height = (y + self.theme.bottom).max(self.theme.min_height);

        // What else holds offsets or line numbers.
        let in_old = |at: usize| range.start <= at && at < range.end;
        let shift_link = |mut link: kition_note_markdown::Link, by: isize| {
            link.range = moved(&link.range, by);
            link.destination_range = link.destination_range.as_ref().map(|inner| moved(inner, by));
            link.text_range = link.text_range.as_ref().map(|inner| moved(inner, by));
            link
        };
        let mut links: Vec<kition_note_markdown::Link> = Vec::with_capacity(self.links.len());
        let mut added = false;
        for link in std::mem::take(&mut self.links) {
            if link.range.start >= range.end {
                if !added {
                    links.extend(parsed_new.links.iter().cloned().map(|link| shift_link(link, range.start as isize)));
                    added = true;
                }
                links.push(shift_link(link, delta));
            } else if !in_old(link.range.start) {
                links.push(link);
            }
        }
        if !added {
            links.extend(parsed_new.links.iter().cloned().map(|link| shift_link(link, range.start as isize)));
        }
        for table in &mut self.tables {
            if table.range.start >= range.end {
                table.range = moved(&table.range, delta);
                for row in table.rows.iter_mut().chain(std::iter::once(&mut table.delimiter)) {
                    row.line = moved(&row.line, delta);
                    row.body = moved(&row.body, delta);
                    for cell in &mut row.cells {
                        cell.segment = moved(&cell.segment, delta);
                        cell.content = moved(&cell.content, delta);
                    }
                }
            }
        }
        let summary = {
            let (old_part_summary, new_part_summary): (&Summary, &Summary) = (&parsed_old.summary, &parsed_new.summary);
            let mut summary = std::mem::take(&mut self.summary);
            let mut headings: Vec<(u8, String, usize)> = summary.headings.drain(..).filter(|(_, _, at)| !in_old(*at)).collect();
            for heading in &mut headings {
                if heading.2 >= range.end {
                    heading.2 = heading.2.saturating_add_signed(delta);
                }
            }
            headings.extend(new_part_summary.headings.iter().map(|(level, title, at)| (*level, title.clone(), at + range.start)));
            headings.sort_by_key(|heading| heading.2);
            summary.headings = headings;
            let adjust = |total: usize, old: usize, new: usize| (total + new).saturating_sub(old);
            summary.tags = adjust(summary.tags, old_part_summary.tags, new_part_summary.tags);
            summary.block_ids = adjust(summary.block_ids, old_part_summary.block_ids, new_part_summary.block_ids);
            summary.tasks_open = adjust(summary.tasks_open, old_part_summary.tasks_open, new_part_summary.tasks_open);
            summary.tasks_done = adjust(summary.tasks_done, old_part_summary.tasks_done, new_part_summary.tasks_done);
            summary.words = adjust(summary.words, old_part_summary.words, new_part_summary.words);
            summary
        };
        for (line, _) in &mut self.picture_lines {
            if *line >= next_first {
                *line = line.saturating_add_signed(line_delta);
            }
        }
        for block in &mut self.mermaid {
            if block.range.start >= range.end {
                block.range = moved(&block.range, delta);
                block.code = block.code.saturating_add_signed(delta);
                block.first = block.first.saturating_add_signed(line_delta);
                block.last = block.last.saturating_add_signed(line_delta);
            }
        }
        let mut chunk_ranges = std::mem::take(&mut self.chunk_ranges);
        if recut {
            let tail: Vec<Range<usize>> = chunk_ranges.iter().skip(index + 2).map(|later| moved(later, delta)).collect();
            chunk_ranges.truncate(index);
            chunk_ranges.extend(replaced.iter().cloned());
            chunk_ranges.extend(tail);
        } else {
            if let Some(edited) = chunk_ranges.get_mut(index) {
                edited.end = new_end;
            }
            for later in chunk_ranges.iter_mut().skip(index + 1) {
                *later = moved(later, delta);
            }
        }
        let mut callouts = std::mem::take(&mut laid.callouts);
        for callout in &mut callouts {
            if callout.range.start >= range.end {
                callout.range = moved(&callout.range, delta);
            }
        }

        self.text = text.to_owned();
        self.styles = styles;
        self.placed = placed;
        self.height = height;
        self.links = links;
        self.summary = summary;
        self.chunk_ranges = chunk_ranges;
        self.laid =
            Some(crate::Laid { base, callouts, active: active.clone(), selection: selection.clone(), folded: Vec::new(), width: self.theme.width });
        #[cfg(test)]
        TAKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(test)]
        if recut {
            RECUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        true
    }
}
