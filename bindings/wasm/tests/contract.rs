//! kition-sdk's contract check (`kition_kit::testing::check_contract`) on a real note.
//!
//! One step of the check cannot hold for a text format: it first opens the bytes
//! `b"not a document"` and expects an error, but every byte string is a note (valid UTF-8 opens
//! as text; invalid UTF-8 opens read-only and saves its bytes unchanged, design 2.3). So the check
//! runs on `ProbeNote`, which forwards every member to `NoteDocument` and refuses only that one
//! probe. Everything else the check asserts (size, render and render_area box means, revision,
//! dirty, undo and redo, disabled commands, check, save and reopen) is the real document's. The
//! wrapper goes when kition-sdk's check takes a "no invalid bytes" option.

use kition_kit::diagnostics::Diagnostic;
use kition_kit::{KitCommand, KitDocument};
use kition_note_kit::NoteDocument;
use serde_json::{Value, json};

/// The bytes `check_contract` opens to see that bad input is an error.
const SDK_GARBAGE_PROBE: &[u8] = b"not a document";

struct ProbeNote(NoteDocument);

impl KitDocument for ProbeNote {
    fn open(name: &str, bytes: &[u8]) -> Result<Self, String> {
        if bytes == SDK_GARBAGE_PROBE {
            return Err("the contract's garbage probe (every byte string is a note)".into());
        }
        NoteDocument::open(name, bytes).map(Self)
    }
    fn create(name: &str, width: u32, height: u32) -> Result<Self, String> {
        NoteDocument::create(name, width, height).map(Self)
    }
    fn width(&self) -> u32 {
        self.0.width()
    }
    fn height(&self) -> u32 {
        self.0.height()
    }
    fn revision(&self) -> u64 {
        self.0.revision()
    }
    fn dirty(&self) -> bool {
        self.0.dirty()
    }
    fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.0.render(x, y, scale, width, height)
    }
    fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.0.render_area(x0, y0, x1, y1, width, height)
    }
    fn execute(&mut self, command: &str, params: Value) -> Result<Value, String> {
        self.0.execute(command, params)
    }
    fn inspect(&self) -> Result<Value, String> {
        self.0.inspect()
    }
    fn commands(&self) -> Vec<KitCommand> {
        self.0.commands()
    }
    fn disabled_commands(&self) -> Vec<String> {
        self.0.disabled_commands()
    }
    fn save(&self, name: Option<&str>) -> Result<Vec<u8>, String> {
        self.0.save(name)
    }
    fn mark_saved(&mut self) {
        self.0.mark_saved();
    }
    fn check(&self) -> Vec<Diagnostic> {
        self.0.check()
    }
}

const FIXTURE: &str = "# Plan\n\nGoals for the quarter.\n\n| Item | Owner |\n| ---- | ----- |\n| Ship | Ann   |\n";

#[test]
fn keeps_the_kit_contract() {
    kition_kit::testing::check_contract::<ProbeNote>(FIXTURE.as_bytes(), "fixture.md", |doc| {
        doc.execute("text.insert", json!({ "at": 6, "text": " and a much longer heading line" })).map(|_| ())
    })
    .unwrap();
}

#[test]
fn the_probe_wrapper_changes_nothing_but_the_garbage_probe() {
    // The real document opens the probe as a note; that is the one step the wrapper answers.
    let note = NoteDocument::open("fixture.md", SDK_GARBAGE_PROBE).unwrap();
    assert_eq!(note.save(None).unwrap(), SDK_GARBAGE_PROBE);
    assert!(ProbeNote::open("fixture.md", SDK_GARBAGE_PROBE).is_err());
    let probe = ProbeNote::open("fixture.md", FIXTURE.as_bytes()).unwrap();
    let real = NoteDocument::open("fixture.md", FIXTURE.as_bytes()).unwrap();
    assert_eq!(probe.save(None).unwrap(), real.save(None).unwrap());
    assert_eq!(probe.render(0.0, 0.0, 1.0, 612, 792).unwrap(), real.render(0.0, 0.0, 1.0, 612, 792).unwrap());
}
