//! The commands end to end on `NoteDocument`: typed errors on empty params, one-place edits,
//! comments, and the agent-facing catalogue.

use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use kition_kit::session::KitSessionDocument;
use serde_json::{Value, json};

use crate::NoteDocument;
use crate::commands::SPECS;

const NOTE: &str = "---\ntitle: Plan\ntags:\n  - work\nauthor:\n  name: Ann\n---\n# Plan\n\nGoals for the quarter. ^goals\n\n## Risks\n\n- [ ] Hire ^t1\n- [x] Budget\n\n| Item | Owner |\n| ---- | ----- |\n| Ship | Ann   |\n| Test | Bo    |\n\nSee [[Plan#Risks]] and [the doc](doc.md). #project\n\n## Owners\n\nAnn \u{1F600} and Bo.\n";

fn note() -> NoteDocument {
    NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap()
}

fn run(doc: &mut NoteDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

/// The lines of `after` that differ from `before` (same line count assumed).
fn changed_lines(before: &str, after: &str) -> Vec<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (before.split('\n').collect(), after.split('\n').collect());
    assert_eq!(a.len(), b.len(), "line count changed:\n{after}");
    a.iter().zip(&b).filter(|(x, y)| x != y).map(|(_, y)| (*y).to_owned()).collect()
}

const REASONS: &[&str] = &["invalid-params:", "not-found:", "ambiguous:", "out-of-range:", "stale:", "read-only:", "refused:"];

fn typed(error: &str) -> bool {
    REASONS.iter().any(|reason| error.starts_with(reason))
}

#[test]
fn every_command_survives_empty_params() {
    for read_only in [false, true] {
        let mut doc = if read_only { NoteDocument::open("bad.md", b"# Plan\n\xFF\n").unwrap() } else { note() };
        // The editor's commands act on the selection, so empty params are enough for them.
        for spec in SPECS.iter().filter(|spec| !crate::commands::is_editor(spec.id)) {
            let before = (doc.text().to_owned(), doc.revision());
            match doc.execute(spec.id, json!({})) {
                Ok(_) => {
                    if spec.mutates && !matches!(spec.id, "edit.undo" | "edit.redo") {
                        panic!("{} changed the note with empty params", spec.id);
                    }
                }
                Err(error) => {
                    assert!(typed(&error), "{}: an untyped error: {error}", spec.id);
                    assert_eq!((doc.text().to_owned(), doc.revision()), before, "{} failed and changed the note", spec.id);
                }
            }
            // Null params and wrong types are typed errors too.
            if let Err(error) = doc.execute(spec.id, Value::Null) {
                assert!(typed(&error), "{}: {error}", spec.id);
            }
            if let Err(error) = doc.execute(spec.id, json!({ "table": "x", "line": -1, "heading": 3, "key": 5, "markdown": [] })) {
                assert!(typed(&error), "{}: {error}", spec.id);
            }
        }
    }
}

#[test]
fn every_agent_command_has_a_schema_and_no_file_parameter() {
    // kition-headless hides a command whose parameters use these words (`FILE_PARAMS`).
    const FILE_WORDS: &[&str] = &["path", "paths", "folder", "dir", "directory", "output", "outputdir", "file", "files", "target"];
    let doc = note();
    let listed = doc.commands();
    assert_eq!(listed.len(), SPECS.len());
    for command in listed {
        let schema = command.schema.unwrap_or_else(|| panic!("{} has no schema", command.id));
        let fields = schema.as_object().unwrap();
        for (name, spec) in fields {
            assert!(!FILE_WORDS.contains(&name.to_ascii_lowercase().as_str()), "{} has a file-like parameter {name}", command.id);
            assert!(
                spec["description"].as_str().is_some_and(|description| description.ends_with('.')),
                "{}.{name} needs a one-sentence description",
                command.id
            );
        }
        let notation = command.params.unwrap();
        let words: Vec<String> = notation.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map(str::to_ascii_lowercase).collect();
        assert!(!words.iter().any(|word| FILE_WORDS.contains(&word.as_str())), "{}: notation {notation:?} names a file word", command.id);
    }
    let manifest: Value = serde_json::from_str(include_str!("../../../kit.json")).unwrap();
    let excluded: Vec<&str> = manifest["agent"]["exclude"].as_array().unwrap().iter().map(|id| id.as_str().unwrap()).collect();
    assert_eq!(excluded, crate::commands::excluded());
}

