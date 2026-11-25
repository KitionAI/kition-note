//! Command-level round trips: each test edits a note only with commands, checks the edit took,
//! saves the Markdown, reopens it and compares the whole document: the reopened note saves back to
//! the very same bytes, and every read command answers as it did before the save (outline,
//! properties, tables, links, tasks, code blocks). Comments are left out: they live beside the text
//! and never reach the `.md` (`comments_live_beside_the_text`).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use serde_json::{Value, json};

use crate::NoteDocument;

const PLAN: &str = "---\ntitle: Plan\ntags:\n  - work\n---\n# Plan\n\nGoals for the quarter. ^goals\n\n## Risks\n\n- [ ] Hire ^t1\n- [x] Budget\n\n## Owners\n\nAnn \u{1F600} and Bo 中文.\n";

fn open(text: &str) -> NoteDocument {
    NoteDocument::open("plan.md", text.as_bytes()).unwrap()
}

fn run(doc: &mut NoteDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params.clone()).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

/// Puts the selection from `anchor` to `focus` (characters of the body).
fn select(doc: &mut NoteDocument, anchor: usize, focus: usize) {
    run(doc, "select.set", json!({ "anchor": anchor, "focus": focus }));
}

/// Selects the first occurrence of `text` in the body.
fn select_text(doc: &mut NoteDocument, text: &str) {
    let body = doc.body();
    let start = body[..body.find(text).unwrap_or_else(|| panic!("no {text:?} in {body:?}"))].chars().count();
    select(doc, start, start + text.chars().count());
}

/// Everything the read commands answer about the note.
fn reads(doc: &mut NoteDocument) -> Value {
    let tables = run(doc, "table.read", json!({}))["tables"].as_u64().unwrap();
    let each: Vec<Value> = (0..tables).map(|table| run(doc, "table.read", json!({ "table": table }))).collect();
    json!({
        "outline": run(doc, "outline.read", json!({})),
        "properties": run(doc, "property.list", json!({})),
        "tables": each,
        "links": run(doc, "link.list", json!({})),
        "tasks": run(doc, "task.list", json!({})),
        "code": run(doc, "code.blocks", json!({})),
    })
}

/// Saves, reopens and compares: the same bytes back, and the same answers to every read.
fn round_trip(doc: &mut NoteDocument) -> NoteDocument {
    let before = reads(doc);
    let bytes = doc.save(None).unwrap();
    let mut back = NoteDocument::open("plan.md", &bytes).unwrap();
    assert_eq!(back.save(None).unwrap(), bytes, "the reopened note saves different bytes");
    assert_eq!(reads(&mut back), before, "the reopened note reads differently");
    back
}

#[test]
fn sections_blocks_and_headings_survive_save_and_reopen() {
    let mut doc = open(PLAN);
    run(&mut doc, "section.replace", json!({ "heading": ["Plan", "Owners"], "markdown": "Cy and Di." }));
    run(&mut doc, "section.insert", json!({ "heading": ["Plan", "Risks"], "position": "before", "markdown": "## Context\n\nWhy now." }));
    run(&mut doc, "section.move", json!({ "heading": ["Plan", "Owners"], "toHeading": ["Plan", "Risks"], "position": "before" }));
    run(&mut doc, "heading.setLevel", json!({ "heading": ["Plan", "Context"], "level": 3 }));
    run(&mut doc, "block.replace", json!({ "blockId": "goals", "markdown": "Goals for the year. ^goals" }));
    run(&mut doc, "block.append", json!({ "markdown": "The end." }));
    let edited = doc.text().to_owned();
    assert!(edited.contains("### Context") && edited.contains("Goals for the year.") && edited.ends_with("The end.\n"), "{edited}");
    assert!(edited.find("## Owners").unwrap() < edited.find("## Risks").unwrap(), "{edited}");
    let back = round_trip(&mut doc);
    assert_eq!(back.text(), edited);
}

#[test]
fn properties_survive_save_and_reopen() {
    let mut doc = open(PLAN);
    run(&mut doc, "property.set", json!({ "key": "title", "value": "Road map" }));
    run(&mut doc, "property.set", json!({ "key": "tags", "value": ["work", "q4"] }));
    run(&mut doc, "property.set", json!({ "key": "done", "value": true }));
    run(&mut doc, "property.set", json!({ "key": "owner", "value": "Ann 中文" }));
    run(&mut doc, "property.rename", json!({ "key": "owner", "to": "lead" }));
    run(&mut doc, "property.delete", json!({ "key": "done" }));
    let listed = run(&mut doc, "property.list", json!({}));
    let shown = listed.to_string();
    assert!(shown.contains("Road map") && shown.contains("q4") && shown.contains("lead") && !shown.contains("\"done\""), "{shown}");
    let mut back = round_trip(&mut doc);
    assert_eq!(run(&mut back, "property.list", json!({})), listed);
}

