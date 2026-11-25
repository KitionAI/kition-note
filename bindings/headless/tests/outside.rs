//! The premise guard when the `.md` file itself changes: a colleague's program (this kit's engine,
//! its own instance) opens the note, edits it and writes it back between the agent's observation
//! (`note_inspect`, its `revision`) and the agent's write with that `basis`, so the server sees
//! only a changed file and reads it again.
//!
//! The note's addresses (`bindings/wasm/src/premise.rs`): the root `note` holds the whole text;
//! a write names its place in it by a position, a line, a table's number or an occurrence (refused
//! `relocated` when others inserted or deleted text before it, `changed` when they edited what it
//! reads or replaces, applied when they changed only what follows or another part), or by a name,
//! a heading path, a property key or a block id (applied wherever the name finds the same text,
//! refused when it finds other text or none). Comment threads are objects with their own ids.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kits::tools::{CallOptions, Guard};
use serde_json::{Value, json};

fn toggle(line: usize) -> Value {
    step("task.toggle", json!({ "line": line, "done": true }))
}

/// s1: the colleague edits another section; the agent's writes by line, by name and by table apply,
/// and `others` names what changed: the root's body and the section it is in.
#[tokio::test(flavor = "multi_thread")]
async fn s1_an_unrelated_change_applies_and_others_names_only_what_changed() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Closing, shorter." }))]);
    let ran = f.run(&basis, toggle(17)).await.unwrap();
    let others = &ran["others"];
    assert_eq!(others["by"], json!(["outside"]), "{ran}");
    let changed: Vec<&Value> = others["changed"].as_array().unwrap().iter().map(|change| &change["id"]).collect();
    assert_eq!(changed, [&json!("note"), &json!("section:6")], "{others}");
    assert_eq!(others["changed"][0]["fields"], json!(["body"]), "{others}");
    for list in ["created", "deleted", "moved"] {
        assert!(others[list].as_array().is_none_or(Vec::is_empty), "{others}");
    }
    println!("s1: others for one unrelated edit: {} bytes", others.to_string().len());
    let basis = revision(&ran);
    f.colleague(&[("text.insert", json!({ "at": 0, "text": "" })), ("property.set", json!({ "key": "owner", "value": "Ana" }))]);
    // Before the place, but by name: the section is found with the same text.
    f.run(&basis, step("section.insert", json!({ "heading": ["Launch plan", "Notes"], "position": "end", "markdown": "Mine." }))).await.unwrap();
    let text = f.text();
    assert!(
        text.contains("- [x] Draft the announcement") && text.contains("Closing, shorter.") && text.contains("owner: Ana") && text.contains("Mine."),
        "{text}"
    );
    f.shutdown().await;
}

/// s2: the colleague edits the very text the agent's write reads: refused `changed`, nothing
/// written; the retry after another observation applies.
#[tokio::test(flavor = "multi_thread")]
async fn s2_the_same_text_is_refused_changed_and_the_retry_applies() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("table.setCell", json!({ "table": 0, "row": 2, "column": 1, "text": "Theirs" }))]);
    let before = f.text();
    let cell = step("table.setCell", json!({ "table": 0, "row": 2, "column": 1, "text": "Mine" }));
    let refused = f.run(&basis, cell.clone()).await.unwrap_err();
    assert_eq!((code(&refused), reasons(&refused)), ("conflict".to_owned(), "changed".to_owned()), "{refused}");
    assert_eq!(f.text(), before, "nothing written");
    // Another cell of the same table: the table is what the write reads, so it is refused too.
    let basis = f.observe().await;
    f.colleague(&[("table.setCell", json!({ "table": 0, "row": 1, "column": 1, "text": "Theirs too" }))]);
    assert_eq!(reasons(&f.run(&basis, cell.clone()).await.unwrap_err()), "changed");
    let basis = f.observe().await;
    f.run(&basis, cell).await.unwrap();
    assert!(f.text().contains("| 2 | Mine |"), "{}", f.text());
    // A task's own line.
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Draft the announcement", "replace": "Draft the post" }))]);
    assert_eq!(reasons(&f.run(&basis, toggle(17)).await.unwrap_err()), "changed");
    f.shutdown().await;
}

