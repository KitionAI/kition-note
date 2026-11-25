//! The premise guard over many pairs of a colleague's change and an agent's write, picked by a
//! seeded generator (`SEED`, `ROUNDS`) and once over every pair. Each round starts from the same
//! note on disk; a comment thread is added; the agent observes; a colleague's program makes one of
//! ten kinds of change to the file; the agent makes one of five writes with the observed `basis`.
//! Each write form has a rule for when it must be refused:
//!
//! - W1 by id, `comment.resolve {thread}`: refused when the thread is no longer in the document the
//!   server holds after the colleague's change (read after the round with `comment.threads`,
//!   without `basis`).
//! - W2 by index, `table.setCell {table: 0, row: 2, column: 1}` (the "Feedback round" cell):
//!   refused for K4 (a table inserted before it) and K10 (that cell edited); applied for any other.
//! - W3 by name, `section.insert` at the end of the section `Launch plan / Tasks`: refused for K6
//!   (the section renamed); applied for any other (K5 edits a task inside it, not where it inserts).
//! - W4 by point, `text.replace` of the 8 characters at the offset where "Feedback round" starts:
//!   refused when the file now holds other characters than "Feedback round" from that offset (text
//!   inserted or deleted before it with another length, or the cell edited).
//! - W5 structural, `section.move` of `Tasks` after `Timeline`: refused for K6 (`Tasks` renamed);
//!   applied for any other (an edit inside the section moves with it).
//!
//! The test fails when a write that must be refused is applied ("applied wrongly") or a write that
//! may apply is refused ("refused false"); an error that is no conflict is reported as "failed at
//! kit".
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::needless_range_loop)]

mod common;

use std::collections::BTreeMap;

use common::*;
use serde_json::{Value, json};

const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
const ROUNDS: usize = 50;

/// splitmix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

const KINDS: [&str; 10] = [
    "K1 notes text edited (after all)",
    "K2 paragraph before Goals",
    "K3 Budget cell edited",
    "K4 table inserted before Timeline",
    "K5 a task in Tasks toggled",
    "K6 Tasks renamed",
    "K7 Goals moved to the end",
    "K8 property status set",
    "K9 Notes deleted",
    "K10 the Feedback round cell edited",
];

const FORMS: [&str; 5] = ["W1 by id", "W2 by index", "W3 by name", "W4 by point", "W5 structural"];

fn change(kind: usize) -> Vec<(&'static str, Value)> {
    match kind {
        0 => vec![("text.findReplace", json!({ "find": "Some closing notes.", "replace": "Some closing remarks." }))],
        1 => vec![("section.insert", json!({ "heading": ["Launch plan", "Goals"], "position": "before", "markdown": "A new paragraph." }))],
        2 => vec![("table.setCell", json!({ "table": 1, "row": 1, "column": 1, "text": "150" }))],
        3 => vec![("table.add", json!({ "heading": ["Launch plan", "Goals"], "position": "end", "columns": 2, "header": ["A", "B"] }))],
        4 => vec![("task.toggle", json!({ "blockId": "draft" }))],
        5 => vec![("text.findReplace", json!({ "find": "## Tasks", "replace": "## To do" }))],
        6 => vec![("section.move", json!({ "heading": ["Launch plan", "Goals"], "toHeading": ["Launch plan", "Notes"], "position": "after" }))],
        7 => vec![("property.set", json!({ "key": "status", "value": "review" }))],
        8 => vec![("section.delete", json!({ "heading": ["Launch plan", "Notes"] }))],
        _ => vec![("table.setCell", json!({ "table": 0, "row": 2, "column": 1, "text": "Feedback rounds" }))],
    }
}

fn offset(text: &str, find: &str) -> usize {
    text.find(find).map(|byte| text[..byte].chars().count()).unwrap()
}

fn write(form: usize, thread: &str) -> Value {
    match form {
        0 => json!({ "command": "comment.resolve", "params": { "thread": thread } }),
        1 => json!({ "command": "table.setCell", "params": { "table": 0, "row": 2, "column": 1, "text": "Second round" } }),
        2 => {
            json!({ "command": "section.insert", "params": { "heading": ["Launch plan", "Tasks"], "position": "end", "markdown": "- [ ] Send the invite" } })
        }
        3 => {
            let at = offset(NOTE, "Feedback round");
            json!({ "command": "text.replace", "params": { "from": at, "to": at + 8, "text": "Review" } })
        }
        _ => {
            json!({ "command": "section.move", "params": { "heading": ["Launch plan", "Tasks"], "toHeading": ["Launch plan", "Timeline"], "position": "after" } })
        }
    }
}

