use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolOverlay, ToolPointer};
use serde_json::json;

use crate::NoteDocument;

/// Registers the `kition-fonts` checkout's faces (glyphs need widths for selection rectangles).
fn fonts() -> bool {
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kition-fonts"));
    kition_note_fonts::register_dir(&dir) > 0
}

fn note(text: &str) -> NoteDocument {
    NoteDocument::open("a.md", text.as_bytes()).unwrap()
}

fn at(doc: &mut NoteDocument, anchor: usize, focus: usize) {
    doc.execute("select.set", json!({ "anchor": anchor, "focus": focus })).unwrap();
}

fn key(doc: &mut NoteDocument, name: &str) {
    doc.key(name, ToolMods::default(), 1.0).unwrap();
}

fn shift() -> ToolMods {
    ToolMods { shift: true, ..ToolMods::default() }
}

#[test]
fn typing_inserts_at_the_caret_and_undoes_in_one_step() {
    let mut doc = note("Hello\n");
    at(&mut doc, 5, 5);
    for c in [" ", "w", "o", "r", "l", "d"] {
        KitTools::text(&mut doc, c, 1.0).unwrap();
    }
    assert_eq!(doc.body(), "Hello world\n");
    assert_eq!(doc.editor.focus, 11);
    key(&mut doc, "ArrowLeft");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.body(), "Hello\n", "the typing run undoes as one step");
}

#[test]
fn backspace_and_delete_take_graphemes_and_selections() {
    let mut doc = note("a👍🏽b");
    let end = "a👍🏽b".chars().count();
    at(&mut doc, end - 1, end - 1);
    key(&mut doc, "Backspace");
    assert_eq!(doc.body(), "ab", "the emoji with its modifier is one grapheme");
    at(&mut doc, 0, 0);
    key(&mut doc, "Delete");
    assert_eq!(doc.body(), "b");
    let mut doc = note("one two three");
    at(&mut doc, 4, 8);
    key(&mut doc, "Backspace");
    assert_eq!(doc.body(), "one three");
}

#[test]
fn enter_goes_on_with_lists_and_ends_them_on_an_empty_item() {
    let mut doc = note("- first");
    at(&mut doc, 7, 7);
    key(&mut doc, "Enter");
    assert_eq!(doc.body(), "- first\n- ");
    key(&mut doc, "Enter");
    assert_eq!(doc.body(), "- first\n", "an empty item ends the list");
    let mut doc = note("1. one\n- [x] done");
    at(&mut doc, 6, 6);
    key(&mut doc, "Enter");
    assert!(doc.body().starts_with("1. one\n2. "), "{:?}", doc.body());
    let len = doc.body().chars().count();
    at(&mut doc, len, len);
    key(&mut doc, "Enter");
    assert!(doc.body().ends_with("- [x] done\n- [ ] "), "a new task is open: {:?}", doc.body());
}

#[test]
fn backspace_right_after_a_marker_takes_a_character_as_obsidian_does() {
    // Measured in Obsidian 1.14.5 (web/tests/compare-editing.mjs): Backspace after `- ` takes
    // the space, not the whole marker.
    let mut doc = note("- ");
    at(&mut doc, 2, 2);
    key(&mut doc, "Backspace");
    assert_eq!(doc.body(), "-");
}

#[test]
fn bold_wraps_and_unwraps_the_selection() {
    let mut doc = note("make this strong");
    at(&mut doc, 5, 9);
    doc.execute("format.bold", json!({})).unwrap();
    assert_eq!(doc.body(), "make **this** strong");
    assert_eq!(doc.selected_text(), "this");
    doc.execute("format.bold", json!({})).unwrap();
    assert_eq!(doc.body(), "make this strong");
    assert_eq!(doc.selected_text(), "this");
}

#[test]
fn headings_and_lists_rewrite_line_prefixes() {
    let mut doc = note("Title\nsecond");
    at(&mut doc, 2, 2);
    doc.execute("format.heading2", json!({})).unwrap();
    assert_eq!(doc.body(), "## Title\nsecond");
    doc.execute("format.heading1", json!({})).unwrap();
    assert_eq!(doc.body(), "# Title\nsecond");
    let all = doc.body().chars().count();
    at(&mut doc, 0, all);
    doc.execute("list.bullet", json!({})).unwrap();
    assert_eq!(doc.body(), "- # Title\n- second");
    doc.execute("list.bullet", json!({})).unwrap();
    assert_eq!(doc.body(), "# Title\nsecond");
}

#[test]
fn the_selection_follows_an_edit_made_elsewhere() {
    let mut doc = note("alpha beta");
    at(&mut doc, 6, 10);
    doc.execute("text.insert", json!({ "at": 0, "text": ">> " })).unwrap();
    assert_eq!(doc.selected_text(), "beta");
    doc.execute("text.insert", json!({ "at": doc.body().chars().count(), "text": "!" })).unwrap();
    assert_eq!(doc.selected_text(), "beta");
}

#[test]
fn pasted_lines_keep_their_breaks_and_the_caret_lands_after_them() {
    let mut doc = note("ab");
    at(&mut doc, 1, 1);
    KitTools::text(&mut doc, "x\r\ny", 1.0).unwrap();
    assert_eq!(doc.body(), "ax\nyb");
    assert_eq!(doc.editor.focus, 4);
}

#[test]
fn shift_arrows_extend_and_the_overlays_show_caret_or_selection() {
    if !fonts() {
        eprintln!("skipped: no fonts (set KITION_FONTS_DIR)");
        return;
    }
    let mut doc = note("hello");
    at(&mut doc, 0, 0);
    let caret = doc.state(0.0, 0.0, ToolMods::default(), 1.0);
    assert!(matches!(caret.overlays.first(), Some(ToolOverlay::Line { .. })));
    doc.key("ArrowRight", shift(), 1.0).unwrap();
    doc.key("ArrowRight", shift(), 1.0).unwrap();
    assert_eq!(doc.selected_text(), "he");
    let selection = doc.state(0.0, 0.0, ToolMods::default(), 1.0);
    assert!(matches!(selection.overlays.first(), Some(ToolOverlay::Highlight { .. })));
    key(&mut doc, "ArrowRight");
    assert!(doc.editor.is_collapsed());
    assert_eq!(doc.editor.focus, 2, "a collapsed move goes to the selection's end");
}

#[test]
fn a_click_places_the_caret_and_a_double_click_selects_a_word() {
    let mut doc = note("first line\nsecond line");
    let caret = doc.view().caret(11);
    let event = |action| ToolPointer {
        action,
        x: f64::from(caret.x) + 0.1,
        y: f64::from(caret.top + 2.0),
        mods: ToolMods::default(),
        pressure: 1.0,
        zoom: 1.0,
    };
    doc.pointer(&event(PointerAction::Down)).unwrap();
    assert_eq!(doc.editor.focus, 11);
    doc.pointer(&event(PointerAction::DoubleClick)).unwrap();
    assert_eq!(doc.selected_text(), "second");
}

#[test]
fn a_command_from_elsewhere_is_its_own_undo_step() {
    let mut doc = note("Goals.\n");
    at(&mut doc, 6, 6);
    KitTools::text(&mut doc, " More", 1.0).unwrap();
    // An accepted suggestion, run while the typing run is still open.
    doc.execute("text.replace", json!({ "from": 0, "to": 5, "text": "Aims" })).unwrap();
    assert_eq!(doc.body(), "Aims. More\n");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.body(), "Goals. More\n", "Undo takes back the suggestion only");
}

#[test]
fn a_change_to_several_lines_undoes_in_one_step() {
    let mut doc = note("one\ntwo\nthree");
    let all = doc.body().chars().count();
    at(&mut doc, 0, all);
    doc.execute("list.bullet", json!({})).unwrap();
    assert_eq!(doc.body(), "- one\n- two\n- three");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.body(), "one\ntwo\nthree");
}

#[test]
fn a_mixed_ending_note_keeps_each_line_s_ending() {
    let mut doc = note("one\r\ntwo\nthree\r\n");
    doc.execute("select.set", json!({ "line": 1, "column": 100 })).unwrap();
    KitTools::text(&mut doc, "X", 1.0).unwrap();
    key(&mut doc, "End");
    key(&mut doc, "Enter");
    assert_eq!(doc.body(), "oneX\r\n\ntwo\nthree\r\n");
}