#[test]
fn table_cell_edits_change_only_that_cell() {
    let mut doc = note();
    run(&mut doc, "table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Cy", "expected": "Ann" }));
    let after = String::from_utf8(doc.save(None).unwrap()).unwrap();
    assert_eq!(changed_lines(NOTE, &after), vec!["| Ship | Cy    |"]);
    let stale = doc.execute("table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Di", "expected": "Ann" })).unwrap_err();
    assert!(stale.starts_with("stale:"), "{stale}");
    let table = run(&mut doc, "table.read", json!({ "table": 0 }));
    assert_eq!(table["rows"], json!([["Ship", "Cy"], ["Test", "Bo"]]));
    assert_eq!(run(&mut doc, "table.read", json!({}))["tables"], 1);
}

#[test]
fn table_structure_commands_and_undo() {
    let mut doc = note();
    run(&mut doc, "table.insertRow", json!({ "table": 0, "at": 3, "cells": ["Fix", "Cy"] }));
    assert!(doc.text().contains("| Test | Bo    |\n| Fix  | Cy    |\n"));
    run(&mut doc, "table.insertColumn", json!({ "table": 0, "at": 2, "header": "Due", "align": "right" }));
    run(&mut doc, "table.align", json!({ "table": 0, "column": 0, "align": "center" }));
    run(&mut doc, "table.moveRow", json!({ "table": 0, "from": 1, "to": 2 }));
    run(&mut doc, "table.moveColumn", json!({ "table": 0, "from": 0, "to": 1 }));
    run(&mut doc, "table.deleteRow", json!({ "table": 0, "row": 3 }));
    run(&mut doc, "table.deleteColumn", json!({ "table": 0, "column": 2 }));
    run(&mut doc, "table.format", json!({ "table": 0 }));
    let table = run(&mut doc, "table.read", json!({ "table": 0 }));
    assert_eq!(table["header"], json!(["Owner", "Item"]));
    assert_eq!(table["rows"], json!([["Bo", "Test"], ["Ann", "Ship"]]));
    for _ in 0..8 {
        run(&mut doc, "edit.undo", json!({}));
    }
    assert_eq!(doc.text(), NOTE);
    run(&mut doc, "table.add", json!({ "heading": ["Plan", "Owners"], "columns": 2, "header": ["A", "B"] }));
    assert!(doc.text().contains("Ann \u{1F600} and Bo.\n\n| A | B |\n| --- | --- |\n|  |  |\n"), "{}", doc.text());
    assert_eq!(run(&mut doc, "table.read", json!({}))["tables"], 2);
}

#[test]
fn property_commands_change_only_their_lines() {
    let mut doc = note();
    run(&mut doc, "property.set", json!({ "key": "title", "value": "Road map" }));
    let after = String::from_utf8(doc.save(None).unwrap()).unwrap();
    assert_eq!(changed_lines(NOTE, &after), vec!["title: Road map"]);
    run(&mut doc, "property.set", json!({ "key": "tags", "value": ["work", "q4"] }));
    assert!(doc.text().starts_with("---\ntitle: Road map\ntags:\n  - work\n  - q4\nauthor:"));
    let refused = doc.execute("property.set", json!({ "key": "author", "value": "Bo" })).unwrap_err();
    assert!(refused.starts_with("refused: note.property-complex"), "{refused}");
    run(&mut doc, "property.rename", json!({ "key": "title", "to": "name" }));
    run(&mut doc, "property.delete", json!({ "key": "tags" }));
    let listed = run(&mut doc, "property.list", json!({}));
    assert_eq!(listed["properties"][0]["key"], "name");
    assert_eq!(listed["properties"][1]["complex"], "nested map");
    run(&mut doc, "property.set", json!({ "key": "done", "value": true }));
    assert!(doc.text().contains("  name: Ann\ndone: true\n---\n"));
}

