//! In memory == saved: every write family an agent uses, on every file of `conformance/` and
//! `corpus/`, leaves a document whose saved file reads back to the same model: the same body (the
//! text a receipt tells as the root's `body`), the same line style registers (`eol`, `bom`), the
//! same inspect. A write a file cannot take (no table, no task) is skipped and counted.
//!
//! Comment threads are the one thing a saved file does not hold: they live in the object model the
//! host stores, never in the `.md` (AGENTS.md). Their absence after a reread is checked here as
//! that rule, not as a difference.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;

use kition_note_kit::{KitDocument, NoteDocument};
use serde_json::{Value, json};

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["conformance", "corpus"] {
        let mut names: Vec<_> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        names.sort();
        for path in names {
            files.push((format!("{dir}/{}", path.file_name().unwrap().to_string_lossy()), std::fs::read(&path).unwrap()));
        }
    }
    files
}

fn read(doc: &mut NoteDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params).unwrap()
}

/// The writes to try on `doc`, with params taken from what it holds.
fn writes(doc: &mut NoteDocument) -> Vec<(&'static str, Value)> {
    let text = doc.text().to_owned();
    let length = text.chars().count();
    let headings = read(doc, "outline.read", json!({}))["headings"].as_array().cloned().unwrap_or_default();
    let tasks = read(doc, "tasks.list", json!({}))["tasks"].as_array().cloned().unwrap_or_default();
    let tables = read(doc, "table.read", json!({}))["tables"].as_u64().unwrap_or(0);
    let links = read(doc, "links.list", json!({ "kinds": ["wikilink", "markdown"] }))["links"].as_array().cloned().unwrap_or_default();
    let properties = read(doc, "property.list", json!({}))["properties"].as_array().cloned().unwrap_or_default();
    let mut writes = vec![
        ("text.insert", json!({ "at": length / 2, "text": "X" })),
        ("text.findReplace", json!({ "find": "the", "replace": "THE", "occurrence": "all" })),
        ("note.write", json!({ "markdown": format!("{text}\nAppended by note.write.\n") })),
        ("block.append", json!({ "markdown": "Appended." })),
        ("property.set", json!({ "key": "c8", "value": ["a", "b"] })),
        ("table.create", json!({ "columns": 2, "rows": 2, "header": ["A", "B"] })),
        ("link.insert", json!({ "at": length, "destination": "Other note" })),
        ("comment.add", json!({ "from": 0, "to": length.min(5), "body": "A comment." })),
        ("file.setLineEndings", json!({ "eol": "crlf" })),
        ("file.setLineEndings", json!({ "eol": "lf", "bom": false })),
    ];
    if length > 4 {
        writes.push(("text.replace", json!({ "from": 1, "to": 3, "text": "é😀" })));
        writes.push(("text.delete", json!({ "from": length - 2, "to": length })));
    }
    if let (Some(first), Some(last)) = (headings.first(), headings.last()) {
        writes.push(("section.insert", json!({ "heading": first["path"], "position": "end", "markdown": "Inserted." })));
        writes.push(("section.replace", json!({ "line": first["line"], "markdown": "Replaced." })));
        writes.push(("section.delete", json!({ "line": last["line"] })));
        writes.push(("heading.setLevel", json!({ "line": first["line"], "level": 3 })));
        writes.push(("block.append", json!({ "line": first["line"], "markdown": "Appended under." })));
        if first["line"] != last["line"] {
            writes.push(("section.move", json!({ "line": first["line"], "toLine": last["line"], "position": "after" })));
        }
        writes.push(("comment.add", json!({ "line": first["line"], "body": "On a heading." })));
    }
    if let Some(task) = tasks.first() {
        writes.push(("task.toggle", json!({ "line": task["line"] })));
        writes.push(("block.setId", json!({ "line": task["line"], "id": "c8task" })));
    }
    if tables > 0 {
        writes.push(("table.setCell", json!({ "table": 0, "row": 0, "column": 0, "text": "Cell | piped" })));
        writes.push(("table.insertRow", json!({ "table": 0, "at": 1, "cells": ["x"] })));
        writes.push(("table.insertColumn", json!({ "table": 0, "at": 0, "header": "H", "align": "right" })));
        writes.push(("table.deleteColumn", json!({ "table": 0, "column": 0 })));
        writes.push(("table.moveRow", json!({ "table": 0, "from": 1, "to": 1 })));
        writes.push(("table.align", json!({ "table": 0, "column": 0, "align": "center" })));
        writes.push(("table.format", json!({ "table": 0 })));
    }
    if let Some(link) = links.first() {
        writes.push(("link.update", json!({ "line": link["line"], "index": link["index"], "destination": "Renamed" })));
        writes.push(("links.retarget", json!({ "from": link["destination"], "to": "Elsewhere" })));
    }
    if let Some(property) = properties.first() {
        writes.push(("property.rename", json!({ "key": property["key"], "to": "renamed-key" })));
        writes.push(("property.delete", json!({ "key": property["key"], "occurrence": 1 })));
    }
    writes
}

/// What a read of the document says of the model: body, registers, inspect without the session.
fn model(doc: &NoteDocument) -> (String, Value, Value) {
    let inspected = doc.inspect().unwrap();
    let mut content = kition_kit::document_content(inspected.clone(), kition_note_kit::EDITOR_STATE);
    // Comments are not in the file (above); everything else read from the file must agree.
    if let Some(map) = content.as_object_mut() {
        map.remove("comments");
        map.remove("openThreads");
    }
    (doc.text().to_owned(), inspected["registers"].clone(), content)
}

#[test]
fn every_write_family_on_every_file_saves_what_it_holds() {
    let (mut checked, mut skipped, mut differ) = (0, 0, Vec::new());
    for (name, bytes) in fixtures() {
        let probe = NoteDocument::open("note.md", &bytes).unwrap();
        if probe.is_read_only() {
            skipped += 1;
            continue;
        }
        let mut doc = NoteDocument::open("note.md", &bytes).unwrap();
        for (command, params) in writes(&mut doc) {
            let mut doc = NoteDocument::open("note.md", &bytes).unwrap();
            if doc.execute(command, params.clone()).is_err() {
                skipped += 1;
                continue;
            }
            checked += 1;
            let saved = doc.save(None).unwrap();
            let reread = NoteDocument::open("note.md", &saved).unwrap();
            if model(&doc) != model(&reread) {
                differ.push(format!("{name}: {command} {params}"));
            }
            if command == "comment.add" {
                assert_eq!(reread.inspect().unwrap()["comments"], json!({ "open": 0, "resolved": 0 }), "a comment went into the file");
            }
            // Saved again unchanged: byte for byte.
            assert_eq!(reread.save(None).unwrap(), saved, "{name}: {command}: a reread saves other bytes");
        }
    }
    println!("in memory == saved: {checked} writes checked, {} differ, {skipped} skipped", differ.len());
    assert!(differ.is_empty(), "{differ:#?}");
}