async fn threads(f: &Fixture, basis: &str) -> Vec<String> {
    let read = f.ok("run", json!({ "basis": basis, "steps": [{ "command": "comment.threads", "params": {} }] })).await;
    // The whole list: a cut answer could leave the round's thread out.
    assert!(read["completed"][0].get("truncated").is_none(), "{read}");
    read["completed"][0]["result"]["threads"].as_array().unwrap().iter().map(|thread| thread["id"].as_str().unwrap().to_owned()).collect()
}

fn necessary(form: usize, kind: usize, thread_gone: bool, now: &str) -> bool {
    match form {
        0 => thread_gone,
        1 => matches!(kind, 3 | 9),
        2 | 4 => kind == 5,
        _ => now.chars().skip(offset(NOTE, "Feedback round")).take(14).collect::<String>() != "Feedback round",
    }
}

/// One round: the note reset, a thread added, observed, the colleague's change, the agent's write;
/// its class.
async fn round(f: &Fixture, kind: usize, form: usize) -> (&'static str, String) {
    colleague_writes(&f.file(), NOTE);
    let reset = f.observe().await;
    let added = f
        .ok("run", json!({ "basis": reset, "steps": [{ "command": "comment.add", "params": { "quote": "Beta build", "body": "Which build?" } }] }))
        .await;
    let thread = added["completed"][0]["result"]["thread"].as_str().unwrap().to_owned();
    let basis = revision(&added);
    f.colleague(&change(kind));
    let after_colleague = f.text();
    let outcome = f.run(&basis, json!([write(form, &thread)])).await;
    let latest = match &outcome {
        Ok(answer) => revision(answer),
        Err(refused) => refused["revision"].as_str().map_or_else(|| basis.clone(), str::to_owned),
    };
    let gone = !threads(f, &latest).await.contains(&thread);
    let needed = necessary(form, kind, gone, &after_colleague);
    let class = match (&outcome, needed) {
        (Ok(_), false) => "applied",
        (Ok(_), true) => "applied wrongly",
        (Err(refused), needed) if code(refused) == "conflict" => {
            if needed {
                "refused necessary"
            } else {
                "refused false"
            }
        }
        (Err(_), _) => "failed at kit",
    };
    let reason = match &outcome {
        Err(refused) if code(refused) == "conflict" => reasons(refused),
        Err(refused) => format!("{refused}"),
        Ok(_) => String::new(),
    };
    (class, reason)
}

fn print(table: &BTreeMap<(usize, usize), Vec<&'static str>>, totals: &BTreeMap<&'static str, usize>) {
    println!("| change \\ write | {} |", FORMS.join(" | "));
    println!("|---|{}", "---|".repeat(FORMS.len()));
    for (kind, name) in KINDS.iter().enumerate() {
        let cells: Vec<String> =
            (0..FORMS.len()).map(|form| table.get(&(kind, form)).map(|classes| classes.join(", ")).unwrap_or_else(|| "-".to_owned())).collect();
        println!("| {name} | {} |", cells.join(" | "));
    }
    println!("totals: {totals:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_mixed_table() {
    let mut rng = Rng(SEED);
    let mut table: BTreeMap<(usize, usize), Vec<&'static str>> = BTreeMap::new();
    let mut totals: BTreeMap<&'static str, usize> = BTreeMap::new();
    for number in 0..ROUNDS {
        let (kind, form) = ((rng.next() % 10) as usize, (rng.next() % 5) as usize);
        let f = Fixture::new();
        let (class, reason) = round(&f, kind, form).await;
        f.shutdown().await;
        if class != "applied" {
            println!("round {number}: {} x {}: {class} {reason}", KINDS[kind], FORMS[form]);
        }
        table.entry((kind, form)).or_default().push(class);
        *totals.entry(class).or_default() += 1;
    }
    print(&table, &totals);
    assert_eq!(totals.get("applied wrongly"), None, "{totals:?}");
    assert_eq!(totals.get("refused false"), None, "{totals:?}");
}

/// The same rules over every cell once (the seeded rounds leave cells out).
#[tokio::test(flavor = "multi_thread")]
async fn every_cell_once() {
    let mut table: BTreeMap<(usize, usize), Vec<&'static str>> = BTreeMap::new();
    let mut totals: BTreeMap<&'static str, usize> = BTreeMap::new();
    for kind in 0..KINDS.len() {
        for form in 0..FORMS.len() {
            let f = Fixture::new();
            let (class, reason) = round(&f, kind, form).await;
            f.shutdown().await;
            if class != "applied" {
                println!("{} x {}: {class} {reason}", KINDS[kind], FORMS[form]);
            }
            table.entry((kind, form)).or_default().push(class);
            *totals.entry(class).or_default() += 1;
        }
    }
    print(&table, &totals);
    assert_eq!(totals.get("applied wrongly"), None, "{totals:?}");
    assert_eq!(totals.get("refused false"), None, "{totals:?}");
}