#[test]
fn section_and_heading_commands() {
    let mut doc = note();
    let read = run(&mut doc, "section.read", json!({ "heading": ["Plan", "Owners"] }));
    assert_eq!(read["markdown"], "## Owners\n\nAnn \u{1F600} and Bo.\n");
    assert_eq!(read["line"], 24);
    run(&mut doc, "section.replace", json!({ "heading": ["Owners"], "markdown": "Cy." }));
    assert!(doc.text().ends_with("## Owners\n\nCy.\n"));
    run(&mut doc, "section.insert", json!({ "line": 12, "position": "before", "markdown": "## Context" }));
    assert!(doc.text().contains("^goals\n\n## Context\n\n## Risks\n"));
    run(&mut doc, "section.move", json!({ "heading": ["Owners"], "toHeading": ["Risks"], "position": "before" }));
    assert!(doc.text().contains("## Context\n\n## Owners\n\nCy.\n\n## Risks\n"), "{}", doc.text());
    run(&mut doc, "heading.setLevel", json!({ "heading": ["Plan", "Context"], "level": 3 }));
    assert!(doc.text().contains("### Context\n"));
    run(&mut doc, "section.delete", json!({ "heading": ["Plan", "Context"] }));
    assert!(!doc.text().contains("Context"));
    let outline = run(&mut doc, "outline.read", json!({ "limit": 2 }));
    assert_eq!(outline["headings"][1]["path"], json!(["Plan", "Owners"]));
    assert_eq!(outline["next"], "2");
    let error = doc.execute("section.read", json!({ "heading": ["Nope"] })).unwrap_err();
    assert!(error.starts_with("not-found"), "{error}");
}