/// s3: the colleague deletes the section the agent's write names: refused `relocated` (the name
/// finds nothing: no object was deleted, the root still holds the text).
#[tokio::test(flavor = "multi_thread")]
async fn s3_a_deleted_target_is_refused() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("section.delete", json!({ "heading": ["Launch plan", "Budget"] }))]);
    let before = f.text();
    let refused = f
        .run(&basis, step("section.replace", json!({ "heading": ["Launch plan", "Budget"], "markdown": "| Item | Cost |\n| --- | --- |" })))
        .await
        .unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    assert_eq!(f.text(), before);
    f.shutdown().await;
}

/// s3, the thread: another agent deletes the comment thread the agent replies to through a raw
/// batch write (the shared comment types declare its contract), and the reply with the agent's
/// earlier `basis` is refused `deleted`.
#[tokio::test(flavor = "multi_thread")]
async fn s3_a_thread_deleted_by_another_agent_refuses_a_reply() {
    let f = Fixture::new();
    let added = f.ok("run", json!({ "steps": [{ "command": "comment.add", "params": { "quote": "Beta build", "body": "Which build?" } }] })).await;
    let thread = added["completed"][0]["result"]["thread"].as_str().unwrap().to_owned();
    let basis = revision(&added);
    f.call_by("agent-other", "batch", json!({ "ops": [{ "op": "delete", "target": thread }] })).await;
    let refused = f.run(&basis, step("comment.reply", json!({ "thread": thread, "body": "Build 2." }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "deleted", "{refused}");
    f.shutdown().await;
}

/// s4: a table inserted before the table a write numbers refuses it (`relocated`); one inserted
/// after it does not. The same for a line: text with a line break above it refuses, below it
/// applies.
#[tokio::test(flavor = "multi_thread")]
async fn s4_an_insertion_before_an_index_refuses_and_after_it_applies() {
    let f = Fixture::new();
    let cell = step("table.setCell", json!({ "table": 1, "row": 1, "column": 1, "text": "120" }));
    let basis = f.observe().await;
    f.colleague(&[("table.add", json!({ "heading": ["Launch plan", "Goals"], "position": "end", "columns": 2, "header": ["A", "B"] }))]);
    assert_eq!(reasons(&f.run(&basis, cell.clone()).await.unwrap_err()), "relocated");
    let basis = f.observe().await;
    f.colleague(&[("table.add", json!({ "columns": 2, "header": ["C", "D"] }))]);
    // The colleague's first table made the Budget table the third.
    let cell = step("table.setCell", json!({ "table": 2, "row": 1, "column": 1, "text": "120" }));
    f.run(&basis, cell).await.unwrap();
    assert!(f.text().contains("| Venue | 120 |"), "{}", f.text());
    let basis = f.observe().await;
    let line = f.text().lines().position(|line| line.contains("Book the review")).unwrap() + 1;
    f.colleague(&[("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "before", "markdown": "A new paragraph." }))]);
    assert_eq!(reasons(&f.run(&basis, toggle(line)).await.unwrap_err()), "relocated");
    let basis = f.observe().await;
    let line = f.text().lines().position(|line| line.contains("Book the review")).unwrap() + 1;
    f.colleague(&[("section.insert", json!({ "heading": ["Launch plan", "Notes"], "position": "end", "markdown": "A last paragraph." }))]);
    f.run(&basis, toggle(line)).await.unwrap();
    assert!(f.text().contains("- [x] Book the review meeting"), "{}", f.text());
    f.shutdown().await;
}

/// s5: the colleague moves the section the agent's write is in to the end. A write by position
/// (an offset inside it, its line) is refused; the same write by its heading path applies where
/// the section is now.
#[tokio::test(flavor = "multi_thread")]
async fn s5_a_moved_target_refuses_a_write_by_position_and_not_by_name() {
    let f = Fixture::new();
    let at = NOTE.find("Ship the beta").map(|byte| NOTE[..byte].chars().count()).unwrap();
    let basis = f.observe().await;
    f.colleague(&[("section.move", json!({ "heading": ["Launch plan", "Goals"], "toHeading": ["Launch plan", "Notes"], "position": "after" }))]);
    let before = f.text();
    let refused = f.run(&basis, step("text.insert", json!({ "at": at, "text": "Really " }))).await.unwrap_err();
    assert!(matches!(reasons(&refused).as_str(), "relocated" | "changed"), "{refused}");
    // Line 10 is the Tasks heading now: its text changed in place.
    let refused = f.run(&basis, step("section.insert", json!({ "line": 10, "position": "end", "markdown": "- Mine" }))).await.unwrap_err();
    assert!(matches!(reasons(&refused).as_str(), "relocated" | "changed"), "{refused}");
    assert_eq!(f.text(), before);
    f.run(&basis, step("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "end", "markdown": "- Mine" }))).await.unwrap();
    let text = f.text();
    assert!(text.find("- Mine").unwrap() > text.find("Some closing notes.").unwrap(), "{text}");
    f.shutdown().await;
}

/// s6: the section holding a task (its parent in the outline) moves: a write by the task's block
/// id applies to it where it is now; a write by its line is refused `relocated`.
#[tokio::test(flavor = "multi_thread")]
async fn s6_a_moved_parent_refuses_by_line_and_not_by_block_id() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("section.move", json!({ "heading": ["Launch plan", "Tasks"], "toHeading": ["Launch plan", "Notes"], "position": "after" }))]);
    assert_eq!(reasons(&f.run(&basis, toggle(18)).await.unwrap_err()), "relocated");
    f.run(&basis, step("task.toggle", json!({ "blockId": "book", "done": true }))).await.unwrap();
    assert!(f.text().contains("- [x] Book the review meeting ^book"), "{}", f.text());
    f.shutdown().await;
}

/// s7: the colleague swaps the names of two sections: a write by one name is refused
/// `relocated`, since the name finds the other section's text.
#[tokio::test(flavor = "multi_thread")]
async fn s7_a_name_swap_refuses_a_write_by_name() {
    let f = Fixture::new();
    let basis = f.observe().await;
    let swapped = NOTE.replace("## Goals", "## Swap").replace("## Notes", "## Goals").replace("## Swap", "## Notes");
    colleague_writes(&f.file(), &swapped);
    let refused = f
        .run(&basis, step("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "end", "markdown": "- Mine" })))
        .await
        .unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    assert_eq!(f.text(), swapped);
    f.shutdown().await;
}

/// s8: a rename away from the name a write uses refuses it, for a section and a property; a
/// property next to it renamed does not.
#[tokio::test(flavor = "multi_thread")]
async fn s8_a_rename_away_refuses_a_write_by_name() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "## Notes", "replace": "## Closing" }))]);
    let refused = f.run(&basis, step("section.delete", json!({ "heading": ["Launch plan", "Notes"] }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    let basis = f.observe().await;
    f.colleague(&[("property.rename", json!({ "key": "status", "to": "state" }))]);
    let refused = f.run(&basis, step("property.set", json!({ "key": "status", "value": "review" }))).await.unwrap_err();
    assert!(matches!(reasons(&refused).as_str(), "relocated" | "changed"), "{refused}");
    let basis = f.observe().await;
    f.colleague(&[("property.rename", json!({ "key": "state", "to": "phase" }))]);
    f.run(&basis, step("property.set", json!({ "key": "title", "value": "Launch" }))).await.unwrap();
    assert!(f.text().contains("title: Launch\n"), "{}", f.text());
    f.shutdown().await;
}

/// s9: the guard's three modes against one unrelated change: `premise` applies, `epoch`
/// refuses, `off` applies and still says what others changed.
#[tokio::test(flavor = "multi_thread")]
async fn s9_epoch_premise_and_off() {
    for (guard, applies) in [(Guard::Premise, true), (Guard::Epoch, false), (Guard::Off, true)] {
        let f = Fixture::with(NOTE, CallOptions { guard, ..CallOptions::default() });
        let basis = f.observe().await;
        f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Closing." }))]);
        let answer = f.run(&basis, toggle(17)).await;
        assert_eq!(answer.is_ok(), applies, "{guard:?}: {answer:?}");
        let answer = answer.unwrap_or_else(|refused| refused);
        assert_eq!(answer["others"]["changed"][0]["id"], json!("note"), "{guard:?}: {answer}");
        f.shutdown().await;
    }
}

/// s10: the same with another agent of the same server writing (no file reread): text inserted
/// before an offset refuses the agent's write at it, text after it does not.
#[tokio::test(flavor = "multi_thread")]
async fn s10_another_agent_before_and_after_an_offset() {
    let f = Fixture::new();
    let at = NOTE.find("Feedback round").map(|byte| NOTE[..byte].chars().count()).unwrap();
    let write = step("text.replace", json!({ "from": at, "to": at + 8, "text": "Review", "expected": "Feedback" }));
    let basis = f.observe().await;
    f.call_by("agent-other", "run", json!({ "steps": [{ "command": "text.insert", "params": { "at": 0, "text": "" } }, { "command": "property.set", "params": { "key": "owner", "value": "Ana" } }] })).await;
    let refused = f.run(&basis, write.clone()).await.unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    assert!(refused["others"]["by"].as_array().unwrap().iter().any(|by| by == "agent-other"), "{refused}");
    let at = f.text().find("Feedback round").map(|byte| f.text()[..byte].chars().count()).unwrap();
    let write = step("text.replace", json!({ "from": at, "to": at + 8, "text": "Review", "expected": "Feedback" }));
    let basis = f.observe().await;
    f.call_by("agent-other", "run", json!({ "steps": [{ "command": "section.insert", "params": { "heading": ["Launch plan", "Notes"], "position": "end", "markdown": "Theirs." } }] })).await;
    f.run(&basis, write).await.unwrap();
    assert!(f.text().contains("| 2 | Review round |") && f.text().contains("Theirs."), "{}", f.text());
    f.shutdown().await;
}

/// Repeated unrelated changes: each round a colleague's edit of the Notes section and the agent's
/// write in Tasks with the basis before it. `premise` must refuse none, `epoch` all.
#[tokio::test(flavor = "multi_thread")]
async fn unrelated_rounds_premise_refuses_none_epoch_all() {
    for (guard, expected) in [(Guard::Premise, 0), (Guard::Epoch, 50)] {
        let f = Fixture::with(NOTE, CallOptions { guard, ..CallOptions::default() });
        let mut refused = 0;
        for round in 0..50 {
            let basis = f.observe().await;
            f.colleague(&[(
                "section.insert",
                json!({ "heading": ["Launch plan", "Notes"], "position": "end", "markdown": format!("Line {round}.") }),
            )]);
            let done = round % 2 == 0;
            if f.run(&basis, step("task.toggle", json!({ "line": 17, "done": done }))).await.is_err() {
                refused += 1;
            }
        }
        println!("unrelated rounds, {guard:?}: refused {refused}/50");
        assert_eq!(refused, expected, "{guard:?}");
        f.shutdown().await;
    }
}

/// The projection's ids as `projection.read` answers them now: `(label, id)` of `kind`.
async fn ids(f: &Fixture, kind: &str) -> Vec<(String, String)> {
    let read = f.ok("run", json!({ "steps": [{ "command": "projection.read", "params": { "kinds": [kind] } }] })).await;
    read["completed"][0]["result"]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|object| (object["label"].as_str().unwrap().to_owned(), object["id"].as_str().unwrap().to_owned()))
        .collect()
}

async fn id_of(f: &Fixture, kind: &str, label: &str) -> String {
    ids(f, kind).await.into_iter().find(|(found, _)| found == label).unwrap_or_else(|| panic!("no {kind} {label:?}")).1
}

/// What `others` says the root's change carries (`carries.what`).
fn carried(answer: &Value) -> String {
    answer["others"]["changed"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|change| change["id"] == "note")
        .and_then(|change| change["carries"]["what"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The projected objects a change set lists in `list` (`changed`, `deleted`, ...): `(id, fields)`.
fn listed(changes: &Value, list: &str) -> Vec<(String, Vec<String>)> {
    changes[list]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|change| change["id"] != "note")
        .map(|change| {
            let fields = change["fields"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
            (change["id"].as_str().unwrap_or_default().to_owned(), fields)
        })
        .collect()
}

/// s11: the colleague edits section A (Notes); `others` names section A and
/// nothing else, and the agent's write to section B (Tasks) by its id applies.
#[tokio::test(flavor = "multi_thread")]
async fn s11_others_name_the_section_a_colleague_edited_and_a_write_by_id_elsewhere_applies() {
    let f = Fixture::new();
    let notes = id_of(&f, "section", "Launch plan › Notes").await;
    let tasks = id_of(&f, "section", "Launch plan › Tasks").await;
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Closing, shorter." }))]);
    let ran = f.run(&basis, step("section.insert", json!({ "section": tasks, "position": "end", "markdown": "- [ ] Mine" }))).await.unwrap();
    // (The section is an object of the object model; its own text is its `body`.)
    assert_eq!(listed(&ran["others"], "changed"), [(notes.clone(), vec!["body".to_owned()])], "{}", ran["others"]);
    assert_eq!(ran["others"]["changed"].as_array().unwrap().len(), 2, "{}", ran["others"]);
    // The receipt names the agent's own section.
    assert!(listed(&ran["changes"], "changed").iter().any(|(id, fields)| *id == tasks && fields.contains(&"body".to_owned())), "{}", ran["changes"]);
    assert!(f.text().contains("- [ ] Mine") && f.text().contains("Closing, shorter."), "{}", f.text());
    f.shutdown().await;
}

/// s12: a section renamed by the colleague keeps its id: a write naming it by id applies
/// where it is now, while the same write by its old heading path is refused `relocated`.
#[tokio::test(flavor = "multi_thread")]
async fn s12_a_renamed_section_held_by_id_applies_and_by_its_old_path_is_relocated() {
    let f = Fixture::new();
    let notes = id_of(&f, "section", "Launch plan › Notes").await;
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "## Notes", "replace": "## Closing" }))]);
    let by_path = step("section.insert", json!({ "heading": ["Launch plan", "Notes"], "position": "end", "markdown": "Mine." }));
    let refused = f.run(&basis, by_path).await.unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    let ran = f.run(&basis, step("section.insert", json!({ "section": notes, "position": "end", "markdown": "Mine." }))).await.unwrap();
    assert_eq!(listed(&ran["others"], "changed"), [(notes.clone(), vec!["heading".to_owned(), "path".to_owned()])], "{}", ran["others"]);
    assert!(f.text().ends_with("## Closing\n\nSome closing notes.\n\nMine.\n"), "{}", f.text());
    assert_eq!(id_of(&f, "section", "Launch plan › Closing").await, notes);
    // A move keeps it too.
    let basis = f.observe().await;
    f.colleague(&[("section.move", json!({ "heading": ["Launch plan", "Closing"], "toHeading": ["Launch plan", "Goals"], "position": "before" }))]);
    f.run(&basis, step("section.insert", json!({ "section": notes, "position": "end", "markdown": "Moved too." }))).await.unwrap();
    let text = f.text();
    assert!(text.find("Moved too.").unwrap() < text.find("## Goals").unwrap(), "{text}");
    // An edit of what it replaces refuses it, by id as by name.
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Their notes." }))]);
    let refused = f.run(&basis, step("section.replace", json!({ "section": notes, "markdown": "Mine only." }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "changed", "{refused}");
    f.shutdown().await;
}

/// s13: the colleague deletes the section a write names by id.
/// `others` names it deleted, and the write is refused `deleted`: the place's coverage answer
/// says its object is gone (`Covered.gone`).
#[tokio::test(flavor = "multi_thread")]
async fn s13_a_deleted_section_held_by_id_is_refused_deleted() {
    let f = Fixture::new();
    let budget = id_of(&f, "section", "Launch plan › Budget").await;
    let budget_table = id_of(&f, "table", "Item | Cost").await;
    let basis = f.observe().await;
    f.colleague(&[("section.delete", json!({ "heading": ["Launch plan", "Budget"] }))]);
    let refused = f.run(&basis, step("section.replace", json!({ "section": budget, "markdown": "Mine." }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "deleted", "{refused}");
    let deleted: Vec<String> = listed(&refused["others"], "deleted").into_iter().map(|(id, _)| id).collect();
    assert_eq!(deleted, [budget.clone(), budget_table.clone()], "{}", refused["others"]);
    // After observing again the id names nothing: the kit says so.
    let failed = f.run(&f.observe().await, step("section.replace", json!({ "section": budget, "markdown": "Mine." }))).await.unwrap_err();
    assert!(failed.to_string().contains(&format!("the note has no {budget} now")), "{failed}");
    f.shutdown().await;
}

/// The comment threads as `comment.threads` answers them now.
async fn threads_now(f: &Fixture) -> Vec<Value> {
    let read = f.ok("run", json!({ "steps": [{ "command": "comment.threads", "params": {} }] })).await;
    read["completed"][0]["result"]["threads"].as_array().unwrap().clone()
}

/// s14: comment threads are not in the `.md`; when the colleague's program changes the
/// file the server reads it again, and the thread survives with its id, found again by the text it
/// quotes. When the colleague deletes that text, the thread is kept (`gone`, the product's rule for
/// an edited-away anchor) and `others` tells it on the root's change, which carries it orphaned.
#[tokio::test(flavor = "multi_thread")]
async fn s14_a_thread_survives_a_change_of_the_file_and_an_orphan_is_told() {
    let f = Fixture::new();
    let added = f.ok("run", json!({ "steps": [{ "command": "comment.add", "params": { "quote": "Beta build", "body": "Which build?" } }] })).await;
    let thread = added["completed"][0]["result"]["thread"].as_str().unwrap().to_owned();
    let basis = revision(&added);
    f.colleague(&[("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "before", "markdown": "A new paragraph." }))]);
    let ran = f.run(&basis, step("comment.reply", json!({ "thread": thread, "body": "Build 2." }))).await.unwrap();
    assert!(ran["others"]["changed"].as_array().unwrap().iter().any(|change| change["id"] == "note"), "{}", ran["others"]);
    let threads = threads_now(&f).await;
    assert_eq!(threads.len(), 1, "{threads:?}");
    assert_eq!(threads[0]["id"], json!(thread));
    assert_eq!(threads[0]["comments"].as_array().unwrap().len(), 2);
    let text = f.text();
    let at = text[..text.find("Beta build").unwrap()].chars().count();
    assert_eq!((threads[0]["from"].clone(), threads[0]["gone"].clone()), (json!(at), Value::Null), "{threads:?}");
    // The quoted text deleted: kept, orphaned, and told.
    let basis = revision(&ran);
    f.colleague(&[("table.deleteRow", json!({ "table": 0, "row": 1 }))]);
    let ran = f.run(&basis, step("comment.resolve", json!({ "thread": thread }))).await.unwrap();
    // (kition-sdk compares what the root holds when a file is read again, and threads are beside
    // it: the root's change tells the thread.)
    let what = carried(&ran);
    assert!(what.contains(&format!("comment thread {thread} orphaned: the text it quotes, \"Beta build\", is gone")), "{}", ran["others"]);
    let threads = threads_now(&f).await;
    assert_eq!(
        (threads[0]["id"].clone(), threads[0]["gone"].clone(), threads[0]["resolved"].clone()),
        (json!(thread), json!(true), json!(true)),
        "{threads:?}"
    );
    f.shutdown().await;
}

/// s15: a task added right below the task a write names by line leaves that line as it
/// was: the write applies. One added right above it moves the line: refused `relocated`, not
/// `changed`.
#[tokio::test(flavor = "multi_thread")]
async fn s15_a_line_added_right_below_applies_and_right_above_is_relocated() {
    let f = Fixture::new();
    let end_of = |text: &str, line: &str| {
        let byte = text.find(line).unwrap() + line.len();
        text[..byte].chars().count()
    };
    let basis = f.observe().await;
    let at = end_of(&f.text(), "- [ ] Book the review meeting ^book");
    f.colleague(&[("text.insert", json!({ "at": at, "text": "\n- [ ] Theirs below" }))]);
    f.run(&basis, toggle(18)).await.unwrap();
    assert!(f.text().contains("- [x] Book the review meeting ^book\n- [ ] Theirs below"), "{}", f.text());
    let basis = f.observe().await;
    let at = end_of(&f.text(), "- [ ] Draft the announcement ^draft");
    f.colleague(&[("text.insert", json!({ "at": at, "text": "\n- [ ] Theirs above" }))]);
    let refused = f.run(&basis, toggle(18)).await.unwrap_err();
    assert_eq!(reasons(&refused), "relocated", "{refused}");
    f.shutdown().await;
}
