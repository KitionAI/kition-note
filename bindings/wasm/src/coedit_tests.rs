//! Co-editing in the editor (Kit UI protocol `replica/import`, managed co-editing L3): another
//! person's changes come in while this person has a caret, a selection, an open typing run and
//! folds, and the editor keeps all of them where the person left them.
use kition_kit::KitDocument;
use kition_kit::session::{self, KitSessionDocument};
use kition_kit::tools::{KitTools, ToolMods};
use serde_json::json;

use crate::NoteDocument;

/// Two editors of one note: `a` opened the file, `b` joined from its replica.
fn pair(text: &str) -> (NoteDocument, NoteDocument) {
    let a = NoteDocument::open("shared.md", text.as_bytes()).unwrap();
    let snapshot = session::__snapshot(&a).unwrap();
    let b = NoteDocument::open_with_replica("shared.md", &[], &snapshot, &[]).unwrap();
    (a, b)
}

/// Runs `edit` on `from` and hands its changes to `to`, as the sync server would.
fn send(from: &mut NoteDocument, to: &mut NoteDocument, edit: impl FnOnce(&mut NoteDocument)) {
    let before = session::__version(from);
    edit(from);
    let changes = session::__export_changes(from, &before).unwrap();
    KitSessionDocument::import_changes(to, &changes).unwrap();
}

/// Everything each has that the other lacks, both ways.
fn sync(a: &mut NoteDocument, b: &mut NoteDocument) {
    let (va, vb) = (session::__version(a), session::__version(b));
    let from_a = session::__export_changes(a, &vb).unwrap();
    let from_b = session::__export_changes(b, &va).unwrap();
    KitSessionDocument::import_changes(a, &from_b).unwrap();
    KitSessionDocument::import_changes(b, &from_a).unwrap();
}

fn select(doc: &mut NoteDocument, anchor: usize, focus: usize) {
    doc.execute("select.set", json!({ "anchor": anchor, "focus": focus })).unwrap();
}

fn typing(doc: &mut NoteDocument, text: &str) {
    for c in text.chars() {
        KitTools::text(doc, &c.to_string(), 1.0).unwrap();
    }
}

/// The selection as `(from, to)` in Unicode characters, as `select.get` answers it.
fn selection(doc: &mut NoteDocument) -> (u64, u64) {
    let got = doc.execute("select.get", json!({})).unwrap();
    (got["from"].as_u64().unwrap(), got["to"].as_u64().unwrap())
}

fn caret(doc: &NoteDocument) -> &str {
    doc.body().get(doc.editor.focus..).unwrap()
}

#[test]
fn the_caret_stays_by_its_word_when_others_edit_before_and_after_it_at_once() {
    let (mut a, mut b) = pair("One two three.\nFour five six.\n");
    // A's caret is before "three".
    select(&mut a, 8, 8);
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 0, "text": "Zero. " })).unwrap();
        b.execute("text.insert", json!({ "at": 25, "text": " and" })).unwrap();
    });
    assert_eq!(a.body(), "Zero. One two three.\nFour and five six.\n");
    assert!(caret(&a).starts_with("three."), "the caret moved with its text: {:?}", caret(&a));
}

#[test]
fn a_selection_keeps_its_text_through_edits_on_both_sides() {
    let (mut a, mut b) = pair("alpha beta gamma delta\n");
    select(&mut a, 6, 16); // "beta gamma"
    send(&mut b, &mut a, |b| {
        b.execute("text.replace", json!({ "from": 0, "to": 5, "text": "ALPHA!" })).unwrap();
        b.execute("text.insert", json!({ "at": 23, "text": " epsilon" })).unwrap();
    });
    let (from, to) = selection(&mut a);
    let text: String = a.body().chars().skip(from as usize).take((to - from) as usize).collect();
    assert_eq!(text, "beta gamma");
}

