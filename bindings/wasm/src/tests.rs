use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use kition_kit::session::KitSessionDocument;
use kition_kit::session::model::{Exchange, ReplicaPort};
use serde_json::json;

use crate::NoteDocument;

const PLAN: &str = "# Plan\n\nGoals \u{1F600} here\n";

fn plan() -> NoteDocument {
    NoteDocument::open("plan.md", PLAN.as_bytes()).unwrap()
}

#[test]
fn opens_edits_and_saves_only_the_edit() {
    let mut doc = plan();
    assert!(!doc.dirty());
    assert_eq!(doc.save(None).unwrap(), PLAN.as_bytes());
    doc.execute("text.insert", json!({ "at": 6, "text": " ahead" })).unwrap();
    assert!(doc.dirty());
    assert_eq!(doc.save(Some("plan.md")).unwrap(), "# Plan ahead\n\nGoals \u{1F600} here\n".as_bytes());
    doc.mark_saved();
    assert!(!doc.dirty());
}

#[test]
fn positions_count_in_the_requested_unit() {
    let mut doc = plan();
    // "here" starts after the emoji: scalar 16, UTF-16 17.
    let read = doc.execute("text.read", json!({ "from": 17, "to": 21, "unit": "utf16" })).unwrap();
    assert_eq!(read["text"], "here");
    assert_eq!(read["length"], 22);
    let scalar = doc.execute("text.read", json!({ "from": 16, "to": 20 })).unwrap();
    assert_eq!(scalar["text"], "here");
    let error = doc.execute("text.insert", json!({ "at": 15, "text": "x", "unit": "utf16" })).unwrap_err();
    assert!(error.starts_with("out-of-range"), "{error}");
    doc.execute("text.replace", json!({ "from": 17, "to": 21, "text": "there", "unit": "utf16", "expected": "here" })).unwrap();
    assert!(doc.text().ends_with("there\n"));
}

#[test]
fn a_stale_range_is_refused_and_changes_nothing() {
    let mut doc = plan();
    let error = doc.execute("text.delete", json!({ "from": 0, "to": 6, "expected": "# Goal" })).unwrap_err();
    assert!(error.starts_with("stale"), "{error}");
    assert_eq!(doc.text(), PLAN);
    assert_eq!(doc.revision(), 0);
}

#[test]
fn empty_and_bad_params_are_typed_errors() {
    let mut doc = plan();
    for command in ["text.insert", "text.delete", "text.replace"] {
        let error = doc.execute(command, json!({})).unwrap_err();
        assert!(error.starts_with("invalid-params"), "{command}: {error}");
    }
    assert!(doc.execute("text.read", json!([1])).unwrap_err().starts_with("invalid-params"));
    assert!(doc.execute("text.read", json!({ "unit": "bytes" })).unwrap_err().starts_with("invalid-params"));
    assert!(doc.execute("text.delete", json!({ "from": 5, "to": 2 })).is_err());
    assert_eq!(doc.text(), PLAN);
}

#[test]
fn undo_and_redo_step_the_replica() {
    let mut doc = plan();
    // Undo stays enabled (the editor also undoes merged drafts); with nothing to undo it does nothing.
    assert!(!doc.disabled_commands().contains(&"edit.undo".to_owned()));
    assert_eq!(doc.inspect().unwrap()["canUndo"], false);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], false);
    doc.execute("text.insert", json!({ "at": 0, "text": "x" })).unwrap();
    assert_eq!(doc.inspect().unwrap()["canUndo"], true);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), PLAN);
    doc.execute("edit.redo", json!({})).unwrap();
    assert!(doc.text().starts_with("x# Plan"));
}

