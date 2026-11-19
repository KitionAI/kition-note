//! Every command says what it does and declares what it answers, and answers exactly that
//! (`kition_kit::testing::check_commands`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;
use serde_json::{Map, Value, json};

#[test]
fn every_command_says_what_it_does() {
    let doc = NoteDocument::open("note.md", b"# Note\n").unwrap();
    // No exemptions: every command declares what it answers, view-only ones included.
    if let Err(problems) = kition_kit::testing::check_commands(&doc.commands(), &[]) {
        panic!("{}", problems.join("\n"));
    }
}

/// `cases/regressions/issue-2`: every command the headless executable lists in
/// `commands.list` (it lists `commands()` as they are) has a description and an output schema.
#[test]
fn every_listed_command_has_a_description_and_an_output_schema() {
    let commands = NoteDocument::open("note.md", b"# Note\n").unwrap().commands();
    assert_eq!(commands.len(), kition_note_kit::commands::SPECS.len(), "commands() lists every command of the catalogue");
    let lacking: Vec<String> = commands
        .iter()
        .filter_map(|command| {
            let mut lacks = Vec::new();
            if command.description.trim().is_empty() {
                lacks.push("description");
            }
            if command.output_schema.is_none() {
                lacks.push("output schema");
            }
            (!lacks.is_empty()).then(|| format!("{}: no {}", command.id, lacks.join(" and ")))
        })
        .collect();
    assert!(lacking.is_empty(), "{} of {} commands lack a description or an output schema:\n{}", lacking.len(), commands.len(), lacking.join("\n"));
}

const SAMPLE: &[u8] = b"---\ntitle: Plan\ntags: [a, b]\n---\n# Plan\n\nSome **bold** text with a [[Other]] link and #tag. ^intro\n\n## Tasks\n\n- [ ] first\n- [x] done\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n```mermaid\ngraph TD; A-->B\n```\n";

/// More of what the answers describe: Markdown links with anchors and text, an embed, a picture,
/// a URL, a code block, a table with two body rows, a task with a block id, a picture on a line of
/// its own.
const RICH: &[u8] = b"---\ntitle: Rich\ntags: [x]\n---\n# Rich\n\nSee [the plan](Plan.md#Risks), [[Other#^intro|alias]], ![[pic.png]], ![alt](img.png) and https://example.com.\n\n- [ ] call ^call\n\n| A | B |\n| :- | -: |\n| 1 | 2 |\n| 3 | 4 |\n\n```rust\nfn main() {}\n```\n\n![[photo.png]]\n";

/// Front matter with a duplicated key and a nested value.
const ODD_PROPERTIES: &[u8] = b"---\na: 1\na: 2\nnested:\n  x: 1\n---\n# Odd\n";

/// Front matter that is not YAML.
const BROKEN_PROPERTIES: &[u8] = b"---\na: [1\n---\n# Broken\n";

/// No front matter: no properties block.
const PLAIN: &[u8] = b"# Plain\n\nText.\n";