#[test]
fn a_picture_line_draws_its_picture_once_its_bytes_arrive() {
    use base64::Engine as _;
    let mut doc = note("Intro\n\n![](pics/dot.png)\n\nAfter\n");
    let wanted = doc.execute("image.wanted", json!({})).unwrap();
    assert_eq!(wanted["images"], json!(["pics/dot.png"]));
    let before = doc.height();
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(40, 300, image::Rgba([86, 69, 212, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let drawn = doc.execute("image.set", json!({ "name": "pics/dot.png", "data": base64::engine::general_purpose::STANDARD.encode(&png) })).unwrap();
    assert_eq!(drawn["drawn"], json!(true));
    assert_eq!(doc.execute("image.wanted", json!({})).unwrap()["images"], json!([]));
    assert!(doc.height() > before || doc.height() >= 1056, "the picture takes its room");
    let pixels = doc.render(0.0, 0.0, 1.0, doc.width(), doc.height()).unwrap();
    assert!(pixels.as_chunks::<4>().0.iter().any(|px| px[..3] == [86, 69, 212]), "the picture is drawn");
    // Bytes that are no picture are not asked for again.
    let mut other = note("![](broken.png)\n");
    other.execute("image.set", json!({ "name": "broken.png", "data": "bm90IGFuIGltYWdl" })).unwrap();
    assert_eq!(other.execute("image.wanted", json!({})).unwrap()["images"], json!([]));
}

#[test]
fn a_click_on_a_link_drawn_as_a_link_asks_to_open_it() {
    let text = "# Links\n\n| Doc | Note |\n|---|---|\n| [[Plan]] | x |\n\nEnd\n";
    let mut doc = note(text);
    doc.execute("select.set", json!({ "line": 7, "column": 1 })).unwrap();
    let index = text[..text.find("[[Plan]]").unwrap() + 3].chars().count();
    let at = doc.execute("select.where", json!({ "at": index })).unwrap();
    let event = ToolPointer {
        action: PointerAction::Down,
        x: at["x"].as_f64().unwrap() + 2.0,
        y: at["y"].as_f64().unwrap() + 5.0,
        mods: ToolMods::default(),
        pressure: 1.0,
        zoom: 1.0,
    };
    let response = doc.pointer(&event).unwrap();
    assert!(format!("{:?}", response.requests).contains("openLink"), "{:?}", response.requests);
    // The host moves the caret (a heading of this note); the press's drag leaves it there.
    doc.execute("select.set", json!({ "line": 1, "column": 1 })).unwrap();
    doc.pointer(&ToolPointer { action: PointerAction::Drag, ..event }).unwrap();
    doc.pointer(&ToolPointer { action: PointerAction::Up, ..event }).unwrap();
    assert_eq!((doc.editor.anchor, doc.editor.focus), (0, 0), "a press that opened a link selects nothing");
}

#[test]
fn a_folded_heading_hides_its_section_and_the_caret_unfolds_it() {
    let text = "# One\n\nfirst body\n\n## Two\n\nsecond body\n\n# Three\n\nthird\n";
    let mut doc = note(text);
    doc.execute("select.set", json!({ "line": 1, "column": 1 })).unwrap();
    let before = doc.view().caret(text.find("# Three").unwrap()).top;
    doc.execute("view.fold", json!({})).unwrap();
    let after = doc.view().caret(text.find("# Three").unwrap()).top;
    assert!(after < before, "the section under # One takes no room: {before} -> {after}");
    assert!(doc.view().is_hidden(text.find("second").unwrap()), "## Two is inside # One's section");
    assert!(!doc.view().is_hidden(text.find("third").unwrap()));
    // Down from the heading skips what the fold hides.
    key(&mut doc, "ArrowDown");
    assert!(doc.editor.focus >= text.find("# Three").unwrap(), "Down lands past the fold: {}", doc.editor.focus);
    // A search hit inside the fold opens it.
    let inside = text[..text.find("second").unwrap()].chars().count();
    doc.execute("select.set", json!({ "anchor": inside })).unwrap();
    assert!(!doc.view().is_hidden(text.find("second").unwrap()), "the fold opened for the caret");
    doc.execute("view.foldAll", json!({})).unwrap();
    assert!(doc.view().is_hidden(text.find("third").unwrap()));
    doc.execute("view.unfoldAll", json!({})).unwrap();
    assert!(!doc.view().is_hidden(text.find("first").unwrap()));
    // Toggle fold folds the caret's own line only (measured in Obsidian): from a section's body
    // nothing folds; on a heading, mid text, the caret stays, and reading it keeps the fold.
    doc.execute("select.set", json!({ "anchor": inside })).unwrap();
    doc.execute("view.fold", json!({})).unwrap();
    assert!(!doc.view().is_hidden(text.find("second").unwrap()), "nothing folds from a body line");
    let two = text[..text.find("## Two").unwrap() + 4].chars().count();
    doc.execute("select.set", json!({ "anchor": two })).unwrap();
    doc.execute("view.fold", json!({})).unwrap();
    doc.execute("select.get", json!({})).unwrap();
    assert!(doc.view().is_hidden(text.find("second").unwrap()), "the fold stays");
    assert_eq!(doc.editor.focus, text.find("## Two").unwrap() + 4, "the caret stays where it was");
}

#[test]
fn a_list_item_with_children_folds_them_as_obsidian_does() {
    let text = "- parent\n\t- child one\n\n\t- child two\n- next\n\t- under next\n- last\n";
    let mut doc = note(text);
    {
        let view = doc.view();
        let folds: Vec<Option<bool>> = view.lines().iter().map(|line| line.fold).collect();
        assert_eq!(folds, [Some(false), None, None, None, Some(false), None, None, None], "items with children fold, the rest do not");
        // The chevron sits just before the item's marker: 5 px before a top-level one.
        assert_eq!(view.fold_x(&view.lines()[0]), -5.0);
    }
    // Toggle fold from a child folds nothing (measured in Obsidian); from the item it folds its
    // children, the caret where it was.
    let child = text[..text.find("child two").unwrap()].chars().count();
    doc.execute("select.set", json!({ "anchor": child })).unwrap();
    doc.execute("view.fold", json!({})).unwrap();
    assert!(!doc.view().is_hidden(text.find("child two").unwrap()), "nothing folds from a child");
    doc.execute("select.set", json!({ "anchor": 3 })).unwrap();
    doc.execute("view.fold", json!({})).unwrap();
    for hidden in ["child one", "child two"] {
        assert!(doc.view().is_hidden(text.find(hidden).unwrap()), "{hidden} is folded away");
    }
    assert!(!doc.view().is_hidden(text.find("- next").unwrap()));
    assert_eq!(doc.editor.focus, 3, "the caret stays where it was");
    assert_eq!(doc.view().lines()[0].fold, Some(true));
    // Pressing the chevron opens it again.
    let (x, y) = {
        let view = doc.view();
        let line = &view.lines()[0];
        (view.theme().margin + view.fold_x(line), line.y + 4.0)
    };
    assert_eq!(doc.view().fold_at(x, y), Some(0));
    assert_eq!(doc.view().fold_at(x + 12.0, y), None, "the text beside it is not the chevron");
    doc.execute("view.unfoldAll", json!({})).unwrap();
    doc.execute("view.foldAll", json!({})).unwrap();
    assert!(doc.view().is_hidden(text.find("under next").unwrap()), "Fold all folds every item with children");
    assert!(!doc.view().is_hidden(text.find("- last").unwrap()));
}

#[test]
fn the_revision_moves_when_the_caret_changes_line_so_the_canvas_repaints() {
    let mut doc = note("# Title\n\nSome **bold** text\n");
    key(&mut doc, "ArrowRight");
    let first = doc.revision();
    key(&mut doc, "ArrowRight");
    assert_eq!(doc.revision(), first, "a move inside a line draws the same pixels");
    key(&mut doc, "ArrowDown");
    key(&mut doc, "ArrowDown");
    assert!(doc.revision() > first, "the caret's new line shows its syntax, the old one hides it");
    assert!(!doc.dirty(), "moving the caret changes nothing to save");
}

#[test]
fn hovering_a_heading_shows_its_chevron_and_a_link_takes_the_pointing_hand() {
    let text = "# Title\n\nSee [[Plan]] here\n";
    let mut doc = note(text);
    let at = |doc: &mut NoteDocument, index: usize| {
        let where_ = doc.execute("select.where", json!({ "at": index })).unwrap();
        (where_["x"].as_f64().unwrap(), where_["y"].as_f64().unwrap() + where_["height"].as_f64().unwrap() / 2.0)
    };
    let hover = |doc: &mut NoteDocument, (x, y): (f64, f64)| {
        doc.pointer(&ToolPointer { action: PointerAction::Move, x, y, mods: ToolMods::default(), pressure: 0.0, zoom: 1.0 }).unwrap()
    };
    let before = doc.revision();
    let title = at(&mut doc, 3);
    let answer = hover(&mut doc, title);
    assert_eq!(doc.view().hover(), Some(0), "the heading under the pointer");
    assert!(doc.revision() > before, "its chevron shows: the canvas repaints");
    assert_eq!(answer.cursor, "text");
    let link = at(&mut doc, text.find("Plan").unwrap() + 1);
    let answer = hover(&mut doc, (link.0 + 2.0, link.1));
    assert_eq!(answer.cursor, "pointer", "a link drawn as a link takes the pointing hand");
    assert_eq!(doc.view().hover(), None);
    let plain = at(&mut doc, text.find("here").unwrap() + 2);
    assert_eq!(hover(&mut doc, plain).cursor, "text");
    // The pointer leaving the note takes the chevron away.
    hover(&mut doc, title);
    let shown = doc.revision();
    doc.execute("view.hoverOut", json!({})).unwrap();
    assert_eq!(doc.view().hover(), None);
    assert!(doc.revision() > shown, "the canvas repaints without the chevron");
}

#[test]
fn a_mermaid_block_keeps_its_diagrams_room_once_the_editor_sizes_it() {
    let text = "Intro\n\n```mermaid\ngraph TD\n  A --> B\n```\n\nAfter\n";
    let mut doc = note(text);
    at(&mut doc, text.len(), text.len());
    let view = doc.execute("diagram.view", json!({})).unwrap();
    assert_eq!(view["wanted"], json!([{ "kind": "mermaid", "source": "graph TD\n  A --> B" }]));
    assert_eq!(view["diagrams"], json!([]));
    let (height, revision) = (doc.height(), doc.revision());
    let set = doc.execute("diagram.set", json!({ "source": "graph TD\n  A --> B", "width": 263, "height": 2000 })).unwrap();
    assert_eq!(set["changed"], json!(true));
    assert!(doc.revision() > revision, "the editor draws again");
    assert!(doc.height() > height, "the diagram takes its room");
    let view = doc.execute("diagram.view", json!({})).unwrap();
    assert_eq!(view["wanted"], json!([]));
    let drawn = &view["diagrams"][0];
    assert_eq!((drawn["width"].as_f64(), drawn["height"].as_f64()), (Some(263.0), Some(2000.0)));
    assert_eq!(drawn["code"], json!(text.find("graph").unwrap()));
    // Only the diagrams in the asked heights.
    assert_eq!(doc.execute("diagram.view", json!({ "top": 5000, "bottom": 6000 })).unwrap()["diagrams"], json!([]));
    // The caret in the block shows its source.
    let code = text.find("graph").unwrap();
    at(&mut doc, code, code);
    assert_eq!(doc.execute("diagram.view", json!({})).unwrap()["diagrams"], json!([]));
    assert!(doc.execute("diagram.set", json!({ "source": "x" })).is_err());
}

#[test]
fn reads_between_keystrokes_keep_the_typing_run_one_undo_step() {
    let mut doc = note("start ");
    at(&mut doc, 6, 6);
    for c in "hello world".chars() {
        KitTools::text(&mut doc, &c.to_string(), 1.0).unwrap();
        // What the editor's overlays read after every keystroke.
        for read in ["code.blocks", "diagram.view", "property.view", "table.geometry", "select.get"] {
            doc.execute(read, json!({})).unwrap();
        }
    }
    assert_eq!(doc.body(), "start hello world");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.body(), "start ", "one Undo takes back the whole run");
}

#[test]
fn a_double_click_groups_characters_as_obsidian_does() {
    use super::word_at;
    let pick = |text: &str, needle: &str, offset: usize| {
        let at = text.find(needle).unwrap() + offset;
        let (start, end) = word_at(text, at);
        text[start..end].to_owned()
    };
    assert_eq!(pick("这是中文段落测试，然后", "中", 0), "这是中文段落测试", "a CJK run up to its punctuation");
    assert_eq!(pick("price 123.45 now", "123", 1), "123");
    assert_eq!(pick("we don't stop", "don", 1), "don");
    assert_eq!(pick("a, b", ",", 0), ",");
    assert_eq!(pick("one  two", "  ", 1), "  ", "spaces group too");
    assert_eq!(pick("end", "end", 3), "end", "at the line's end, the run before");
}

#[test]
fn a_press_anywhere_on_a_task_box_toggles_it() {
    if !fonts() {
        return;
    }
    let mut doc = note("- [ ] task\n\nend\n");
    let end = doc.body().len();
    at(&mut doc, end, end);
    let open = doc.body().find('[').unwrap();
    let caret = doc.view().caret(open);
    // The box's left half: the hidden marker before it hit tests to the line's start.
    let x = f64::from(caret.x + 8.0);
    let y = f64::from(caret.top + caret.height / 2.0);
    for action in [PointerAction::Down, PointerAction::Up] {
        doc.pointer(&ToolPointer { action, x, y, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 }).unwrap();
    }
    assert_eq!(doc.body(), "- [x] task\n\nend\n");
    assert_eq!(doc.editor.focus, end, "the caret stays where it was");
}

fn alt() -> ToolMods {
    ToolMods { alt: true, ..ToolMods::default() }
}

/// The text with the caret (`‸`) where the editor has it.
fn marked(doc: &NoteDocument) -> String {
    let text = doc.body();
    let (from, to) = doc.editor.ordered();
    if from == to {
        return format!("{}‸{}", &text[..from], &text[from..]);
    }
    format!("{}‹{}›{}", &text[..from], &text[from..to], &text[to..])
}

#[test]
fn alt_arrows_move_by_codemirror_groups() {
    // Measured in Obsidian: punctuation is a group of its own, a CJK run is one word, spaces go
    // with the group after them, and a line break with the next line's word.
    for (text, caret, key_name, times, want) in [
        ("one, two", 8, "ArrowLeft", 2, "one‸, two"),
        ("中文段落 test", 0, "ArrowRight", 1, "中文段落‸ test"),
        ("...abc def", 0, "ArrowRight", 1, "...‸abc def"),
        ("[link](url) x", 0, "ArrowRight", 2, "[link‸](url) x"),
        ("we don't", 8, "ArrowLeft", 1, "we don'‸t"),
        ("one\ntwo", 4, "ArrowLeft", 1, "‸one\ntwo"),
        ("one\ntwo", 3, "ArrowRight", 1, "one\ntwo‸"),
        ("one two", 3, "ArrowRight", 1, "one two‸"),
    ] {
        let mut doc = note(text);
        at(&mut doc, caret, caret);
        for _ in 0..times {
            doc.key(key_name, alt(), 1.0).unwrap();
        }
        assert_eq!(marked(&doc), want, "{key_name} in {text:?}");
    }
}

#[test]
fn alt_backspace_and_delete_take_codemirror_groups() {
    for (text, caret, key_name, want) in [
        ("abc 中文段落", 8, "Backspace", "abc ‸"),
        ("one two   ", 10, "Backspace", "one two‸"),
        ("one two ", 8, "Backspace", "one ‸"),
        ("abc...", 6, "Backspace", "abc‸"),
        ("one\ntwo", 4, "Backspace", "one‸two"),
        ("one   two", 3, "Delete", "one‸two"),
        ("one ‸two three", 4, "Delete", "one ‸ three"),
    ] {
        let text_only = text.replace('‸', "");
        let mut doc = note(&text_only);
        at(&mut doc, caret, caret);
        doc.key(key_name, alt(), 1.0).unwrap();
        assert_eq!(marked(&doc), want, "{key_name} in {text:?}");
    }
}

fn undo(doc: &mut NoteDocument) {
    doc.execute("edit.undo", json!({})).unwrap();
}

fn type_in(doc: &mut NoteDocument, text: &str) {
    for c in text.chars() {
        KitTools::text(doc, &c.to_string(), 1.0).unwrap();
    }
}

#[test]
fn undo_steps_join_as_codemirror_s_history_joins_them() {
    // Measured in Obsidian: deleting joins the typing before it, Enter is a step of its own that
    // the typing after it joins, a Cmd+B joins the word typed before it.
    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "abc");
    key(&mut doc, "Backspace");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x ‸");

    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "abc");
    key(&mut doc, "Enter");
    type_in(&mut doc, "def");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x abc‸", "Enter and the typing after it are one step");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x ‸");

    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "abc");
    doc.execute("format.bold", json!({})).unwrap();
    assert_eq!(doc.body(), "x **abc**");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x ‸", "the Cmd+B joins the word typed before it");

    let mut doc = note("ab");
    at(&mut doc, 2, 2);
    key(&mut doc, "Enter");
    key(&mut doc, "Enter");
    undo(&mut doc);
    assert_eq!(marked(&doc), "ab\n‸", "each Enter is a step");

    // A closer stepped over moves the caret only: the step ends there.
    let mut doc = note("a ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "(x)y");
    assert_eq!(doc.body(), "a (x)y");
    undo(&mut doc);
    assert_eq!(marked(&doc), "a (x)‸");
}

