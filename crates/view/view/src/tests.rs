use super::*;

const SAMPLE: &str = "---\ntitle: Launch\ntags: [plan]\n---\n# Launch notes\n\nThree **bold** ideas, some *emphasis*, `inline code`, a [link](https://kition.ai) and a #tag.\n\n## Tasks\n\n- [ ] Write the brief\n- [x] Book the room\n\n> A quote that is long enough to wrap onto a second row of the column so the bar runs along both rows of it.\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n| Name | Owner |\n| --- | --- |\n| Brief | Ana |\n\n---\n\n中文段落，混合 English 和 ==高亮== 文字。\n";

pub(crate) fn fonts() -> bool {
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../kition-fonts"));
    if kition_note_fonts::register_dir(&dir) == 0 {
        eprintln!("skipped: no kition-fonts checkout (set KITION_FONTS_DIR)");
        return false;
    }
    true
}

fn png(path: &std::path::Path, rgba: &[u8], width: u32, height: u32) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(rgba).unwrap();
}

#[test]
fn every_byte_has_a_caret_and_hits_come_back_to_it() {
    if !fonts() {
        return;
    }
    let view = {
        let mut view = View::default();
        view.update(SAMPLE, 0..SAMPLE.len());
        view
    };
    let mut previous = Caret { x: 0.0, top: 0.0, height: 0.0 };
    for (at, _) in SAMPLE.char_indices() {
        // A table's pipes and padding have no place for the caret.
        if !view.has_stop(at) {
            continue;
        }
        let caret = view.caret(at);
        assert!(caret.height > 0.0);
        let in_table = view.styles().lines.get(view.styles().line_of(at)).is_some_and(|line| matches!(line.block, style::Block::Table { .. }));
        assert!(in_table || caret.top > previous.top || (caret.top == previous.top && caret.x >= previous.x), "carets advance at {at}");
        previous = caret;
        // A hit just right of the caret lands on the byte (ends of wrapped rows aside).
        let back = view.hit(caret.x + 0.5, caret.top + caret.height / 2.0);
        let line_end = SAMPLE.get(at..).is_some_and(|rest| rest.starts_with('\n'));
        if !line_end {
            assert!(back == at || view.caret(back).x == caret.x, "hit at {at} came back as {back}");
        }
    }
}

#[test]
fn long_lines_wrap_inside_the_column_and_edges_find_rows() {
    if !fonts() {
        return;
    }
    let mut view = View::default();
    view.update(SAMPLE, 0..SAMPLE.len());
    let quote = SAMPLE.find("> A quote").unwrap();
    let line = view.lines().iter().find(|line| line.range.start == quote).unwrap();
    assert!(line.boxed.rows.len() >= 2, "the quote wraps");
    let right = view.theme().width - view.theme().margin;
    for row in &line.boxed.rows {
        let last = row.stops.last().map_or(0.0, |stop| stop.x);
        let tail = SAMPLE.get(quote + row.start..quote + row.end).unwrap();
        let trailing = tail.len() - tail.trim_end().len();
        assert!(view.theme().margin + last <= right + 8.0 * trailing as f32 + 1.0, "row fits");
    }
    let (start, end) = view.row_edges(quote + 3);
    assert_eq!(start, quote);
    assert!(end < line.range.end, "End stops at the first row");
    let below = view.vertical(quote + 3, view.caret(quote + 3).x, 1);
    assert!(below > end && below <= line.range.end, "Down goes to the second row");
    assert_eq!(view.vertical(0, 0.0, -1), 0);
    assert_eq!(view.vertical(SAMPLE.len(), 0.0, 1), SAMPLE.len());
}

#[test]
fn a_selection_covers_its_rows() {
    if !fonts() {
        return;
    }
    let mut view = View::default();
    view.update(SAMPLE, 0..SAMPLE.len());
    let from = SAMPLE.find("Three").unwrap();
    let to = SAMPLE.find("## Tasks").unwrap() + 3;
    let rects = view.selection(from, to);
    assert!(rects.len() >= 3, "the paragraph's rows, the blank line, and the heading: {rects:?}");
    assert!(rects.iter().all(|rect| rect.x1 > rect.x0 && rect.y1 > rect.y0));
    assert!(view.selection(from, from).is_empty());
}

#[test]
fn the_sample_draws_text_and_outside_is_transparent() {
    if !fonts() {
        return;
    }
    let mut view = View::default();
    view.update(SAMPLE, 0..SAMPLE.len());
    let (width, height) = (view.width(), view.height());
    let rgba = view.frame(0.0, 0.0, 1.0, width as u16, height as u16);
    let inked = rgba.as_chunks::<4>().0.iter().filter(|px| px[0] < 128).count();
    assert!(inked > 2_000, "text draws ({inked} dark pixels)");
    if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
        png(&std::path::Path::new(&dir).join("sample.png"), &rgba, width, height);
        let zoomed = view.frame(40.0, 60.0, 2.0, 900, 700);
        png(&std::path::Path::new(&dir).join("sample-zoom.png"), &zoomed, 900, 700);
    }
    assert_eq!(view.frame(-10.0, 0.0, 1.0, 1, 1), vec![0, 0, 0, 0]);
}

#[test]
fn an_edit_relays_only_its_line() {
    if !fonts() {
        return;
    }
    let mut view = View::default();
    view.update(SAMPLE, 0..SAMPLE.len());
    let before: Vec<_> = view.lines().iter().map(|line| std::sync::Arc::as_ptr(&line.boxed)).collect();
    let edited = SAMPLE.replacen("Three", "Four", 1);
    view.update(&edited, 0..edited.len());
    let after: Vec<_> = view.lines().iter().map(|line| std::sync::Arc::as_ptr(&line.boxed)).collect();
    let changed = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 1, "one line laid out again");
}

#[test]
fn the_live_preview_hides_syntax_away_from_the_caret() {
    if !fonts() {
        return;
    }
    let text = "# Title\n\nSome **bold** and a [link](https://kition.ai).\n\n- [ ] task\n";
    let mut view = View::default();
    view.update(text, 0..0);
    // On the heading's line (the caret's) the marks show; on the paragraph they take no room.
    let bold = text.find("**bold").unwrap();
    assert_eq!(view.caret(bold).x, view.caret(bold + 2).x, "the hidden ** has no width");
    let url = text.find("https").unwrap();
    assert_eq!(view.caret(url).x, view.caret(url + 10).x, "the link's destination is hidden");
    assert_ne!(view.caret(0).x, view.caret(2).x, "the caret's line shows its # mark");
    view.update(text, bold..bold);
    assert_ne!(view.caret(bold).x, view.caret(bold + 2).x, "with the caret on it, the line shows its syntax");
    if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
        view.update(SAMPLE, SAMPLE.len()..SAMPLE.len());
        let (width, height) = (view.width(), view.height());
        png(&std::path::Path::new(&dir).join("preview.png"), &view.frame(0.0, 0.0, 1.0, width as u16, height as u16), width, height);
    }
}

#[test]
fn a_table_is_a_grid_and_the_caret_moves_through_its_cells() {
    if !fonts() {
        return;
    }
    let text = "| Name | Owner |\n| --- | --- |\n| Brief | a long owner name that wraps inside its cell when the column is narrow enough, which it is not yet but will be once the text is long enough to pass the page |\n";
    let mut view = View::default();
    view.update(text, 0..0);
    let grids = view.tables();
    assert_eq!(grids.len(), 1);
    assert_eq!(grids[0].columns.len(), 3, "two columns, three edges");
    assert_eq!(grids[0].rows.len(), 2, "the header and one body row; the delimiter takes no room");
    let pipe = text.find("| Owner").unwrap();
    assert!(!view.has_stop(pipe), "a pipe is no place for the caret");
    assert_eq!(view.snap(pipe, true), pipe + 2, "snapping goes into the next cell");
    // A click in the second column lands in its text.
    let (left, right) = (grids[0].columns[1], grids[0].columns[2]);
    let (top, _) = grids[0].rows[1];
    let hit = view.hit((left + right) / 2.0, top + 12.0);
    let owner = text.find("a long").unwrap();
    assert!(hit >= owner && hit < text.len(), "the hit is in the Owner cell: {hit}");
    // The long cell wraps: its second text line is below its first.
    let first = view.caret(owner);
    let later = view.caret(text.rfind("page").unwrap());
    assert!(later.top > first.top && later.x < right, "the cell wraps inside its column");
    assert_eq!(view.row_edges(owner + 3), (owner, text.rfind(" |").unwrap()), "Home and End are the cell's");
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn timing_a_large_note() {
    let _ = fonts();
    let mut text = String::new();
    while text.len() < 1_000_000 {
        text.push_str("## Section\n\nSome **bold** text with a [link](https://kition.ai) and `code`, then more words to wrap the line across the column width.\n\n- item one\n- item two\n\n");
    }
    let mut view = View::default();
    let start = std::time::Instant::now();
    view.update(&text, 0..0);
    eprintln!("first layout: {:?}", start.elapsed());
    for round in 0..3 {
        let at = text.len() / 2 + round;
        text.insert(at, 'x');
        let start = std::time::Instant::now();
        view.update(&text, at..at);
        eprintln!("keystroke update: {:?}", start.elapsed());
    }
}

#[test]
fn front_matter_reads_as_properties_away_from_the_caret() {
    if !fonts() {
        return;
    }
    let text = "---\ntitle: Launch\ntags:\n  - plan\n  - q4\nstatus: draft\n---\n# Notes\n\nbody\n";
    let mut view = View::default();
    let body = text.find("body").unwrap();
    view.update(text, body..body);
    let row = |at: usize| view.caret(at);
    let (title, tags, item, status) =
        (text.find("title").unwrap(), text.find("tags").unwrap(), text.find("plan").unwrap(), text.find("status").unwrap());
    // One 29 px row a key, 32 px apart (Obsidian's), a block list's items in their key's row.
    assert_eq!(row(tags).top - row(title).top, 32.0);
    assert_eq!(row(status).top - row(tags).top, 32.0);
    assert_eq!(row(item).height, 0.0, "a list item has no row of its own");
    // The editor draws the block: no glyph of the front matter is drawn.
    assert!(view.lines().iter().take(7).all(|line| line.boxed.rows.iter().all(|row| row.glyphs.is_empty())));
    // With the caret given to it, it stays the properties block (Obsidian's live preview never
    // shows the YAML: its properties are edited in the block, and the editor keeps the caret out).
    view.update(text, title..title);
    assert_eq!(view.caret(item).height, 0.0);
    assert!(view.lines().iter().take(7).all(|line| line.boxed.rows.iter().all(|row| row.glyphs.is_empty())));
}

#[test]
fn moving_the_caret_lays_out_only_its_lines_and_matches_a_whole_layout() {
    if !fonts() {
        return;
    }
    let text = "# Title\n\nSome **bold** and a [link](https://kition.ai) that wraps far enough to take a second row of the column, maybe even a third one here.\n\n## Second\n\n- [ ] task with `code`\n- item\n\n> quote\n\n```rust\nfn x() {}\n```\n\nlast line\n";
    let stops = [
        0,
        text.find("Some").unwrap(),
        text.find("## Second").unwrap(),
        text.find("task").unwrap(),
        text.find("fn x").unwrap(),
        text.find("last").unwrap(),
    ];
    for (from, to) in stops.iter().flat_map(|from| stops.iter().map(move |to| (*from, *to))) {
        let mut moved = View::default();
        moved.update(text, from..from);
        moved.update(text, to..to);
        let mut whole = View::default();
        whole.update(text, to..to);
        assert_eq!(moved.styles().bytes, whole.styles().bytes, "flags after {from} -> {to}");
        assert_eq!(moved.height(), whole.height(), "height after {from} -> {to}");
        for at in 0..=text.len() {
            if text.is_char_boundary(at) {
                assert_eq!(moved.caret(at), whole.caret(at), "caret at {at} after {from} -> {to}");
            }
        }
    }
}

#[test]
fn a_table_moved_by_a_line_above_it_matches_a_fresh_layout() {
    if !fonts() {
        return;
    }
    let before = "# A\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\ntext\n\n| c |\n|---|\n| 3 |\n";
    let after = "# A\n\nnew line\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\ntext\n\n| c |\n|---|\n| 3 |\n";
    let mut moved = View::default();
    moved.update(before, 0..0);
    moved.update(after, 0..0);
    let mut fresh = View::default();
    fresh.update(after, 0..0);
    assert_eq!(moved.tables(), fresh.tables());
    for at in 0..=after.len() {
        assert_eq!(moved.caret(at), fresh.caret(at), "caret at {at}");
    }
    // A table inserted above renumbers the ones below.
    let above = "| z |\n|---|\n| 0 |\n\n".to_owned() + after;
    moved.update(&above, 0..0);
    let mut fresh = View::default();
    fresh.update(&above, 0..0);
    assert_eq!(moved.tables(), fresh.tables());
}

#[test]
fn emoji_draw_in_color_as_one_glyph_per_sequence() {
    if !fonts() {
        return;
    }
    let text = "Emoji \u{1F600}\u{1F44D}\u{1F389}\u{1F680}\u{2705}\u{2764}\u{FE0F} family \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} flag \u{1F1E8}\u{1F1F3} key 1\u{FE0F}\u{20E3} hand \u{1F44B}\u{1F3FD} end\n";
    let mut view = View::default();
    view.update(text, text.len() + 1..text.len() + 1);
    let (width, height) = (view.width(), 120);
    let pixels = view.frame(0.0, 0.0, 1.0, width as u16, height as u16);
    let colored = pixels.as_chunks::<4>().0.iter().filter(|px| px[3] > 200 && (i16::from(px[0]) - i16::from(px[2])).abs() > 60).count();
    assert!(colored > 200, "emoji draw in color: {colored} colored pixels");
    // A ZWJ sequence, a flag, and a keycap are one grapheme each: the caret skips them whole.
    let family = text.find('\u{1F468}').unwrap();
    let after = family + "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}".len();
    assert!(view.caret(after).x > view.caret(family).x);
    let flag = text.find('\u{1F1E8}').unwrap();
    let flag_width = view.caret(flag + 8).x - view.caret(flag).x;
    assert!(flag_width > 10.0 && flag_width < 30.0, "a flag is one glyph, not two letters: {flag_width}");
    if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
        png(&std::path::Path::new(&dir).join("emoji.png"), &pixels, width, height);
    }
}