#[test]
fn undo_takes_back_only_this_persons_typing_and_keeps_the_remote_text() {
    // The replica's undo group ends where another person's change to the text comes in (an
    // import that changes what the group changed closes it), so the run is two steps, each
    // putting back its own selection. Changes go one after the other: B takes A's typing first.
    let (mut a, mut b) = pair("Hello\n");
    select(&mut a, 5, 5);
    typing(&mut a, " there");
    sync(&mut a, &mut b);
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 0, "text": "Remote. " })).unwrap();
    });
    typing(&mut a, " friend");
    assert_eq!(a.body(), "Remote. Hello there friend\n");
    a.execute("edit.undo", json!({})).unwrap();
    assert_eq!(a.body(), "Remote. Hello there\n");
    assert!(caret(&a).starts_with('\n'), "the caret goes back to where the step began: {:?}", caret(&a));
    a.execute("edit.undo", json!({})).unwrap();
    assert_eq!(a.body(), "Remote. Hello\n", "never the other person's text");
    assert!(caret(&a).starts_with('\n'), "{:?}", caret(&a));
    assert!(a.body().get(..a.editor.focus).unwrap().ends_with("Hello"), "{:?}", a.body().get(..a.editor.focus));
    a.execute("edit.redo", json!({})).unwrap();
    a.execute("edit.redo", json!({})).unwrap();
    assert_eq!(a.body(), "Remote. Hello there friend\n");
    sync(&mut a, &mut b);
    assert_eq!(b.body(), a.body());
}

#[test]
fn a_fold_stays_on_its_heading_when_lines_come_in_above() {
    let (mut a, mut b) = pair("# One\nbody one\n# Two\nbody two\n");
    select(&mut a, 16, 16);
    a.execute("view.fold", json!({})).unwrap();
    let folded = a.editor.folded.clone();
    assert_eq!(folded.len(), 1, "{folded:?}");
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 6, "text": "more\nlines\n" })).unwrap();
    });
    let fold = a.editor.folded.first().copied().unwrap();
    assert!(a.body().get(fold..).unwrap().starts_with("# Two"), "{:?}", a.body().get(fold..));
}

#[test]
fn a_section_moved_elsewhere_keeps_the_caret_in_its_paragraph() {
    let (mut a, mut b) = pair("# A\nalpha text\n# B\nbeta text\n# C\ngamma text\n");
    // A's caret is in "gamma text", before "text".
    let at = "# A\nalpha text\n# B\nbeta text\n# C\ngamma ".chars().count();
    select(&mut a, at, at);
    send(&mut b, &mut a, |b| {
        b.execute("section.move", json!({ "heading": ["A"], "toHeading": ["B"], "position": "after" })).unwrap();
    });
    assert!(a.body().starts_with("# B\nbeta text\n"), "{:?}", a.body());
    assert!(caret(&a).starts_with("text\n"), "{:?}", caret(&a));
    assert!(a.body().get(..a.editor.focus).unwrap().ends_with("gamma "), "{:?}", a.body().get(..a.editor.focus));
}

#[test]
fn a_remote_change_while_dragging_a_selection_keeps_the_drag_on_its_text() {
    let (mut a, mut b) = pair("first line\nsecond line\n");
    select(&mut a, 11, 11);
    a.key("End", ToolMods { shift: true, ..ToolMods::default() }, 1.0).unwrap();
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 0, "text": "zero line\n" })).unwrap();
    });
    let (from, to) = selection(&mut a);
    let text: String = a.body().chars().skip(from as usize).take((to - from) as usize).collect();
    assert_eq!(text, "second line");
}

#[test]
fn remote_edits_around_wide_characters_keep_the_caret_on_a_character_boundary() {
    let (mut a, mut b) = pair("\u{4E2D}\u{6587} one \u{1F389} two\n");
    // Before "two".
    let at = "\u{4E2D}\u{6587} one \u{1F389} ".chars().count();
    select(&mut a, at, at);
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 1, "text": "\u{1F600}" })).unwrap();
        b.execute("text.replace", json!({ "from": 4, "to": 7, "text": "\u{6587}\u{6587}" })).unwrap();
        b.execute("text.insert", json!({ "at": 12, "text": "!" })).unwrap();
    });
    assert!(a.body().is_char_boundary(a.editor.focus));
    assert!(caret(&a).starts_with("two"), "{:?} in {:?}", caret(&a), a.body());
}