#[test]
fn a_pause_ends_the_undo_step() {
    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "abc");
    // Half a second later (the step's last change moved back in time).
    doc.editor.step.as_mut().unwrap().last -= super::JOIN_DELAY + 1;
    type_in(&mut doc, "def");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x abc‸");
}

#[test]
fn undo_and_redo_put_back_the_selection() {
    let mut doc = note("a word b");
    at(&mut doc, 2, 6);
    doc.execute("format.bold", json!({})).unwrap();
    assert_eq!(marked(&doc), "a **‹word›** b");
    undo(&mut doc);
    assert_eq!(marked(&doc), "a ‹word› b", "Undo selects what was selected before");
    doc.execute("edit.redo", json!({})).unwrap();
    assert_eq!(marked(&doc), "a **‹word›** b", "Redo selects what was selected after");

    let mut doc = note("x abc y");
    at(&mut doc, 2, 5);
    key(&mut doc, "Backspace");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x ‹abc› y");

    let mut doc = note("x abc");
    at(&mut doc, 5, 5);
    key(&mut doc, "Backspace");
    key(&mut doc, "Backspace");
    undo(&mut doc);
    assert_eq!(marked(&doc), "x abc‸", "the caret goes back where the deleting started");

    // Typing, a move, Undo, Redo: the caret where the typing ended.
    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "abc");
    key(&mut doc, "ArrowLeft");
    undo(&mut doc);
    doc.execute("edit.redo", json!({})).unwrap();
    assert_eq!(marked(&doc), "x abc‸");
}

