//! A clean note whose file another program changed is read again by its host (kition-headless):
//! the host carries what only the session held onto the note read from the file
//! (`carry_replica_objects`), checks that the editor state still resolves on it
//! (`restore_editor_state`), and asks the open note to take the file's difference into its own
//! session (`reconcile_reread`), so its undo histories, its selection and its threads stay. These
//! tests make the host's calls in its order.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_note_kit::{KitDocument, KitSessionDocument, NoteDocument, Origin};
use serde_json::{Value, json};

const NOTE: &str = "# Plan\n\nFirst point.\nSecond point.\n\n## Tasks\n\n- [ ] Draft the post\n- [ ] Book the room\n";

fn selection(doc: &mut NoteDocument) -> (Value, Value) {
    let got = doc.execute("select.get", json!({})).unwrap();
    (got["anchor"].clone(), got["focus"].clone())
}

/// What kition-headless does with a clean note whose file now holds `bytes`: the fresh note, or
/// the open one with the file's difference taken in, and whether it was reconciled.
fn reread(open: &mut NoteDocument, bytes: &[u8]) -> Result<(NoteDocument, bool), String> {
    let mut fresh = NoteDocument::open("plan.md", bytes)?;
    fresh.carry_replica_objects(open)?;
    fresh.restore_editor_state(&open.editor_state())?;
    if open.reconcile_reread(&fresh)? {
        open.mark_saved();
        return Ok((std::mem::replace(open, fresh), true));
    }
    Ok((fresh, false))
}

#[test]
fn an_outside_change_keeps_both_undo_histories_and_the_selection() {
    let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
    doc.execute_as("text.findReplace", json!({ "find": "Draft the post", "replace": "Draft the announcement" }), &Origin::agent("agent:a")).unwrap();
    doc.execute("text.findReplace", json!({ "find": "Book the room", "replace": "Book the hall" })).unwrap();
    let saved = String::from_utf8(doc.save(None).unwrap()).unwrap();
    doc.mark_saved();
    let at = saved.find("Second").unwrap();
    doc.execute("select.set", json!({ "anchor": at, "focus": at + 6 })).unwrap();
    let (anchor, focus) = selection(&mut doc);
    // Another program adds a line above the selection.
    let outside = saved.replace("# Plan\n", "# Plan\n\nAdded outside.\n");
    let (mut doc, reconciled) = reread(&mut doc, outside.as_bytes()).unwrap();
    assert!(reconciled, "a valid note reconciles");
    assert_eq!(doc.text(), outside);
    assert!(!doc.dirty(), "the reconciled note is the file");
    let shift = "\nAdded outside.\n".len() as u64;
    assert_eq!(
        selection(&mut doc),
        (json!(anchor.as_u64().unwrap() + shift), json!(focus.as_u64().unwrap() + shift)),
        "the selection follows the change"
    );
    // Each scope takes back only its own step; the outside line is in neither history.
    let (done, _) = doc.undo_in(kition_kit::kdm::UndoScope::Person).unwrap();
    assert!(done);
    assert_eq!(doc.text(), outside.replace("Book the hall", "Book the room"));
    let (done, _) = doc.undo_in(kition_kit::kdm::UndoScope::Agent).unwrap();
    assert!(done);
    assert_eq!(doc.text(), NOTE.replace("# Plan\n", "# Plan\n\nAdded outside.\n"));
    assert!(!doc.undo_in(kition_kit::kdm::UndoScope::Person).unwrap().0, "the outside change is no undo step");
    assert!(doc.text().contains("Added outside."));
}

#[test]
fn a_thread_and_its_id_stay_through_a_reread_with_or_without_history() {
    for history in [false, true] {
        let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
        let added = doc.execute("comment.add", json!({ "quote": "Book the room", "body": "Which room?" })).unwrap();
        let thread = added["thread"].as_str().unwrap().to_owned();
        if history {
            doc.execute("text.findReplace", json!({ "find": "First point.", "replace": "First point, edited." })).unwrap();
        }
        let saved = String::from_utf8(doc.save(None).unwrap()).unwrap();
        doc.mark_saved();
        let (mut doc, reconciled) = reread(&mut doc, saved.replace("## Tasks\n", "## Tasks\n\nA line from outside.\n").as_bytes()).unwrap();
        assert!(reconciled, "history {history}");
        let threads = doc.execute("comment.threads", json!({})).unwrap();
        assert_eq!(threads["threads"][0]["id"], json!(thread), "history {history}: {threads}");
        let text = doc.text().to_owned();
        let at = text[..text.find("Book the room").unwrap()].chars().count();
        assert_eq!(threads["threads"][0]["from"], json!(at), "history {history}: the thread is found where its text went: {threads}");
    }
}

#[test]
fn a_note_without_history_read_again_keeps_its_selection() {
    let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
    let at = NOTE.find("Book").unwrap();
    doc.execute("select.set", json!({ "anchor": at })).unwrap();
    let mut fresh = NoteDocument::open("plan.md", NOTE.replace("# Plan\n", "# Plan, renamed\n").as_bytes()).unwrap();
    fresh.carry_replica_objects(&doc).unwrap();
    fresh.restore_editor_state(&doc.editor_state()).unwrap();
    let shift = ", renamed".len() as u64;
    assert_eq!(selection(&mut fresh).0, json!(at as u64 + shift));
}

#[test]
fn an_editor_state_that_does_not_resolve_is_refused() {
    let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
    let mut state = doc.editor_state();
    state["anchor"] = json!(NOTE.len() + 1);
    let refused = doc.restore_editor_state(&state).unwrap_err();
    assert!(refused.contains("out of range"), "{refused}");
    let mut state = doc.editor_state();
    state["folded"] = json!([NOTE.len() + 1]);
    let refused = doc.restore_editor_state(&state).unwrap_err();
    assert!(refused.contains("out of range"), "{refused}");
}

#[test]
fn restoring_after_a_write_keeps_the_selection_the_write_moved() {
    let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
    let at = NOTE.find("Book").unwrap();
    doc.execute("select.set", json!({ "anchor": at })).unwrap();
    let state = doc.editor_state();
    doc.execute_as("text.insert", json!({ "at": 0, "text": "Top.\n" }), &Origin::agent("agent:a")).unwrap();
    let moved = selection(&mut doc);
    let revision = doc.revision();
    doc.restore_editor_state(&state).unwrap();
    assert_eq!(selection(&mut doc), moved);
    assert_eq!(doc.revision(), revision);
}