#[test]
fn undo_takes_back_only_its_own_scope() {
    // KitModel 1.1 M12a, E14a: the person's Undo keeps the agent's step, the agent's takes back only its own.
    let mut doc = plan();
    let agent = Origin::agent("agent:test");
    doc.execute_as("text.insert", json!({ "at": 0, "text": "A" }), &agent).unwrap();
    doc.execute("text.insert", json!({ "at": PLAN.chars().count() + 1, "text": "P" })).unwrap();
    assert_eq!(doc.text(), format!("A{PLAN}P"));

    let revision = doc.revision();
    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], true);
    assert_eq!(doc.text(), format!("{PLAN}P"), "the agent's undo keeps the person's later typing");
    assert!(doc.revision() > revision, "an undo in the agent's scope is a new revision");
    assert_eq!(doc.execute("edit.redo", json!({ "scope": "agent" })).unwrap()["done"], true);
    assert_eq!(doc.text(), format!("A{PLAN}P"));

    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), format!("A{PLAN}"), "the person's Undo keeps the agent's step");
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], false, "the person has no other step");
    assert_eq!(doc.text(), format!("A{PLAN}"));

    // Run as the agent, with no scope: the agent's own.
    assert_eq!(doc.execute_as("edit.undo", json!({}), &agent).unwrap()["done"], true);
    assert_eq!(doc.text(), PLAN);
    assert!(doc.execute("edit.undo", json!({ "scope": "robot" })).unwrap_err().starts_with("invalid-params"));
}

#[test]
fn a_stored_replica_takes_the_file_as_one_import() {
    let a = plan();
    let snapshot = kition_kit::session::__snapshot(&a).unwrap();
    let changed = "# Plan\n\nGoals \u{1F600} here\nAnd more\n";
    let b = NoteDocument::open_with_replica("plan.md", changed.as_bytes(), &snapshot, &[]).unwrap();
    assert_eq!(b.text(), changed);
    assert_eq!(b.doc_id(), a.doc_id());
    assert!(!b.session.can_undo(), "the import is not the person's undo step");
    // A damaged replica falls back to the file; without a file it is an error.
    assert_eq!(NoteDocument::open_with_replica("plan.md", changed.as_bytes(), b"junk", &[]).unwrap().text(), changed);
    assert!(NoteDocument::open_with_replica("plan.md", &[], b"junk", &[]).is_err());
    assert!(NoteDocument::open_with_replica("plan.md", &[], &snapshot, b"x").is_err());
}

#[test]
fn an_unchanged_note_saves_its_bytes_and_reopens_with_the_same_ids() {
    let mut doc = plan();
    let added = doc.execute("comment.add", json!({ "quote": "Goals", "body": "Which ones?", "created": "2026-11-24T10:00:00Z" })).unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    let saved = doc.save(None).unwrap();
    assert_eq!(saved, PLAN.as_bytes(), "the comment stays out of the file");
    assert_eq!(doc.save(None).unwrap(), saved, "saving again writes the same bytes");
    let snapshot = kition_kit::session::__snapshot(&doc).unwrap();
    let mut reopened = NoteDocument::open_with_replica("plan.md", &saved, &snapshot, &[]).unwrap();
    assert_eq!(reopened.save(None).unwrap(), saved);
    assert_eq!(reopened.doc_id(), doc.doc_id());
    assert_eq!(kition_kit::session::__version(&reopened), kition_kit::session::__version(&doc), "the same file is no import change");
    assert_eq!(reopened.session.state_hash().unwrap(), doc.session.state_hash().unwrap());
    let threads = reopened.execute("comment.threads", json!({})).unwrap();
    assert_eq!(threads["threads"][0]["id"], json!(thread));
    assert_eq!(threads["threads"][0]["comments"][0]["id"], doc.execute("comment.threads", json!({})).unwrap()["threads"][0]["comments"][0]["id"]);
}