#[test]
fn a_change_from_elsewhere_leaves_undo_s_selection_to_the_text() {
    let mut doc = note("a word b");
    at(&mut doc, 2, 6);
    doc.execute("format.bold", json!({})).unwrap();
    // An agent's edit after it: the editor's record is no longer the replica's last step.
    let agent = kition_kit::kdm::Origin::agent("agent:test");
    kition_kit::session::KitSessionDocument::execute_as(&mut doc, "text.replace", json!({ "from": 0, "to": 1, "text": "A" }), &agent).unwrap();
    undo(&mut doc);
    assert_eq!(doc.body(), "A word b");
    assert!(doc.editor.done.is_empty(), "the records are dropped");
}

/// The note `text` (`‸` the caret, `‹…›` a selection) after `keys` (names, or `type:` text).
fn after(text: &str, keys: &[&str]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut plain = String::new();
    let (mut anchor, mut focus) = (None, None);
    for c in &chars {
        match c {
            '‸' => {
                anchor = Some(plain.chars().count());
                focus = anchor;
            }
            '‹' => anchor = Some(plain.chars().count()),
            '›' => focus = Some(plain.chars().count()),
            _ => plain.push(*c),
        }
    }
    let mut doc = note(&plain);
    let end = plain.chars().count();
    at(&mut doc, anchor.unwrap_or(end), focus.or(anchor).unwrap_or(end));
    for name in keys {
        if let Some(typed) = name.strip_prefix("type:") {
            type_in(&mut doc, typed);
        } else if let Some(command) = name.strip_prefix("run:") {
            doc.execute(command, json!({})).unwrap();
        } else {
            let mut mods = ToolMods::default();
            let mut key_name = *name;
            for (prefix, set) in [("Shift+", 0), ("Alt+", 1), ("Cmd+", 2)] {
                if let Some(rest) = key_name.strip_prefix(prefix) {
                    key_name = rest;
                    match set {
                        0 => mods.shift = true,
                        1 => mods.alt = true,
                        _ => mods.command = true,
                    }
                }
            }
            doc.key(key_name, mods, 1.0).unwrap();
        }
    }
    marked(&doc)
}

#[test]
fn lists_in_a_quote_go_on_after_its_marks() {
    // Measured in Obsidian: in a quote, Tab indents after the `>`, an empty item ends the list
    // but not the quote, and Toggle checkbox puts the box after the `>`.
    assert_eq!(after("> - one\n> - two‸", &["Tab"]), "> - one\n> \t- two‸");
    assert_eq!(after("> - one\n> \t- two‸", &["Shift+Tab"]), "> - one\n> - two‸");
    assert_eq!(after("> said‸", &["Tab"]), "> \tsaid‸");
    assert_eq!(after("> > - one\n> > - two‸", &["Tab"]), "> > - one\n> > \t- two‸");
    assert_eq!(after("> - one\n> - ‸", &["Enter"]), "> - one\n> ‸");
    assert_eq!(after("> - [ ] one\n> - [ ] ‸", &["Enter"]), "> - [ ] one\n> ‸");
    assert_eq!(after("> 1. one\n> 2. ‸", &["Enter"]), "> 1. one\n> ‸");
    assert_eq!(after("> > - one\n> > - ‸", &["Enter"]), "> > - one\n> > ‸");
    assert_eq!(after("> - one\n> \t- ‸", &["Enter"]), "> - one\n> - ‸");
    assert_eq!(after("> task‸", &["run:list.toggleCheckbox"]), "> - [ ] task‸");
}

#[test]
fn cmd_left_and_home_stop_at_an_item_s_text_first() {
    assert_eq!(after("- item tex‸t", &["Cmd+ArrowLeft"]), "- ‸item text");
    assert_eq!(after("- item tex‸t", &["Cmd+ArrowLeft", "Cmd+ArrowLeft"]), "‸- item text");
    assert_eq!(after("- [ ] item‸", &["Home"]), "- [ ] ‸item");
    assert_eq!(after("> quoted‸", &["Shift+Cmd+ArrowLeft"]), "> ‹quoted›");
    assert_eq!(after("## Head‸", &["Cmd+ArrowLeft"]), "‸## Head", "a heading's marks are no item's");
}

#[test]
fn a_third_backtick_closes_its_code_block() {
    assert_eq!(after("x\n‸", &["type:```"]), "x\n```‸\n```");
    assert_eq!(after("x\n‸", &["type:```js"]), "x\n```js‸\n```");
    assert_eq!(after("‸abc", &["type:```"]), "```‸\n```abc");
    assert_eq!(after("abc‸", &["type:```"]), "abc```‸\n```");
    assert_eq!(after("- ‸", &["type:```"]), "- ```‸\n  ```");
    assert_eq!(after("> ‸", &["type:```"]), "> ```‸\n> ```");
    assert_eq!(after("x\n‸", &["type:````"]), "x\n````‸\n```", "a fourth is typed");
    assert_eq!(after("x ‸", &["type:``"]), "x ``‸");
}

#[test]
fn comment_and_format_commands_take_what_obsidian_takes() {
    assert_eq!(after("some ‸text", &["run:format.comment"]), "some %%‹text›%%");
    assert_eq!(after("some %%te‸xt%% here", &["run:format.comment"]), "some te‸xt here");
    assert_eq!(after("a ‸ b", &["run:format.comment"]), "a %%‸%% b");
    assert_eq!(after("a‹ word ›b", &["run:format.bold"]), "a **‹word›** b", "the spaces stay outside");
}

#[test]
fn delete_paragraph_keeps_the_caret_s_column() {
    assert_eq!(after("one\nab‸c\nabcde", &["run:edit.deleteParagraph"]), "one\nab‸cde");
    assert_eq!(after("one\nabcd‸e\nab", &["run:edit.deleteParagraph"]), "one\nab‸");
    assert_eq!(after("one\nt‹wo\nabc›ee\nabcd", &["run:edit.deleteParagraph"]), "one\nabc‸d");
    assert_eq!(after("one\ntwo‸", &["run:edit.deleteParagraph"]), "one‸");
}

#[test]
fn a_pasted_address_links_the_selection_and_stays_selected() {
    let paste = |text: &str, address: &str| {
        let mut doc = note(&text.replace(['‹', '›'], ""));
        let from = text.chars().position(|c| c == '‹').unwrap();
        let to = text.chars().position(|c| c == '›').unwrap() - 1;
        at(&mut doc, from, to);
        KitTools::text(&mut doc, address, 1.0).unwrap();
        marked(&doc)
    };
    assert_eq!(paste("see ‹site› now", "https://example.com"), "see ‹[site](https://example.com)› now");
    assert_eq!(paste("see ‹https://a.com› now", "https://example.com"), "see ‹[https://a.com](https://example.com)› now");
}

#[test]
fn alt_enter_follows_the_link_at_the_caret_and_types_nothing() {
    let mut doc = note("see [[Welcome]] now");
    at(&mut doc, 12, 12);
    let response = doc.key("Enter", alt(), 1.0).unwrap();
    assert!(format!("{:?}", response.requests).contains("openLink"), "{:?}", response.requests);
    assert_eq!(doc.body(), "see [[Welcome]] now");
    assert_eq!(after("task‸", &["Alt+Enter"]), "task‸");
}

#[test]
fn an_undo_step_that_changed_nothing_undoes_nothing() {
    // Enter then Backspace: the step's changes come to nothing, so Undo changes nothing (as in
    // Obsidian), and the step before it stays for the next Undo.
    let mut doc = note("start");
    at(&mut doc, 5, 5);
    type_in(&mut doc, "x");
    key(&mut doc, "ArrowLeft");
    key(&mut doc, "ArrowRight");
    key(&mut doc, "Enter");
    key(&mut doc, "Backspace");
    undo(&mut doc);
    assert_eq!(marked(&doc), "startx‸");
    undo(&mut doc);
    assert_eq!(marked(&doc), "start‸");
}

#[test]
fn a_selection_set_forgets_the_column_up_and_down_kept() {
    let mut doc = note(
        "abcdef
x
ab
xyz",
    );
    at(&mut doc, 6, 6);
    key(&mut doc, "ArrowDown");
    // Elsewhere (a search hit, a host): Down goes from there, not from the old column.
    at(&mut doc, 11, 11);
    key(&mut doc, "ArrowDown");
    assert_eq!(marked(&doc), "abcdef\nx\nab\nxy‸z");
}

/// The note `text` (`‸` the caret) after `insert` replaces `from..to` (characters) through the
/// editor, as a person's change.
fn changed(text: &str, from: usize, to: usize, insert: &str) -> String {
    let mut doc = note(text);
    at(&mut doc, from, to);
    if insert.is_empty() {
        key(&mut doc, "Backspace");
    } else {
        type_in(&mut doc, insert);
    }
    doc.body().to_owned()
}