#[test]
fn code_blocks_report_their_box_language_and_code() {
    if !fonts() {
        return;
    }
    let text = "# A\n\n```rust\nfn x() {}\nlet y = 1;\n```\n\ntext\n\n~~~\nplain\n~~~\n";
    let mut view = View::default();
    view.update(text, text.len() + 1..text.len() + 1);
    let blocks = view.code_blocks_in(f32::NEG_INFINITY, f32::INFINITY);
    assert_eq!(blocks.len(), 2, "{blocks:?}");
    assert_eq!(blocks[0].language, "rust");
    assert_eq!(&text[blocks[0].code.clone()], "fn x() {}\nlet y = 1;");
    assert_eq!(blocks[1].language, "");
    assert_eq!(&text[blocks[1].code.clone()], "plain");
    assert!(blocks[0].bottom > blocks[0].top && blocks[1].top > blocks[0].bottom);
    // Only the blocks in the range, a cut one whole.
    let inside = view.code_blocks_in(blocks[0].top + 10.0, blocks[0].top + 12.0);
    assert_eq!(inside, vec![blocks[0].clone()]);
}

#[test]
fn up_and_down_step_one_line_over_padding_between_lines() {
    if !fonts() {
        return;
    }
    let text = "# Title\n\n- one\n- two\n\n> [!tip] Tip\n> body\n\n| a |\n|---|\n| 1 |\n\nend";
    let mut view = View::default();
    view.update(text, text.len() + 1..text.len() + 1);
    let lines: Vec<usize> = text
        .split('\n')
        .scan(0, |at, line| {
            let start = *at;
            *at += line.len() + 1;
            Some(start)
        })
        .collect();
    let line_of = |at: usize| lines.iter().rposition(|start| *start <= at).unwrap();
    // Up from the last line visits every line that draws (the table's delimiter row draws none).
    let mut at = text.len();
    let mut visited = vec![line_of(at)];
    for _ in 0..20 {
        let next = view.vertical(at, 0.0, -1);
        if next == at {
            break;
        }
        at = next;
        visited.push(line_of(at));
    }
    assert_eq!(visited, vec![12, 11, 10, 8, 7, 6, 5, 4, 3, 2, 1, 0], "up: {visited:?}");
    let mut visited_down = vec![line_of(at)];
    for _ in 0..20 {
        let next = view.vertical(at, 0.0, 1);
        if next == at {
            break;
        }
        at = next;
        visited_down.push(line_of(at));
    }
    // Down on the last line goes to its end, still the last line.
    visited_down.dedup();
    assert_eq!(visited_down, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12], "down: {visited_down:?}");
}

#[test]
fn math_draws_as_formulas_away_from_the_caret() {
    if !fonts() {
        return;
    }
    let text = "Inline $E = mc^2$ here.\n\n$$\n\\frac{1}{3}\n$$\n\nend";
    let mut view = View::default();
    view.update(text, text.len() + 1..text.len() + 1);
    // The formula is drawn in its source's first piece; the rest of its source takes no room.
    let dollar = text.find('$').unwrap();
    let closing = text[dollar + 1..].find('$').unwrap() + dollar + 1;
    assert_eq!(view.caret(dollar + 1).x, view.caret(closing).x, "the formula's source is replaced by the formula");
    assert!(view.caret(dollar + 1).x > view.caret(dollar).x + 20.0, "the formula takes its own width");
    let block = text.find("$$").unwrap();
    let fraction = text.find("\\frac").unwrap();
    assert!(view.caret(block).height > 24.0, "the block's first line holds the formula");
    assert_eq!(view.caret(fraction).height, 0.0, "its source lines take no room");
    let drawn = view.lines().iter().flat_map(|line| line.boxed.rows.iter()).flat_map(|row| row.over.iter()).count();
    assert!(drawn >= 1, "the fraction bar is drawn");
    // On the caret's line, the source shows again.
    view.update(text, fraction..fraction);
    assert!(view.caret(fraction).height > 0.0);
}

#[test]
fn mermaid_blocks_keep_their_diagrams_room_away_from_the_caret() {
    if !fonts() {
        return;
    }
    // A diagram in a callout draws (Obsidian renders a callout's content); one in a plain quote
    // is its source as inline code, as Obsidian 1.14.5 shows it.
    let text = "Before\n\n```mermaid\ngraph TD\n  A --> B\n```\n\nAfter\n\n> [!note] Diagram\n> ```mermaid\n> pie\n> ```\n\n> ```mermaid\n> pie plain\n> ```\n";
    let source = "graph TD\n  A --> B";
    let end = text.len();
    let mut view = View::default();
    view.update(text, end..end);
    // Unsized, the block shows its source and is asked for.
    assert_eq!(view.wanted_diagrams(), vec![("mermaid", source.to_owned()), ("mermaid", "pie".to_owned())]);
    assert!(view.diagrams_in(0.0, 1e6).is_empty());
    let after = text.find("After").unwrap();
    let after_before = view.caret(after).top;
    assert!(!view.set_diagram("graph LR", 10.0, 10.0), "a source the note does not hold is not kept");
    assert!(view.set_diagram(source, 263.0, 358.4));
    assert!(!view.set_diagram(source, 263.0, 358.4), "the same size changes nothing");
    view.update(text, end..end);
    let fence = text.find("```mermaid").unwrap();
    let code = text.find("graph").unwrap();
    assert_eq!(view.caret(fence).height, 359.0, "the first line keeps the diagram's room");
    assert_eq!(view.caret(code).height, 0.0, "the source lines take none");
    let drawn = view.diagrams_in(0.0, 1e6);
    assert_eq!(drawn.len(), 1);
    assert_eq!((drawn[0].width, drawn[0].height, drawn[0].code), (263.0, 359.0, code));
    assert_eq!(drawn[0].top, view.caret(fence).top);
    assert!(view.caret(after).top > after_before + 200.0, "the lines below move down");
    assert_eq!(view.wanted_diagrams(), vec![("mermaid", "pie".to_owned())]);
    // The caret in the block: its source shows, and nothing is drawn over it.
    view.update(text, code..code);
    assert!(view.caret(code).height > 0.0);
    assert!(view.diagrams_in(0.0, 1e6).is_empty());
    assert!(view.wanted_diagrams().iter().all(|(_, wanted)| wanted != source), "the size is kept for when the caret leaves");
    // A callout's block's source drops its markers; its diagram sits inside the callout.
    view.update(text, 0..0);
    assert!(view.set_diagram("pie", 100.0, 50.0));
    view.update(text, 0..0);
    let quoted = view.diagrams_in(0.0, 1e6).into_iter().find(|diagram| diagram.source == "pie").unwrap();
    assert!(quoted.left > view.diagrams_in(0.0, 1e6)[0].left);
    // An edit that changes the source asks for the new one and forgets the old.
    let edited = text.replace("A --> B", "A --> C");
    view.update(&edited, 0..0);
    assert!(view.wanted_diagrams().contains(&("mermaid", "graph TD\n  A --> C".to_owned())));
    assert!(view.diagrams_in(0.0, 1e6).iter().all(|diagram| diagram.source == "pie"));
}

#[test]
fn the_room_below_the_note_is_half_the_visible_height() {
    if !fonts() {
        return;
    }
    let text = "# Short\n\nOne line.";
    let mut view = View::default();
    view.update(text, 0..0);
    let page = view.height();
    assert!(view.set_visible_height(800.0));
    assert!(!view.set_visible_height(800.0));
    view.update(text, 0..0);
    let last = view.caret(text.len());
    let expected = last.top + last.height + 400.0;
    assert!((view.height() as f32 - expected).abs() <= 1.0, "{} vs {expected}", view.height());
    assert!(view.height() < page, "a short note has no least height, so it does not scroll");
}

#[test]
fn html_blocks_draw_from_their_opening_tag_to_its_closing_one() {
    if !fonts() {
        return;
    }
    let text = "Intro\n\n<details>\n<summary>More</summary>\n\nHidden body.\n\n</details>\n\n<!-- a comment -->\n\nAfter\n";
    let mut view = View::default();
    view.update(text, 0..0);
    let wanted = view.wanted_diagrams();
    assert_eq!(wanted.len(), 1, "the comment stays its source: {wanted:?}");
    let (kind, source) = &wanted[0];
    assert_eq!(*kind, "html");
    assert!(source.starts_with("<details>") && source.ends_with("</details>") && source.contains("Hidden body."), "{source:?}");
    assert!(view.set_diagram(source, 700.0, 24.0));
    view.update(text, 0..0);
    let body = text.find("Hidden").unwrap();
    assert_eq!(view.caret(body).height, 0.0, "the body inside the block takes no room");
    assert_eq!(view.caret(text.find("<details>").unwrap()).height, 24.0);
    assert!(view.caret(text.find("<!--").unwrap()).height > 0.0);
    let drawn = view.diagrams_in(0.0, 1e6);
    assert_eq!((drawn.len(), drawn[0].kind), (1, "html"));
}