#[test]
fn a_merged_draft_is_taken_in_and_one_undo_of_the_person_takes_it_back() {
    let mut doc = plan();
    // `doc.editor_text()` is read first so a stale editor would show the old text.
    assert_eq!(doc.editor_text().as_ref(), PLAN);
    let snapshot = kition_kit::session::__snapshot(&doc).unwrap();
    let since = kition_kit::session::__version(&doc);
    let mut draft = NoteDocument::open_with_replica("plan.md", &[], &snapshot, &[]).unwrap();
    draft.execute("text.insert", json!({ "at": 6, "text": " ahead" })).unwrap();
    let changes = kition_kit::session::__export_changes(&draft, &since).unwrap();
    let revision = doc.revision();
    let agent = Origin::agent("agent:test");
    let (scope, damage) = KitSessionDocument::merge_changes(&mut doc, &changes, &agent).unwrap();
    assert_eq!(scope, Some(kition_kit::kdm::UndoScope::Person));
    assert_ne!(damage, kition_kit::session::Damage::None);
    let merged = "# Plan ahead\n\nGoals \u{1F600} here\n";
    assert_eq!(doc.text(), merged);
    assert_eq!(doc.editor_text().as_ref(), merged, "the editor shows the merged draft");
    assert!(doc.revision() > revision);
    assert!(doc.dirty());
    // The person approved the draft: the merge is one step of the person's Undo, never the
    // agent's (kition-sdk `Replica::merge_changes`, M12a).
    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(doc.text(), merged);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), PLAN, "one Undo takes the merged draft back");
    assert_eq!(doc.editor_text().as_ref(), PLAN);
}

const GOALS: &str = "# Plan\n\nGoals here\n";

/// A note with the caret after "Goals" and " x" typed there, the typing run still open.
fn typed_note() -> NoteDocument {
    let mut doc = NoteDocument::open("plan.md", GOALS.as_bytes()).unwrap();
    doc.execute("select.set", json!({ "anchor": 13, "focus": 13 })).unwrap();
    for typed in [" ", "x"] {
        kition_kit::tools::KitTools::text(&mut doc, typed, 1.0).unwrap();
    }
    assert_eq!(doc.text(), "# Plan\n\nGoals x here\n");
    assert_eq!((doc.editor.anchor, doc.editor.focus), (15, 15));
    assert!(doc.editor.typing(), "the typing run is open");
    doc
}

#[test]
fn a_draft_merged_while_the_person_types_is_one_undo_step_and_the_caret_follows_it() {
    let mut doc = typed_note();
    let typed = doc.text().to_owned();
    let before = doc.session.state_hash().unwrap();
    // The agent's draft, made on the note as it is now, writes before the caret and after it.
    let since = kition_kit::session::__version(&doc);
    let mut draft = NoteDocument::open_with_replica("plan.md", &[], &kition_kit::session::__snapshot(&doc).unwrap(), &[]).unwrap();
    draft.execute("text.insert", json!({ "at": 6, "text": " ahead" })).unwrap();
    draft.execute("text.insert", json!({ "at": 27, "text": "Done.\n" })).unwrap();
    let changes = kition_kit::session::__export_changes(&draft, &since).unwrap();
    let agent = Origin::agent("agent:test");
    let revision = doc.revision();
    KitSessionDocument::merge_changes(&mut doc, &changes, &agent).unwrap();
    let merged = "# Plan ahead\n\nGoals x here\nDone.\n";
    assert_eq!(doc.text(), merged, "the typed text is kept");
    assert_eq!(doc.editor_text().as_ref(), merged);
    assert!(doc.revision() > revision);
    // The caret stays after the typed "x": moved by the text the draft put before it, not by
    // what it put after it (the draft's change is two places, not one span).
    assert_eq!((doc.editor.anchor, doc.editor.focus), (21, 21));
    assert!(!doc.editor.typing(), "the merge ended the typing run");
    let after = doc.session.state_hash().unwrap();

    // The person's one Undo takes back the merge only, exactly; the agent's takes back nothing.
    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(doc.text(), merged);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), typed);
    assert_eq!(doc.editor_text().as_ref(), typed);
    assert_eq!(doc.session.state_hash().unwrap(), before, "the state before the merge, exactly");
    assert_eq!((doc.editor.anchor, doc.editor.focus), (15, 15), "the selection from before the merge");
    // Redo brings the merge back exactly, with the selection after it.
    assert_eq!(doc.execute("edit.redo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), merged);
    assert_eq!(doc.session.state_hash().unwrap(), after);
    assert_eq!((doc.editor.anchor, doc.editor.focus), (21, 21));
    // Then Undo takes back the merge, and the next one the typing run, with its selection.
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), typed);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), GOALS);
    assert_eq!((doc.editor.anchor, doc.editor.focus), (13, 13));
}