#[test]
fn a_change_in_an_ordered_list_renumbers_it_as_obsidian_does() {
    // Measured in Obsidian: the item on a changed line takes the number after its previous
    // sibling's, the items after it count on, and the list on the line after the change too.
    assert_eq!(changed("1. a\n5. b\n9. c\n", 3, 3, "Q"), "1. Qa\n2. b\n3. c\n");
    assert_eq!(changed("1. a\n\t4. x\n\t7. y\n5. b\n", 3, 3, "Q"), "1. Qa\n\t4. x\n\t5. y\n2. b\n");
    assert_eq!(changed("1. a\n\t4. x\n\t7. y\n5. b\n9. c\n", 9, 9, "Q"), "1. a\n\t4. Qx\n\t5. y\n5. b\n9. c\n");
    assert_eq!(changed("1. a\n\t1. x\n5. b\n", 9, 9, "Q"), "1. a\n\t1. Qx\n2. b\n");
    assert_eq!(changed("1. a\n\n5. b\n\n9. c\n", 3, 3, "Q"), "1. Qa\n\n2. b\n\n3. c\n", "a loose list");
    assert_eq!(changed("3. a\n5. b\n", 3, 3, "Q"), "3. Qa\n4. b\n", "the first item keeps its number");
    assert_eq!(changed("1) a\n5) b\n", 3, 3, "Q"), "1) Qa\n2) b\n");
    assert_eq!(changed("1. a\n\t- x\n5. b\n", 3, 3, "Q"), "1. Qa\n\t- x\n2. b\n");
    assert_eq!(changed("> 1. a\n> 5. b\n", 5, 5, "Q"), "> 1. Qa\n> 2. b\n");
    assert_eq!(changed("9. a\n5. b\n", 3, 3, "Q"), "9. Qa\n10. b\n");
    assert_eq!(changed("1. a\ncont\n5. b\n", 3, 3, "Q"), "1. Qa\ncont\n2. b\n");
    assert_eq!(changed("1. a\n2. b\n3. c\n", 5, 10, ""), "1. a\n2. c\n");
    assert_eq!(changed("1. one\n", 7, 7, "5. x"), "1. one\n2. x");
    assert_eq!(changed("1. a\n\n2. b\n", 5, 5, "7. x"), "1. a\n2. x\n3. b\n");
    assert_eq!(changed("1. a\n3. b\n\t1. x\n\t7. y\n5. c\n", 8, 8, "Q"), "1. a\n2. Qb\n\t1. x\n\t2. y\n3. c\n");
    // Elsewhere, nothing is renumbered.
    assert_eq!(changed("1. one\n3. three\n\nfar\n", 17, 17, "Q"), "1. one\n3. three\n\nQfar\n");
    assert_eq!(changed("```\n1. a\n5. b\n```\n", 7, 7, "Q"), "```\n1. Qa\n5. b\n```\n", "code is no list");
}

#[test]
fn renumbering_keeps_the_caret_and_the_undo_step() {
    let mut doc = note("9. a\n5. b");
    at(&mut doc, 9, 9);
    type_in(&mut doc, "x");
    assert_eq!(marked(&doc), "9. a\n10. bx‸");
    undo(&mut doc);
    assert_eq!(marked(&doc), "9. a\n5. b‸", "the renumbering is in the step it follows");
}

#[test]
fn format_marks_merge_and_come_off_as_obsidian_s() {
    // Measured in Obsidian.
    for (text, command, want) in [
        ("a ‹**b** c› d", "format.bold", "a **‹b c›** d"),
        ("a ‹**b** **c**› d", "format.bold", "a **‹b c›** d"),
        ("a **‹b** c› d", "format.bold", "a **‹b c›** d"),
        ("a **b ‹c** d› e", "format.bold", "a **b ‹c d›** e"),
        ("a ‹*b* c› d", "format.italic", "a *‹b c›* d"),
        ("a **‹b›** c", "format.italic", "a ***‹b›*** c"),
        ("a ***‹b›*** c", "format.italic", "a **‹b›** c"),
        ("a ***‹b›*** c", "format.bold", "a *‹b›* c"),
        ("a *‹b›* c", "format.bold", "a ***‹b›*** c"),
        ("a _‹b›_ c", "format.italic", "a ‹b› c"),
        ("a __‹b›__ c", "format.bold", "a ‹b› c"),
        ("a ‹**word**› b", "format.bold", "a ‹word› b"),
        ("a ‹*b*› d", "format.bold", "a **‹*b*›** d"),
    ] {
        assert_eq!(after(text, &[&format!("run:{command}")]), want, "{command} on {text:?}");
    }
}

#[test]
fn toggle_checkbox_takes_every_line_as_obsidian_s() {
    for (text, want) in [
        ("‹- [x] a\n- [ ] b›", "‹- [x] a\n- [x] b›"),
        ("‹- [ ] a\n- [x] b›", "‹- [x] a\n- [x] b›"),
        ("‹- [x] a\n- [x] b›", "‹- [ ] a\n- [ ] b›"),
        ("‹a\n- [x] b›", "- [ ] ‹a\n- b›"),
        ("‹- [ ] a\nb›", "‹- a\n- [ ] b›"),
        ("‹- a\n- [x] b›", "‹- [ ] a\n- b›"),
        ("‹a\n- b›", "- [ ] ‹a\n- [ ] b›"),
        ("- [/] t‸", "- [ ] t‸"),
        ("- [-] t‸", "- [ ] t‸"),
        ("- [X] a‸", "- [ ] a‸"),
        ("1. [x] a‸", "1. [ ] a‸"),
    ] {
        assert_eq!(after(text, &["run:list.toggleCheckbox"]), want, "on {text:?}");
    }
}

#[test]
fn macos_control_keys_edit_as_in_obsidian() {
    let control = |text: &str, letters: &str| {
        let plain: String = text.replace('‸', "");
        let mut doc = note(&plain);
        let caret = text.chars().position(|c| c == '‸').unwrap();
        at(&mut doc, caret, caret);
        for letter in letters.chars() {
            doc.execute("select.controlKey", json!({ "key": letter.to_string() })).unwrap();
        }
        marked(&doc)
    };
    assert_eq!(control("- item tex‸t", "a"), "‸- item text");
    assert_eq!(control("- ‸item text", "e"), "- item text‸");
    assert_eq!(control("a‸bc", "f"), "ab‸c");
    assert_eq!(control("ab‸c", "b"), "a‸bc");
    assert_eq!(control("ab‸\ncd", "f"), "ab\n‸cd");
    assert_eq!(control("one ‸two\nthree", "k"), "one ‸\nthree");
    assert_eq!(control("one‸\ntwo", "k"), "one‸two");
    assert_eq!(control("one‸\ntwo", "d"), "one‸two");
    assert_eq!(control("one\n‸two", "h"), "one‸two");
    assert_eq!(control("ab‸c", "t"), "acb‸");
    assert_eq!(control("abc‸", "t"), "acb‸");
    assert_eq!(control("‸abc", "t"), "‸abc");
    assert_eq!(control("ab‸c", "o"), "ab‸\nc");
    let mut doc = note("o ne two");
    at(&mut doc, 1, 3);
    doc.execute("select.controlKey", json!({ "key": "k" })).unwrap();
    assert_eq!(marked(&doc), "o‸e two", "a selection goes");
}

#[test]
fn table_keys_change_only_the_lines_they_are_about() {
    // Tab, Shift+Tab, and Enter select the cell they go to (as Obsidian's table editor), past the
    // last cell a new row; a short row gets its missing cells; the padding and pipes stay.
    assert_eq!(after("| a | b |\n| - | - |\n| c‸ | d |", &["Tab"]), "| a | b |\n| - | - |\n| c | ‹d› |");
    assert_eq!(after("| a | b |\n| - | - |\n| c | d‸ |", &["Shift+Tab"]), "| a | b |\n| - | - |\n| ‹c› | d |");
    assert_eq!(after("| a | b |\n| - | - |\n| c | d‸ |", &["Tab"]), "| a | b |\n| - | - |\n| c | d |\n|  ‸|  |");
    assert_eq!(after("| a‸ | b |\n| - | - |\n| c | d |", &["Enter"]), "| a | b |\n| - | - |\n| ‹c› | d |");
    assert_eq!(after("| a | b | c |\n| - | - | - |\n| x‸ |", &["Tab"]), "| a | b | c |\n| - | - | - |\n| x |  ‸|");
    assert_eq!(after("| a \\| x | b |\n| - | - |\n| c‸ | d |", &["Tab"]), "| a \\| x | b |\n| - | - |\n| c | ‹d› |");
    assert_eq!(
        after("| a \\| x | b |\n| - | - |\n| c | d‸ |", &["Shift+Tab", "Shift+Tab", "Shift+Tab"]),
        "| a \\| x | b |\n| - | - |\n| c | d |".replacen("| a \\| x |", "| ‹a \\| x› |", 1)
    );
    assert_eq!(after("| a | b |\n| - | - |\n| c | ‸ |", &["Backspace"]), "| a | b |\n| - | - |\n| c | ‸ |");
    assert_eq!(after("| a | b |\n| - | - |\n| ‸c | d |", &["Backspace"]), "| a | b |\n| - | - |\n| ‸c | d |");
    assert_eq!(after("| a | b |\n| - | - |\n| c‸ | d |", &["Delete"]), "| a | b |\n| - | - |\n| c‸ | d |");
    assert_eq!(after("| a | b |\n| - | - |\n| cc‸ | d |", &["Backspace"]), "| a | b |\n| - | - |\n| c‸ | d |");
    // Without outer pipes, Enter is a new line (Obsidian edits no such table as a table).
    assert_eq!(after("a | b\n- | -\nc | d‸", &["Enter"]), "a | b\n- | -\nc | d\n‸");
    assert_eq!(after("a | b\n- | -\nc‸ | d", &["Tab"]), "a | b\n- | -\nc | ‹d›");
}

