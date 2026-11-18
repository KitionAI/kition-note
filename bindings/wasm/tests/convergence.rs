//! Kition Note against the Kition platform convergence suite (L6): two to five replicas of a note,
//! edited through the engine's own commands (contract 1 `execute`, the contract 2 change
//! exchange, `open_with_replica` to join), and its native and WebAssembly builds against one
//! change log.
//!
//! What the suite's steps are on a note:
//! - the text is the `Note` object's `body`, character for character (paragraphs joined by `\n`);
//! - `SetField` slots 0 and 1 are the note's two registers, `eol` and `bom` (`file.setLineEndings`);
//! - `Create` and `Delete` are a comment thread (`comment.add`, `comment.delete` of its comments);
//! - `Reorder` moves a section (`section.move`): a note's ordered collection is its text; the list
//!   check covers the comment lists;
//! - a note has no blobs, groups or duplicates (`blobs-late`, `reparent` and `duplicate-delete`
//!   skip, none of them required for a note's schema).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;

use kition_convergence::wasm::{self, WasmEngine};
use kition_convergence::{Blobs, KitAdapter, Options, Outcome, Result, Rng, Script};
use kition_kit::KitDocument;
use kition_kit::kdm::{ActorId, Replica};
use kition_kit::session::{self, KitModel, KitSessionDocument};
use kition_note_kdm::NoteModel;
use kition_note_kdm::file::Eol;
use kition_note_kit::NoteDocument;
use serde_json::{Value, json};

const NAME: &str = "Shared.md";
const SCHEMA: &str = include_str!("../../../crates/kdm/kdm-model/model/schema.json");

/// The note every replica starts from: four sections (so sections can move) and a paragraph each.
const START: &str = "# Alpha\nHello world, a shared note.\n# Beta\nSecond section here.\n# Gamma\nThird one.\n# Delta\nThe last.\n";

/// CJK ideographs ("middle", "text") and an emoji, by code point, for typing.
const CJK_MIDDLE: char = '\u{4E2D}';
const CJK_TEXT: char = '\u{6587}';
const EMOJI: char = '\u{1F389}';

struct NoteKit;

fn exec(doc: &mut NoteDocument, command: &str, params: Value) -> Result<Value> {
    doc.execute(command, params).map_err(|error| format!("{command}: {error}"))
}

/// Runs a command; a refusal because of the note's state is no edit (`false`).
fn attempt(doc: &mut NoteDocument, command: &str, params: Value) -> bool {
    doc.execute(command, params).is_ok()
}

fn chars(doc: &NoteDocument) -> usize {
    doc.text().chars().count()
}

/// The comment threads: `(thread id, [comment ids], resolved)`, as `comment.threads` lists them
/// (the same order on every replica with the same state).
fn threads(doc: &mut NoteDocument) -> Result<Vec<(String, Vec<String>, bool)>> {
    let listed = exec(doc, "comment.threads", json!({}))?;
    Ok(listed["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|thread| {
            let id = thread["id"].as_str()?.to_owned();
            let comments = thread["comments"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_str().map(str::to_owned)).collect();
            Some((id, comments, thread["resolved"].as_bool().unwrap_or(false)))
        })
        .collect())
}

/// The headings' titles, in order.
fn headings(doc: &NoteDocument) -> Vec<String> {
    doc.text().lines().filter_map(|line| line.strip_prefix("# ")).map(|title| title.trim().to_owned()).filter(|title| !title.is_empty()).collect()
}

/// The style the note has now: `(crlf, bom)`.
fn style(doc: &NoteDocument) -> Result<(bool, bool)> {
    let style = doc.session().model().style();
    Ok((style.eol == Eol::Crlf, style.bom))
}

/// Sets the line endings to CRLF or LF, keeping the byte-order mark.
fn set_eol(doc: &mut NoteDocument, crlf: bool) -> Result<()> {
    exec(doc, "file.setLineEndings", json!({ "eol": if crlf { "crlf" } else { "lf" } })).map(|_| ())
}

fn set_bom(doc: &mut NoteDocument, bom: bool) -> Result<()> {
    let (crlf, _) = style(doc)?;
    exec(doc, "file.setLineEndings", json!({ "eol": if crlf { "crlf" } else { "lf" }, "bom": bom })).map(|_| ())
}

impl KitAdapter for NoteKit {
    type Doc = NoteDocument;

    fn name(&self) -> &str {
        "kit.note"
    }

    fn schema(&self) -> &str {
        SCHEMA
    }

    /// The four sections of [`START`] and one comment thread on "shared".
    fn create(&self) -> Result<NoteDocument> {
        let mut doc = NoteDocument::create(NAME, 0, 0)?;
        exec(&mut doc, "text.insert", json!({ "at": 0, "text": START }))?;
        exec(&mut doc, "comment.add", json!({ "body": "A first thought.", "quote": "shared", "created": "2026-10-31T00:00:00Z" }))?;
        Ok(doc)
    }