#[test]
fn markdown_obsidian_draws_differently_reads_as_it_does() {
    use crate::style::flag;
    if !fonts() {
        return;
    }
    let text = "Setext\n------\n\n<u>under</u> H<sub>2</sub>O x<sup>2</sup> <mark>hi</mark> <span style=\"color: #ff0000\">red</span>\n\nA [link][ref] and a note^[inline].\n\n[ref]: https://example.com \"Example\"\n\n- [-] cancelled\n- [x] done\n\n> quoted\n> - one\n\n> [!note]\n> - two\n";
    let end = text.len();
    let mut view = View::default();
    view.update(text, end..end);
    let flags = |needle: &str| view.styles().bytes.get(text.find(needle).unwrap()).copied().unwrap_or(0);
    // The setext underline is a line of its own, shown faint, not hidden.
    assert_eq!(flags("------") & flag::HIDE, 0);
    assert!(view.caret(text.find("------").unwrap()).height > 0.0);
    // Inline HTML styles its text; its tags hide.
    assert_ne!(flags("under") & flag::UNDERLINE, 0);
    assert_ne!(flags("2</sub>") & flag::SUB, 0);
    assert_ne!(flags("2</sup>") & flag::SUP, 0);
    assert_ne!(flags("hi</mark>") & flag::MARKED, 0);
    let index = (flags("red</span>") & flag::COLOR) >> flag::COLOR_SHIFT;
    assert_eq!(crate::style::interned_color(index), Some([255, 0, 0, 255]));
    assert_ne!(flags("<u>") & flag::HIDE, 0);
    // A reference link is a link; its definition reads like a footnote's.
    assert_ne!(flags("link]") & flag::LINK, 0);
    assert_ne!(flags("https://example.com") & flag::LINK, 0);
    assert_ne!(flags("ref]:") & flag::FOOTNOTE, 0);
    // An inline footnote shows, small.
    assert_ne!(flags("inline]") & flag::INLINE_NOTE, 0);
    assert_eq!(flags("^[") & flag::HIDE, 0);
    // Any task mark checks the box; only `x` strikes the text.
    assert_ne!(flags("[-]") & flag::TASK, 0);
    assert_ne!(flags("[-]") & flag::HIDE, 0);
    // A list in a plain quote keeps its marker as text; in a callout it is a list.
    let one = text.find("- one").unwrap();
    assert_eq!(view.styles().bytes.get(one).copied().unwrap_or(0) & (flag::HIDE | flag::MARK), 0);
    let two = text.find("- two").unwrap();
    assert_ne!(view.styles().bytes.get(two).copied().unwrap_or(0) & flag::HIDE, 0);
}

#[test]
fn a_reference_link_finds_its_definition_in_another_chunk() {
    if !fonts() {
        return;
    }
    let mut text = String::from("A [link][far] here.\n\n");
    for index in 0..400 {
        text.push_str(&format!("Paragraph {index} with some words to make the note long.\n\n"));
    }
    text.push_str("[far]: https://example.com\n");
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    let at = text.find("link]").unwrap();
    assert_ne!(view.styles().bytes.get(at).copied().unwrap_or(0) & crate::style::flag::LINK, 0);
}

#[test]
fn wrapped_rows_hang_and_nest_as_obsidian_lays_them_out() {
    if !fonts() {
        return;
    }
    let long = "child two with a long line that has to wrap onto the next row of the readable column to see the indent";
    let text = format!(
        "- [ ] task\n  - {long}\n- item:\n\n  > Quoted inside a list item.\n\nA very long address https://example.com/a/very/long/path/that/does/not/fit/on/one/line/of/the/column/at/all?query=1 in text.\n"
    );
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    // A wrapped item's second row starts under its text.
    let child = text.find("child").unwrap();
    let wrapped = text.find("see the").unwrap();
    assert!(view.caret(wrapped).top > view.caret(child).top, "the item wraps");
    let row_start = (child..text.len()).find(|at| text.is_char_boundary(*at) && view.caret(*at).top > view.caret(child).top).unwrap();
    assert!((view.caret(row_start).x - view.caret(child).x).abs() < 1.0, "{} vs {}", view.caret(row_start).x, view.caret(child).x);
    // A quote in a list item is set in by the item's text offset.
    let quoted = text.find("Quoted").unwrap();
    let item = text.find("item:").unwrap();
    let line = view.lines().iter().find(|line| line.range.contains(&quoted)).unwrap().style;
    assert!(view.caret(quoted).x > view.caret(item).x + 10.0, "{} vs {}, {line:?}", view.caret(quoted).x, view.caret(item).x);
    // An address breaks only where its row ends: its first row is full.
    let address = text.find("https://").unwrap();
    assert_eq!(view.caret(address).top, view.caret(text.find("A very").unwrap()).top, "an address wider than a row starts on the row it is in");
    let next_row = (address..text.len()).find(|at| view.caret(*at).top > view.caret(address).top).unwrap();
    assert!(!text[..next_row].ends_with('/'), "no break after a solidus: {:?}", &text[next_row - 5..next_row + 5]);
}