#[test]
fn deleting_a_span_s_closer_takes_its_orphaned_opener() {
    // Measured in Obsidian: only an opener right before the deleted text goes with its closer.
    assert_eq!(after("a **‹b** c›", &["Backspace"]), "a ‸");
    assert_eq!(after("a **‹b** c›", &["type:x"]), "a x‸");
    assert_eq!(after("x `‹a` b›", &["Delete"]), "x ‸");
    assert_eq!(after("a ==‹b== c›", &["Backspace"]), "a ‸");
    assert_eq!(after("a ~~‹b~~ c›", &["Backspace"]), "a ‸");
    assert_eq!(after("a *‹b* c›", &["Backspace"]), "a ‸");
    assert_eq!(after("a **‹b** c **d**›", &["Backspace"]), "a ‸");
    assert_eq!(after("a ‹c **b›** d", &["Backspace"]), "a ‸** d", "a closer stays");
    assert_eq!(after("a **b‹c** d›", &["Backspace"]), "a **b‸");
    assert_eq!(after("==a ‹b== c›", &["Backspace"]), "==a ‸");
    assert_eq!(after("a **‹b**›", &["Backspace"]), "a **‸", "the closer only: nothing more");
    let mut doc = note("a **b** c");
    at(&mut doc, 4, 9);
    doc.execute("edit.cut", json!({})).unwrap();
    assert_eq!(doc.body(), "a ");
}

#[test]
fn a_bracket_over_several_lines_types_its_pair() {
    assert_eq!(after("‹a\nb›", &["type:("]), "(‸)");
    assert_eq!(after("‹a\nb›", &["type:\""]), "\"‸\"");
    assert_eq!(after("‹a\nb›", &["type:*"]), "*‸*");
    assert_eq!(after("‹a\nb›", &["type:`"]), "`‹a\nb›`");
    assert_eq!(after("‹a\nb›", &["type:="]), "=‹a\nb›=");
}

#[test]
fn insert_commands_place_their_blocks_as_obsidian_s() {
    for (text, command, want) in [
        ("a‸", "codeBlock.insert", "```\na‸\n```"),
        ("‹a\nb›", "codeBlock.insert", "```\n‹a\nb›\n```"),
        ("a ‹b› c", "codeBlock.insert", "```\na ‹b› c\n```"),
        ("a\n‸", "codeBlock.insert", "a\n```\n‸\n```"),
        ("a‸", "mathBlock.insert", "$$\na‸\n$$"),
        ("‸", "mathBlock.insert", "$$\n‸\n$$"),
        ("a\n‸", "rule.insert", "a\n\n---\n‸"),
        ("‸", "rule.insert", "\n---\n‸"),
        ("ab‸c", "rule.insert", "ab\n\n---\n‸c"),
        ("a\n‸", "callout.insert", "a\n\n> [!‹NOTE›] Title\n> Contents\n"),
        ("a‸", "callout.insert", "> [!‹NOTE›]\n> a"),
        ("‹a\nb›", "callout.insert", "> [!‹NOTE›]\n> a\n> b"),
        ("a‸", "table.insert", "a\n\n|‸     |     |\n| --- | --- |\n|     |     |\n"),
        ("‸", "table.insert", "\n|‸     |     |\n| --- | --- |\n|     |     |\n"),
        ("a ‹b›", "wikilink.insert", "a [[b‸]]"),
        ("a ‹b›", "embed.insert", "a ![[b‸]]"),
        ("a ‸", "wikilink.insert", "a [[‸]]"),
        ("a ‹b›", "tag.insert", "a #‸b"),
    ] {
        assert_eq!(after(text, &[&format!("run:{command}")]), want, "{command} on {text:?}");
    }
}

/// Drags the selection `from..to` (characters) of `text` from character `press` to `drop`.
fn drag_selection(text: &str, from: usize, to: usize, press: usize, drop: usize, alt: bool) -> String {
    assert!(fonts(), "kition-fonts is needed to hit-test");
    let mut doc = note(text);
    at(&mut doc, from, to);
    let place = |doc: &mut NoteDocument, index: usize| {
        let at = doc.execute("select.where", json!({ "at": index })).unwrap();
        (at["x"].as_f64().unwrap() + 0.5, at["y"].as_f64().unwrap() + at["height"].as_f64().unwrap() / 2.0)
    };
    let mods = ToolMods { alt, ..ToolMods::default() };
    let (px, py) = place(&mut doc, press);
    let (dx, dy) = place(&mut doc, drop);
    for (action, x, y) in [
        (PointerAction::Down, px, py),
        (PointerAction::Drag, (px + dx) / 2.0, (py + dy) / 2.0),
        (PointerAction::Drag, dx, dy),
        (PointerAction::Up, dx, dy),
    ] {
        doc.pointer(&ToolPointer { action, x, y, mods, pressure: 1.0, zoom: 1.0 }).unwrap();
    }
    marked(&doc)
}

#[test]
fn a_dragged_selection_moves_where_it_is_dropped() {
    // Measured in Obsidian (CodeMirror's drag and drop, its events dispatched over CDP).
    assert_eq!(drag_selection("one two three\n\nend", 4, 7, 5, 13, false), "one  three‹two›\n\nend");
    assert_eq!(drag_selection("one two three\nfour five\n\nend", 4, 7, 5, 19, false), "one  three\nfour ‹two›five\n\nend");
    assert_eq!(drag_selection("one two three", 8, 13, 9, 0, false), "‹three›one two ");
    assert_eq!(drag_selection("one two three", 4, 7, 5, 13, true), "one two three‹two›", "Alt copies");
    assert_eq!(drag_selection("one two three", 4, 8, 5, 10, false), "one th‹two ›ree");
    assert_eq!(drag_selection("one\ntwo\nthree", 4, 7, 5, 13, false), "one\n\nthree‹two›");
    assert_eq!(drag_selection("one two three", 4, 7, 5, 6, false), "one ‸two three", "dropped on itself");
    assert_eq!(drag_selection("one two three", 4, 7, 5, 5, false), "one t‸wo three", "a click in the selection");
}

#[test]
fn delete_at_a_folded_line_s_end_takes_what_the_fold_hides() {
    for (text, fold_at, want) in [("- a\n\t- b\n\t- c\n- d", 3, "- a‸\n- d"), ("# H\nbody\n# I\nmore", 3, "# H‸\n# I\nmore")] {
        let mut doc = note(text);
        at(&mut doc, fold_at, fold_at);
        doc.execute("view.fold", json!({})).unwrap();
        key(&mut doc, "Delete");
        assert_eq!(marked(&doc), want, "Delete after folding {text:?}");
    }
}

#[test]
fn a_fold_s_end_is_a_caret_stop_drawn_at_its_line_s_end() {
    // Measured in Obsidian: Right at a folded line's end goes past the fold (the section's last
    // character), Left after it comes back there, and Fold all leaves a caret already there.
    let fold = |text: &str, caret: usize| {
        let mut doc = note(text);
        at(&mut doc, caret, caret);
        doc.execute("view.fold", json!({})).unwrap();
        doc
    };
    let mut doc = fold("# H\nbody\n# I", 3);
    key(&mut doc, "ArrowRight");
    assert_eq!(marked(&doc), "# H\nbody‸\n# I");
    assert!(doc.view().is_hidden(5), "the fold stays");
    assert_eq!(doc.caret_rect(), {
        let caret = doc.view().caret(3);
        json!({ "x": caret.x, "y": caret.top, "height": caret.height })
    });
    key(&mut doc, "ArrowLeft");
    assert_eq!(marked(&doc), "# H‸\nbody\n# I");
    let mut doc = fold("# H\nbody\n# I", 3);
    at(&mut doc, 9, 9);
    key(&mut doc, "ArrowLeft");
    assert_eq!(marked(&doc), "# H\nbody‸\n# I");
    let mut doc = note("# H\nbody\n# I\nmore");
    at(&mut doc, 8, 8);
    doc.execute("view.foldAll", json!({})).unwrap();
    assert_eq!(marked(&doc), "# H\nbody‸\n# I\nmore", "Fold all leaves the caret at the fold's end");
    key(&mut doc, "ArrowDown");
    assert_eq!(marked(&doc), "# H\nbody\n# I\nmore‸");
}

#[test]
fn backspace_at_a_fold_s_end_takes_what_it_hides() {
    let mut doc = note("# H\nbody\n# I");
    at(&mut doc, 3, 3);
    doc.execute("view.fold", json!({})).unwrap();
    key(&mut doc, "ArrowRight");
    key(&mut doc, "Backspace");
    assert_eq!(marked(&doc), "# H‸\n# I");
    // Up onto a folded line's end from the same column stays before the fold.
    let mut doc = note("# H\nbody\n# I");
    at(&mut doc, 12, 12);
    doc.execute("view.foldAll", json!({})).unwrap();
    key(&mut doc, "ArrowUp");
    assert_eq!(marked(&doc), "# H‸\nbody\n# I");
}

