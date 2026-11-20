//! `document.identity` and `document.fork` (Kit Process Protocol section 5.6, KitModel M7a)
//! through the real `kition-note` process: a `.md` file is the note's text and nothing else (the
//! replica, and the comments in it, are the host's, design "the file is the text"), so it embeds no
//! replica and carries no document id. Opening it makes a new replica each time. A host learns that
//! without parsing the file, and a fork of a note is the same bytes, with no id.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use kition_kit_host::{HeadlessKit, LaunchOptions};
use kition_note_kit::{KitDocument, NoteDocument};
use serde_json::json;

const LIMIT: Duration = Duration::from_secs(30);

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-note-identity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

async fn launch(dir: &Path) -> HeadlessKit {
    let mut options = LaunchOptions::new(env!("CARGO_BIN_EXE_kition-note"));
    options.workspace = Some(dir.to_path_buf());
    HeadlessKit::launch(options).await.unwrap()
}

/// A note the kit saved after an edit: properties, a heading, a task, CRLF line ends.
fn note() -> Vec<u8> {
    let mut doc = NoteDocument::open("Plan.md", b"---\r\ntitle: Plan\r\n---\r\n\r\n# Plan\r\n\r\n- [ ] Draft\r\n").unwrap();
    let end = doc.text().chars().count();
    doc.execute("text.insert", json!({ "at": end, "text": "Closing words.\n" })).unwrap();
    doc.save(Some("Plan.md")).unwrap()
}

#[tokio::test]
async fn a_note_file_embeds_no_replica_so_it_carries_no_identity_and_forks_unchanged() {
    let dir = workspace("plain");
    let bytes = note();
    std::fs::write(dir.join("Plan.md"), &bytes).unwrap();
    let kit = launch(&dir).await;
    assert!(kit.serves("document.identity"), "{}", kit.capabilities());

    // A note, a byte copy of it and an empty note (made for the kit before it saved one) carry no
    // identity: the file is the text.
    std::fs::copy(dir.join("Plan.md"), dir.join("Copy.md")).unwrap();
    std::fs::write(dir.join("New.md"), b"").unwrap();
    for path in ["Plan.md", "Copy.md", "New.md"] {
        assert_eq!(kit.identity(path, LIMIT).await.unwrap().doc_id, None, "{path}");
    }

    // A fork is the same bytes, with no id, and opens as the same note.
    let fork = kit.fork("Plan.md", "Fork.md", LIMIT).await.unwrap();
    assert_eq!(fork.doc_id, None);
    assert_eq!(fork.name, "Fork.md");
    assert!(!fork.version.is_empty());
    assert_eq!(std::fs::read(dir.join("Fork.md")).unwrap(), bytes, "a file without a replica forks unchanged");
    assert_eq!(std::fs::read(dir.join("Plan.md")).unwrap(), bytes, "the source is unchanged");
    let opened = NoteDocument::open("Fork.md", &std::fs::read(dir.join("Fork.md")).unwrap()).unwrap();
    assert_eq!(opened.text(), NoteDocument::open("Plan.md", &bytes).unwrap().text());
    let empty = kit.fork("New.md", "NewCopy.md", LIMIT).await.unwrap();
    assert_eq!(empty.doc_id, None);
    assert_eq!(std::fs::read(dir.join("NewCopy.md")).unwrap(), b"");

    // A fork never replaces a file.
    std::fs::write(dir.join("Taken.md"), b"# Someone else's note\n").unwrap();
    let taken = kit.fork("Plan.md", "Taken.md", LIMIT).await.unwrap_err();
    assert_eq!(taken.reason(), Some("file_conflict"), "{taken}");
    assert_eq!(std::fs::read(dir.join("Taken.md")).unwrap(), b"# Someone else's note\n", "the existing file is kept");
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}