#[test]
fn a_draft_made_before_the_person_typed_merges_with_the_typing_and_undoes_alone() {
    // The agent's draft opens a comment thread on the note as it was; meanwhile the person types,
    // the run still open. Both are there after the merge, and one Undo takes back the thread only.
    let mut doc = NoteDocument::open("plan.md", GOALS.as_bytes()).unwrap();
    let since = kition_kit::session::__version(&doc);
    let mut draft = NoteDocument::open_with_replica("plan.md", &[], &kition_kit::session::__snapshot(&doc).unwrap(), &[]).unwrap();
    let agent = Origin::agent("agent:test");
    draft.execute_as("comment.add", json!({ "quote": "Goals", "body": "Which ones?", "created": "2026-11-25T10:00:00Z" }), &agent).unwrap();
    let changes = kition_kit::session::__export_changes(&draft, &since).unwrap();
    doc.execute("select.set", json!({ "anchor": 13, "focus": 13 })).unwrap();
    for typed in [" ", "x"] {
        kition_kit::tools::KitTools::text(&mut doc, typed, 1.0).unwrap();
    }
    let typed = "# Plan\n\nGoals x here\n";
    assert_eq!(doc.text(), typed);
    assert!(doc.editor.typing(), "the typing run is open");
    let before = doc.session.state_hash().unwrap();
    KitSessionDocument::merge_changes(&mut doc, &changes, &agent).unwrap();
    assert_eq!(doc.text(), typed, "the typed text is kept");
    assert_eq!((doc.editor.anchor, doc.editor.focus), (15, 15), "the merge leaves the text and the caret");
    let threads = doc.execute("comment.threads", json!({})).unwrap();
    assert_eq!(threads["threads"].as_array().map(Vec::len), Some(1));
    assert_eq!(threads["threads"][0]["comments"][0]["body"], "Which ones?");

    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.execute("comment.threads", json!({})).unwrap()["threads"], json!([]));
    assert_eq!(doc.text(), typed, "Undo takes back the merge, not the typing");
    assert_eq!(doc.session.state_hash().unwrap(), before);
    assert_eq!((doc.editor.anchor, doc.editor.focus), (15, 15));
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), GOALS);
    assert_eq!((doc.editor.anchor, doc.editor.focus), (13, 13));
}

#[test]
fn a_thread_the_merge_empties_is_deleted_in_the_merge_step_and_one_undo_brings_it_back() {
    // An agent's draft deletes one comment of a two-comment thread while the person deletes the
    // other: the merge takes both removals in and the repair deletes the thread left with none,
    // inside the merge's one step (kition-sdk `KitSession::merge_changes`, data model 6.6 D9).
    // The person's one Undo gives back the exact state before the merge (the thread with the
    // comment the person kept), the next one the person's deletion; Redo the repaired state.
    let mut doc = plan();
    let added = doc.execute("comment.add", json!({ "quote": "Goals", "body": "Which ones?", "created": "2026-11-25T10:00:00Z" })).unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    let first = added["comment"].as_str().unwrap().to_owned();
    let replied = doc.execute("comment.reply", json!({ "thread": thread, "body": "These.", "created": "2026-11-25T10:01:00Z" })).unwrap();
    let second = replied["comment"].as_str().unwrap().to_owned();
    let both = doc.execute("comment.threads", json!({})).unwrap();
    let both_state = doc.session.state_hash().unwrap();
    let since = kition_kit::session::__version(&doc);
    let mut draft = NoteDocument::open_with_replica("plan.md", &[], &kition_kit::session::__snapshot(&doc).unwrap(), &[]).unwrap();
    let agent = Origin::agent("agent:test");
    draft.execute_as("comment.delete", json!({ "thread": thread, "comment": first }), &agent).unwrap();
    let changes = kition_kit::session::__export_changes(&draft, &since).unwrap();
    doc.execute("comment.delete", json!({ "thread": thread, "comment": second })).unwrap();
    let one = doc.execute("comment.threads", json!({})).unwrap();
    assert_eq!(one["threads"][0]["id"], json!(thread), "the person's delete leaves the thread");
    assert_eq!(one["threads"][0]["comments"].as_array().map(Vec::len), Some(1));
    let before = doc.session.state_hash().unwrap();

    let (scope, damage) = KitSessionDocument::merge_changes(&mut doc, &changes, &agent).unwrap();
    assert_eq!(scope, Some(kition_kit::kdm::UndoScope::Person));
    assert_ne!(damage, kition_kit::session::Damage::None);
    assert_eq!(doc.execute("comment.threads", json!({})).unwrap()["threads"], json!([]), "no empty thread");
    assert_eq!(doc.text(), PLAN);
    let merged = doc.session.state_hash().unwrap();

    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.session.state_hash().unwrap(), before, "one Undo: exactly the state before the merge");
    assert_eq!(doc.execute("comment.threads", json!({})).unwrap(), one, "the thread is back with the comment the person kept");
    assert_eq!(doc.execute("edit.redo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.session.state_hash().unwrap(), merged, "Redo: exactly the repaired merge");
    assert_eq!(doc.execute("comment.threads", json!({})).unwrap()["threads"], json!([]));
    // Undo again, then the person's own deletion is the next step.
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.session.state_hash().unwrap(), before);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.session.state_hash().unwrap(), both_state);
    assert_eq!(doc.execute("comment.threads", json!({})).unwrap(), both);
}

