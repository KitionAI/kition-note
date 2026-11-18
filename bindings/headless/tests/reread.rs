//! An agent's writes, then a colleague's change of the `.md`: the server reads the file again into
//! the session the agent wrote in (kition-sdk's reread contract), so the agent's next write applies
//! and its undo still takes back only its own steps, leaving the colleague's change.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn an_outside_change_after_an_agents_write_keeps_the_agents_undo() {
    let f = Fixture::new();
    let basis = f.observe().await;
    let ran = f.run(&basis, step("task.toggle", json!({ "line": 17, "done": true }))).await.unwrap();
    f.colleague(&[("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Closing, shorter." }))]);
    let ran = f.run(&revision(&ran), step("task.toggle", json!({ "line": 18, "done": true }))).await.unwrap();
    let text = f.text();
    assert!(
        text.contains("- [x] Draft the announcement") && text.contains("- [x] Book the review meeting") && text.contains("Closing, shorter."),
        "{text}"
    );
    let ran = f.run(&revision(&ran), step("edit.undo", json!({}))).await.unwrap();
    let text = f.text();
    assert!(
        text.contains("- [x] Draft the announcement") && text.contains("- [ ] Book the review meeting") && text.contains("Closing, shorter."),
        "{text}"
    );
    f.run(&revision(&ran), step("edit.undo", json!({}))).await.unwrap();
    let text = f.text();
    assert!(text.contains("- [ ] Draft the announcement") && text.contains("Closing, shorter."), "the colleague's change stays: {text}");
    f.shutdown().await;
}