/// One call: the note it runs on, and its parameters. `$thread` and `$comment` stand for the
/// ids of a comment thread added first.
type Call = (&'static [u8], Value);

/// The calls each command is checked with: at least one, and every shape of answer it has.
fn calls(id: &str) -> Vec<Call> {
    let on = |params: Value| vec![(SAMPLE, params)];
    match id {
        "text.read" => vec![(SAMPLE, json!({})), (SAMPLE, json!({ "from": 0, "to": 3, "unit": "utf16" }))],
        "outline.read" => vec![(SAMPLE, json!({})), (SAMPLE, json!({ "limit": 1 }))],
        "section.read" => on(json!({ "heading": ["Plan", "Tasks"] })),
        "block.read" => on(json!({ "blockId": "intro" })),
        "property.list" => vec![(SAMPLE, json!({})), (ODD_PROPERTIES, json!({})), (BROKEN_PROPERTIES, json!({})), (PLAIN, json!({}))],
        "table.read" => vec![(SAMPLE, json!({})), (SAMPLE, json!({ "table": 0 })), (RICH, json!({ "table": 0 }))],
        "links.list" => vec![(SAMPLE, json!({})), (RICH, json!({}))],
        "tasks.list" => vec![(SAMPLE, json!({})), (RICH, json!({}))],
        "text.find" => vec![(SAMPLE, json!({ "find": "bold" })), (SAMPLE, json!({ "find": "t", "limit": 1 }))],
        "page.layout" => vec![(SAMPLE, json!({})), (RICH, json!({ "page": 1, "pageSize": "a4" }))],
        "file.export" => vec![
            (SAMPLE, json!({ "format": "html" })),
            (SAMPLE, json!({ "format": "html", "standalone": false })),
            (SAMPLE, json!({ "format": "pdf", "pageSize": "a4" })),
        ],
        "comment.threads" => vec![(SAMPLE, json!({})), (PLAIN, json!({}))],
        "template.expand" => on(json!({ "markdown": "# {{title}} {{date}}", "title": "T", "date": "2026-10-23T14:05" })),
        "text.insert" => on(json!({ "at": 0, "text": "x" })),
        "text.delete" => on(json!({ "from": 0, "to": 1, "expected": "-" })),
        "text.replace" => on(json!({ "from": 0, "to": 3, "text": "+++" })),
        "text.findReplace" => on(json!({ "find": "bold", "replace": "strong", "occurrence": "all" })),
        "note.write" => on(json!({ "markdown": "# New\n" })),
        "section.replace" => on(json!({ "heading": ["Plan", "Tasks"], "markdown": "- [ ] new\n" })),
        "section.insert" => on(json!({ "heading": ["Plan", "Tasks"], "position": "end", "markdown": "More." })),
        "section.delete" => on(json!({ "heading": ["Plan", "Tasks"] })),
        "section.move" => on(json!({ "heading": ["Plan", "Tasks"], "toHeading": ["Plan"], "position": "before" })),
        "heading.setLevel" => on(json!({ "heading": ["Plan", "Tasks"], "level": 3 })),
        "block.replace" => on(json!({ "blockId": "intro", "markdown": "New text." })),
        "block.append" => on(json!({ "markdown": "Appended." })),
        "block.setId" => on(json!({ "blockId": "intro", "id": "start" })),
        "task.toggle" => vec![(SAMPLE, json!({ "line": 11 })), (RICH, json!({ "blockId": "call", "done": true }))],
        "property.set" => on(json!({ "key": "status", "value": ["a", "b"] })),
        "property.delete" => on(json!({ "key": "title" })),
        "property.rename" => on(json!({ "key": "title", "to": "name" })),
        "table.create" => on(json!({ "columns": 2, "header": ["X", "Y"] })),
        "table.setCell" => on(json!({ "table": 0, "row": 1, "column": 0, "text": "9", "expected": "1" })),
        "table.insertRow" => on(json!({ "table": 0, "at": 1, "cells": ["5", "6"] })),
        "table.insertColumn" => on(json!({ "table": 0, "at": 1, "header": "C" })),
        "table.deleteRow" => on(json!({ "table": 0, "row": 1 })),
        "table.deleteColumn" => on(json!({ "table": 0, "column": 1 })),
        "table.moveRow" => vec![(RICH, json!({ "table": 0, "from": 1, "to": 2 }))],
        "table.moveColumn" => on(json!({ "table": 0, "from": 0, "to": 1 })),
        "table.align" => on(json!({ "table": 0, "column": 0, "align": "center" })),
        "table.format" => on(json!({ "table": 0 })),
        "link.insert" => on(json!({ "at": 0, "destination": "Plan" })),
        "link.update" => on(json!({ "line": 7, "index": 0, "destination": "Else" })),
        "links.retarget" => on(json!({ "from": "Other", "to": "Renamed" })),
        "comment.add" => vec![(SAMPLE, json!({ "body": "Hi", "quote": "bold" })), (SAMPLE, json!({ "body": "On the whole note" }))],
        "comment.reply" => on(json!({ "thread": "$thread", "body": "Re", "name": "Ann" })),
        "comment.resolve" => on(json!({ "thread": "$thread" })),
        "comment.move" => on(json!({ "thread": "$thread", "quote": "text" })),
        "comment.delete" => on(json!({ "thread": "$thread", "comment": "$comment" })),
        "file.setLineEndings" => on(json!({ "eol": "crlf", "bom": true })),
        "selection.set" => on(json!({ "anchor": 0, "focus": 4 })),
        "diagram.set" => on(json!({ "source": "graph TD; A-->B", "width": 100, "height": 80 })),
        "properties.view" => vec![(SAMPLE, json!({})), (PLAIN, json!({}))],
        "image.wanted" => vec![(SAMPLE, json!({})), (RICH, json!({}))],
        "image.set" => on(json!({ "name": "a.png", "data": "" })),
        "selection.replace" => on(json!({ "text": "x" })),
        "view.width" => on(json!({ "width": 800, "height": 600 })),
        "view.scheme" => on(json!({ "scheme": "dark" })),
        "selection.where" => on(json!({ "at": 0 })),
        "view.blockAt" => on(json!({ "x": 10, "y": 10 })),
        "format.highlightColor" => on(json!({ "color": "red" })),
        "selection.controlKey" => on(json!({ "key": "a" })),
        "code.blocks" | "table.geometry" | "diagrams.view" => vec![(SAMPLE, json!({})), (RICH, json!({ "top": 0, "bottom": 100000 }))],
        "edit.undo" | "edit.redo" => vec![(SAMPLE, json!({})), (SAMPLE, json!({ "scope": "agent" }))],
        _ => on(json!({})),
    }
}

/// Makes what a call needs first: a comment thread, an edit to undo, an undo to redo, a sized
/// diagram, a selection for the editor's commands. Answers the thread's and its first comment's ids.
fn prepare(doc: &mut NoteDocument, id: &str) -> (String, String) {
    let quote = if doc.text().contains("Some") { json!({ "body": "Check", "quote": "Some" }) } else { json!({ "body": "Check" }) };
    let added = doc.execute("comment.add", quote).unwrap();
    if id == "edit.undo" || id == "edit.redo" {
        doc.execute("text.insert", json!({ "at": 0, "text": "x" })).unwrap();
    }
    if id == "edit.redo" {
        doc.execute("edit.undo", json!({})).unwrap();
    }
    if id == "diagrams.view" {
        // A sized diagram is drawn: its box is answered too.
        doc.execute("diagram.set", json!({ "source": "graph TD; A-->B", "width": 100, "height": 80 })).unwrap();
    }
    if kition_note_kit::commands::is_editor(id) && id != "selection.set" {
        doc.execute("selection.set", json!({ "anchor": 40, "focus": 44 })).unwrap();
    }
    (added["thread"].as_str().unwrap().to_owned(), added["comment"].as_str().unwrap().to_owned())
}

/// Whether `answer` is what `output` declares: the answer is checked as a required parameter of
/// that schema, so its type, required fields, closed objects, enums and `oneOf` branches all hold.
fn conforms(output: &Value, answer: &Value) -> Result<(), String> {
    let mut node = output.clone();
    node["required"] = Value::Bool(true);
    let mut given = Map::new();
    given.insert("answer".into(), answer.clone());
    kition_kit::schema::check_params(&json!({ "answer": node }), &given)
}

#[test]
fn every_command_answers_what_it_declares() {
    let commands = NoteDocument::open("note.md", SAMPLE).unwrap().commands();
    let mut problems = Vec::new();
    for command in &commands {
        let Some(output) = &command.output_schema else {
            problems.push(format!("{}: declares no output schema", command.id));
            continue;
        };
        for (note, params) in calls(&command.id) {
            let mut doc = NoteDocument::open("note.md", note).unwrap();
            let (thread, comment) = prepare(&mut doc, &command.id);
            let params: Value = serde_json::from_str(&params.to_string().replace("$thread", &thread).replace("$comment", &comment)).unwrap();
            match doc.execute(&command.id, params.clone()) {
                Err(error) => problems.push(format!("{} {params}: {error}", command.id)),
                Ok(answer) => {
                    if let Err(error) = conforms(output, &answer) {
                        problems.push(format!("{} {params}: answered {answer}, which is not what it declares: {error}", command.id));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn every_output_schema_is_valid() {
    for command in NoteDocument::open("note.md", SAMPLE).unwrap().commands() {
        if let Some(schema) = &command.output_schema {
            kition_kit::schema::validate_output(schema).unwrap_or_else(|error| panic!("{}: {error}", command.id));
        }
    }
}