#[test]
fn round_four_editing_as_obsidian_does() {
    // Measured in Obsidian.
    assert_eq!(after("- a\n  b‸", &["Enter"]), "- a\n  b\n- ‸", "Enter on a continuation line starts the next item");
    assert_eq!(after("- a\n  b‸c", &["Enter"]), "- a\n  b\n- ‸c");
    assert_eq!(after("1. a\n   b‸", &["Enter"]), "1. a\n   b\n2. ‸");
    assert_eq!(after("- [ ] a\n      b‸", &["Enter"]), "- [ ] a\n      b\n- [ ] ‸");
    assert_eq!(after("> - a\n>   b‸", &["Enter"]), "> - a\n>   b\n> - ‸");
    assert_eq!(after("> 1. a\n> 2. b‸", &["Tab"]), "> 1. a\n> \t1. b‸", "a nested item in a quote counts from 1");
    assert_eq!(after("ab‸c\nabc\nabc", &["PageDown"]), "abc\nabc\nabc‸", "a page past the end goes to it");
    assert_eq!(after("abc\nabc\nab‸c", &["PageUp"]), "‸abc\nabc\nabc");
    assert_eq!(after("ab‸c\nabc", &["Shift+PageDown"]), "ab‹c\nabc›");
}

#[test]
fn up_and_down_enter_a_table_at_a_cell_s_text_start() {
    assert!(fonts());
    assert_eq!(
        after("ab‸c\n\n| a | b |\n| - | - |\n| c | d |\n\nabc", &["ArrowDown", "ArrowDown"]),
        "abc\n\n| ‸a | b |\n| - | - |\n| c | d |\n\nabc"
    );
    assert_eq!(after("| a | b |\n| - | - |\n| c‸ | d |", &["ArrowUp"]), "| ‸a | b |\n| - | - |\n| c | d |");
    assert_eq!(after("| a | b |\n| - | - |\n| c‸ | d |\n\nabc", &["ArrowDown", "ArrowDown"]), "| a | b |\n| - | - |\n| c | d |\n\n‸abc");
}

#[test]
fn word_moves_stop_at_the_hidden_marks_of_spans_away_from_the_caret() {
    // Measured in Obsidian (live preview hides those marks, one atomic range each).
    assert_eq!(after("a **bold** c‸", &["Alt+ArrowLeft", "Alt+ArrowLeft"]), "a **bold**‸ c");
    assert_eq!(after("a **bold** ‸c", &["Alt+ArrowLeft"]), "a **bold**‸ c");
    assert_eq!(after("‸a **bold** c", &["Alt+ArrowRight", "Alt+ArrowRight"]), "a ‸**bold** c");
    assert_eq!(after("a **bold** c‸", &["Shift+Alt+ArrowLeft", "Shift+Alt+ArrowLeft"]), "a **bold**‹ c›");
}

/// Timings on a large note (`LARGE_NOTE=<path> cargo test --release -p kition-note-kit -- --ignored large_note_timings --nocapture`).
#[test]
#[ignore]
fn large_note_timings() {
    let Some(path) = std::env::var_os("LARGE_NOTE") else { return };
    fonts();
    let text = std::fs::read_to_string(path).unwrap();
    // OPENS=n opens the note n times first (to profile opening).
    for _ in 0..std::env::var("OPENS").ok().and_then(|opens| opens.parse().ok()).unwrap_or(0) {
        let doc = note(&text);
        let _ = doc.render(0.0, 0.0, 1.0, 816, 900);
    }
    let started = std::time::Instant::now();
    let mut doc = note(&text);
    eprintln!("open {:?}", started.elapsed());
    let started = std::time::Instant::now();
    let _ = doc.render(0.0, 0.0, 1.0, 816, 900);
    eprintln!("first frame {:?}", started.elapsed());
    let middle = text.find("## Section 150").unwrap() + "## Section 150".len();
    let chars = text.get(..middle).unwrap().chars().count();
    at(&mut doc, chars, chars);
    for c in ["a", "b", "c", "d", "e"] {
        let started = std::time::Instant::now();
        KitTools::text(&mut doc, c, 1.0).unwrap();
        let typed = started.elapsed();
        let _ = doc.render(0.0, 0.0, 1.0, 816, 900);
        eprintln!("keystroke {typed:?} (+ frame {:?})", started.elapsed() - typed);
    }
    let rounds = std::env::var("ROUNDS").ok().and_then(|rounds| rounds.parse().ok()).unwrap_or(3);
    for _ in 0..rounds {
        let started = std::time::Instant::now();
        key(&mut doc, "Enter");
        eprintln!("enter {:?}", started.elapsed());
        key(&mut doc, "Backspace");
    }
    let started = std::time::Instant::now();
    doc.execute("select.get", json!({})).unwrap();
    eprintln!("select.get {:?}", started.elapsed());
    // What a host asks after each step: the revision and the summary (after an edit, then again).
    KitTools::text(&mut doc, "z", 1.0).unwrap();
    // INSPECTS=n types and reads the summary n times more first (to profile it).
    for _ in 0..std::env::var("INSPECTS").ok().and_then(|count| count.parse().ok()).unwrap_or(0) {
        KitTools::text(&mut doc, "z", 1.0).unwrap();
        let started = std::time::Instant::now();
        let _ = doc.inspect().unwrap().to_string();
        eprintln!("inspect after typing {:?}", started.elapsed());
    }
    for _ in 0..2 {
        let started = std::time::Instant::now();
        let _ = doc.revision();
        let revised = started.elapsed();
        let started = std::time::Instant::now();
        let seen = doc.inspect().unwrap().to_string();
        eprintln!("revision {revised:?}, inspect {:?} ({} bytes)", started.elapsed(), seen.len());
    }
}

#[test]
fn end_and_home_on_a_wrapped_row_go_as_obsidian_s() {
    assert!(fonts());
    let text = "The quick brown fox jumps over the lazy dog while the five boxing wizards jump quickly and a wizard job is to vex chumps quickly in fog, then the sphinx of black quartz judges my vow and packs my box with five dozen liquor jugs before the night is over.";
    let mut doc = note(text);
    doc.execute("view.width", json!({ "width": 700 })).unwrap();
    // Where the second row starts.
    let top = |doc: &mut NoteDocument, at: usize| doc.execute("select.where", json!({ "at": at })).unwrap()["y"].as_f64().unwrap();
    let first = top(&mut doc, 0);
    let second = (1..text.len()).find(|at| top(&mut doc, *at) > first + 2.0).unwrap();
    assert_eq!(&text[second - 1..second], " ", "rows break after a space");
    at(&mut doc, 10, 10);
    key(&mut doc, "End");
    assert_eq!(doc.editor.focus, second, "End goes after the row's trailing space");
    let caret = doc.caret_rect();
    assert!((caret["y"].as_f64().unwrap() - first).abs() < 4.0, "drawn on the first row: {caret}");
    KitTools::text(&mut doc, "X", 1.0).unwrap();
    assert_eq!(&doc.body()[second - 1..second + 1], " X", "typing goes there");
    let mut doc = note(text);
    doc.execute("view.width", json!({ "width": 700 })).unwrap();
    at(&mut doc, 10, 10);
    key(&mut doc, "End");
    key(&mut doc, "End");
    assert_eq!(doc.editor.focus, text.len(), "a second End goes to the line's end");
    at(&mut doc, second + 5, second + 5);
    key(&mut doc, "Home");
    assert_eq!(doc.editor.focus, second, "Home goes to the row's start");
    key(&mut doc, "Home");
    assert_eq!(doc.editor.focus, 0, "a second Home goes to the line's start");
    key(&mut doc, "End");
    at(&mut doc, 10, 10);
    doc.key("End", shift(), 1.0).unwrap();
    assert_eq!((doc.editor.anchor, doc.editor.focus), (10, second), "Shift+End selects to the row's end");
}

#[test]
fn end_in_a_table_cell_stays_at_its_text_s_end() {
    assert_eq!(after("| A | B |\n|---|---|\n| o‸ne | two |", &["End"]), "| A | B |\n|---|---|\n| one‸ | two |");
}

#[test]
fn undo_selection_and_the_remaining_default_keys_as_obsidian_s() {
    // Measured in Obsidian: Cmd+U takes the caret back before the last moves (moves soon after
    // each other are one step), and after an edit undoes it; Cmd+Shift+U redoes.
    let control = |doc: &mut NoteDocument, name: &str| doc.execute("select.controlKey", json!({ "key": name })).unwrap();
    let mut doc = note("one two");
    at(&mut doc, 7, 7);
    doc.key("ArrowLeft", alt(), 1.0).unwrap();
    doc.key("ArrowLeft", alt(), 1.0).unwrap();
    control(&mut doc, "undoSelection");
    assert_eq!(marked(&doc), "one two‸");
    control(&mut doc, "redoSelection");
    assert_eq!(marked(&doc), "‸one two");
    let mut doc = note("x ");
    at(&mut doc, 2, 2);
    type_in(&mut doc, "ab");
    control(&mut doc, "undoSelection");
    assert_eq!(marked(&doc), "x ‸", "after an edit, Undo selection undoes it");
    // Ctrl+Alt+H deletes a group back; Cmd+Delete to the row's end.
    let mut doc = note("one two");
    at(&mut doc, 7, 7);
    control(&mut doc, "groupH");
    assert_eq!(marked(&doc), "one ‸");
    let command = ToolMods { command: true, ..ToolMods::default() };
    let mut doc = note("one two three");
    at(&mut doc, 4, 4);
    doc.key("Delete", command, 1.0).unwrap();
    assert_eq!(marked(&doc), "one ‸");
}