    fn join(&self, snapshot: &[u8], blobs: &Blobs) -> Result<NoteDocument> {
        // A note references no blobs: its bundle is empty.
        NoteDocument::open_with_replica(NAME, &[], snapshot, &wasm::encode_blobs(blobs))
    }

    fn version(&self, doc: &mut NoteDocument) -> Result<Vec<u8>> {
        Ok(session::__version(doc))
    }

    fn export_changes(&self, doc: &mut NoteDocument, since: &[u8]) -> Result<Vec<u8>> {
        session::__export_changes(doc, since)
    }

    fn import_changes(&self, doc: &mut NoteDocument, bytes: &[u8]) -> Result<()> {
        KitSessionDocument::import_changes(doc, bytes).map(|_| ())
    }

    fn snapshot(&self, doc: &mut NoteDocument) -> Result<Vec<u8>> {
        session::__snapshot(doc)
    }

    fn state(&self, doc: &mut NoteDocument) -> Result<Value> {
        let snapshot = session::__snapshot(doc)?;
        let replica = Replica::load(NoteModel::schema(), &snapshot, ActorId::random()).map_err(|error| error.to_string())?;
        replica.state_json().map_err(|error| error.to_string())
    }

    fn undo(&self, doc: &mut NoteDocument) -> Result<bool> {
        KitSessionDocument::undo(doc).map(|(done, _)| done)
    }

    fn redo(&self, doc: &mut NoteDocument) -> Result<bool> {
        KitSessionDocument::redo(doc).map(|(done, _)| done)
    }

    fn random_edit(&self, doc: &mut NoteDocument, rng: &mut Rng) -> Result<Option<String>> {
        let len = chars(doc);
        let at = rng.below(len + 1);
        let end = (at + 1 + rng.below(6)).min(len);
        let value = rng.next_u64() % 1000;
        let (label, done) = match rng.below(100) {
            0..30 => {
                let word = match rng.below(3) {
                    0 => format!("w{value} "),
                    1 => [CJK_MIDDLE, CJK_TEXT].iter().collect(),
                    _ => EMOJI.to_string(),
                };
                ("type", attempt(doc, "text.insert", json!({ "at": at, "text": word })))
            }
            30..42 if end > at => ("delete", attempt(doc, "text.delete", json!({ "from": at, "to": end }))),
            42..52 => ("enter", attempt(doc, "text.insert", json!({ "at": at, "text": "\n" }))),
            52..58 => {
                let breaks: Vec<usize> = doc.text().chars().enumerate().filter(|(_, c)| *c == '\n').map(|(i, _)| i).collect();
                let Some(&at) = rng.pick(&breaks) else { return Ok(None) };
                ("merge", attempt(doc, "text.delete", json!({ "from": at, "to": at + 1 })))
            }
            58..64 => {
                let (crlf, _) = style(doc)?;
                ("line endings", set_eol(doc, !crlf).is_ok())
            }
            64..68 => {
                let (_, bom) = style(doc)?;
                ("byte-order mark", set_bom(doc, !bom).is_ok())
            }
            68..76 if end > at => (
                "comment",
                attempt(doc, "comment.add", json!({ "body": format!("note {value}"), "from": at, "to": end, "created": "2026-10-31T00:00:00Z" })),
            ),
            76..82 => {
                let threads = threads(doc)?;
                let Some((thread, _, resolved)) = rng.pick(&threads).cloned() else { return Ok(None) };
                ("resolve", attempt(doc, "comment.resolve", json!({ "thread": thread, "resolved": !resolved })))
            }
            82..88 => {
                let threads = threads(doc)?;
                let Some((thread, _, _)) = rng.pick(&threads).cloned() else { return Ok(None) };
                (
                    "reply",
                    attempt(doc, "comment.reply", json!({ "thread": thread, "body": format!("reply {value}"), "created": "2026-10-31T00:00:00Z" })),
                )
            }
            88..92 => {
                let threads = threads(doc)?;
                let Some((thread, comments, _)) = rng.pick(&threads).cloned() else { return Ok(None) };
                let Some(comment) = rng.pick(&comments).cloned() else { return Ok(None) };
                ("delete comment", attempt(doc, "comment.delete", json!({ "thread": thread, "comment": comment })))
            }
            _ => {
                let headings = headings(doc);
                let (Some(from), Some(to)) = (rng.pick(&headings).cloned(), rng.pick(&headings).cloned()) else { return Ok(None) };
                if from == to {
                    return Ok(None);
                }
                ("move section", attempt(doc, "section.move", json!({ "heading": [from], "toHeading": [to], "position": "after" })))
            }
        };
        Ok(done.then(|| label.to_owned()))
    }

