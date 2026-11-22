//! The preview an agent gets of `conformance/basic.md` (KHP A39) is the expected one,
//! `conformance/expected/basic.png`, within the tolerance the kit states in
//! `conformance/expected/tolerance.json`, compared as `khp check` compares them: drawn by the kit's
//! own engine with the fonts it ships, through the `view.screenshot` a kits server asks for.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit_server::protocol::ScreenshotScope;
use kition_kit_server::{KitBackend, OpenRequest, ScreenshotRequest};
use kition_note_kit::NoteDocument;
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn decode(png: &[u8]) -> image::RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap().to_rgba8()
}

#[test]
fn the_preview_of_basic_is_the_expected_preview_within_the_stated_tolerance() {
    assert_eq!(kition_note::add_shipped_fonts(), kition_note_fonts::CHECKOUT_FILES.len(), "the shipped fonts are built in");
    let root = std::env::temp_dir().join(format!("kition-note-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::copy(repo().join("conformance/basic.md"), root.join("basic.md")).unwrap();
    let mut kit = Headless::<NoteDocument>::with_replica_port(
        KitInfo::from_manifest(include_str!("../../../kit.json"), &[], kition_note_kit::EDITOR_STATE).unwrap(),
    );
    let folder = HostFolder::new(&root).unwrap();
    kit.set_host(folder.host());
    let opened = kit.open(OpenRequest { file: &folder.file("basic.md").unwrap(), read_only: true, create: None }).unwrap();
    let preview = decode(
        &kit.screenshot(&opened.id, &ScreenshotRequest { scope: ScreenshotScope::Canvas, max_size: 1024, scale: 1.0, target: None }).unwrap().png,
    );
    let first = *preview.get_pixel(0, 0);
    assert!(preview.pixels().any(|pixel| *pixel != first), "the preview is not one colour");

    let expected = decode(&std::fs::read(repo().join("conformance/expected/basic.png")).unwrap());
    let tolerance: Value = serde_json::from_slice(&std::fs::read(repo().join("conformance/expected/tolerance.json")).unwrap()).unwrap();
    let delta = tolerance["maxChannelDelta"].as_u64().unwrap();
    let allowed = tolerance["maxDifferentPixels"].as_u64().unwrap();
    assert_eq!(preview.dimensions(), expected.dimensions());
    let different =
        preview.pixels().zip(expected.pixels()).filter(|(a, b)| a.0.iter().zip(b.0.iter()).any(|(a, b)| u64::from(a.abs_diff(*b)) > delta)).count()
            as u64;
    assert!(different <= allowed, "{different} pixels differ from expected/basic.png, the tolerance allows {allowed}");
    let _ = std::fs::remove_dir_all(root);
}
