//! Regression: the note's own render (thumbnails, previews, a host
//! without the editor) draws what the editor otherwise draws over it: the front matter as the
//! properties block, and the text open comment threads are anchored to, tinted. Once the editor
//! asks for the properties block (`properties.view`), it draws it and the render leaves its room
//! blank. A test binary of its own: fonts are the process's.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;
use serde_json::json;

const NOTE: &[u8] = b"---\ndate: 2026-11-09\nsleep: 7.5\nweather: frost, then bright sun\ntags: [journal, daily]\n---\n# Monday 9 November\n\nTwo weeks of beta with the twelve pilot marinas.\n\nThe launch date is 2 March 2027.\n";

fn fonts() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kition-fonts");
    let dir = std::env::var_os("KITION_FONTS_DIR").map_or(dir, std::path::PathBuf::from);
    assert!(kition_note_fonts::register_dir(&dir) > 0, "needs the fonts: set KITION_FONTS_DIR (README, Fonts)");
}

/// (dark pixels, yellow-tinted pixels) in the document's rows `top..bottom`.
fn count(doc: &NoteDocument, top: u32, bottom: u32) -> (usize, usize) {
    let width = doc.width();
    let pixels = doc.render(0.0, f64::from(top), 1.0, width, bottom - top).unwrap();
    let dark = pixels.as_chunks::<4>().0.iter().filter(|px| px[3] > 0 && px[0] < 120 && px[1] < 120 && px[2] < 120).count();
    let tint = pixels.as_chunks::<4>().0.iter().filter(|px| px[0] > 220 && px[1] > 190 && px[2] < 190 && px[0] - px[2] > 60).count();
    (dark, tint)
}

/// Where the first heading's text is (the properties block is above it).
fn heading_top(doc: &mut NoteDocument) -> u32 {
    let text = std::str::from_utf8(NOTE).unwrap();
    let at = text[..text.find("# Monday").unwrap()].chars().count();
    let caret = doc.execute("selection.where", json!({ "at": at })).unwrap();
    caret["y"].as_f64().map_or(0, |top| top as u32)
}

#[test]
fn properties_and_comment_anchors_are_drawn() {
    fonts();
    let mut doc = NoteDocument::open("journal.md", NOTE).unwrap();
    let top = heading_top(&mut doc);
    assert!(top > 100, "the properties block has its room ({top})");
    // Below the inline title (the file's name, rows 30 to 70).
    let (ink, _) = count(&doc, 80, top);
    assert!(ink > 400, "the properties block is drawn: its heading, keys and values ({ink} dark pixels above {top})");

    // An open thread tints its text; a resolved one does not.
    let (_, before) = count(&doc, top, top + 300);
    assert_eq!(before, 0, "no thread, no tint");
    let beta = doc.execute("comment.add", json!({ "quote": "twelve pilot marinas", "body": "Two to three weeks?", "author": "Ana" })).unwrap();
    doc.execute("comment.add", json!({ "quote": "2 March 2027", "body": "Fixed by January?", "author": "Tom" })).unwrap();
    let (_, two) = count(&doc, top, top + 300);
    assert!(two > 200, "the anchored text is tinted ({two})");
    doc.execute("comment.resolve", json!({ "thread": beta["thread"] })).unwrap();
    let (_, one) = count(&doc, top, top + 300);
    assert!(one > 50 && one < two, "a resolved thread is not tinted ({one} of {two})");

    // The editor draws the properties block itself once it asks for it.
    let view = doc.execute("properties.view", json!({})).unwrap();
    assert!(view["rows"].as_array().is_some_and(|rows| rows.len() == 4), "{view}");
    let (ink, _) = count(&doc, 80, top);
    assert!(ink < 20, "the editor's overlay draws it, not the render ({ink})");
}
