//! What each write rests on beyond the text it names, as behaviour: another writer (a colleague's
//! program) really rewrites the file between the agent's observation and its write. For each kind
//! of difference, one colleague change that alters what the write lands on or what its result
//! depends on (the write must be refused) and one that does not (it must apply).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use serde_json::{Value, json};

/// One probe: the colleague's steps, the agent's write, and whether it must be refused.
struct Probe {
    name: &'static str,
    colleague: Vec<(&'static str, Value)>,
    write: (&'static str, Value),
    refused: bool,
}

fn probe(name: &'static str, colleague: Vec<(&'static str, Value)>, write: (&'static str, Value), refused: bool) -> Probe {
    Probe { name, colleague, write, refused }
}

fn find_replace(find: &str, replace: &str) -> (&'static str, Value) {
    ("text.findReplace", json!({ "find": find, "replace": replace }))
}

/// The id `projection.read` gives the section read as `label`.
async fn section(f: &Fixture, label: &str) -> String {
    let read = f.ok("run", json!({ "steps": [{ "command": "projection.read", "params": { "kinds": ["section"] } }] })).await;
    let objects = read["completed"][0]["result"]["objects"].as_array().unwrap().clone();
    objects.iter().find(|object| object["label"] == json!(label)).unwrap()["id"].as_str().unwrap().to_owned()
}

/// Runs every probe; answers the ones whose outcome is not the one wanted.
async fn run(probes: Vec<Probe>) -> Vec<String> {
    let mut wrong = Vec::new();
    for probe in probes {
        let f = Fixture::new();
        let (command, mut params) = probe.write.clone();
        if let Some(label) = params.get("section").and_then(Value::as_str).and_then(|label| label.strip_prefix("label:")) {
            params["section"] = json!(section(&f, label).await);
        }
        let basis = f.observe().await;
        f.colleague(&probe.colleague);
        let outcome = f.run(&basis, step(command, params)).await;
        let refused = matches!(&outcome, Err(error) if code(error) == "conflict");
        let failed = matches!(&outcome, Err(error) if code(error) != "conflict");
        println!(
            "{}: {}",
            probe.name,
            if refused {
                format!("refused {}", reasons(outcome.as_ref().unwrap_err()))
            } else if failed {
                "failed at kit".to_owned()
            } else {
                "applied".to_owned()
            }
        );
        if refused != probe.refused && !(probe.refused && failed) {
            wrong.push(format!("{}: wanted {}, {outcome:?}", probe.name, if probe.refused { "refused" } else { "applied" }));
        }
        f.shutdown().await;
    }
    wrong
}

/// A position counted in UTF-16 units rests on the units before it: a character before it
/// replaced by one of the same count of characters but more units moves it.
#[tokio::test(flavor = "multi_thread")]
async fn a_utf16_position_rests_on_the_units_before_it() {
    let at = NOTE[..NOTE.find("the beta").unwrap()].encode_utf16().count();
    let write = ("text.replace", json!({ "from": at, "to": at + 3, "text": "our", "unit": "utf16", "expected": "the" }));
    let wrong = run(vec![
        probe("utf16, an astral character before", vec![find_replace("Intro text", "\u{1f600}ntro text")], write.clone(), true),
        probe("utf16, an edit after", vec![find_replace("Some closing notes.", "Other notes.")], write, false),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `text.findReplace` by occurrence rests on the matches before it; `all` on every match.
#[tokio::test(flavor = "multi_thread")]
async fn a_find_and_replace_rests_on_the_matches_it_counts() {
    let first = ("text.findReplace", json!({ "find": "beta", "replace": "pilot" }));
    let all = ("text.findReplace", json!({ "find": "beta", "replace": "pilot", "occurrence": "all" }));
    let wrong = run(vec![
        probe("occurrence 1, a match added before", vec![find_replace("Intro text", "Intro beta text")], first.clone(), true),
        probe("occurrence 1, a match added after", vec![find_replace("Some closing notes.", "Some beta notes.")], first, false),
        probe("all, a match added", vec![find_replace("Some closing notes.", "Some beta notes.")], all.clone(), true),
        probe("all, no match added", vec![find_replace("Some closing notes.", "Other notes.")], all, false),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A section's extent rests on the level of the heading that ends it: demoted, the section takes in
/// the next one.
#[tokio::test(flavor = "multi_thread")]
async fn a_section_rests_on_the_heading_that_ends_it() {
    let demote_tasks = vec![find_replace("## Tasks", "### Tasks")];
    let demote_budget = vec![find_replace("## Budget", "### Budget")];
    let elsewhere = vec![find_replace("Some closing notes.", "Other notes.")];
    let goals = json!(["Launch plan", "Goals"]);
    let wrong = run(vec![
        probe(
            "section.replace by heading, next demoted",
            demote_tasks.clone(),
            ("section.replace", json!({ "heading": goals, "markdown": "- One" })),
            true,
        ),
        probe(
            "section.replace by heading, elsewhere",
            elsewhere.clone(),
            ("section.replace", json!({ "heading": goals, "markdown": "- One" })),
            false,
        ),
        probe("section.replace by line, next demoted", demote_tasks.clone(), ("section.replace", json!({ "line": 10, "markdown": "- One" })), true),
        probe(
            "section.replace by id, next demoted",
            demote_tasks.clone(),
            ("section.replace", json!({ "section": "label:Launch plan › Goals", "markdown": "- One" })),
            true,
        ),
        probe(
            "section.replace by id, elsewhere",
            elsewhere.clone(),
            ("section.replace", json!({ "section": "label:Launch plan › Goals", "markdown": "- One" })),
            false,
        ),
        probe("section.delete by line, next demoted", demote_budget.clone(), ("section.delete", json!({ "line": 20 })), true),
        probe("section.delete by id, next demoted", demote_budget, ("section.delete", json!({ "section": "label:Launch plan › Timeline" })), true),
        probe("section.delete by line, elsewhere", elsewhere.clone(), ("section.delete", json!({ "line": 20 })), false),
        probe(
            "section.insert after the section by heading, next demoted",
            demote_tasks.clone(),
            ("section.insert", json!({ "heading": goals, "position": "after", "markdown": "- Two" })),
            true,
        ),
        // (`end` is the end of the section's own text, before its first subsection: the same place.)
        probe(
            "section.insert at the end of its own text by heading, next demoted",
            demote_tasks,
            ("section.insert", json!({ "heading": goals, "position": "end", "markdown": "- Two" })),
            false,
        ),
        probe(
            "block.append by heading, a subsection added at its end",
            vec![find_replace("| 2 | Feedback round |\n", "| 2 | Feedback round |\n\n### Later\n\nSoon.\n")],
            ("block.append", json!({ "heading": ["Launch plan", "Timeline"], "markdown": "Done." })),
            true,
        ),
        probe(
            "block.append by heading, elsewhere",
            elsewhere,
            ("block.append", json!({ "heading": ["Launch plan", "Timeline"], "markdown": "Done." })),
            false,
        ),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Links named by their line and index, or by their target, rest on the links counted.
#[tokio::test(flavor = "multi_thread")]
async fn links_rest_on_the_links_they_count() {
    let update = ("link.update", json!({ "line": 8, "index": 0, "destination": "schedule" }));
    let retarget = ("links.retarget", json!({ "from": "timeline", "to": "schedule" }));
    let wrong = run(vec![
        probe("link.update, a link added before on the line", vec![find_replace("Intro text with a", "Intro [[plan]] with a")], update.clone(), true),
        probe("link.update, another line edited", vec![find_replace("Some closing notes.", "Other notes.")], update, false),
        probe("links.retarget, a link to it added", vec![find_replace("Some closing notes.", "See [[timeline]].")], retarget.clone(), true),
        probe("links.retarget, text edited", vec![find_replace("Some closing notes.", "Other notes.")], retarget, false),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A comment anchored by a quote's occurrence rests on the matches before it.
#[tokio::test(flavor = "multi_thread")]
async fn a_quote_rests_on_the_matches_it_counts() {
    let add = ("comment.add", json!({ "quote": "beta", "body": "Which?" }));
    let wrong = run(vec![
        probe("comment.add, a match added before", vec![find_replace("Intro text", "Intro beta text")], add.clone(), true),
        probe("comment.add, an edit elsewhere", vec![find_replace("Some closing notes.", "Other notes.")], add, false),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A write that assigns a value another writer changed since the agent's observation is refused
/// (lost update), even when it does not read the old value.
#[tokio::test(flavor = "multi_thread")]
async fn a_value_another_changed_is_not_assigned_again() {
    let wrong = run(vec![
        probe(
            "property.set, the same key set",
            vec![("property.set", json!({ "key": "status", "value": "final" }))],
            ("property.set", json!({ "key": "status", "value": "review" })),
            true,
        ),
        probe(
            "property.set, another key set",
            vec![("property.set", json!({ "key": "title", "value": "Plan" }))],
            ("property.set", json!({ "key": "status", "value": "review" })),
            false,
        ),
        probe(
            "task.toggle, the same box set",
            vec![("task.toggle", json!({ "line": 18, "done": true }))],
            ("task.toggle", json!({ "line": 18, "done": true })),
            true,
        ),
        probe(
            "heading.setLevel, the same level set",
            vec![("heading.setLevel", json!({ "line": 10, "level": 3 }))],
            ("heading.setLevel", json!({ "line": 10, "level": 3 })),
            true,
        ),
        probe(
            "heading.setLevel, another heading's level set",
            vec![("heading.setLevel", json!({ "line": 15, "level": 3 }))],
            ("heading.setLevel", json!({ "line": 10, "level": 3 })),
            false,
        ),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A write by a key of the front matter rests on the front matter it is in: when another writer
/// removes that front matter, the write is refused.
// Find and replace act on the body only. The front matter is now
// removed with property commands (every key deleted) instead of `text.findReplace` on its fence.
#[tokio::test(flavor = "multi_thread")]
async fn a_property_rests_on_the_front_matter_it_is_in() {
    let set = ("property.set", json!({ "key": "status", "value": "review" }));
    let removed = vec![("property.delete", json!({ "key": "title" })), ("property.delete", json!({ "key": "status" }))];
    let wrong = run(vec![
        probe("property.set, the front matter removed", removed, set.clone(), true),
        probe("property.set, the body edited", vec![find_replace("Some closing notes.", "Other notes.")], set, false),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// What a write holds beyond what it reads (held more): another writer's change of text the write
/// neither reads nor replaces lets it apply.
#[tokio::test(flavor = "multi_thread")]
async fn a_change_of_what_a_write_does_not_read_lets_it_apply() {
    let wrong = run(vec![
        probe(
            "heading.setLevel by id, the heading renamed",
            vec![find_replace("## Goals", "## Aims")],
            ("heading.setLevel", json!({ "section": "label:Launch plan › Goals", "level": 3 })),
            false,
        ),
        probe(
            "block.setId by block id, the task's text edited",
            vec![find_replace("Book the review meeting", "Book the room")],
            ("block.setId", json!({ "blockId": "book", "id": "booking" })),
            false,
        ),
        probe(
            "table.deleteRow, another row edited",
            vec![find_replace("| 2 | Feedback round |", "| 2 | Feedback |")],
            ("table.deleteRow", json!({ "table": 0, "row": 1 })),
            false,
        ),
        probe(
            "table.moveRow, the header edited",
            vec![find_replace("| Week | Milestone |", "| Week | Goal |")],
            ("table.moveRow", json!({ "table": 0, "from": 1, "to": 2 })),
            false,
        ),
        probe(
            "table.align, a body cell edited",
            vec![find_replace("| 1 | Beta build |", "| 1 | Beta |")],
            ("table.align", json!({ "table": 0, "column": 1, "align": "right" })),
            false,
        ),
    ])
    .await;
    assert!(wrong.is_empty(), "{wrong:#?}");
}