#[test]
#[ignore]
fn timing_of_an_update_on_a_large_note() {
    // Run by hand: KITION_LARGE_NOTE=<file> cargo test --release -p kition-note-view timing -- --ignored --nocapture
    let Some(path) = std::env::var_os("KITION_LARGE_NOTE") else { return };
    if !fonts() {
        return;
    }
    let text = std::fs::read_to_string(path).unwrap();
    let middle = text.find("## Section 1000").unwrap() + 15;
    let mut view = View::default();
    let started = std::time::Instant::now();
    view.update(&text, middle..middle);
    eprintln!("first update: {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    let mut edited = text.clone();
    for round in 0..3 {
        edited.insert(middle + round, 'y');
        let at = middle + round + 1;
        let started = std::time::Instant::now();
        let chunks = std::time::Instant::now();
        let _ = crate::chunk::chunks(&edited);
        let chunk_ms = chunks.elapsed().as_secs_f64() * 1000.0;
        view.update(&edited, at..at);
        eprintln!("edit {round}: update {:.1} ms (chunking alone {chunk_ms:.1} ms)", started.elapsed().as_secs_f64() * 1000.0);
    }
}

/// A note of many self-contained sections and some that are not (a table, a callout, a fence).
fn sectioned_note(sections: usize) -> String {
    let mut text = String::from("---\ntitle: Large\n---\n# Large\n");
    for i in 0..sections {
        text.push_str(&format!(
            "\n## Section {i}\n\nParagraph {i} with **bold**, *italic*, `code`, a [link](https://example.com/{i}) and a #tag. Some words to wrap the line a little further along the column.\n\n- item one {i}\n- item two\n\t- nested {i}\n- [ ] task {i}\n\n1. first\n2. second\n\n> quoted {i}\n\n中文段落 {i}：这是一段中文。\n"
        ));
        if i % 25 == 7 {
            text.push_str(&format!("\n| A | B |\n| --- | --- |\n| {i} | cell |\n\n> [!note] Callout {i}\n> Body.\n\n```js\nconst x = {i}\n```\n"));
        }
    }
    text
}

/// Sections of code blocks in several languages, with comments and strings over several lines
/// (their highlighting carries from line to line).
fn code_sections(sections: usize) -> String {
    let mut text = String::new();
    for i in 0..sections {
        text.push_str(&format!(
            "\n## Code {i}\n\n```js\n/* a comment\n   over lines {i} */\nconst s = `a\nb`; // end\nfunction f(x) {{ return x + {i} }}\n```\n\n```python\ndef g():\n    \"\"\"doc\n    string\"\"\"\n    return {i}\n```\n\n```rust\nfn h() -> u32 {{ /* x */ {i} }}\n```\n\n```sql\nSELECT a FROM t WHERE b = '{i}';\n```\n\n- item\n\n  ```css\n  .a {{ color: red; }}\n  ```\n\n> ```yaml\n> key: {i}\n> ```\n"
        ));
    }
    text
}

/// The parts of a view an edit's update must leave as the whole update does.
fn snapshot(view: &View) -> String {
    let mut out = String::new();
    for line in &view.placed {
        out.push_str(&format!(
            "{:?} {:.2} {:?} {:.2} {} {:?} {:.2} {:.2} {:.2}\n",
            line.range,
            line.y,
            line.style,
            line.boxed.height,
            line.boxed.rows.len(),
            line.fold,
            line.space,
            line.pad_top,
            line.pad_bottom
        ));
    }
    out.push_str(&format!("height {}\nbytes {:?}\nlines {:?}\nstarts {:?}\n", view.height, view.styles.bytes, view.styles.lines, view.styles.starts));
    out.push_str(&format!(
        "links {:?}\n",
        view.links.iter().map(|link| (link.range.clone(), link.text_range.clone(), link.destination_range.clone())).collect::<Vec<_>>()
    ));
    out.push_str(&format!("tables {:?}\n", view.tables.iter().map(|table| table.range.clone()).collect::<Vec<_>>()));
    out.push_str(&format!("summary {:?}\nchunks {:?}\npictures {:?}\n", view.summary, view.chunk_ranges, view.picture_lines));
    if let Some(laid) = &view.laid {
        out.push_str(&format!(
            "base {:?} {:?} {:?}\ncallouts {:?}\n",
            laid.base.bytes,
            laid.base.lines,
            laid.base.starts,
            laid.callouts.iter().map(|callout| callout.range.clone()).collect::<Vec<_>>()
        ));
    }
    out
}

#[test]
fn an_edits_update_leaves_the_view_as_the_whole_update_does() {
    if !fonts() {
        return;
    }
    edits_match_the_whole_update(sectioned_note(120), None, "Section 60", EDITS, 0x5eed);
    // No front matter and an inline title: edits in the first chunk take the edit's way too.
    let plain = sectioned_note(60).replacen("---\ntitle: Large\n---\n", "", 1);
    edits_match_the_whole_update(plain, Some("Plain"), "# Large", EDITS, 0x5eed);
}

#[test]
fn line_breaks_that_cut_chunks_otherwise_leave_the_view_as_the_whole_update_does() {
    if !fonts() {
        return;
    }
    // Mostly line breaks and blank lines (and deletions joining lines): chunks cut and joined.
    let recut_before = crate::edit::RECUT.load(std::sync::atomic::Ordering::Relaxed);
    let edits = ["\n", "\n\n", "\n", "x", "\n- ", "\n\n", "# ", "y "];
    for seed in [0x5eed, 0xbeef, 0xface] {
        edits_match_the_whole_update(sectioned_note(120), None, "Section 60", &edits, seed);
    }
    let recut = crate::edit::RECUT.load(std::sync::atomic::Ordering::Relaxed) - recut_before;
    assert!(recut > 40, "chunks were cut otherwise {recut} times");
}

#[test]
fn a_line_break_after_a_heading_leaves_the_view_as_the_whole_update_does() {
    if !fonts() {
        return;
    }
    // Typed at a heading's end, the line break's bytes match the next chunk's start as well.
    let mut text = sectioned_note(120);
    let mut view = View::default();
    let at = text.find("## Section 60").unwrap() + "## Section 60".len();
    view.update(&text, at..at);
    // Enter, Backspace, two Enters, two Backspaces.
    for (insert, removed) in [("\n", 0), ("", 1), ("\n\n", 0), ("", 2)] {
        text.replace_range(at..at + removed, insert);
        let caret = at + insert.len();
        view.update(&text, caret..caret);
        let mut whole = View::default();
        whole.update(&text, caret..caret);
        view.realize(0.0, f32::MAX);
        whole.realize(0.0, f32::MAX);
        assert_eq!(snapshot(&view), snapshot(&whole), "after {insert:?}");
    }
}

#[test]
fn a_large_notes_guessed_lines_and_tables_lay_out_as_the_exact_layout_once_realized() {
    if !fonts() {
        return;
    }
    let mut text = sectioned_note(400);
    text.push_str(&code_sections(60));
    let mut view = View::default();
    view.update(&text, 0..0);
    assert!(view.styles.starts.len() > ESTIMATE_FROM_LINES && view.tables.len() > 10);
    // Far from the caret, code blocks are not highlighted yet.
    assert!(view.lights.unlit().len() > 100, "{} blocks unlit", view.lights.unlit().len());
    // Far from the caret, lines and whole tables are guessed.
    let last_table = view.styles.line_of(view.tables.last().unwrap().range.start);
    assert!(view.placed[last_table].boxed.estimated);
    // In view of a table's rows, its grid comes laid out.
    let top = view.placed[last_table].y;
    view.realize_between(top, top + 1.0);
    assert!(view.tables_in(top, top + 1.0).iter().any(|grid| grid.index == view.tables.len() - 1));
    view.realize(0.0, f32::MAX);
    // Once laid out, every block is highlighted as parsing the note whole highlights it.
    assert!(view.lights.unlit().is_empty());
    assert_eq!(view.laid.as_ref().unwrap().base.bytes, Styles::of(&text).bytes);
    let active = view.laid.as_ref().unwrap().active.clone();
    let (exact, height) = layout::lay_out(
        &view.theme,
        &view.text,
        &view.styles,
        &view.tables,
        &[],
        &[],
        &active,
        &[],
        &[],
        false,
        None,
        &[],
        &mut layout::Cache::default(),
    );
    assert_eq!(view.placed.len(), exact.len());
    for (line, (realized, laid)) in view.placed.iter().zip(&exact).enumerate() {
        assert_eq!(*realized.boxed, *laid.boxed, "line {line}");
        assert_eq!(
            (realized.y, realized.space, realized.pad_top, realized.pad_bottom),
            (laid.y, laid.space, laid.pad_top, laid.pad_bottom),
            "line {line}"
        );
    }
    assert_eq!(view.height, height);
}

#[test]
fn edits_in_a_large_note_with_code_leave_the_view_as_the_whole_update_does() {
    if !fonts() {
        return;
    }
    // A large note: its code blocks far from the caret are highlighted only when laid out.
    let mut text = sectioned_note(100);
    text.push_str(&code_sections(120));
    let edits = ["x", "y", "\n", "*/", "/*", "```", "\"", "`", " ", "z", "\n\n", "$$"];
    edits_match_the_whole_update(text, None, "## Code 60", &edits, 0xc0de);
}

#[test]
fn metadata_lines_typed_in_a_large_note_leave_the_view_as_the_whole_update_does() {
    if !fonts() {
        return;
    }
    // `---` lines open and close metadata blocks across chunks (pulldown-cmark reads one wherever
    // a block starts with `---`): such an edit takes the whole update, the others the edit's way.
    let mut text = sectioned_note(100);
    text.push_str(&code_sections(60));
    let edits = ["x", "\n", "---", "\n---\n", "...", "\n...\n", "$$", "\n\n", " ", "```"];
    for seed in [0x3e7a, 0xd0c5] {
        edits_match_the_whole_update_taking(text.clone(), None, "## Code 30", &edits, seed, 20);
    }
}

const EDITS: &[&str] = &["x", "y", " ", "\n", "- ", "**", "# ", "中", "`", "\n\n", "> ", "1. ", "```", "| a |", "[r]: x", "\t"];

/// Random edits of `text` from `start` on (with `edits`), each update held to a fresh view's
/// whole update; at least `least` of the 400 take the edit's way.
fn edits_match_the_whole_update_taking(mut text: String, title: Option<&str>, start: &str, edits: &[&str], seed: u64, least: usize) {
    let fresh = || {
        let mut view = View::default();
        if let Some(title) = title {
            view.set_title(Some(title.to_owned()));
        }
        view
    };
    let mut view = fresh();
    let mut caret = text.find(start).unwrap() + start.len();
    view.update(&text, caret..caret);
    let mut seed = seed;
    let mut next = |bound: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize % bound.max(1)
    };
    let taken_before = crate::edit::TAKEN.load(std::sync::atomic::Ordering::Relaxed);
    for round in 0..400 {
        // Mostly typing near the caret, sometimes elsewhere, sometimes deleting.
        if next(10) == 0 {
            caret = next(text.len());
        }
        while !text.is_char_boundary(caret) {
            caret -= 1;
        }
        if next(4) == 0 && caret > 0 {
            let mut start = caret - 1;
            while !text.is_char_boundary(start) {
                start -= 1;
            }
            text.replace_range(start..caret, "");
            caret = start;
        } else {
            let insert = edits[next(edits.len())];
            text.insert_str(caret, insert);
            caret += insert.len();
        }
        view.update(&text, caret..caret);
        let mut whole = fresh();
        whole.update(&text, caret..caret);
        // A large note guesses the heights of lines far from the caret: compare laid out whole.
        view.realize(0.0, f32::MAX);
        whole.realize(0.0, f32::MAX);
        let (fast, full) = (snapshot(&view), snapshot(&whole));
        if fast != full {
            let at = fast.lines().zip(full.lines()).position(|(a, b)| a != b).unwrap_or(0);
            let (a, b) = (fast.lines().nth(at).unwrap_or(""), full.lines().nth(at).unwrap_or(""));
            // Where in the snapshot line they part (a long line shows from there).
            let from = a.chars().zip(b.chars()).position(|(x, y)| x != y).unwrap_or(0).saturating_sub(60);
            // Whether the note's chunks parse as the whole note does (else the chunker is wrong,
            // not the edit's update).
            let ranges = crate::chunk::chunks(&text);
            let labels = crate::style::reference_labels(&text);
            let chunked = crate::chunk::ChunkCache::default().joined(&text, &ranges, &labels, &mut Vec::new());
            let whole_styles = Styles::of(&text);
            if let Some(line) = chunked.styles.lines.iter().zip(&whole_styles.lines).position(|(a, b)| a != b) {
                let at = whole_styles.starts[line];
                let chunk = ranges.iter().find(|range| range.contains(&at)).cloned().unwrap_or(0..0);
                let before = ranges.iter().rev().find(|range| range.end <= chunk.start).cloned().unwrap_or(0..0);
                eprintln!("chunks parse otherwise at line {line}: chunk {:?} after {:?}", text.get(chunk.clone()), text.get(before));
            }
            // The first byte whose styles differ, and the text around it.
            let byte = view.styles.bytes.iter().zip(&whole.styles.bytes).position(|(x, y)| x != y);
            let near = byte.map(|byte| {
                (byte, caret, text.get(byte.saturating_sub(60)..byte).unwrap_or(""), text.get(byte..(byte + 40).min(text.len())).unwrap_or(""))
            });
            panic!(
                "round {round}: the edit's update differs at snapshot line {at} (from character {from}):\n  edit  {}\n  whole {}\n  text near the caret {:?}\n  first styled byte apart (byte, caret, text before and after it) {near:?}",
                a.chars().skip(from).take(300).collect::<String>(),
                b.chars().skip(from).take(300).collect::<String>(),
                text.get(caret.saturating_sub(80)..(caret + 40).min(text.len())).unwrap_or("")
            );
        }
    }
    let taken = crate::edit::TAKEN.load(std::sync::atomic::Ordering::Relaxed) - taken_before;
    assert!(taken >= least, "the edit's update was taken {taken} times of 400");
}

fn edits_match_the_whole_update(text: String, title: Option<&str>, start: &str, edits: &[&str], seed: u64) {
    edits_match_the_whole_update_taking(text, title, start, edits, seed, 101);
}

