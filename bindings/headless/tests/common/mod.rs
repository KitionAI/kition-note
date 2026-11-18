//! What `outside.rs` and `premises.rs` share: a note on disk, the `kition-note` executable driven
//! through the kits' MCP server's tools (`kition_kits::tools`, the library `kition-kits-mcp`
//! serves) as an agent drives it, and a colleague's program: this kit's engine, an instance of its
//! own sharing nothing with the server, opening the file, running commands and writing it back, so
//! the server sees only a changed file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use kition_kits::tools::{self, CallOptions};
use kition_kits::{KitDefinition, KitManager};
use kition_note_kit::{KitDocument, NoteDocument};
use serde_json::{Value, json};

pub const ME: &str = "agent-me";
pub const FILE: &str = "plan.md";

/// The note every scenario starts from: properties, sections, tasks with block ids, two tables,
/// links.
pub const NOTE: &str = "---
title: Launch plan
status: draft
---

# Launch plan

Intro text with a [[timeline]] link.

## Goals

- Ship the beta to ten teams
- Collect feedback within two weeks

## Tasks

- [ ] Draft the announcement ^draft
- [ ] Book the review meeting ^book

## Timeline

| Week | Milestone |
| --- | --- |
| 1 | Beta build |
| 2 | Feedback round |

## Budget

| Item | Cost |
| --- | --- |
| Venue | 100 |

## Notes

Some closing notes.
";

pub fn kit() -> &'static KitDefinition {
    KitDefinition::from_manifest_text(include_str!("../../../../kit.json")).unwrap()
}

/// Sets the file's modification time an hour before its own, so that only its content tells the
/// server it changed.
fn backdate(file: &Path, before: std::time::SystemTime) {
    std::fs::File::options().write(true).open(file).unwrap().set_modified(before - Duration::from_secs(3600)).unwrap();
}

/// A colleague's program: this kit's engine, its own instance, opening the file from disk,
/// running `steps` and writing the file back.
pub fn colleague(file: &Path, steps: &[(&str, Value)]) -> Vec<Value> {
    let at = std::fs::metadata(file).unwrap().modified().unwrap();
    let mut doc = NoteDocument::open(FILE, &std::fs::read(file).unwrap()).unwrap();
    let answers = steps
        .iter()
        .map(|(command, params)| doc.execute(command, params.clone()).unwrap_or_else(|error| panic!("colleague's {command}: {error}")))
        .collect();
    std::fs::write(file, doc.save(Some(FILE)).unwrap()).unwrap();
    backdate(file, at);
    answers
}

/// A colleague's editor writing the whole file as `text`.
pub fn colleague_writes(file: &Path, text: &str) {
    let at = std::fs::metadata(file).unwrap().modified().unwrap();
    std::fs::write(file, text).unwrap();
    backdate(file, at);
}

pub fn on_disk(file: &Path) -> String {
    std::fs::read_to_string(file).unwrap()
}

static WORKSPACES: AtomicUsize = AtomicUsize::new(0);

fn workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-note-agent-{}-{}", std::process::id(), WORKSPACES.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

pub struct Fixture {
    pub kits: KitManager,
    pub options: CallOptions,
    pub workspace: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        Self::with(NOTE, CallOptions::default())
    }

    pub fn with(text: &str, options: CallOptions) -> Self {
        let workspace = workspace();
        std::fs::write(workspace.join(FILE), text).unwrap();
        let kits = KitManager::new(Some(workspace.clone()), vec![(kit(), Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-note"))))]);
        Self { kits, options, workspace }
    }

    pub fn file(&self) -> PathBuf {
        self.workspace.join(FILE)
    }

    pub fn colleague(&self, steps: &[(&str, Value)]) -> Vec<Value> {
        colleague(&self.file(), steps)
    }

    pub fn text(&self) -> String {
        on_disk(&self.file())
    }

    pub async fn call(&self, action: &str, mut input: Value) -> Result<Value, Value> {
        input["path"] = json!(FILE);
        input["draft"] = json!(false);
        match tools::call_as(&self.kits, &format!("note_{action}"), &input, self.options, ME).await {
            Some(Ok(output)) => Ok(output.output),
            Some(Err(error)) => Err(error.structured()),
            None => panic!("note_{action} is no tool"),
        }
    }

    /// A call by another agent of the same server (`other`), as a second MCP connection makes it.
    pub async fn call_by(&self, other: &str, action: &str, mut input: Value) -> Value {
        input["path"] = json!(FILE);
        input["draft"] = json!(false);
        match tools::call_as(&self.kits, &format!("note_{action}"), &input, self.options, other).await {
            Some(Ok(output)) => output.output,
            Some(Err(error)) => panic!("{other}'s note_{action}: {}", error.structured()),
            None => panic!("note_{action} is no tool"),
        }
    }

    pub async fn ok(&self, action: &str, input: Value) -> Value {
        self.call(action, input).await.unwrap_or_else(|error| panic!("note_{action}: {error}"))
    }

    pub async fn refused(&self, action: &str, input: Value) -> Value {
        match self.call(action, input).await {
            Err(error) => error,
            Ok(answer) => panic!("note_{action} was expected to be refused: {answer}"),
        }
    }

    /// The `revision` of an inspect now.
    pub async fn observe(&self) -> String {
        revision(&self.ok("inspect", json!({})).await)
    }

    pub async fn run(&self, basis: &str, steps: Value) -> Result<Value, Value> {
        self.call("run", json!({ "basis": basis, "steps": steps })).await
    }

    pub async fn shutdown(self) {
        self.kits.shutdown().await;
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

pub fn step(command: &str, params: Value) -> Value {
    json!([{ "command": command, "params": params }])
}

pub fn revision(answer: &Value) -> String {
    answer["revision"].as_str().unwrap_or_else(|| panic!("a revision: {answer}")).to_owned()
}

/// The conflicts' reasons, joined (`changed`, `relocated+changed`).
pub fn reasons(refused: &Value) -> String {
    let conflicts = refused["conflicts"].as_array().or_else(|| refused["error"]["data"]["conflicts"].as_array());
    conflicts
        .unwrap_or_else(|| panic!("conflicts: {refused}"))
        .iter()
        .map(|conflict| conflict["reason"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join("+")
}

/// A refused write's error code.
pub fn code(refused: &Value) -> String {
    refused["code"].as_str().or_else(|| refused["error"]["code"].as_str()).unwrap_or_default().to_owned()
}
