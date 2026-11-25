//! A check of the declaration table against the handlers: `premise::ADDRESSES` is written by hand;
//! this asks the handler itself (`run::plan`, through `premise::footprint_of` and `reads_of`)
//! whether it agrees, for every write that names a place and every parameter of a sample call of
//! it:
//!
//! - a parameter declared a place moves what the write reads or writes for some other value (or
//!   makes it fail): it really names where the write acts;
//! - any other parameter never moves where the write reads to find its place, whatever its value.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_note_kit::premise::{ADDRESSES, footprint_of, reads_of};
use serde_json::{Value, json};

const NOTE: &str = "---\ntitle: Launch plan\nstatus: draft\n---\n\n# Launch plan\n\nIntro text with a [[timeline]] link and beta.\n\n## Goals\n\n- Ship the beta\n- Collect feedback\n\n## Tasks\n\n- [ ] Draft the announcement ^draft\n- [ ] Book the review meeting ^book\n- [ ] Draft the invite ^draft\n\n## Timeline\n\n| Week | Milestone |\n| --- | --- |\n| 1 | Beta build |\n| 2 | Feedback round |\n\n## Budget\n\n| Item | Cost |\n| --- | --- |\n| Venue | 100 |\n";

/// Sample calls: every place parameter of every write appears in one.
fn samples() -> Vec<(&'static str, Value)> {
    let goals = json!(["Launch plan", "Goals"]);
    vec![
        ("text.insert", json!({ "at": 10, "text": "X", "unit": "scalar" })),
        ("text.delete", json!({ "from": 10, "to": 12 })),
        ("text.replace", json!({ "from": 10, "to": 12, "text": "Y", "expected": " L" })),
        ("text.findReplace", json!({ "find": "beta", "replace": "gamma", "occurrence": 1, "regex": false, "caseSensitive": true })),
        ("section.replace", json!({ "heading": goals, "markdown": "- One", "withHeading": false })),
        ("section.replace", json!({ "line": 10, "markdown": "- One" })),
        ("section.replace", json!({ "section": "section:2", "markdown": "- One" })),
        ("section.insert", json!({ "heading": goals, "position": "end", "markdown": "- Two" })),
        ("section.insert", json!({ "line": 15, "position": "start", "markdown": "- Two" })),
        ("section.insert", json!({ "section": "section:2", "position": "end", "markdown": "- Two" })),
        ("section.delete", json!({ "heading": goals })),
        ("section.delete", json!({ "line": 15 })),
        ("section.delete", json!({ "section": "section:3" })),
        ("section.move", json!({ "heading": goals, "toHeading": ["Launch plan", "Timeline"], "position": "after" })),
        ("section.move", json!({ "line": 10, "toLine": 21, "position": "before" })),
        ("section.move", json!({ "section": "section:2", "toSection": "section:4", "position": "after" })),
        ("heading.setLevel", json!({ "heading": goals, "level": 3, "style": "atx" })),
        ("heading.setLevel", json!({ "line": 15, "level": 3 })),
        ("heading.setLevel", json!({ "section": "section:3", "level": 3 })),
        ("block.replace", json!({ "blockId": "book", "markdown": "- [ ] Booked" })),
        ("block.append", json!({ "heading": goals, "markdown": "The end." })),
        ("block.append", json!({ "line": 15, "markdown": "The end." })),
        ("block.append", json!({ "section": "section:3", "markdown": "The end." })),
        ("block.setId", json!({ "line": 12, "id": "ship" })),
        ("block.setId", json!({ "blockId": "draft", "occurrence": 1, "id": "first" })),
        ("task.toggle", json!({ "line": 17, "done": true })),
        ("task.toggle", json!({ "blockId": "book" })),
        ("task.toggle", json!({ "task": "task:1", "done": true })),
        ("property.set", json!({ "key": "status", "value": "review" })),
        ("property.delete", json!({ "key": "status", "occurrence": 1 })),
        ("property.rename", json!({ "key": "status", "to": "state" })),
        ("table.add", json!({ "line": 12, "columns": 2, "rows": 1, "header": ["A", "B"] })),
        ("table.add", json!({ "heading": goals, "position": "end", "columns": 2 })),
        ("table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Alpha" })),
        ("table.insertRow", json!({ "table": 0, "at": 2, "cells": ["3", "C"] })),
        ("table.insertColumn", json!({ "table": 0, "at": 1, "header": "H", "align": "left" })),
        ("table.deleteRow", json!({ "table": 0, "row": 1 })),
        ("table.deleteColumn", json!({ "table": 0, "column": 1 })),
        ("table.moveRow", json!({ "table": 0, "from": 1, "to": 2 })),
        ("table.moveColumn", json!({ "table": 0, "from": 0, "to": 1 })),
        ("table.align", json!({ "table": 0, "column": 1, "align": "center" })),
        ("table.format", json!({ "table": 0 })),
        ("link.insert", json!({ "at": 10, "destination": "Other", "text": "o", "style": "wikilink", "embed": false })),
        ("link.update", json!({ "line": 8, "index": 0, "destination": "Elsewhere" })),
        ("link.retarget", json!({ "from": "timeline", "to": "schedule" })),
        ("comment.add", json!({ "from": 10, "to": 14, "body": "B", "author": "person:a", "name": "A", "created": "2026-11-13T00:00:00Z" })),
        ("comment.add", json!({ "quote": "Draft", "occurrence": 1, "body": "B" })),
        ("comment.add", json!({ "heading": goals, "body": "B" })),
        ("comment.add", json!({ "line": 10, "body": "B" })),
        ("comment.move", json!({ "thread": "t", "quote": "beta", "occurrence": 1 })),
        ("comment.move", json!({ "thread": "t", "from": 3, "to": 5 })),
        ("comment.move", json!({ "thread": "t", "heading": ["Launch plan", "Goals"] })),
        ("comment.move", json!({ "thread": "t", "line": 10 })),
    ]
}