#[test]
#[ignore]
fn timing_of_parsing_a_large_note() {
    // Run by hand: KITION_LARGE_NOTE=<file> cargo test --release -p kition-note-view timing_of_parsing -- --ignored --nocapture
    let Some(path) = std::env::var_os("KITION_LARGE_NOTE") else { return };
    let text = std::fs::read_to_string(path).unwrap();
    let time = |label: &str, run: &dyn Fn()| {
        let started = std::time::Instant::now();
        run();
        eprintln!("{label}: {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    };
    time("chunks", &|| drop(crate::chunk::chunks(&text)));
    time("styles", &|| drop(crate::style::Styles::of(&text)));
    time("index", &|| drop(kition_note_markdown::index(&text)));
    time("pulldown alone", &|| {
        let _ = pulldown_cmark::Parser::new_ext(&text, kition_note_markdown::options()).into_offset_iter().count();
    });
    time("reference labels", &|| drop(crate::style::reference_labels(&text)));
    time("chunked parse", &|| drop(crate::chunk::ChunkCache::default().parse(&text)));
}

#[test]
#[ignore]
fn timing_loop_of_chunked_parsing() {
    // For a sampling profiler: KITION_LARGE_NOTE=<file> cargo test --release -p kition-note-view timing_loop -- --ignored
    let Some(path) = std::env::var_os("KITION_LARGE_NOTE") else { return };
    let text = std::fs::read_to_string(path).unwrap();
    for _ in 0..40 {
        drop(crate::chunk::ChunkCache::default().parse(&text));
    }
}

#[test]
fn the_caret_shows_only_the_syntax_it_touches_as_obsidian_does() {
    use crate::style::{Styles, flag};
    let text = "A **bold** and [link](https://x.y) end.\n\n## Head\n\n> quote\n\n- item\n";
    // Whether the byte at `needle` (plus `offset`) hides with the selection at `selection`.
    let hidden = |selection: std::ops::Range<usize>, needle: &str, offset: usize| {
        let mut styles = Styles::of(text);
        let start = text.get(..selection.start).and_then(|before| before.rfind('\n')).map_or(0, |at| at + 1);
        let end = text.get(selection.end..).and_then(|after| after.find('\n')).map_or(text.len(), |at| selection.end + at);
        styles.conceal(text, &(start..end), &selection);
        let at = text.find(needle).unwrap() + offset;
        styles.bytes.get(at).is_some_and(|value| value & flag::HIDE != 0)
    };
    let at = |needle: &str, offset: usize| {
        let at = text.find(needle).unwrap() + offset;
        at..at
    };
    // In the bold: its marks show, the link's syntax on the same line does not.
    assert!(!hidden(at("bold", 1), "**bold", 0));
    assert!(hidden(at("bold", 1), "](https", 0));
    // Just after its closing marks it still shows; past them it hides.
    assert!(!hidden(at("** and", 2), "**bold", 0));
    assert!(hidden(at(" end.", 2), "**bold", 0));
    // In the link: its brackets and address show.
    assert!(!hidden(at("link]", 1), "](https", 0));
    // A heading's marks and a quote's show with the caret anywhere on their line.
    assert!(!hidden(at("Head", 2), "## Head", 0));
    assert!(!hidden(at("quote", 2), "> quote", 0));
    // A list item's marker shows only when the selection reaches it.
    assert!(hidden(at("item", 2), "- item", 0));
    assert!(!hidden(at("- item", 0), "- item", 0));
    // Away from the caret's lines, everything hides.
    assert!(hidden(at("item", 2), "**bold", 0));
}

#[test]
fn four_spaces_indent_a_list_as_a_tab_does_as_obsidian_lays_them_out() {
    if !fonts() {
        return;
    }
    let text = "plain\n\n- [ ] parent\n\t- tabfirst\n    - four\n\t- tabbed\n    1. ordered\n\t1. numbered\n- item\n    continuation four\n- item\n  continuation two\n\n> - quoted\n>     - nested quoted\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let x = |needle: &str| view.caret(text.find(needle).unwrap()).x;
    let left = x("plain");
    // Four spaces are one indent (Obsidian's 2.25 em `cm-indent`), as wide as a tab.
    assert!((x("four") - x("tabbed")).abs() < 0.5, "{} vs {}", x("four"), x("tabbed"));
    assert!((x("ordered") - x("numbered")).abs() < 0.5, "{} vs {}", x("ordered"), x("numbered"));
    // A continuation indented by whole indents is not padded (36 px in at 16 px text); one by two
    // spaces is, by 1 em (24 px in).
    assert!((x("continuation four") - left - 36.0).abs() < 0.5, "{}", x("continuation four") - left);
    let two = x("continuation two") - left;
    assert!((two - 24.0).abs() < 1.5, "{two}");
    // A quote's text past its marks indents the same way.
    assert!((x("- nested") - x("- quoted") - 36.0).abs() < 0.5, "{} vs {}", x("- nested"), x("- quoted"));
}

#[test]
fn tables_fit_the_column_and_break_and_typeset_cells_as_obsidian_does() {
    if !fonts() {
        return;
    }
    let long = "A cell with a very long text that has to wrap inside the table cell because the table is wider than the column allows";
    let text = format!(
        "| Long | Table |\n| --- | --- |\n| {long} | short |\n\n| A | B |\n| --- | --- |\n| top<br>bottom | $x^2$ |\n\nKeys and a<br>break here.\n\nend\n"
    );
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    let edges = |needle: &str| {
        let at = text.find(needle).unwrap();
        let line = view.lines().iter().find(|line| line.range.contains(&at)).unwrap();
        line.boxed.rows[0].table.as_ref().unwrap().columns.clone()
    };
    // A table whose text is wider than the column is at most the column wide (the browser's
    // automatic table layout), its short column as wide as its text or Obsidian's 6 ch.
    let columns = edges("A cell");
    let right = *columns.last().unwrap();
    assert!((right - view.theme().content_width()).abs() < 1.0, "{right} vs {}", view.theme().content_width());
    let short = columns[2] - columns[1];
    assert!(short < 80.0 && short > 50.0, "{columns:?}");
    // `<br>` starts a new row, in a cell and in a paragraph.
    let x = |needle: &str| view.caret(text.find(needle).unwrap());
    assert!(x("bottom").top > x("top<br>").top, "{:?} vs {:?}", x("bottom"), x("top<br>"));
    assert!(x("break here").top > x("Keys").top);
    assert!((x("break here").x - x("Keys").x).abs() < 0.5);
    // Inline math in a cell draws as its formula (in its first piece, the rest of its source
    // takes no room).
    assert_eq!(x("^2$").x, x("2$ |").x);
}

#[test]
fn nested_callouts_and_quotes_in_callouts_nest_as_obsidian_renders_them() {
    if !fonts() {
        return;
    }
    let long = "An example body that is long enough to wrap onto a second row of the readable column inside the callout box.";
    let text = format!(
        "> [!note] Outer\n> Outer body.\n>\n> > [!warning] Inner\n> > Inner body.\n\n> [!info] Q\n> > Quoted.\n\n> [!example] E\n> {long}\n\nend\n"
    );
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    let line = |needle: &str| {
        let at = text.find(needle).unwrap();
        view.lines().iter().find(|line| line.range.contains(&at)).unwrap().clone()
    };
    let x = |needle: &str| view.caret(text.find(needle).unwrap()).x;
    // The nested callout keeps its own type and the outer one's.
    let inner = line("Inner body").style;
    assert_eq!((inner.callout, inner.outer), (theme::callout_type("warning"), theme::callout_type("note")));
    // Its text 24 px further in (the outer box's padding), its box 12 px inside the outer's top,
    // and below its last line its own 28 px and the outer box's 28 px.
    assert!((x("Inner body") - x("Outer body") - 24.0).abs() < 0.5, "{} vs {}", x("Inner body"), x("Outer body"));
    assert_eq!(line("Inner\n").pad_top, 12.0);
    assert_eq!(line("Inner body").pad_bottom, 56.0);
    assert_eq!(line("Outer body").pad_bottom, 0.0, "the outer callout goes on around the nested one");
    // A plain quote in a callout is a rendered quote: its text 26 px in.
    assert!((x("Quoted") - x("Outer body") - 26.0).abs() < 0.5, "{} vs {}", x("Quoted"), x("Outer body"));
    // A callout's text wraps clear of its box's 12 px right padding.
    let wrapped = line("An example");
    assert!(wrapped.boxed.rows.len() > 1);
    let right = view.theme().margin + view.theme().content_width() - 12.0;
    for row in &wrapped.boxed.rows {
        let end = row.stops.iter().map(|stop| stop.x).fold(0.0, f32::max) + view.theme().margin;
        assert!(end <= right + 0.5, "{end} > {right}");
    }
}

#[test]
fn an_indented_code_block_is_indented_inline_code_as_obsidian_shows_it() {
    if !fonts() {
        return;
    }
    let text = "After.\n\n    An indented code block\n    with two lines.\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let first = text.find("An indented").unwrap();
    let second = text.find("with two").unwrap();
    let line = view.lines().iter().find(|line| line.range.contains(&first)).unwrap();
    assert_eq!(line.style.block, style::Block::Indented);
    // Its rows are text rows (24 px at 16 px text), not a code block's.
    assert_eq!(view.caret(second).top - view.caret(first).top, 24.0);
    // Its first indent is one list indent (36 px), then the code as inline code, on every line:
    // its first letter (and the caret there) after the code's 4.2 px padding, 40.2 px in, as
    // measured in Obsidian.
    let left = view.caret(0).x;
    for at in [first, second] {
        let x = view.caret(at).x - left;
        assert!((x - 40.2).abs() < 0.5, "{x}");
    }
    let flags = &view.styles().bytes;
    assert!(flags[second - 4..second].iter().all(|value| value & style::flag::CODE == 0), "the indent is not code");
    assert!(flags[second] & style::flag::CODE != 0);
}

#[test]
fn a_math_block_is_at_least_a_text_line_tall_as_obsidian_sets_it() {
    if !fonts() {
        return;
    }
    let text = "Top.\n\n$$\na^2 + b^2 = c^2\n$$\n\n$$\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}$$\n\nThe end.\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let height = |needle: &str| view.lines().iter().find(|line| text[line.range.clone()].starts_with(needle)).unwrap().boxed.height;
    // A short formula sits in a text line's box (24 px) inside MathJax's 16 px padding: 56 px.
    assert!((height("$$") - 56.0).abs() < 0.5, "{}", height("$$"));
    // A tall one is its own height in the padding (Obsidian's 74.4 px; its fonts differ a little).
    let tall = height("$$\\sum");
    assert!((tall - 74.4).abs() < 3.0, "{tall}");
}

#[test]
fn a_callout_title_and_a_cells_link_look_as_obsidian_draws_them() {
    if !fonts() {
        return;
    }
    let text = "> [!tip] Title with **bold** and `code`\n> Body.\n\n| A | B |\n| --- | --- |\n| [a longer link](https://example.com) | x |\n\n| A | B |\n| --- | --- |\n| [a longer link](page) | x |\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = |needle: &str| {
        let at = text.find(needle).unwrap();
        view.lines().iter().find(|line| line.range.contains(&at)).unwrap().clone()
    };
    // The title is semibold in the kind's color, its bold 800, its inline code in the text's color.
    let title = line("Title");
    let tip = theme::callout_color(&Theme::default(), theme::callout_type("tip"));
    assert!(title.boxed.styles.iter().any(|style| style.weight == 600 && style.color == tip), "{:?}", title.boxed.styles);
    assert!(title.boxed.styles.iter().any(|style| style.weight == 800 && style.color == tip), "{:?}", title.boxed.styles);
    assert!(title.boxed.styles.iter().any(|style| style.family == kition_note_fonts::Family::Mono && style.color == view.theme().text));
    // A link to the web in a cell has the external link icon after it (0.9 em more room).
    let edges = |needle: &str| line(needle).boxed.rows[0].table.as_ref().unwrap().columns.clone();
    let (web, page) = (edges("https://"), edges("(page)"));
    assert!((web[1] - page[1] - 0.9 * 16.0).abs() < 1.0, "{web:?} vs {page:?}");
}

#[test]
fn a_plain_quote_shows_block_syntax_as_text_as_obsidian_does() {
    if !fonts() {
        return;
    }
    let text = "> ## A heading in a quote\n> Text.\n\n> Code:\n> ```js\n> const a = 1\n> ```\n\n> [!note] Callout\n> ## A rendered heading\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = |needle: &str| {
        let at = text.find(needle).unwrap();
        view.lines().iter().find(|line| line.range.contains(&at)).unwrap().clone()
    };
    // A heading in a plain quote is text, its marks shown.
    assert_eq!(line("A heading").style.block, style::Block::Paragraph);
    let marks = text.find("## A heading").unwrap();
    assert!(view.caret(marks + 3).x > view.caret(marks).x + 10.0, "the marks take room");
    // A code block in a plain quote is lines of inline code, its fences too, 24 px apart.
    for needle in ["```js", "const a", "```\n\n"] {
        let at = text.find(needle).unwrap();
        assert_eq!(line(needle).style.block, style::Block::Paragraph, "{needle}");
        assert!(view.styles().bytes[at] & style::flag::CODE != 0, "{needle}");
    }
    assert_eq!(view.caret(text.find("const").unwrap()).top - view.caret(text.find("```js").unwrap()).top, 24.0);
    // A callout renders its heading.
    assert!(matches!(line("A rendered").style.block, style::Block::Heading(2)));
}

#[test]
fn a_callouts_code_block_and_table_sit_inside_it_as_obsidian_renders_them() {
    if !fonts() {
        return;
    }
    let text = "> [!note] Code\n> ```python\n> print(\"hi\")\n> ```\n\n> [!warning] Table\n> | A | B |\n> | --- | --- |\n> | 1 | 2 |\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = |needle: &str| {
        let at = text.find(needle).unwrap();
        view.lines().iter().find(|line| line.range.contains(&at)).unwrap().clone()
    };
    let left = view.caret(text.find("Code").unwrap()).x - 24.0 - 1.4 * 16.0;
    // The code 40 px in (the callout's 24 px and the box's 16 px), its quote marks hidden; its
    // fences the box's 12 px padding, its line 24 px.
    let print = view.caret(text.find("print").unwrap()).x;
    assert!((print - left - 40.0).abs() < 1.0, "{print} vs {left}");
    assert_eq!(line("```python").boxed.height, 12.0);
    assert_eq!(line("print").boxed.height, 24.0);
    let boxes = view.code_blocks_in(0.0, 1e6);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].language, "python");
    assert_eq!(boxes[0].right, view.theme().width - view.theme().margin - 12.0);
    // The table 24 px in, without margins of its own (they fold into the callout's).
    let header = line("| A |");
    assert_eq!(header.boxed.rows[0].table.as_ref().unwrap().columns[0], 24.0);
    assert_eq!(header.pad_top, 0.0);
}

