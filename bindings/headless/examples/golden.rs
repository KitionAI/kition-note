//! Writes the expected preview of `conformance/basic.md`, `conformance/golden/basic.png`, that
//! `khp check` compares `note_inspect {preview: true}` with (the Kit Harness Protocol's
//! conformance check): drawn by the kit's own engine with the fonts the kit ships, through the
//! same `view.screenshot` (canvas, at most 1024 pixels) a kits server asks the headless kit for.
//!
//! `cargo run -p kition-note --example golden` (with kition-fonts beside the repository or
//! `KITION_FONTS_DIR` set, as for the executable). Review the picture before committing it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit_server::protocol::ScreenshotScope;
use kition_kit_server::{KitBackend, OpenRequest, ScreenshotRequest};
use kition_note_kit::NoteDocument;

/// The longest side a kits server asks a preview for when the call gives no `maxSize`.
const MAX_SIZE: u32 = 1024;

fn main() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        kition_note::add_shipped_fonts(),
        kition_note_fonts::CHECKOUT_FILES.len(),
        "the build found no fonts: set KITION_FONTS_DIR (README, Fonts)"
    );
    let root = std::env::temp_dir().join(format!("kition-note-golden-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::copy(repo.join("conformance/basic.md"), root.join("basic.md")).unwrap();
    let mut kit = Headless::<NoteDocument>::with_replica_port(KitInfo::from_manifest(include_str!("../../../kit.json"), &[], &[]).unwrap());
    let folder = HostFolder::new(&root).unwrap();
    kit.set_host(folder.host());
    let opened = kit.open(OpenRequest { file: &folder.file("basic.md").unwrap(), read_only: true, create: None }).unwrap();
    let shot =
        kit.screenshot(&opened.id, &ScreenshotRequest { scope: ScreenshotScope::Canvas, max_size: MAX_SIZE, scale: 1.0, target: None }).unwrap();
    let target = repo.join("conformance/golden/basic.png");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, &shot.png).unwrap();
    println!("conformance/golden/basic.png: {} x {}", shot.width, shot.height);
    let _ = std::fs::remove_dir_all(root);
}
