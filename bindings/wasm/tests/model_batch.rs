//! An edit through the object model (`model/batch`, what extensions and workspace scripts use) is
//! a change of the note like a command's: the revision moves, so the editor draws it again, and the
//! note has unsaved changes, so it can be saved. Before, a batch changed the text and the undo
//! history but left the revision and `dirty` as they were (the note did not implement
//! `KitDocument::batch_applied`): the editor showed the old text and Save stayed off.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_kit::session::model::{Exchange, session_batch};
use kition_note_kit::NoteDocument;
use serde_json::{Value, json};

const NOTE: &str = "# quarterly business review\n\nrevenue grew in the north\n";

fn note() -> NoteDocument {
    NoteDocument::open("Review.md", NOTE.as_bytes()).unwrap()
}

/// Replaces the first heading's text through the object model, as the title-case extension does.
fn retitle() -> Value {
    json!({ "ops": [
        { "op": "text.delete", "target": "root", "field": "body", "index": 2, "count": 25 },
        { "op": "text.insert", "target": "root", "field": "body", "index": 2, "text": "Quarterly Business Review" },
    ] })
}

const RETITLED: &str = "# Quarterly Business Review\n\nrevenue grew in the north\n";

#[test]
fn a_session_batch_moves_the_revision_and_marks_the_note_unsaved() {
    let mut doc = note();
    let before = doc.revision();
    assert!(!doc.dirty());
    session_batch(&mut doc, &retitle(), false).unwrap();
    assert_eq!(doc.text(), RETITLED);
    assert_eq!(doc.inspect().unwrap()["canUndo"], json!(true));
    assert!(doc.revision() > before, "the editor draws a batch's change only when the revision moves");
    assert!(doc.dirty(), "a batch's change is an unsaved change");
    assert_eq!(doc.save(None).unwrap(), RETITLED.as_bytes());
    doc.mark_saved();
    assert!(!doc.dirty());
    // Undo takes the batch back as one step, and that is a change again.
    let saved = doc.revision();
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.text(), NOTE);
    assert!(doc.revision() > saved);
    assert!(doc.dirty());
}

#[test]
fn the_view_and_the_outline_follow_a_session_batch() {
    let mut doc = note();
    let height = doc.height();
    let picture = doc.render(0.0, 0.0, 1.0, 612, 200).unwrap();
    let lines: String = (0..40).map(|n| format!("line {n}\n\n")).collect();
    session_batch(
        &mut doc,
        &json!({ "ops": [{ "op": "text.insert", "target": "root", "field": "body", "index": NOTE.len(), "text": lines }] }),
        false,
    )
    .unwrap();
    assert!(doc.height() > height, "the view is laid out again for the batch's text");
    session_batch(&mut doc, &retitle(), false).unwrap();
    assert_ne!(doc.render(0.0, 0.0, 1.0, 612, 200).unwrap(), picture, "the heading is drawn as the batch left it");
    let outline = doc.inspect().unwrap().to_string();
    assert!(outline.contains("Quarterly Business Review"), "{outline}");
}

#[test]
fn the_caret_follows_a_session_batch() {
    let mut doc = note();
    // The caret after "north".
    let end = NOTE.chars().count() - 1;
    doc.execute("select.set", json!({ "anchor": end, "focus": end })).unwrap();
    session_batch(&mut doc, &json!({ "ops": [{ "op": "text.insert", "target": "root", "field": "body", "index": 0, "text": "Draft\n\n" }] }), false)
        .unwrap();
    doc.execute("select.replace", json!({ "text": "!" })).unwrap();
    assert_eq!(doc.text(), "Draft\n\n# quarterly business review\n\nrevenue grew in the north!\n");
}

#[test]
fn a_batch_that_changes_nothing_leaves_the_revision() {
    let mut doc = note();
    let before = doc.revision();
    session_batch(&mut doc, &json!({ "load": [{ "target": "root" }] }), false).unwrap();
    assert_eq!(doc.revision(), before);
    assert!(!doc.dirty());
}

#[test]
fn a_command_inside_a_batch_counts_once() {
    let mut by_command = note();
    let mut by_batch = note();
    by_command.execute("text.insert", json!({ "at": 0, "text": "x" })).unwrap();
    session_batch(&mut by_batch, &json!({ "ops": [{ "op": "command", "id": "text.insert", "params": { "at": 0, "text": "x" } }] }), false).unwrap();
    assert_eq!(by_batch.text(), by_command.text());
    assert_eq!(by_batch.revision(), by_command.revision());
    assert!(by_batch.dirty());
}

#[test]
fn a_batch_over_the_change_exchange_moves_the_revision_once() {
    let mut doc = note();
    let before = doc.revision();
    Exchange::new().batch(&mut doc, &retitle(), false).unwrap();
    assert_eq!(doc.text(), RETITLED);
    assert_eq!(doc.revision(), before + 1);
    assert!(doc.dirty());
}