#[test]
fn coloured_highlights_toggle_as_obsidian_s() {
    for (text, color, want) in [
        ("a ‹b› c", Some("red"), "a ==🔴‹b›== c"),
        ("a wo‸rd c", Some("blue"), "a ==🔵wo‸rd== c"),
        ("a ==‹b›== c", Some("green"), "a ==🟢‹b›== c"),
        ("a ‸ c", Some("purple"), "a ==🟣‸== c"),
        ("a ==🔴‹b›== c", Some("red"), "a ‹b› c"),
        ("a ==🔴‹b›== c", Some("blue"), "a ==🔵‹b›== c"),
        ("a ==🔴‹b›== c", None, "a ‹b› c"),
    ] {
        let command = match color {
            Some(color) => {
                let mut doc = note(&text.replace(['‹', '›', '‸'], ""));
                let chars: Vec<char> = text.chars().collect();
                let (a, f) = match (chars.iter().position(|c| *c == '‹'), chars.iter().position(|c| *c == '‸')) {
                    (Some(a), _) => (a, chars.iter().position(|c| *c == '›').unwrap() - 1),
                    (None, caret) => (caret.unwrap(), caret.unwrap()),
                };
                at(&mut doc, a, f);
                doc.execute("format.highlightColor", json!({ "color": color })).unwrap();
                marked(&doc)
            }
            None => after(text, &["run:format.highlight"]),
        };
        assert_eq!(command, want, "{color:?} on {text:?}");
    }
}

#[test]
fn fold_more_and_fold_less_go_a_level_at_a_time() {
    let text = "# A\na\n## B\nb\n### C\nc\n# D\nd\n";
    let mut doc = note(text);
    at(&mut doc, 1 + text.find("\nc").unwrap(), 1 + text.find("\nc").unwrap());
    doc.execute("view.foldMore", json!({})).unwrap();
    assert_eq!(doc.editor.folded, [text.find("### C").unwrap()], "the innermost first");
    doc.execute("view.foldMore", json!({})).unwrap();
    doc.execute("view.foldMore", json!({})).unwrap();
    assert_eq!(doc.editor.folded.len(), 3, "then outward");
    doc.execute("view.foldLess", json!({})).unwrap();
    assert!(!doc.editor.folded.contains(&0), "Fold less opens the outermost");
    // Measured in Obsidian: from a section's body, Fold more folds that section, then nothing more.
    let mut doc = note("# A\na\n## B\nb\n# D\nd\n");
    at(&mut doc, 4, 4);
    doc.execute("view.foldMore", json!({})).unwrap();
    doc.execute("view.foldMore", json!({})).unwrap();
    assert_eq!(doc.editor.folded, [0]);
    doc.execute("view.foldLess", json!({})).unwrap();
    assert!(doc.editor.folded.is_empty());
}

#[test]
fn a_point_in_a_callout_says_so_and_whether_the_caret_is_in_it() {
    assert!(fonts());
    let text = "intro\n\n> [!note] Title\n> body\n\nend";
    let mut doc = note(text);
    let end = text.chars().count();
    at(&mut doc, end, end);
    let body = doc.execute("select.where", json!({ "at": text.find("body").unwrap() + 1 })).unwrap();
    let point = json!({ "x": body["x"].as_f64().unwrap(), "y": body["y"].as_f64().unwrap() + 4.0 });
    assert_eq!(doc.execute("view.blockAt", point.clone()).unwrap(), json!({ "callout": true, "active": false }));
    let inside = text.find("body").unwrap();
    at(&mut doc, inside, inside);
    // With the caret in it the callout shows its Markdown: no callout drawn there, so the menu opens.
    let answer = doc.execute("view.blockAt", point).unwrap();
    assert!(answer["callout"] == false || answer["active"] == true, "{answer}");
    let intro = doc.execute("select.where", json!({ "at": 1 })).unwrap();
    let outside = json!({ "x": intro["x"].as_f64().unwrap(), "y": intro["y"].as_f64().unwrap() + 4.0 });
    assert_eq!(doc.execute("view.blockAt", outside).unwrap()["callout"], false);
}

#[test]
fn a_task_box_under_the_pointer_takes_the_arrow() {
    let text = "Intro\n\n- [ ] Remove unused copies after testing.\n- [x] Done one\n";
    let mut doc = note(text);
    let hover = |doc: &mut NoteDocument, (x, y): (f64, f64)| {
        doc.pointer(&ToolPointer { action: PointerAction::Move, x, y, mods: ToolMods::default(), pressure: 0.0, zoom: 1.0 }).unwrap()
    };
    let at = |doc: &mut NoteDocument, index: usize| {
        let where_ = doc.execute("select.where", json!({ "at": index })).unwrap();
        (where_["x"].as_f64().unwrap(), where_["y"].as_f64().unwrap() + where_["height"].as_f64().unwrap() / 2.0)
    };
    // The box is drawn where the hidden `[ ]` is: its middle, then its left edge.
    for (line, word) in [("- [ ] Remove", "Remove"), ("- [x] Done", "Done")] {
        let open = text.find(line).unwrap() + 2;
        let (x, y) = at(&mut doc, open);
        let (text_x, _) = at(&mut doc, text.find(word).unwrap());
        assert_eq!(hover(&mut doc, ((x + text_x) / 2.0 - 4.0, y)).cursor, "default", "the arrow over {line}'s box");
        assert_eq!(hover(&mut doc, (x + 2.0, y)).cursor, "default", "the arrow at {line}'s box's left edge");
        assert_eq!(hover(&mut doc, (text_x + 20.0, y)).cursor, "text", "the I-beam over {line}'s text");
    }
}

#[test]
fn the_host_s_color_scheme_repaints_the_note_and_keeps_its_place() {
    assert!(fonts());
    let text = "# Plan\n\nSome text with a [link](https://kition.ai) and a #tag.\n\n> [!note] Note\n> Inside\n";
    let mut doc = note(text);
    doc.execute("view.width", json!({ "width": 800, "height": 600 })).unwrap();
    let revision = doc.revision();
    let light = doc.render(0.0, 0.0, 1.0, 200, 120).unwrap();
    assert_eq!(&light[0..4], &[255, 255, 255, 255], "the light paper");
    let caret = doc.execute("select.where", json!({ "at": 20 })).unwrap();
    let answer = doc.execute("view.scheme", json!({ "scheme": "dark" })).unwrap();
    assert_eq!(answer["changed"], json!(true));
    assert_ne!(doc.revision(), revision, "the canvas repaints");
    let dark = doc.render(0.0, 0.0, 1.0, 200, 120).unwrap();
    assert_eq!(&dark[0..4], &[30, 30, 30, 255], "the dark paper");
    assert_eq!(doc.execute("select.where", json!({ "at": 20 })).unwrap(), caret, "nothing moves");
    assert!(!doc.dirty(), "the scheme is not an edit");
    assert_eq!(doc.execute("view.scheme", json!({ "scheme": "dark" })).unwrap()["changed"], json!(false));
    assert!(doc.execute("view.scheme", json!({ "scheme": "sepia" })).unwrap_err().starts_with("invalid-params"));
    doc.execute("view.scheme", json!({ "scheme": "light" })).unwrap();
    assert_eq!(doc.render(0.0, 0.0, 1.0, 200, 120).unwrap(), light, "back to the same light pixels");
}

#[test]
fn every_shift_arrow_counts_when_the_host_calls_between_two_keys() {
    // Kit-editor runs an agent's step (`ai/run`, `ai/inspect`) between two keys in an editor state
    // of the agent's own: the person's state is read, the agent's put in place, and the person's
    // put back (kition-sdk `kitEngine.worker.ts`, `inAgentState`). Each key still moves the
    // selection one character: after 21 keys it is the 21 characters before the caret.
    let text = "# Plan\n\nGoals for the quarter.\n\nClosing words.\n";
    let mut doc = note(text);
    at(&mut doc, 29, 29);
    let mut agent: Option<serde_json::Value> = None;
    for pressed in 1..=21 {
        doc.key("ArrowLeft", shift(), 1.0).unwrap();
        let person = doc.editor_state();
        if let Some(state) = &agent {
            doc.restore_editor_state(state).unwrap();
        }
        doc.inspect().unwrap();
        doc.execute("select.get", json!({})).unwrap();
        agent = Some(doc.editor_state());
        doc.restore_editor_state(&person).unwrap();
        assert_eq!((doc.editor.anchor, doc.editor.focus), (29, 29 - pressed), "after key {pressed}");
    }
    let selection = doc.execute("select.get", json!({})).unwrap();
    assert_eq!((selection["from"].clone(), selection["to"].clone()), (json!(8), json!(29)));
    assert_eq!(selection["text"], "Goals for the quarter");
}
