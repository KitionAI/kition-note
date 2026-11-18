//! Premises the scenarios do not show: writes by no place, and values that happen to spell an
//! object's id.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use serde_json::json;

/// A write by no place (`block.append` at the end of the note) after a colleague's change of the
/// body elsewhere applies: its content premise is answered from the body at `basis`, and refused
/// when the colleague wrote at the end too. The text it inserts is the word `note`, the root's
/// object id: it holds nothing by that.
#[tokio::test(flavor = "multi_thread")]
async fn a_write_by_no_place_applies_after_an_unrelated_change() {
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Intro text", "replace": "Introduction" }))]);
    f.run(&basis, step("block.append", json!({ "markdown": "note" }))).await.unwrap();
    f.run(&f.observe().await, step("text.insert", json!({ "at": 10, "text": "note" }))).await.unwrap();
    let basis = f.observe().await;
    f.colleague(&[("block.append", json!({ "markdown": "Theirs." }))]);
    let refused = f.run(&basis, step("block.append", json!({ "markdown": "Mine." }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "changed", "{refused}");
    f.shutdown().await;
}

/// A write by no place (`block.append` at the note's end) holds a content premise, and
/// `context.content` gets the body at `basis` whole, however many states others made since: after
/// nine changes of a property by another agent the write applies, and after one at the end it is
/// refused.
#[tokio::test(flavor = "multi_thread")]
async fn content_at_gets_the_whole_body_after_many_changes() {
    let f = Fixture::new();
    let basis = f.observe().await;
    for round in 0..9 {
        f.call_by(
            "agent-other",
            "run",
            json!({ "steps": [{ "command": "property.set", "params": { "key": "status", "value": format!("draft {round}") } }] }),
        )
        .await;
    }
    f.run(&basis, step("block.append", json!({ "markdown": "Mine." }))).await.unwrap();
    let basis = f.observe().await;
    f.call_by("agent-other", "run", json!({ "steps": [{ "command": "block.append", "params": { "markdown": "Theirs." } }] })).await;
    let refused = f.run(&basis, step("block.append", json!({ "markdown": "Mine again." }))).await.unwrap_err();
    assert_eq!(reasons(&refused), "changed", "{refused}");
    f.shutdown().await;
}

/// A text inserted that is exactly a comment thread's id names nothing: another agent's resolve of
/// the thread does not refuse it.
#[tokio::test(flavor = "multi_thread")]
async fn a_text_spelling_a_thread_id_holds_nothing() {
    let f = Fixture::new();
    let added = f
        .ok(
            "run",
            json!({ "basis": f.observe().await, "steps": [{ "command": "comment.add", "params": { "quote": "Beta build", "body": "Which?" } }] }),
        )
        .await;
    let thread = added["completed"][0]["result"]["thread"].as_str().unwrap().to_owned();
    let basis = revision(&added);
    f.call_by("agent-other", "run", json!({ "basis": basis, "steps": [{ "command": "comment.resolve", "params": { "thread": thread } }] })).await;
    f.run(&basis, step("text.insert", json!({ "at": 10, "text": thread }))).await.unwrap();
    f.shutdown().await;
}

/// Receipt sizes: the note's object model has one object holding the text, so a write's
/// receipt is one change of the root's `body` (an 80-character excerpt and the length), however
/// many headings, list items or matches it touched; a comment's is the thread and its comment.
#[tokio::test(flavor = "multi_thread")]
async fn receipt_bytes() {
    let subsections = |count: usize| (0..count).map(|n| format!("### Part {n}\n\nText {n}.\n")).collect::<Vec<_>>().join("\n");
    let note = format!("# Doc\n\n## Moved\n\n{}\n## Small\n\nOne line.\n\n## Wide\n\n{}\n\n## Last\n\nEnd.\n", subsections(200), "word ".repeat(200));
    let five = format!("# Doc\n\n## Moved\n\n{}\n## Last\n\nEnd.\n", subsections(5));
    let mut sizes = Vec::new();
    for (what, text, command, params) in [
        ("one property", note.clone(), "property.set", json!({ "key": "status", "value": "review" })),
        (
            "one move (a section of one line)",
            note.clone(),
            "section.move",
            json!({ "heading": ["Doc", "Small"], "toHeading": ["Doc", "Last"], "position": "after" }),
        ),
        (
            "a parent with 5 children moved",
            five,
            "section.move",
            json!({ "heading": ["Doc", "Moved"], "toHeading": ["Doc", "Last"], "position": "after" }),
        ),
        (
            "a parent with 200 children moved",
            note.clone(),
            "section.move",
            json!({ "heading": ["Doc", "Moved"], "toHeading": ["Doc", "Last"], "position": "after" }),
        ),
        ("the same edit at 200 places", note.clone(), "text.findReplace", json!({ "find": "word", "replace": "term", "occurrence": "all" })),
        ("a comment", note.clone(), "comment.add", json!({ "quote": "One line.", "body": "Why?" })),
    ] {
        let f = Fixture::with(&text, kition_kits::tools::CallOptions::default());
        let ran = f.ok("run", json!({ "basis": f.observe().await, "steps": [{ "command": command, "params": params }] })).await;
        let changes = &ran["changes"];
        assert!(changes.is_object(), "{ran}");
        sizes.push(format!("{what}: {} bytes", changes.to_string().len()));
        f.shutdown().await;
    }
    println!("receipts:\n{}", sizes.join("\n"));
}

/// A run's later step is checked on the note as the run's earlier steps leave it (kition-sdk
/// premise planning): after a colleague's change elsewhere, a run that adds a task
/// and then toggles a task below it by line applies; the same run after the colleague edited
/// that task is refused `changed`.
#[tokio::test(flavor = "multi_thread")]
async fn a_later_step_is_checked_after_the_earlier_steps() {
    let f = Fixture::new();
    let run = || {
        json!([
            { "command": "section.insert", "params": { "heading": ["Launch plan", "Goals"], "position": "end", "markdown": "- Third goal" } },
            { "command": "task.toggle", "params": { "line": 20, "done": true } },
        ])
    };
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Other notes." }))]);
    f.run(&basis, run()).await.unwrap();
    assert!(f.text().contains("- [x] Book the review meeting ^book"), "{}", f.text());
    let f = Fixture::new();
    let basis = f.observe().await;
    f.colleague(&[("text.findReplace", json!({ "find": "Book the review meeting", "replace": "Book the room" }))]);
    let refused = f.run(&basis, run()).await.unwrap_err();
    assert_eq!(reasons(&refused), "changed", "{refused}");
    f.shutdown().await;
}