#[test]
fn tables_survive_save_and_reopen() {
    let mut doc = open(PLAN);
    run(&mut doc, "table.add", json!({ "heading": ["Plan", "Owners"], "columns": 2, "header": ["Item", "Owner"] }));
    run(&mut doc, "table.setCell", json!({ "table": 0, "row": 1, "column": 0, "text": "Ship" }));
    run(&mut doc, "table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Ann" }));
    run(&mut doc, "table.insertRow", json!({ "table": 0, "at": 2, "cells": ["Test", "Bo 中文"] }));
    run(&mut doc, "table.insertColumn", json!({ "table": 0, "at": 2, "header": "Due", "align": "right" }));
    run(&mut doc, "table.align", json!({ "table": 0, "column": 0, "align": "center" }));
    run(&mut doc, "table.moveRow", json!({ "table": 0, "from": 1, "to": 2 }));
    run(&mut doc, "table.format", json!({ "table": 0 }));
    let table = run(&mut doc, "table.read", json!({ "table": 0 }));
    assert_eq!(table["header"], json!(["Item", "Owner", "Due"]));
    assert_eq!(table["rows"][0][0], "Test");
    let mut back = round_trip(&mut doc);
    assert_eq!(run(&mut back, "table.read", json!({ "table": 0 })), table);
}

#[test]
fn links_survive_save_and_reopen() {
    let mut doc = open(&format!("{PLAN}\nSee [[Plan#Risks]] and [the doc](doc.md).\n"));
    let line = doc.text().lines().position(|line| line.starts_with("See [[")).unwrap() + 1;
    run(&mut doc, "link.update", json!({ "line": line, "index": 1, "destination": "docs/doc.md" }));
    let at = doc.text().chars().count() - 1;
    run(&mut doc, "link.insert", json!({ "at": at, "destination": "Owners", "text": "people" }));
    run(&mut doc, "link.retarget", json!({ "from": "Plan", "to": "Road map" }));
    let links = run(&mut doc, "link.list", json!({}));
    let shown = links.to_string();
    assert!(shown.contains("docs/doc.md") && shown.contains("Road map") && shown.contains("Owners"), "{shown}");
    let mut back = round_trip(&mut doc);
    assert_eq!(run(&mut back, "link.list", json!({})), links);
}

#[test]
fn tasks_survive_save_and_reopen() {
    let mut doc = open(PLAN);
    run(&mut doc, "task.toggle", json!({ "blockId": "t1" }));
    select_text(&mut doc, "Budget");
    run(&mut doc, "list.toggleCheckbox", json!({}));
    let tasks = run(&mut doc, "task.list", json!({}));
    assert!(doc.text().contains("- [x] Hire ^t1") && doc.text().contains("- [ ] Budget"), "{}", doc.text());
    let mut back = round_trip(&mut doc);
    assert_eq!(run(&mut back, "task.list", json!({})), tasks);
}

#[test]
fn inline_and_block_formatting_survive_save_and_reopen() {
    let mut doc = open("Make this strong, slanted, gone and code.\n\nA heading\n\nA quote\n\nx squared\n");
    for (text, command) in
        [("this", "format.bold"), ("slanted", "format.italic"), ("gone", "format.strike"), ("code", "format.code"), ("x squared", "format.math")]
    {
        select_text(&mut doc, text);
        run(&mut doc, command, json!({}));
    }
    select_text(&mut doc, "A heading");
    run(&mut doc, "format.heading2", json!({}));
    select_text(&mut doc, "A quote");
    run(&mut doc, "format.quote", json!({}));
    let edited = doc.text().to_owned();
    for expected in ["**this**", "*slanted*", "~~gone~~", "`code`", "## A heading", "> A quote", "$x squared$"] {
        assert!(edited.contains(expected), "{expected} missing:\n{edited}");
    }
    let back = round_trip(&mut doc);
    assert_eq!(back.text(), edited);
}

#[test]
fn inserted_blocks_survive_save_and_reopen() {
    let mut doc = open("# Plan\n\nEnd.\n");
    let end = doc.body().chars().count();
    for command in ["rule.insert", "codeBlock.insert", "callout.insert", "mathBlock.insert", "footnote.insert", "table.insert"] {
        select(&mut doc, end, end);
        run(&mut doc, command, json!({}));
    }
    let edited = doc.text().to_owned();
    for expected in ["---", "```", "> [!", "$$", "[^", "|"] {
        assert!(edited.contains(expected), "{expected} missing:\n{edited}");
    }
    let back = round_trip(&mut doc);
    assert_eq!(back.text(), edited);
}
