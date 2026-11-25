//! Call context: the note keeps editor state (the selection, folds, the view's width and the
//! heading under the pointer) that no agent's command reads. Every write an agent is offered
//! declares no `reads`, and runs the same whatever the person's editor shows; `check_context`
//! passes with the kit's editor-state list.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::CallContext;
use kition_note_kit::{EDITOR_STATE, KitDocument, NoteDocument};
use serde_json::{Value, json};

const NOTE: &str = "---\ntitle: Launch plan\nstatus: draft\n---\n\n# Launch plan\n\nIntro text with a [[timeline]] link.\n\n## Goals\n\n- Ship the beta\n- Collect feedback\n\n## Tasks\n\n- [ ] Draft the announcement ^draft\n- [ ] Book the review meeting ^book\n\n## Timeline\n\n| Week | Milestone |\n| --- | --- |\n| 1 | Beta build |\n| 2 | Feedback round |\n";

/// One call of every write an agent is offered.
fn samples() -> Vec<(&'static str, Value)> {
    vec![
        ("text.insert", json!({ "at": 10, "text": "X" })),
        ("text.delete", json!({ "from": 10, "to": 12 })),
        ("text.replace", json!({ "from": 10, "to": 12, "text": "Y" })),
        ("text.findReplace", json!({ "find": "beta", "replace": "gamma" })),
        ("note.write", json!({ "markdown": "# New\n" })),
        ("section.replace", json!({ "heading": ["Launch plan", "Goals"], "markdown": "- One" })),
        ("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "end", "markdown": "- Two" })),
        ("section.delete", json!({ "heading": ["Launch plan", "Goals"] })),
        ("section.move", json!({ "heading": ["Launch plan", "Goals"], "toHeading": ["Launch plan", "Timeline"], "position": "after" })),
        ("heading.setLevel", json!({ "heading": ["Launch plan", "Goals"], "level": 3 })),
        ("block.replace", json!({ "blockId": "draft", "markdown": "- [ ] Draft it" })),
        ("block.append", json!({ "markdown": "The end." })),
        ("block.setId", json!({ "line": 12, "id": "ship" })),
        ("task.toggle", json!({ "line": 17 })),
        ("property.set", json!({ "key": "status", "value": "review" })),
        ("property.delete", json!({ "key": "status" })),
        ("property.rename", json!({ "key": "status", "to": "state" })),
        ("table.add", json!({ "columns": 2 })),
        ("table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Alpha" })),
        ("table.insertRow", json!({ "table": 0, "at": 2 })),
        ("table.insertColumn", json!({ "table": 0, "at": 1 })),
        ("table.deleteRow", json!({ "table": 0, "row": 1 })),
        ("table.deleteColumn", json!({ "table": 0, "column": 1 })),
        ("table.moveRow", json!({ "table": 0, "from": 1, "to": 2 })),
        ("table.moveColumn", json!({ "table": 0, "from": 0, "to": 1 })),
        ("table.align", json!({ "table": 0, "column": 1, "align": "center" })),
        ("table.format", json!({ "table": 0 })),
        ("link.insert", json!({ "at": 10, "destination": "Other" })),
        ("link.update", json!({ "line": 8, "index": 0, "destination": "Elsewhere" })),
        ("comment.add", json!({ "quote": "Beta build", "body": "Which?", "created": "2026-11-13T00:00:00Z", "author": "person:a" })),
        ("file.setLineEndings", json!({ "eol": "crlf" })),
    ]
}

/// A note opened with the person's editor somewhere: a selection, a fold, a width.
fn opened(selection: Value) -> NoteDocument {
    let mut doc = NoteDocument::open("plan.md", NOTE.as_bytes()).unwrap();
    doc.execute("select.set", selection).unwrap();
    doc
}

#[test]
fn no_write_an_agent_is_offered_reads_editor_state() {
    let manifest: Value = serde_json::from_str(include_str!("../../../kit.json")).unwrap();
    let excluded: Vec<&str> = manifest["agent"]["exclude"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    let doc = NoteDocument::create("a.md", 1, 1).unwrap();
    let offered: Vec<_> = doc.commands().into_iter().filter(|command| command.schema.is_some() && !excluded.contains(&command.id.as_str())).collect();
    for command in &offered {
        assert!(command.reads.is_empty(), "{} declares reads {:?}", command.id, command.reads);
    }
    let sampled: Vec<&str> = samples().iter().map(|(id, _)| *id).collect();
    for command in offered
        .iter()
        .filter(|command| command.mutates && !matches!(command.id.as_str(), "edit.undo" | "edit.redo" | "comment.reply" | "comment.resolve"))
    {
        assert!(sampled.contains(&command.id.as_str()), "{} has no sample here", command.id);
    }
}

#[test]
fn every_write_does_the_same_whatever_the_selection() {
    for (command, params) in samples() {
        let mut answers = Vec::new();
        for selection in [json!({ "anchor": 0 }), json!({ "anchor": 40, "focus": 120 }), json!({ "line": 17 })] {
            let mut doc = opened(selection);
            let answer = doc.execute(command, params.clone()).unwrap_or_else(|error| panic!("{command}: {error}"));
            let answer = if command == "comment.add" { json!(null) } else { answer };
            answers.push((answer, String::from_utf8(doc.save(None).unwrap()).unwrap()));
        }
        assert!(answers.windows(2).all(|pair| pair[0] == pair[1]), "{command} differs with the selection: {answers:#?}");
    }
}

#[test]
fn check_context_passes_with_the_editor_state_list_and_set_context_refuses_what_the_note_has_not() {
    let mut doc = opened(json!({ "anchor": 40, "focus": 120 }));
    kition_kit::testing::check_context(&mut doc, &CallContext::default(), EDITOR_STATE).unwrap();
    for context in [
        CallContext { on: vec![json!("note")], ..CallContext::default() },
        CallContext { part: Some(json!({ "page": 1 })), ..CallContext::default() },
        CallContext { time: Some(1.0), ..CallContext::default() },
    ] {
        let refused = doc.set_context(&context).unwrap_err();
        assert!(refused.contains("not available"), "{refused}");
    }
}