/// Other values a place could take.
fn place_variants(name: &str, value: &Value) -> Vec<Value> {
    match value {
        Value::Number(number) => {
            let n = number.as_u64().unwrap();
            [n + 1, n.saturating_sub(1), n + 5].into_iter().map(|n| json!(n)).collect()
        }
        Value::Array(_) => vec![json!(["Launch plan", "Tasks"]), json!(["Launch plan", "Timeline"]), json!(["Launch plan", "Budget"])],
        Value::String(_) => match name {
            "blockId" => vec![json!("draft"), json!("book")],
            "key" => vec![json!("title"), json!("status")],
            "position" => vec![json!("before"), json!("after"), json!("start"), json!("end")],
            "find" | "quote" => vec![json!("Draft"), json!("Launch"), json!("beta")],
            "from" => vec![json!("Other")],
            // The projection's ids, as a note opened with this text gives them.
            "section" | "toSection" => vec![json!("section:3"), json!("section:4"), json!("section:5")],
            "task" => vec![json!("task:2"), json!("task:3")],
            other => panic!("no variants for the place {other}"),
        },
        other => panic!("no variants for {other}"),
    }
}

/// Other values a value could take.
fn value_variants(value: &Value) -> Vec<Value> {
    match value {
        Value::Bool(flag) => vec![json!(!flag)],
        Value::Number(number) => vec![json!(number.as_u64().unwrap() + 1)],
        Value::String(text) => vec![json!(format!("{text} other")), json!("left"), json!("utf16"), json!("markdown"), json!("")],
        Value::Array(items) => vec![json!([]), json!(items.iter().chain(items.iter()).collect::<Vec<_>>())],
        other => vec![other.clone()],
    }
}

#[test]
fn the_declared_places_are_what_each_handler_reads_to_find_its_place() {
    let mut problems = Vec::new();
    let mut checked = 0;
    for addresses in ADDRESSES.iter().filter(|addresses| !addresses.places.is_empty()) {
        let calls: Vec<Value> = samples().into_iter().filter(|(id, _)| *id == addresses.command).map(|(_, params)| params).collect();
        assert!(!calls.is_empty(), "{} has no sample", addresses.command);
        for place in addresses.places {
            if !calls.iter().any(|params| params.get(*place).is_some()) {
                problems.push(format!("{}: no sample gives the place {place}", addresses.command));
            }
        }
        for params in calls {
            let base = footprint_of(addresses.command, &params, NOTE);
            assert!(base.is_some(), "{} {params} fails on the note", addresses.command);
            let base_reads = reads_of(addresses.command, &params, NOTE);
            for (name, value) in params.as_object().unwrap() {
                checked += 1;
                let with = |other: &Value| {
                    let mut params = params.clone();
                    params[name.as_str()] = other.clone();
                    params
                };
                if addresses.places.contains(&name.as_str()) {
                    let moves = place_variants(name, value).iter().any(|other| footprint_of(addresses.command, &with(other), NOTE) != base);
                    if !moves {
                        problems.push(format!("{}.{name}: declared a place, but no other value moves the write", addresses.command));
                    }
                } else if addresses.targets != Some(name.as_str()) && !addresses.objects.contains(&name.as_str()) {
                    for other in value_variants(value) {
                        let reads = reads_of(addresses.command, &with(&other), NOTE);
                        if reads.is_some() && reads != base_reads {
                            problems.push(format!("{}.{name}: not a place, but {other} moves where the write reads", addresses.command));
                        }
                    }
                }
            }
        }
    }
    println!("{checked} parameters of sample calls checked");
    assert!(problems.is_empty(), "{problems:#?}");
}
