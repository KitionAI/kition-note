//! Regression: a row inserted into a table that has only its header
//! (`table.add {rows: 0}`) goes after the delimiter row, so the table stays a table.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;
use serde_json::json;

fn text(doc: &NoteDocument) -> String {
    String::from_utf8(doc.save(None).unwrap()).unwrap()
}

#[test]
fn a_row_goes_after_the_delimiter_of_a_header_only_table() {
    let mut doc = NoteDocument::open("plan.md", b"# Plan\n").unwrap();
    doc.execute("table.add", json!({ "columns": 2, "rows": 0, "header": ["Task", "Owner"] })).unwrap();
    doc.execute("table.insertRow", json!({ "table": 0, "at": 1, "cells": ["Book venue", "Ana"] })).unwrap();
    doc.execute("table.insertRow", json!({ "table": 0, "at": 2, "cells": ["Order food", "Raj"] })).unwrap();
    let text = text(&doc);
    let lines: Vec<&str> = text.lines().filter(|line| line.starts_with('|')).collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert!(lines[0].contains("Task") && lines[1].contains("---"), "the header, then the delimiter: {text}");
    assert!(lines[2].contains("Book venue") && lines[3].contains("Order food"), "{text}");
    // The note reopens with one well-formed table of two body rows.
    let again = NoteDocument::open("plan.md", text.as_bytes()).unwrap();
    assert!(again.check().is_empty(), "{:?}", again.check().iter().map(|found| &found.code).collect::<Vec<_>>());
    let cell = doc.execute("table.setCell", json!({ "table": 0, "row": 2, "column": 1, "text": "Raj K." }));
    assert!(cell.is_ok(), "{cell:?}");
}