#[test]
fn a_cell_with_a_word_wider_than_the_column_keeps_the_table_in_the_column() {
    if !fonts() {
        return;
    }
    let text = format!("| A | B |\n|---|---|\n| line1 {} | x |\n| next | y |\n\nend\n", "a".repeat(300));
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    let at = text.find("line1").unwrap();
    let line = view.lines().iter().find(|line| line.range.contains(&at)).unwrap();
    let columns = line.boxed.rows[0].table.as_ref().unwrap().columns.clone();
    let right = *columns.last().unwrap();
    assert!(right <= view.theme().content_width() + 1.0, "{columns:?}");
    // The long word breaks over the cell's rows.
    assert!(line.boxed.height > 3.0 * 16.0 * 1.3, "{}", line.boxed.height);
}

#[test]
fn display_math_in_a_list_a_quote_and_a_callout_renders_as_obsidian_lays_it_out() {
    if !fonts() {
        return;
    }
    let text = "- Item:\n\n  $$\n  x = 1\n  $$\n\n> Quote:\n> $$\n> y = 2\n> $$\n\n> [!note] Callout\n> $$\n> z = 3\n> $$\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = |needle: &str| view.lines().iter().find(|line| text[line.range.clone()].starts_with(needle)).unwrap().clone();
    // Each draws its formula (glyphs on its first line), not its source.
    for opening in ["  $$", "> $$\n> y", "> $$\n> z"] {
        let at = text.find(opening).unwrap();
        let placed = view.lines().iter().find(|line| line.range.start == at).unwrap();
        assert!(!placed.boxed.rows[0].glyphs.is_empty(), "{opening:?}");
    }
    // In a list item its indent is a 25 px line above the 56 px block, and its lines take no
    // list padding.
    let item = line("  $$");
    assert_eq!((item.boxed.height, item.pad_bottom), (81.0, 0.0));
    // In a plain quote it sits between two text lines: 104 px.
    let at = text.find("> $$\n> y").unwrap();
    let quoted = view.lines().iter().find(|line| line.range.start == at).unwrap();
    assert_eq!(quoted.boxed.height, 104.0);
    // In a callout it sits right under the title, 12 px above the box's end.
    assert_eq!(line("> [!note]").pad_bottom, 0.0);
    // (on the block's first line: its other lines take no room and draw no box)
    let at = text.find("> $$\n> z").unwrap();
    let block = view.lines().iter().find(|line| line.range.start == at).unwrap();
    assert_eq!(block.pad_bottom, 12.0);
}

#[test]
fn a_www_address_draws_as_a_link_as_obsidian_draws_it() {
    let text = "A URL www.example.com and www.x here.\n";
    let styles = style::Styles::of(text);
    let at = text.find("www.example").unwrap();
    assert!(styles.bytes[at..at + "www.example.com".len()].iter().all(|value| value & style::flag::LINK != 0));
    assert!(styles.bytes[at + "www.example.com".len()] & style::flag::LINK == 0);
    let short = text.find("www.x").unwrap();
    assert!(styles.bytes[short] & style::flag::LINK == 0);
}

#[test]
fn an_inline_footnote_keeps_its_texts_styling_as_obsidian_renders_it() {
    let text = "One^[inline *note* text] here.\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let flags = &view.styles().bytes;
    let star = text.find("*note").unwrap();
    assert!(flags[star] & style::flag::HIDE != 0, "the emphasis marks hide away from the caret");
    assert!(flags[star + 1] & style::flag::EMPHASIS != 0 && flags[star + 1] & style::flag::INLINE_NOTE != 0);
    assert!(flags[text.find("inline").unwrap()] & style::flag::INLINE_NOTE != 0);
}

#[test]
fn the_caret_in_an_element_shows_every_mark_inside_it_as_obsidian_does() {
    use crate::style::{Styles, flag};
    let text = "==mark with **bold** in== end *x*\n\nA <kbd>Ctrl</kbd> key.\n";
    let at = |needle: &str| text.find(needle).unwrap();
    let conceal = |caret: usize| {
        let mut styles = Styles::of(text);
        let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
        let end = text[caret..].find('\n').map_or(text.len(), |at| caret + at);
        styles.conceal(text, &(start..end), &(caret..caret));
        styles
    };
    // In the highlight: its marks and the bold's inside it show; the italic's after it do not.
    let styles = conceal(at("mark with") + 2);
    assert!(styles.bytes[at("**bold")] & flag::HIDE == 0);
    assert!(styles.bytes[at("*x*")] & flag::HIDE != 0);
    // In the bold inside it: the highlight's marks show too.
    let styles = conceal(at("bold**") + 2);
    assert!(styles.bytes[at("==mark")] & flag::HIDE == 0);
    // Shown, a highlight's marks are on its yellow.
    assert!(styles.bytes[at("==mark")] & flag::HIGHLIGHT != 0);
    // In an inline HTML element: both its tags show, and its text loses the element's styling.
    let styles = conceal(at("Ctrl") + 4);
    assert!(styles.bytes[at("<kbd>")] & flag::HIDE == 0 && styles.bytes[at("</kbd>")] & flag::HIDE == 0);
    assert!(styles.bytes[at("Ctrl")] & flag::CODE == 0);
    // Away from it, it is a key again.
    let styles = conceal(at("key."));
    assert!(styles.bytes[at("Ctrl")] & flag::CODE != 0 && styles.bytes[at("<kbd>")] & flag::HIDE != 0);
}

#[test]
fn an_indented_heading_is_a_text_line_as_obsidian_shows_it() {
    use crate::style::{Styles, flag};
    let text = "Top.\n\n  ## Indented heading\n\nend\n";
    let mut styles = Styles::of(text);
    let line = styles.line_of(text.find("Indented").unwrap());
    assert_eq!(styles.lines[line].block, style::Block::Paragraph);
    let end = text.len();
    styles.conceal(text, &(end..end), &(end..end));
    let hashes = text.find("## ").unwrap();
    assert!((hashes..hashes + 3).all(|at| styles.bytes[at] & flag::HIDE != 0), "its marks hide away from the caret");
    assert!(styles.bytes[hashes - 1] & flag::HIDE == 0, "its indent stays");
    assert!(styles.bytes[text.find("Indented").unwrap()] & (flag::HIDE | flag::STRONG) == 0);
}

#[test]
fn a_numbered_items_number_starts_where_obsidian_starts_it() {
    if !fonts() {
        return;
    }
    let text = "plain\n\n1. One\n\n10. Ten\n100. Hundred\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let x = |needle: &str| view.caret(text.find(needle).unwrap()).x;
    let left = x("plain");
    // `1. ` keeps its text 30 px in; a wider number starts 12 px in and pushes its text on
    // (Obsidian: `10. ` 40 px, `100. ` 49 px).
    assert!((x("One") - left - 30.0).abs() < 1.0, "{}", x("One") - left);
    // (its first digit's end is past 12 px)
    assert!(x("0. Ten") - left > 12.0 + 4.0 && x("00. Hundred") - left > 12.0 + 4.0);
    assert!(x("Ten") - left > 37.0 && x("Hundred") - x("Ten") > 7.0, "{} {}", x("Ten") - left, x("Hundred") - left);
}

#[test]
fn a_footnote_definitions_indented_line_indents_as_obsidian_draws_it() {
    if !fonts() {
        return;
    }
    let text = "Text[^n].\n\n[^n]: A note.\n    Its second line.\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let left = view.caret(text.find("Text").unwrap()).x;
    let x = view.caret(text.find("Its").unwrap()).x - left;
    // One indent at the definition's 14 px: 31.5 px.
    assert!((x - 31.5).abs() < 0.5, "{x}");
}

#[test]
fn a_footnote_definition_and_a_done_task_reveal_as_obsidian_reveals_them() {
    use crate::style::{Styles, flag};
    let text = "- [x] done task\n\nText[^1].\n\n[^1]: The definition with **bold**.\n";
    let at = |needle: &str| text.find(needle).unwrap();
    let conceal = |caret: usize| {
        let mut styles = Styles::of(text);
        let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
        let end = text[caret..].find('\n').map_or(text.len(), |at| caret + at);
        styles.conceal(text, &(start..end), &(caret..caret));
        styles
    };
    // In a definition's text: its label and the bold's marks stay hidden.
    let styles = conceal(at("The def") + 5);
    assert!(styles.bytes[at("[^1]:")] & flag::HIDE != 0 && styles.bytes[at("**bold")] & flag::HIDE != 0);
    // In its bold: the bold's marks show; in its label: the label shows.
    assert!(conceal(at("bold**") + 2).bytes[at("**bold")] & flag::HIDE == 0);
    assert!(conceal(at("[^1]:") + 1).bytes[at("[^1]:")] & flag::HIDE == 0);
    if !fonts() {
        return;
    }
    // A done task with its marker shown is struck through, its marker too.
    let mut view = View::default();
    view.update(text, 0..0);
    let line = view.lines().iter().find(|line| line.range.start == 0).unwrap();
    assert!(line.boxed.styles.iter().all(|style| style.strike), "{:?}", line.boxed.styles);
}

#[test]
fn the_caret_in_a_folded_section_is_drawn_at_the_folded_lines_end() {
    if !fonts() {
        return;
    }
    let text = "# Head\n\nhidden one\nhidden two\n\n# Next\n";
    let mut view = View::default();
    let end = text.find("\n\n# Next").unwrap();
    view.update_folded(text, end..end, &[0]);
    let head = view.lines().iter().find(|line| line.range.start == 0).unwrap().clone();
    let hidden = view.lines().iter().find(|line| text[line.range.clone()] == *"hidden two").unwrap().clone();
    assert_eq!((head.fold, hidden.boxed.height), (Some(true), 0.0), "the section is folded");
    // Obsidian's fold ends at the section's last character, the caret there drawn on the
    // folded line, at its end.
    let caret = view.caret(end);
    let last = head.boxed.rows.last().unwrap();
    assert_eq!(caret.top, head.y + last.top);
    assert!((caret.x - view.theme().margin - last.stops.last().unwrap().x).abs() < 0.5);
}

#[test]
fn up_and_down_into_a_quote_or_callout_land_as_obsidian_does() {
    if !fonts() {
        return;
    }
    let text = "plain line\n> abc def\n\nplain line\n> [!note] Title\n> body\n\nab\n> > deep\n";
    let mut view = View::default();
    view.update(text, 0..0);
    // Obsidian lands on the line as it shows with the caret on it, its marks in the quote's
    // padding: from a line's start, Down lands at the quote line's start (`‸> abc`).
    let down = view.vertical(0, view.caret(0).x, 1);
    assert_eq!(down, text.find("> abc").unwrap());
    // From past the marks' width, at the text (`> ‸abc`).
    let from = text.find("ab\n> >").unwrap();
    let past = view.vertical(from + 2, view.theme().margin + 30.0, 1);
    assert_eq!(past, text.find("deep").unwrap());
    // From among the marks, between them: nearest the goal (a nested quote's `> ‸> deep`).
    let marks = text.find("> > deep").unwrap();
    let among = view.vertical(
        from,
        view.theme().margin + view.theme().indent_of(view.lines().iter().find(|line| line.range.start == marks).unwrap().style) - 13.0,
        1,
    );
    assert!(among > marks && among < text.find("deep").unwrap(), "{among}");
    // Into a callout's body from its title's start: the body line's start.
    let title = text.find("> [!note]").unwrap();
    let down = view.vertical(title, view.theme().margin, 1);
    assert_eq!(down, text.find("> body").unwrap());
}

#[test]
fn a_footnote_definitions_wrapped_indented_line_hangs_as_obsidian_lays_it_out() {
    if !fonts() {
        return;
    }
    let long = "a long continuation line that keeps going and going so that it has to wrap onto another row of the column for sure, and then more";
    let text = format!("Text[^n].\n\n[^n]: A note.\n    {long}\n\nend\n");
    let mut view = View::default();
    view.update(&text, text.len()..text.len());
    let first = text.find("a long").unwrap();
    let row_start = (first..text.len()).find(|at| view.caret(*at).top > view.caret(first).top).unwrap();
    assert!((view.caret(row_start).x - view.caret(first).x).abs() < 0.5, "{} vs {}", view.caret(row_start).x, view.caret(first).x);
}