#[test]
fn presence_shares_replica_positions_and_another_editor_places_them() {
    let (mut a, mut b) = pair("# Title\n\nFirst paragraph.\nSecond \u{1F389} paragraph.\n");
    assert_eq!(a.presence_caret()["caret"], serde_json::Value::Null, "nothing before the person clicks or types");
    // A selects "Second 🎉" (the emoji counts one character).
    let start = "# Title\n\nFirst paragraph.\n".chars().count();
    select(&mut a, start, start + 8);
    let shared = a.presence_caret();
    assert_eq!(shared["caret"], json!({ "object": "note", "field": "body", "offset": start + 8 }));
    assert_eq!(shared["anchor"]["offset"], start);
    assert_eq!(shared["selection"], json!(["note"]));

    // B places A's caret where B's own caret at that position is drawn, and the selection on that line.
    let placed = b.presence_locate(&json!([{ "caret": shared["caret"], "anchor": shared["anchor"] }]));
    select(&mut b, start + 8, start + 8);
    let own = b.caret_rect();
    assert_eq!(placed[0]["caret"]["y"], own["y"]);
    assert_eq!(placed[0]["caret"]["x"], own["x"]);
    let rects = placed[0]["rects"].as_array().unwrap();
    assert!(!rects.is_empty());
    assert!(rects.iter().all(|rect| rect["width"].as_f64().unwrap() > 0.0));

    // Another object, a position past the text, and junk are not placed.
    let none = b.presence_locate(&json!([
        { "caret": { "object": "other", "field": "body", "offset": 1 } },
        { "caret": { "object": "note", "field": "body", "offset": 100_000 } },
        { "caret": "junk" },
    ]));
    for place in none.as_array().unwrap() {
        assert_eq!(place["caret"], serde_json::Value::Null);
    }
    assert_eq!(b.presence_locate(&json!("not a list")), json!([]));
}

#[test]
fn a_peer_caret_moves_with_the_text_another_person_typed_before_it() {
    // Positions travel as characters, so after both replicas take a change, a peer's shared
    // offset names the same place in both.
    let (mut a, mut b) = pair("alpha beta\n");
    select(&mut a, 6, 6);
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 0, "text": "zero " })).unwrap();
    });
    let shared = a.presence_caret();
    assert_eq!(shared["caret"]["offset"], 11);
    let placed = b.presence_locate(&json!([{ "caret": shared["caret"] }]));
    select(&mut b, 11, 11);
    assert_eq!(placed[0]["caret"]["x"], b.caret_rect()["x"]);
}

#[test]
fn a_caret_another_person_moved_says_so_until_this_person_moves_it() {
    let (mut a, mut b) = pair("alpha beta\n");
    select(&mut a, 6, 6);
    assert_eq!(a.caret_rect().get("remote"), None);
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 0, "text": "zero\n" })).unwrap();
    });
    assert_eq!(a.caret_rect()["remote"], true, "the editor keeps it where it is on screen");
    // A change after the caret moves nothing: the caret is still where the other change left it.
    send(&mut b, &mut a, |b| {
        b.execute("text.insert", json!({ "at": 16, "text": "!" })).unwrap();
    });
    assert_eq!(a.caret_rect()["remote"], true);
    typing(&mut a, "x");
    assert_eq!(a.caret_rect().get("remote"), None, "the person's own typing is followed again");
}

#[test]
fn a_co_editors_save_merged_into_this_editor_adds_nothing_twice() {
    // A and B co-edit; A saves. B, with an edit of its own since, takes the saved file in
    // ("Merge its changes"): what B already has from A must not come in a second time.
    let (mut a, mut b) = pair("Goals.\n\nClosing words.\n");
    send(&mut a, &mut b, |a| {
        a.execute("text.insert", json!({ "at": 6, "text": " From A." })).unwrap();
    });
    let saved = a.save(None).unwrap();
    b.execute("text.insert", json!({ "at": 0, "text": "B first. " })).unwrap();
    b.merge_file(&saved).unwrap();
    assert_eq!(b.body(), "B first. Goals. From A.\n\nClosing words.\n");
}
