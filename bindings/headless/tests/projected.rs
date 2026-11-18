//! The note's projection in the object model: sections, tasks, tables, links and properties are
//! projected objects of kition-sdk (computed from the body, never stored), so `note_query`,
//! `note_batch` and kit-env reach them; raw batch writes apply only to types that declare a
//! premise contract (the shared comment types), and a raw write of the body is refused.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::path::PathBuf;

use common::*;
use kition_env::{EnvKit, GenerateOptions, Oracle, OracleOptions};
use serde_json::{Value, json};

/// The id `projection.read` gives the object of `kind` read as `label`.
async fn id_where(f: &Fixture, kind: &str, label: &str) -> String {
    let read = f.ok("run", json!({ "steps": [{ "command": "projection.read", "params": { "kinds": [kind] } }] })).await;
    let objects = read["completed"][0]["result"]["objects"].as_array().unwrap().clone();
    objects.iter().find(|object| object["label"] == json!(label)).unwrap_or_else(|| panic!("{read}"))["id"].as_str().unwrap().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn sections_hold_their_tasks_and_tables_in_the_object_model() {
    let f = Fixture::new();
    let tasks = id_where(&f, "section", "Launch plan › Tasks").await;
    let timeline = id_where(&f, "section", "Launch plan › Timeline").await;
    let read = f.ok("query", json!({ "objects": [tasks, timeline, "root"] })).await;
    let objects = read["objects"].as_array().unwrap();
    assert_eq!(objects[0]["fields"]["level"], json!(2), "{read}");
    assert_eq!(objects[0]["fields"]["tasks"].as_array().map(Vec::len), Some(2), "{read}");
    assert_eq!(objects[1]["fields"]["tables"].as_array().map(Vec::len), Some(1), "{read}");
    // Sections nest by heading level: `## Tasks` is among the sections of `# Launch plan`.
    let top = id_where(&f, "section", "Launch plan").await;
    assert_eq!(objects[2]["fields"]["sections"], json!([top]), "{read}");
    let read = f.ok("query", json!({ "objects": [top] })).await;
    assert_eq!(read["objects"][0]["fields"]["sections"].as_array().map(Vec::len), Some(5), "{read}");
    assert!(read["objects"][0]["fields"]["sections"].as_array().unwrap().contains(&json!(tasks)), "{read}");
    // Nothing projected is written into the file.
    assert_eq!(f.text(), NOTE);
    f.shutdown().await;
}

/// A task's box set through the object model runs `task.toggle`; another writer's toggle of the
/// same task since `basis` refuses it (`changed`), a change elsewhere does not.
#[tokio::test(flavor = "multi_thread")]
async fn a_batch_set_of_a_task_box_rests_on_that_box() {
    let f = Fixture::new();
    let book = id_where(&f, "task", "Book the review meeting ^book").await;
    let set = |basis: &str| json!({ "basis": basis, "ops": [{ "op": "set", "target": book, "field": "checked", "value": true }] });
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Other notes." }))]);
    f.call("batch", set(&basis)).await.unwrap();
    assert!(f.text().contains("- [x] Book the review meeting ^book"), "{}", f.text());
    let basis = f.observe().await;
    f.colleague(&[("task.toggle", json!({ "blockId": "book", "done": false }))]);
    let refused = f.call("batch", set(&basis)).await.unwrap_err();
    assert_eq!(reasons(&refused), "changed", "{refused}");
    assert!(f.text().contains("- [ ] Book the review meeting ^book"), "{}", f.text());
    f.shutdown().await;
}

/// Raw writes need their type's premise contract (`batchOps`). The body is written by commands
/// and declares none, so a raw write of it is refused before anything runs. The comment types are
/// kition-sdk's shared ones, which declare theirs, so a raw write of a thread's `resolved` or of
/// its anchor applies.
#[tokio::test(flavor = "multi_thread")]
async fn raw_batch_writes_apply_only_to_types_with_a_premise_contract() {
    let f = Fixture::new();
    let added = f.ok("run", json!({ "steps": [{ "command": "comment.add", "params": { "quote": "Beta build", "body": "Which?" } }] })).await;
    let thread = added["completed"][0]["result"]["thread"].as_str().unwrap().to_owned();
    let refused =
        f.call("batch", json!({ "ops": [{ "op": "text.insert", "target": "root", "field": "body", "index": 0, "text": "x" }] })).await.unwrap_err();
    assert!(refused.to_string().contains("premise contract"), "{refused}");
    assert_eq!(f.text(), NOTE);
    f.ok("batch", json!({ "ops": [{ "op": "set", "target": thread, "field": "resolved", "value": true }] })).await;
    f.ok("batch", json!({ "ops": [{ "op": "set", "target": thread, "field": "anchor", "value": { "from": 0, "to": 1 } }] })).await;
    let read = f.ok("run", json!({ "steps": [{ "command": "comment.threads", "params": {} }] })).await;
    let threads = read["completed"][0]["result"]["threads"].as_array().unwrap().clone();
    let written = threads.iter().find(|candidate| candidate["id"] == json!(thread)).unwrap_or_else(|| panic!("{read}"));
    assert_eq!(written["resolved"], json!(true), "{read}");
    assert_eq!(f.text(), NOTE);
    f.shutdown().await;
}

/// kit-env samples edits of the projection (a task's box, a heading's level, a section deleted)
/// and every task's oracle trace scores 1.
#[tokio::test(flavor = "multi_thread")]
async fn kit_env_makes_tasks_from_the_projection() {
    let f = Fixture::new();
    let kit = EnvKit::new(kit(), Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-note"))));
    let generated =
        kition_env::generate(&kit, &f.workspace, &GenerateOptions { count: 12, seed: 7, inverse_share: 0.0, ..Default::default() }).await.unwrap();
    assert!(generated.skipped.is_empty(), "{:?}", generated.skipped);
    assert!(!generated.tasks.is_empty(), "given up {:?}, excluded {:?}", generated.given_up, generated.excluded);
    let mut kinds = std::collections::BTreeSet::new();
    for task in &generated.tasks {
        let truth: &Value = &task.ground_truth;
        kinds.insert(truth.to_string().contains("\"Task\"") || truth.to_string().contains("\"Section\""));
        let mut oracle = Oracle::new(task, OracleOptions::default());
        let trace = kition_env::trace(&kit, &f.workspace, task, &mut oracle).await.unwrap();
        assert_eq!(trace.reward.reward, 1.0, "{}", trace.to_line());
    }
    assert!(kinds.contains(&true), "{:?}", generated.tasks.iter().map(|task| task.instruction.clone()).collect::<Vec<_>>());
    f.shutdown().await;
}