#[test]
fn empty_and_invalid_front_matter_show_as_obsidian_shows_them() {
    if !fonts() {
        return;
    }
    // Empty: no room at all, the heading where it would be without it.
    let text = "---\n---\n# Head\n\nBody.\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let head = view.lines().iter().find(|line| text[line.range.clone()].starts_with("# Head")).unwrap();
    let plain = {
        let mut other = View::default();
        let bare = "# Head\n\nBody.\n";
        other.update(bare, bare.len()..bare.len());
        other.lines()[0].y
    };
    assert_eq!(head.y, plain);
    // Not YAML: its source, in red monospace on 24 px lines.
    let text = "---\ntitle: [unclosed\n---\n# Head\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = view.lines().iter().find(|line| text[line.range.clone()].starts_with("title")).unwrap();
    assert_eq!(line.style.block, style::Block::Frontmatter);
    assert_eq!(line.boxed.height, 24.0);
    assert!(line.boxed.styles.iter().any(|style| style.color == view.theme().invalid && style.family == kition_note_fonts::Family::Mono));
}

#[test]
fn a_callout_in_a_plain_quote_is_its_text_as_obsidian_shows_it() {
    let text = "> A quote:\n> > [!tip] Inner\n> > - item\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let at = text.find("Inner").unwrap();
    let line = view.lines().iter().find(|line| line.range.contains(&at)).unwrap();
    assert_eq!(line.style.callout, 0);
    let marker = text.find("[!tip]").unwrap();
    assert!(view.styles().bytes[marker + 2] & style::flag::COMMENT == style::flag::COMMENT, "its kind is faint");
}

#[test]
fn a_callouts_quote_and_nested_list_lay_out_as_obsidian_renders_them() {
    if !fonts() {
        return;
    }
    let text = "> [!note] Title\n> > quoted\n> - item\n>   - nested item\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = |needle: &str| view.lines().iter().find(|line| text[line.range.clone()].contains(needle)).unwrap().clone();
    // The list after the quote is 16 px from it (their rendered margins).
    assert_eq!(line("- item").space, 16.0);
    // The nested item's text is a list indent (36 px) past its parent's.
    let x = |needle: &str| view.caret(text.find(needle).unwrap()).x;
    assert!((x("nested item") - x("item\n") - 36.0).abs() < 0.5, "{} vs {}", x("nested item"), x("item\n"));
}

#[test]
fn a_nested_list_in_a_callout_has_its_indent_guide_as_obsidian_draws_it() {
    if !fonts() {
        return;
    }
    let text = "> [!note] Title\n> - item\n>   - nested item\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let nested = view.caret(text.find("nested").unwrap());
    let parent = view.caret(text.find("item\n").unwrap());
    // 13.6 px left of the parent item's text, down the nested line.
    let x = (parent.x - 13.6).round() as f64;
    let y = (nested.top + nested.height / 2.0) as f64;
    let pixel = |x: f64| {
        let rgba = view.frame(x, y, 1.0, 1, 1);
        [rgba[0], rgba[1], rgba[2]]
    };
    let tint = pixel(x - 6.0);
    assert_ne!(pixel(x - 0.5), tint, "a guide at {x}");
}

#[test]
fn front_matter_that_is_not_yaml_marks_its_errors_line_as_obsidian_does() {
    if !fonts() {
        return;
    }
    let text = "---\ntitle: [unclosed\ntags: - bad\n: no key\n---\n# Head\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let line = view.lines().iter().find(|line| text[line.range.clone()].starts_with("tags")).unwrap();
    assert!(line.style.error);
    let row = &line.boxed.rows[0];
    let marker = row.behind.iter().find(|rect| rect.color == view.theme().invalid).expect("the marker");
    assert!((marker.y1 - marker.y0 - 16.5).abs() < 0.01 && marker.x0 < 1.0 && marker.x1 > 6.0, "{marker:?}");
    assert!(row.glyphs.first().is_some_and(|glyph| line.boxed.styles[usize::from(glyph.style)].color == [255, 255, 255, 255]));
}

#[test]
fn a_plus_bullets_text_starts_further_in_as_obsidian_draws_it() {
    if !fonts() {
        return;
    }
    let text = "- dash\n* star\n+ plus\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let x = |needle: &str| view.caret(text.find(needle).unwrap()).x;
    assert_eq!(x("dash"), x("star"));
    assert!(x("plus") - x("dash") > 1.5 && x("plus") - x("dash") < 4.0, "{}", x("plus") - x("dash"));
}

#[test]
fn a_quote_in_a_list_item_starts_where_obsidian_starts_it() {
    if !fonts() {
        return;
    }
    let text = "plain\n\n- item:\n\n  > Quoted.\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let x = view.caret(text.find("Quoted").unwrap()).x - view.caret(0).x;
    // Obsidian 1.14.5: 38.3 px in at 16 px text.
    assert!((x - 38.3).abs() < 0.5, "{x}");
}

#[test]
fn a_caret_a_fold_hides_shows_the_folded_lines_syntax_as_obsidian_does() {
    let text = "# Head\n\nhidden\n\n# Next\n";
    let end = text.find("\n\n# Next").unwrap();
    let mut view = View::default();
    view.update_folded(text, end..end, &[0]);
    // Obsidian's caret at the fold's end is on the heading's line: its `# ` shows.
    assert!(view.styles().bytes[0] & style::flag::HIDE == 0);
    // Elsewhere the heading's marks hide.
    let next = text.find("Next").unwrap();
    view.update_folded(text, next..next, &[0]);
    assert!(view.styles().bytes[0] & style::flag::HIDE != 0);
    // And back (the caret-only relayout path).
    view.update_folded(text, end..end, &[0]);
    assert!(view.styles().bytes[0] & style::flag::HIDE == 0);
}

#[test]
fn a_fields_property_row_is_28_px_as_obsidians() {
    if !fonts() {
        return;
    }
    let text = "---\ntitle: Plan\nnumber: 3.14\ndone: true\ndate: 2026-10-01\nwhen: 2026-10-01T09:30\nlink: \"[[x]]\"\n---\n# Body\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let height = |key: &str| view.lines().iter().find(|line| text[line.range.clone()].starts_with(key)).unwrap().boxed.height;
    assert_eq!((height("title"), height("link")), (29.0, 29.0));
    assert_eq!((height("number"), height("done"), height("date"), height("when")), (28.0, 28.0, 28.0, 28.0));
}

#[test]
fn a_quotes_lazy_line_starts_at_the_edge_and_up_from_it_lands_at_the_quotes_start_as_obsidian_does() {
    if !fonts() {
        return;
    }
    let text = "> abc\nabc\n";
    let mut view = View::default();
    let from = text.find("\nabc").unwrap() + 1;
    view.update(text, from..from);
    // (The second line is the quote's lazy line: Obsidian draws its text at the column's edge.)
    assert_eq!(view.caret(from).x, view.theme().margin);
    assert_eq!(view.vertical(from, view.caret(from).x, -1), 0);
}

#[test]
fn the_caret_before_a_padded_first_piece_is_drawn_after_its_padding_as_obsidian_draws_it() {
    if !fonts() {
        return;
    }
    let text = "plain\n\n`code` at the start\n\n10. ten\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let left = view.caret(0).x;
    // Inline code's first letter: after its 0.3 em (4.2 px) padding.
    let code = view.caret(text.find("code`").unwrap()).x - left;
    assert!((code - 4.2).abs() < 0.3, "{code}");
    // A number's start: 12 px in, where the number is drawn.
    let number = view.caret(text.find("10.").unwrap()).x - left;
    assert!((number - 12.0).abs() < 0.3, "{number}");
}

#[test]
fn a_list_items_lazy_line_is_a_plain_line_as_obsidian_draws_it() {
    if !fonts() {
        return;
    }
    let text = "- item one\nlazy text\n\nend\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let lazy = view.lines().iter().find(|line| text[line.range.clone()].starts_with("lazy")).unwrap();
    assert_eq!((lazy.pad_top, lazy.pad_bottom), (0.0, 0.0));
    assert_eq!(view.caret(text.find("lazy").unwrap()).x, view.theme().margin);
}

#[test]
fn a_coloured_highlight_hides_its_emoji_and_draws_its_colour() {
    use crate::style::flag;
    // Obsidian 1.14 writes a highlight's colour as an emoji after its `==`.
    let text = "a ==🔴red== b ==plain==\n\nend";
    let end = text.len();
    let mut view = View::default();
    view.update(text, end..end);
    let flags = |needle: &str| view.styles().bytes.get(text.find(needle).unwrap()).copied().unwrap_or(0);
    assert_ne!(flags("🔴") & flag::MARK, 0, "the emoji is syntax");
    assert_ne!(flags("🔴") & flag::HIDE, 0, "hidden away from the caret");
    assert_ne!(flags("red==") & flag::HIGHLIGHT, 0);
    let index = (flags("red==") & flag::COLOR) >> flag::COLOR_SHIFT;
    assert_eq!(crate::style::interned_color(index), Some([248, 193, 200, 254]), "red, behind the text");
    assert_eq!(flags("plain") & flag::COLOR, 0, "a plain highlight keeps the theme's yellow");
}

#[test]
fn a_folded_properties_block_keeps_only_its_headings_room_as_obsidians() {
    if !fonts() {
        return;
    }
    let text = "---\ntitle: Plan\ntags:\n  - a\n---\n# Body\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    let body = |view: &View| view.lines().iter().find(|line| text[line.range.clone()].starts_with("# Body")).unwrap().y;
    let open = body(&view);
    view.set_properties_folded(true);
    view.update(text, text.len()..text.len());
    // Obsidian's folded block: its 51.2 px box (8 px, the 27 px heading, 16 px) and 32 px below.
    let first = view.lines()[0].clone();
    assert_eq!((first.boxed.height, first.pad_top, first.pad_bottom), (0.0, 43.0, 40.2));
    assert!(view.lines().iter().take_while(|line| line.style.block == style::Block::Property).all(|line| line.boxed.height == 0.0));
    let folded = body(&view);
    assert!(folded < open);
    // Open again: as before.
    view.set_properties_folded(false);
    view.update(text, text.len()..text.len());
    assert_eq!(body(&view), open);
}

#[test]
fn a_rule_block_later_in_the_note_is_no_front_matter_as_obsidian_reads_it() {
    // pulldown-cmark reads `---`, text, `---` anywhere as a metadata block; Obsidian's editor has
    // front matter only at the note's start: these are rules around Markdown.
    let text = "Para\n\n---\nAfter **a** rule.\n\n> Quote line\n---\nEnd\n";
    let styles = style::Styles::of(text);
    let line = |needle: &str| styles.lines[styles.line_of(text.find(needle).unwrap())];
    assert_eq!(line("---\nAfter").block, style::Block::Rule);
    assert_eq!(line("After").block, style::Block::Paragraph);
    assert_eq!(line("> Quote").quote, 1);
    assert_eq!(line("---\nEnd").block, style::Block::Rule);
    assert!(styles.bytes[text.find("a**").unwrap()] & style::flag::STRONG != 0);
    // The note's own front matter stays front matter.
    let front = "---\ntitle: x\n---\nBody\n";
    assert_eq!(style::Styles::of(front).lines[1].block, style::Block::Frontmatter);
}