    fn script(&self, doc: &mut NoteDocument, step: &Script) -> Result<bool> {
        match step {
            // Two registers of the `Note` object: each step changes its field.
            Script::SetField { slot: 0, .. } => {
                let (crlf, _) = style(doc)?;
                set_eol(doc, !crlf)?;
            }
            Script::SetField { .. } => {
                let (_, bom) = style(doc)?;
                set_bom(doc, !bom)?;
            }
            Script::Create => {
                exec(doc, "comment.add", json!({ "body": "A new thread.", "created": "2026-10-31T00:00:00Z" }))?;
            }
            Script::Delete => {
                // The first thread: deleting its last comment removes it.
                let Some((thread, comments, _)) = threads(doc)?.into_iter().next() else { return Ok(false) };
                for comment in comments {
                    exec(doc, "comment.delete", json!({ "thread": thread, "comment": comment }))?;
                }
            }
            Script::Reorder { which } => {
                // Different sections for 0 and 1: the first after the second, the last before the
                // one above it.
                let headings = headings(doc);
                if headings.len() < 4 {
                    return Ok(false);
                }
                let (moved, to, position) = if *which == 0 {
                    (headings[0].clone(), headings[1].clone(), "after")
                } else {
                    (headings[headings.len() - 1].clone(), headings[headings.len() - 2].clone(), "before")
                };
                exec(doc, "section.move", json!({ "heading": [moved], "toHeading": [to], "position": position }))?;
            }
            Script::InsertText { at, text } => {
                exec(doc, "text.insert", json!({ "at": at, "text": text }))?;
            }
            Script::DeleteText { at, len } => {
                exec(doc, "text.delete", json!({ "from": at, "to": at + len }))?;
            }
            Script::SplitParagraph { at } => {
                exec(doc, "text.insert", json!({ "at": at, "text": "\n" }))?;
            }
            Script::MergeParagraph { at } => {
                exec(doc, "text.delete", json!({ "from": at, "to": at + 1 }))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn text(&self, doc: &mut NoteDocument) -> Result<Option<String>> {
        Ok(Some(doc.text().to_owned()))
    }

    /// The view matches a fresh replica of the same state: the same text, the same file, the same
    /// comment threads.
    fn check_view(&self, doc: &mut NoteDocument) -> Result<()> {
        let snapshot = session::__snapshot(doc)?;
        let mut fresh = self.join(&snapshot, &Blobs::new())?;
        if doc.text() != fresh.text() {
            return Err(format!("the view's text differs from a fresh replica's: {:?} and {:?}", doc.text(), fresh.text()));
        }
        if doc.save(None)? != fresh.save(None)? {
            return Err("the view saves differently from a fresh replica".into());
        }
        let (mine, theirs) = (exec(doc, "comment.threads", json!({}))?, exec(&mut fresh, "comment.threads", json!({}))?);
        if mine != theirs {
            return Err(format!(
                "the view's comment threads differ from a fresh replica's: {}",
                kition_convergence::state::first_difference(&theirs, &mine).unwrap_or_default()
            ));
        }
        Ok(())
    }

    fn export_file(&self, doc: &mut NoteDocument) -> Result<Option<Vec<u8>>> {
        doc.save(None).map(Some)
    }

    /// Typing one character, and painting the top of the note.
    fn keystroke(&self, doc: &mut NoteDocument, rng: &mut Rng) -> Result<()> {
        let letter = char::from(b'a' + rng.below(26) as u8);
        let at = rng.below(chars(doc) + 1);
        exec(doc, "text.insert", json!({ "at": at, "text": letter.to_string() }))?;
        doc.render_area(0, 0, 800, 600, 800, 600).map(|_| ())
    }
}

#[test]
fn converges() {
    let report = kition_convergence::run(&NoteKit, &Options::from_env());
    println!("{}", report.render());
    report.save_if_requested().unwrap();
    for check in &report.checks {
        match &check.outcome {
            Outcome::Failed(reason) => panic!("{} failed: {reason}\n{}", check.id, report.render()),
            Outcome::Skipped(reason) if check.required => panic!("{} is required for L6 but skipped: {reason}", check.id),
            _ => {}
        }
    }
    assert!(report.level6(), "{}", report.render());
}

/// Native and WebAssembly engines reach one state from one change log. Set `KITION_WASM_ENGINE`
/// to a folder with `kition_note_kit.js` and its `.wasm` (a package's `engine/`, or a
/// `wasm-pack` output).
#[test]
fn wasm_parity() {
    let Some(engine) = WasmEngine::from_env("kition_note_kit", "NoteDocument", NAME) else {
        println!("SKIP wasm_parity: KITION_WASM_ENGINE is not set");
        return;
    };
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("note-convergence");
    for seed in [1, 2] {
        wasm::check_parity(&NoteKit, &engine, &scratch, seed).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
    }
}