#[test]
fn blocks_tasks_and_links() {
    let mut doc = note();
    assert_eq!(run(&mut doc, "block.read", json!({ "blockId": "goals" }))["markdown"], "Goals for the quarter. ^goals");
    run(&mut doc, "block.replace", json!({ "blockId": "goals", "markdown": "Goals for the year." }));
    assert!(doc.text().contains("Goals for the year. ^goals\n"));
    let toggled = run(&mut doc, "task.toggle", json!({ "blockId": "t1" }));
    assert_eq!(toggled["done"], true);
    assert!(doc.text().contains("- [x] Hire ^t1"));
    let tasks = run(&mut doc, "task.list", json!({ "heading": ["Plan", "Risks"] }));
    assert_eq!(tasks["tasks"][0]["text"], "Hire");
    assert_eq!(tasks["tasks"][0]["blockId"], "t1");
    run(&mut doc, "block.setId", json!({ "line": 26, "id": "owners" }));
    run(&mut doc, "block.append", json!({ "markdown": "The end." }));
    assert!(doc.text().ends_with("Bo. ^owners\n\nThe end.\n"), "{}", doc.text());
    let links = run(&mut doc, "link.list", json!({}));
    let kinds: Vec<&str> = links["links"].as_array().unwrap().iter().map(|link| link["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["wikilink", "markdown", "tag", "blockId", "blockId", "blockId", "tag"]);
    assert_eq!(links["links"][0]["state"], "unknown");
    run(&mut doc, "link.update", json!({ "line": 22, "index": 1, "destination": "docs/doc.md" }));
    assert!(doc.text().contains("[the doc](docs/doc.md)"));
    let at = doc.text().chars().count();
    run(&mut doc, "link.insert", json!({ "at": at, "destination": "Owners", "text": "people" }));
    assert!(doc.text().ends_with("The end.\n[[Owners|people]]"));
    run(&mut doc, "link.retarget", json!({ "from": "Plan", "to": "Road map" }));
    assert!(doc.text().contains("[[Road map#Risks]]"));
}

#[test]
fn text_commands_find_and_write() {
    let mut doc = note();
    // Find and replace act on the body only, so the front matter's
    // `name: Ann` is no longer counted.
    let found = run(&mut doc, "text.find", json!({ "find": "ann", "caseSensitive": false }));
    assert_eq!(found["count"], 2);
    let replaced = run(&mut doc, "text.findReplace", json!({ "find": "Bo", "replace": "Cy", "occurrence": "all" }));
    assert_eq!(replaced["replaced"], 2);
    let whole = doc.text().replace("Goals", "Aims");
    run(&mut doc, "note.write", json!({ "markdown": whole }));
    assert!(doc.text().contains("Aims for the quarter"));
    let expanded = run(&mut doc, "template.expand", json!({ "markdown": "# {{title}} {{date}}", "title": "Day", "date": "2026-10-23" }));
    assert_eq!(expanded["markdown"], "# Day 2026-10-23");
    let utf16 = run(&mut doc, "text.find", json!({ "find": "and Bo", "unit": "utf16" }));
    assert_eq!(utf16["count"], 0);
    let emoji = run(&mut doc, "text.find", json!({ "find": "and Cy.", "unit": "utf16" }));
    let scalar = run(&mut doc, "text.find", json!({ "find": "and Cy." }));
    assert_eq!(emoji["matches"][0]["from"].as_u64().unwrap(), scalar["matches"][0]["from"].as_u64().unwrap() + 1);
}

#[test]
fn comments_live_beside_the_text() {
    let mut doc = note();
    let added = doc
        .execute_as(
            "comment.add",
            json!({ "body": "Why?", "quote": "Goals for the quarter", "created": "2026-10-23T10:00:00Z" }),
            &Origin::agent("agent:test"),
        )
        .unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    run(&mut doc, "comment.reply", json!({ "thread": thread, "body": "Because.", "author": "person:ann", "created": "2026-10-23T10:01:00Z" }));
    run(&mut doc, "text.insert", json!({ "at": 0, "text": "Intro\n" }));
    let threads = run(&mut doc, "comment.threads", json!({}));
    let first = &threads["threads"][0];
    assert_eq!(first["comments"][0]["author"], "agent:test");
    assert_eq!(first["comments"][1]["body"], "Because.");
    assert_eq!(first["quote"], "Goals for the quarter");
    let from = first["from"].as_u64().unwrap() as usize;
    assert_eq!(doc.text().chars().skip(from).take(5).collect::<String>(), "Goals");
    run(&mut doc, "comment.resolve", json!({ "thread": thread }));
    assert_eq!(doc.inspect().unwrap()["comments"]["resolved"], 1);
    run(&mut doc, "comment.move", json!({ "thread": thread, "quote": "Budget" }));
    assert_eq!(run(&mut doc, "comment.threads", json!({}))["threads"][0]["quote"], "Budget");
    let comment = run(&mut doc, "comment.threads", json!({}))["threads"][0]["comments"][1]["id"].as_str().unwrap().to_owned();
    run(&mut doc, "comment.delete", json!({ "thread": thread, "comment": comment }));
    assert_eq!(run(&mut doc, "comment.threads", json!({}))["threads"][0]["comments"].as_array().unwrap().len(), 1);
    // Comments never reach the file.
    assert!(!String::from_utf8(doc.save(None).unwrap()).unwrap().contains("Why?"));
    assert!(doc.execute("comment.reply", json!({ "thread": "nope", "body": "x" })).unwrap_err().starts_with("not-found"));
}

#[test]
fn line_endings_are_set_in_one_step() {
    let mut doc = NoteDocument::open("mixed.md", b"one\r\ntwo\nthree\rfour\n").unwrap();
    assert!(doc.check().iter().any(|found| found.code == "note.mixed-line-endings"));
    run(&mut doc, "file.setLineEndings", json!({ "eol": "crlf" }));
    assert_eq!(doc.save(None).unwrap(), b"one\r\ntwo\r\nthree\r\nfour\r\n");
    assert!(doc.check().is_empty());
    run(&mut doc, "edit.undo", json!({}));
    assert_eq!(doc.save(None).unwrap(), b"one\r\ntwo\nthree\rfour\n");
}

#[test]
fn structural_commands_take_the_notes_line_endings() {
    let crlf = NOTE.replace('\n', "\r\n");
    let mut doc = NoteDocument::open("plan.md", crlf.as_bytes()).unwrap();
    run(&mut doc, "table.insertRow", json!({ "table": 0, "at": 3, "cells": ["Fix", "Cy"] }));
    run(&mut doc, "property.set", json!({ "key": "new", "value": "x" }));
    let saved = String::from_utf8(doc.save(None).unwrap()).unwrap();
    assert!(!saved.replace("\r\n", "").contains('\n'), "a bare LF slipped in");
    assert!(saved.contains("| Fix  | Cy    |\r\n"));
}

#[test]
fn inspect_summarises_the_note() {
    let doc = note();
    let seen = doc.inspect().unwrap();
    assert_eq!(seen["outline"][1]["path"], json!(["Plan", "Risks"]));
    assert_eq!(seen["tables"][0]["rows"], 3);
    assert_eq!(seen["tasks"], json!({ "open": 1, "done": 1 }));
    assert_eq!(seen["links"]["wikilinks"], 1);
    assert_eq!(seen["properties"][2]["complex"], true);
    assert_eq!(seen["registers"]["eol"], "lf");
}