#[test]
fn a_restored_state_is_taken_in_and_one_undo_takes_it_back() {
    let mut doc = plan();
    let earlier = kition_kit::session::__snapshot(&doc).unwrap();
    doc.execute("text.insert", json!({ "at": 0, "text": "Draft. " })).unwrap();
    let edited = doc.text().to_owned();
    assert_eq!(doc.editor_text().as_ref(), edited);
    let revision = doc.revision();
    KitSessionDocument::restore_state(&mut doc, &earlier, &Origin::person("user:test")).unwrap();
    assert_eq!(doc.text(), PLAN);
    assert_eq!(doc.editor_text().as_ref(), PLAN, "the editor shows the restored state");
    assert!(doc.revision() > revision);
    assert_eq!(doc.execute("edit.undo", json!({})).unwrap()["done"], true);
    assert_eq!(doc.text(), edited, "one Undo takes the restore back");
    // Another document's state is refused, changing nothing.
    let other = kition_kit::session::__snapshot(&plan()).unwrap();
    assert!(KitSessionDocument::restore_state(&mut doc, &other, &Origin::person("user:test")).is_err());
    assert_eq!(doc.text(), edited);
}

#[test]
fn the_object_model_is_served_over_the_change_exchange() {
    let mut doc = plan();
    let snapshot = kition_kit::session::__snapshot(&doc).unwrap();
    let mut exchange = Exchange::new();
    let described = exchange.describe(&mut doc).unwrap();
    assert!(described.to_string().contains("\"Note\""));
    let reopened = <NoteDocument as ReplicaPort>::model_open_replica("plan.md", &snapshot, &[]).unwrap();
    assert_eq!(reopened.text(), PLAN);
    assert!(<NoteDocument as ReplicaPort>::model_open_replica("plan.md", b"junk", &[]).is_err());
}

#[test]
fn only_markdown_names_open() {
    assert!(NoteDocument::open("plan.txt", PLAN.as_bytes()).is_err());
    assert!(NoteDocument::open("PLAN.MARKDOWN", PLAN.as_bytes()).is_ok());
    assert!(NoteDocument::create("new.md", 0, 0).unwrap().text().is_empty());
    // A host creates a document named by a title only.
    let untitled = NoteDocument::create("untitled", 1920, 1080).unwrap();
    assert!(untitled.text().is_empty());
    assert!(untitled.commands().iter().any(|command| command.id == "text.insert"));
}

// ---- The file: synthetic edges, read-only files, external changes ------------------------------

#[test]
fn every_edge_saves_back_byte_for_byte() {
    for (label, bytes) in kition_note_kdm::file::bench::EDGE_FILES {
        let doc = NoteDocument::open("edge.md", bytes).unwrap();
        assert_eq!(doc.save(None).unwrap(), *bytes, "{label}");
        let snapshot = kition_kit::session::__snapshot(&doc).unwrap();
        let shared = NoteDocument::open_with_replica("edge.md", &[], &snapshot, &[]).unwrap();
        assert_eq!(shared.save(None).unwrap(), *bytes, "{label}: a co-editor without the file");
    }
}

