//! A command with a parameter schema refuses a key the schema does not name, and says which
//! (platform rule E7a, kit requirement AX15): an agent that misspells a parameter is told instead
//! of the command running without it.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;
use serde_json::json;

fn doc() -> NoteDocument {
    NoteDocument::create("Note.md", 0, 0).unwrap()
}

#[test]
fn every_command_refuses_an_unknown_parameter() {
    if let Err(problems) = kition_kit::testing::check_unknown_params(doc, &[]) {
        panic!("{} problems:\n{}", problems.len(), problems.join("\n"));
    }
}

#[test]
fn a_misspelled_parameter_names_the_one_it_resembles() {
    let error = doc().execute("text.read", json!({ "form": 0 })).unwrap_err();
    assert!(error.contains("`form`") && error.contains("did you mean `from`"), "{error}");
}

#[test]
fn every_key_a_command_reads_is_in_its_schema() {
    let mut note = doc();
    note.execute("note.write", json!({ "markdown": "# Plan\n\nWrite it.\n" })).unwrap();
    // text.insert checks `expected` against the empty range it inserts at.
    note.execute("text.insert", json!({ "at": 18, "text": "x", "expected": "" })).unwrap();
    // select.set: a line and column, or a document point, instead of anchor and focus.
    note.execute("select.set", json!({ "line": 3, "column": 2 })).unwrap();
    // comment.move anchors a thread to a heading, as comment.add does.
    let added = note.execute("comment.add", json!({ "body": "Check", "quote": "Write" })).unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    note.execute("comment.move", json!({ "thread": thread, "heading": ["Plan"] })).unwrap();
}