#[test]
fn a_math_block_ending_a_callout_ends_it_once_as_obsidian_lays_it_out() {
    if !fonts() {
        return;
    }
    let text = "> [!note] A callout with math\n> $$\n> \\sqrt{a^2+b^2}\n> $$\n\nInline after.\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    // The block's lines after its first take no room: only its first ends the callout (12 px),
    // and the empty line after it is the note's 24 px (Obsidian: the box, then the line).
    let lines = view.lines();
    let pads: Vec<f32> = lines.iter().filter(|line| line.style.block == style::Block::Math).map(|line| line.pad_bottom).collect();
    assert_eq!(pads, [12.0, 0.0, 0.0]);
    let after = lines.iter().find(|line| text[line.range.clone()].starts_with("Inline")).unwrap();
    let first = &lines[1];
    assert_eq!(after.y, first.y + first.boxed.height + 12.0 + 24.0);
}

#[test]
fn a_picture_alone_in_a_list_item_or_quote_is_drawn_where_its_text_starts() {
    if !fonts() {
        return;
    }
    // As pasted from a document site: a picture is often a list item of its own.
    let text = "- Overseas\n    - ![](shot.png)\n    - next\n> ![](quote.png)\n- see ![](inline.png) here\n";
    let mut view = View::default();
    view.update(text, text.len()..text.len());
    assert_eq!(view.wanted_pictures(), vec!["quote.png".to_owned(), "shot.png".to_owned()]);
    let pixels = [220u8, 60, 60, 255].repeat(40 * 20);
    view.set_picture("shot.png", 40, 20, &pixels);
    view.set_picture("quote.png", 40, 20, &pixels);
    view.update(text, text.len()..text.len());
    assert_eq!(view.drawn_pictures(), vec!["quote.png".to_owned(), "shot.png".to_owned()]);
    let image = |view: &View, line: usize| view.placed[line].boxed.rows.iter().find_map(|row| row.image.clone()).map(|(_, rect)| rect);
    let stop = |line: usize, byte: usize| view.placed[line].boxed.rows[0].stops.iter().find(|stop| stop.byte == byte).map(|stop| stop.x);
    // The picture starts where the next item's text does, right of its bullet, which stays drawn.
    let shot = image(&view, 1).expect("the list item's picture is drawn");
    assert_eq!(Some(shot.x0), stop(2, 6));
    assert!(shot.x0 > 0.0);
    assert!(view.placed[1].boxed.rows[0].over.iter().any(|mark| mark.dot), "the item keeps its bullet");
    assert!(view.placed[1].boxed.height >= 20.0);
    assert!(image(&view, 3).is_some_and(|rect| rect.x0 > 0.0), "the quote's picture is drawn inside the quote");
    // A picture among words stays a link.
    assert!(image(&view, 4).is_none());
    if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
        let (width, height) = (view.width(), view.height());
        png(&std::path::Path::new(&dir).join("list-pictures.png"), &view.frame(0.0, 0.0, 1.0, width as u16, height as u16), width, height);
    }
    // Its line shows its source again when the caret is on it.
    let start = text.find("    - ![]").unwrap();
    view.update(text, start..start);
    assert!(image(&view, 1).is_none());
}

/// WCAG's contrast ratio of two opaque colors.
fn contrast(a: [u8; 4], b: [u8; 4]) -> f64 {
    let luminance = |rgba: [u8; 4]| {
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(rgba[0]) + 0.7152 * channel(rgba[1]) + 0.0722 * channel(rgba[2])
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn the_dark_palette_reads_on_its_paper() {
    let (light, dark) = (Theme::default(), Theme::dark());
    assert_eq!(light.scheme, Scheme::Light);
    assert_eq!(dark.scheme, Scheme::Dark);
    for (name, color) in [
        ("text", dark.text),
        ("heading", dark.heading),
        ("link", dark.link),
        ("accent", dark.accent),
        ("secondary", dark.secondary),
        ("math", dark.math),
    ] {
        let ratio = contrast(color, dark.paper);
        assert!(ratio >= 4.5, "dark {name} {color:?} on the paper is {ratio:.2}:1, under WCAG AA");
    }
    // Text on its inline backgrounds: a tag's pill, inline code, a highlight.
    for (name, color, back) in
        [("tag", dark.link, dark.tag_background), ("code", dark.text, dark.code_background), ("highlight", dark.text, dark.highlight)]
    {
        let ratio = contrast(color, back);
        assert!(ratio >= 4.0, "dark {name} text on its background is {ratio:.2}:1");
    }
    // The faint syntax stays faint, but no fainter than the light palette's.
    assert!(contrast(dark.muted, dark.paper) >= contrast(light.muted, light.paper));
    // Code tokens on the code block (comments are faint by design, as the syntax).
    for color in [
        dark.code_keyword,
        dark.code_literal,
        dark.code_number,
        dark.code_variable,
        dark.code_operator,
        dark.code_property,
        dark.code_punctuation,
        dark.code_link,
    ] {
        let ratio = contrast(color, dark.code_block);
        assert!(ratio >= 4.5, "dark code token {color:?} on the code block is {ratio:.2}:1");
    }
    // A tag's pill and inline code draw their own shapes: they must stay told apart.
    assert_ne!(dark.tag_background, dark.code_background);
    assert_ne!(dark.tag_background, dark.highlight);
    // Each callout's title color reads on its tint.
    for kind in 1..=u8::try_from(theme::CALLOUT_TYPES.len()).unwrap() {
        let ratio = contrast(theme::callout_color(&dark, kind), theme::callout_tint(&dark, kind));
        assert!(ratio >= 3.0, "dark callout {kind}'s title on its tint is {ratio:.2}:1");
    }
    // A nested callout's box shows on the outer one's in either palette.
    for theme in [&light, &dark] {
        let outer = theme::callout_tint(theme, 1);
        assert_ne!(theme::callout_tint_over(theme, outer, 8), outer, "{:?}", theme.scheme);
    }
    // The coloured highlights are dark under the dark palette's light text.
    for (_, color) in style::HIGHLIGHT_COLORS {
        assert_eq!(light.highlight_background(color), [color[0], color[1], color[2], 255]);
        assert!(contrast(dark.text, dark.highlight_background(color)) >= 4.5);
    }
}

#[test]
fn the_palettes_share_every_length() {
    let (light, dark) = (Theme::default(), Theme::dark());
    let geometry = |theme: &Theme| {
        (theme.width, theme.margin, theme.top, theme.bottom, theme.min_height, theme.image_height, theme.body_size, theme.code_size, theme.leading)
    };
    assert_eq!(geometry(&light), geometry(&dark));
    // The light palette is the default, unchanged by a round trip through the dark one.
    assert_eq!(light.with_scheme(Scheme::Dark).with_scheme(Scheme::Light), light);
    assert_eq!(light.with_scheme(Scheme::Dark), dark);
    // A fitted width and height survive a switch.
    let mut view = View::default();
    view.set_width(1280.0);
    view.set_visible_height(700.0);
    let before = geometry(view.theme());
    assert!(view.set_scheme(Scheme::Dark));
    assert_eq!(geometry(view.theme()), before);
    assert_eq!(Scheme::parse("dark"), Some(Scheme::Dark));
    assert_eq!(Scheme::parse("light"), Some(Scheme::Light));
    assert_eq!(Scheme::parse("sepia"), None);
}

#[test]
fn switching_the_scheme_repaints_and_moves_nothing() {
    if !fonts() {
        return;
    }
    let text = format!("{SAMPLE}\n> [!note] Note\n> Body\n> > [!warning] Nested\n> > Inside\n\n==🔵blue== and ==🟡yellow==\n");
    let mut view = View::default();
    view.update(&text, 0..0);
    let (width, height) = (view.width(), view.height());
    let carets: Vec<_> = (0..text.len()).filter(|at| text.is_char_boundary(*at)).map(|at| view.caret(at)).collect();
    let light = view.frame(0.0, 0.0, 1.0, width as u16, height as u16);
    assert!(!view.set_scheme(Scheme::Light), "already light");
    assert!(view.set_scheme(Scheme::Dark));
    assert!(!view.set_scheme(Scheme::Dark), "already dark");
    view.update(&text, 0..0);
    assert_eq!((view.width(), view.height()), (width, height), "the page keeps its size");
    let moved: Vec<_> = (0..text.len()).filter(|at| text.is_char_boundary(*at)).map(|at| view.caret(at)).collect();
    assert_eq!(carets, moved, "no character moves");
    let dark = view.frame(0.0, 0.0, 1.0, width as u16, height as u16);
    let paper = Theme::dark().paper;
    assert_eq!(dark.get(0..4), Some(&paper[..]), "the paper is the dark palette's");
    // No light paper is left, and the text is lighter than the paper.
    let white = dark.as_chunks::<4>().0.iter().filter(|px| px[..3] == [255, 255, 255]).count();
    assert_eq!(white, 0, "{white} white pixels on the dark paper");
    let lit = dark.as_chunks::<4>().0.iter().filter(|px| px[0] > 160 && px[1] > 160 && px[2] > 160).count();
    assert!(lit > 2_000, "light text draws on the dark paper ({lit} pixels)");
    if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
        png(&std::path::Path::new(&dir).join("sample-dark.png"), &dark, width, height);
    }
    // Back to light: the same pixels as before the switch.
    assert!(view.set_scheme(Scheme::Light));
    view.update(&text, 0..0);
    assert_eq!(view.frame(0.0, 0.0, 1.0, width as u16, height as u16), light, "the light paint is as it was");
}

#[test]
fn what_follows_a_coloured_highlight_keeps_its_own_color() {
    if !fonts() {
        return;
    }
    // Every coloured highlight first, so their colours are among the ones the view keeps.
    let mut text = String::new();
    for (emoji, _) in style::HIGHLIGHT_COLORS {
        text.push_str(&format!("=={emoji}one== text, and math $E=mc^2$\n\n"));
    }
    text.push_str("==🔵blue== then [a link](https://kition.ai), [[a page]], #tag, `code`, *emphasis* and **strong**.\n\n");
    text.push_str("==🟢$x^2$ inside==, and math $a+b$ after\n\nend\n");
    let math = kition_note_fonts::math_face(latex_rust::STIX_TWO_MATH_OTF).unwrap();
    let highlights: Vec<[u8; 4]> = style::HIGHLIGHT_COLORS.iter().map(|(_, color)| *color).collect();
    for scheme in [Scheme::Light, Scheme::Dark] {
        let mut view = View::default();
        view.set_scheme(scheme);
        view.update(&text, text.len()..text.len());
        let theme = view.theme().clone();
        let mut formulas = 0;
        for line in view.lines() {
            for glyph in line.boxed.rows.iter().flat_map(|row| row.glyphs.iter()) {
                let style = line.boxed.styles[usize::from(glyph.style)];
                let source = &text[line.range.clone()];
                assert!(!highlights.contains(&style.color), "{scheme:?}: a glyph of {source:?} is drawn in a highlight's colour {:?}", style.color);
                if glyph.face == math {
                    formulas += 1;
                    assert_eq!(style.color, theme.text, "{scheme:?}: the formula in {source:?} keeps the text's color");
                }
            }
        }
        assert!(formulas > 0, "the formulas are drawn");
        if let Some(dir) = std::env::var_os("KITION_NOTE_VIEW_PNG") {
            let (width, height) = (view.width(), view.height());
            let name = format!("highlight-then-math-{}.png", if scheme == Scheme::Dark { "dark" } else { "light" });
            png(&std::path::Path::new(&dir).join(name), &view.frame(0.0, 0.0, 1.0, width as u16, height as u16), width, height);
        }
    }
}