#[test]
fn an_edit_in_a_crlf_note_changes_only_its_bytes() {
    let mut doc = NoteDocument::open("plan.md", b"\xEF\xBB\xBFone\r\ntwo\r\nthree\r\n").unwrap();
    // "two" starts at 4; text pasted with CRLF endings takes the note's.
    doc.execute("text.insert", json!({ "at": 4, "text": "new\r\nline\r\n" })).unwrap();
    assert_eq!(doc.save(None).unwrap(), b"\xEF\xBB\xBFone\r\nnew\r\nline\r\ntwo\r\nthree\r\n");
    assert!(!doc.text().contains('\r'));
}

#[test]
fn a_mixed_note_keeps_every_cr_through_an_edit() {
    let mut doc = NoteDocument::open("mixed.md", b"one\r\ntwo\nthree\rfour\n").unwrap();
    assert_eq!(doc.inspect().unwrap()["registers"]["eol"], "mixed");
    doc.execute("text.insert", json!({ "at": 0, "text": "zero\n" })).unwrap();
    assert_eq!(doc.save(None).unwrap(), b"zero\none\r\ntwo\nthree\rfour\n");
}

#[test]
fn invalid_utf8_opens_read_only_and_saves_its_bytes() {
    let bytes = kition_note_kdm::file::bench::INVALID_UTF8_FILE;
    let mut doc = NoteDocument::open("broken.md", bytes).unwrap();
    assert!(doc.is_read_only());
    assert_eq!(doc.text(), "# Note\n\u{FFFD}\u{FFFD} broken\n");
    let error = doc.execute("text.insert", json!({ "at": 0, "text": "x" })).unwrap_err();
    assert!(error.starts_with("read-only"), "{error}");
    assert_eq!(doc.execute("text.read", json!({ "from": 0, "to": 6 })).unwrap()["text"], "# Note");
    assert!(doc.disabled_commands().contains(&"text.insert".to_owned()));
    assert_eq!(doc.save(None).unwrap(), bytes);
    let found = doc.check();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, "note.invalid-utf8");
    assert!(doc.merge_file(b"# Note\n").unwrap_err().starts_with("read-only"));
}

#[test]
fn an_external_change_merges_with_unsaved_edits() {
    let mut doc = NoteDocument::open("plan.md", b"# Plan\n\nFirst point.\nSecond point.\n").unwrap();
    doc.execute("text.insert", json!({ "at": 19, "text": ", edited here" })).unwrap();
    let answer = doc.merge_file(b"# Plan\n\nFirst point.\nSecond point, edited outside.\n").unwrap();
    assert_eq!(answer["changed"], true);
    assert_eq!(doc.text(), "# Plan\n\nFirst point, edited here.\nSecond point, edited outside.\n");
    // The person's edit is still theirs to undo; the merge is not an undo step.
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(doc.text(), "# Plan\n\nFirst point.\nSecond point, edited outside.\n");
    // The same file again changes nothing; a later change merges from it.
    assert_eq!(doc.merge_file(b"# Plan\n\nFirst point.\nSecond point, edited outside.\n").unwrap()["changed"], false);
    assert!(doc.merge_file(b"\xFF").unwrap_err().starts_with("note.invalid-utf8"));
}

#[test]
fn an_external_change_of_a_clean_note_leaves_it_saved() {
    let mut doc = NoteDocument::open("plan.md", b"# Plan\n").unwrap();
    doc.mark_saved();
    assert_eq!(doc.merge_file(b"# Plan\n\nAgreed in a draft.\n").unwrap()["changed"], true);
    assert_eq!(doc.text(), "# Plan\n\nAgreed in a draft.\n");
    assert!(!doc.dirty(), "a clean note that took the file has nothing to save");
    // With unsaved edits the merge keeps them, and they are still unsaved.
    doc.execute("text.insert", json!({ "at": 6, "text": " now" })).unwrap();
    doc.merge_file(b"# Plan\n\nAgreed in a draft.\nMore.\n").unwrap();
    assert_eq!(doc.text(), "# Plan now\n\nAgreed in a draft.\nMore.\n");
    assert!(doc.dirty());
}

#[test]
fn an_external_change_of_line_endings_is_taken() {
    let mut doc = NoteDocument::open("plan.md", b"a\nb\n").unwrap();
    assert_eq!(doc.merge_file(b"a\r\nb\r\n").unwrap()["changed"], true);
    assert_eq!(doc.save(None).unwrap(), b"a\r\nb\r\n");
}

#[test]
fn a_stored_replica_takes_the_files_line_style() {
    let a = NoteDocument::open("plan.md", b"a\nb\n").unwrap();
    let snapshot = kition_kit::session::__snapshot(&a).unwrap();
    let b = NoteDocument::open_with_replica("plan.md", b"a\r\nb\r\nc\r\n", &snapshot, &[]).unwrap();
    assert_eq!(b.text(), "a\nb\nc\n");
    assert_eq!(b.save(None).unwrap(), b"a\r\nb\r\nc\r\n");
    assert_eq!(b.doc_id(), a.doc_id());
    // A file that is not UTF-8 opens read-only, without the replica.
    assert!(NoteDocument::open_with_replica("plan.md", b"\xFF", &snapshot, &[]).unwrap().is_read_only());
}

#[test]
fn inspect_from_the_view_s_chunks_matches_the_whole_note_indexed() {
    let text = "---\ntitle: A\ntags: [x]\n---\n# One\n\nbody #tag\n\nSetext\n------\n\n- [ ] open\n- [x] done\n\n### Deep\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> [!tip] Hi\n> there\n\n# Two\n\nwords here ^block\n";
    let doc = NoteDocument::open("a.md", text.as_bytes()).unwrap();
    let inspected = doc.inspect().unwrap();
    let ix = kition_note_markdown::index(text);
    let outline: Vec<serde_json::Value> = ix
        .headings
        .iter()
        .map(|heading| json!({ "path": heading.path, "level": heading.level, "line": ix.lines.line_of(heading.range.start) + 1 }))
        .collect();
    assert_eq!(inspected["outline"], json!(outline));
    assert_eq!(inspected["words"], json!(ix.words));
    assert_eq!(inspected["tasks"], json!({ "open": 1, "done": 1 }));
    assert_eq!(inspected["links"]["tags"], json!(ix.tags.len()));
    assert_eq!(inspected["links"]["blockIds"], json!(ix.block_ids.len()));
    assert_eq!(inspected["callouts"], json!(ix.callouts.len()));
    assert_eq!(inspected["tables"][0]["line"], json!(ix.lines.line_of(ix.tables[0].range.start) + 1));
    assert_eq!(inspected["properties"].as_array().map(Vec::len), Some(2));
    assert_eq!(inspected["heading"], json!("One"));
}

#[test]
fn an_agents_reply_is_shown_from_the_objects_it_names_only() {
    // `modelReply` (kition-sdk `__model_reply`) reads the tool ids of the objects a reply names,
    // never the whole note's: a reply naming one of many threads comes back as the full read shows
    // it, and one naming none comes back unchanged.
    let mut doc = plan();
    let added: Vec<String> = (0..200)
        .map(|index| {
            let answer = doc
                .execute("comment.add", json!({ "quote": "Goals", "body": format!("Comment {index}"), "created": "2026-11-25T10:00:00Z" }))
                .unwrap();
            answer["thread"].as_str().unwrap().to_owned()
        })
        .collect();
    let thread = added[100].clone();
    let full = kition_kit::kdm::tool_ids::ToolIds::of_replica(doc.session.replica());
    let mut threads = serde_json::Map::new();
    threads.insert(thread.clone(), json!({ "open": true }));
    let naming = json!({ "thread": thread, "threads": threads, "note": "Goals" });
    let shown: serde_json::Value = serde_json::from_str(&kition_kit::session::__model_reply(&doc, &naming.to_string()).unwrap()).unwrap();
    assert_eq!(shown, full.show_json(&naming));
    let none = json!({ "words": 3, "outline": [{ "path": ["Plan"], "line": 1 }], "text": "Goals" });
    assert_eq!(kition_kit::session::__model_reply(&doc, &none.to_string()).unwrap(), none.to_string());
}
