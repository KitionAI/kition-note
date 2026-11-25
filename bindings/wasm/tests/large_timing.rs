//! Timings of the editor's steps on a large note (run by hand: cargo test --release --test
//! large_timing -- --ignored --nocapture, with KITION_LARGE_NOTE naming a Markdown file).

use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolPointer};
use kition_note_kit::NoteDocument;
use std::time::Instant;

#[test]
#[ignore]
fn steps_on_a_large_note() {
    let Some(path) = std::env::var_os("KITION_LARGE_NOTE") else { return };
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kition-fonts");
    let faces = kition_note_fonts::register_dir(&dir);
    eprintln!("faces: {faces}");
    let bytes = std::fs::read(path).unwrap();
    let time = |label: &str, started: Instant| eprintln!("{label}: {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    let started = Instant::now();
    let mut doc = NoteDocument::open("large.md", &bytes).unwrap();
    time("open", started);
    let started = Instant::now();
    let _ = doc.render(0.0, 0.0, 1.0, 1280, 760).unwrap();
    time("first render", started);
    let started = Instant::now();
    let _ = doc.inspect();
    time("first inspect", started);
    let started = Instant::now();
    let _ = doc.inspect();
    time("second inspect", started);
    // What the editor asks right after (in its order).
    let started = Instant::now();
    let _ = KitTools::tools(&doc);
    let _ = KitTools::select_tool(&mut doc, "text", 1.0);
    time("tools and select", started);
    for read in ["table.geometry", "code.blocks", "property.view", "diagram.view", "image.wanted"] {
        let started = Instant::now();
        let _ = doc.execute(read, serde_json::json!({ "top": 0, "bottom": 3000 }));
        time(&format!("after open: {read}"), started);
        let started = Instant::now();
        let answer = doc.execute(read, serde_json::json!({})).map(|value| value.to_string().len()).unwrap_or(0);
        time(&format!("after open: {read} unbounded ({answer} bytes)"), started);
    }
    let started = Instant::now();
    let _ = KitTools::state(&mut doc, 0.0, 0.0, ToolMods::default(), 1.0);
    time("after open: tool state", started);
    let started = Instant::now();
    let _ = doc.execute("view.width", serde_json::json!({ "width": 1280, "height": 760 }));
    time("after open: view.width", started);
    let started = Instant::now();
    let _ = doc.render(0.0, 0.0, 2.0, 2560, 1520).unwrap();
    time("after open: render at 2x", started);
    for (label, y) in [("click heading", 40.0), ("click section", 140.0), ("click heading again", 40.0)] {
        let event = ToolPointer { action: PointerAction::Down, x: 700.0, y, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 };
        let started = Instant::now();
        doc.pointer(&event).unwrap();
        time(&format!("{label}: pointer"), started);
        let started = Instant::now();
        let _ = doc.revision();
        time(&format!("{label}: revision"), started);
        let started = Instant::now();
        let _ = doc.inspect();
        time(&format!("{label}: inspect"), started);
        let started = Instant::now();
        let _ = doc.render(0.0, 0.0, 1.0, 1280, 760).unwrap();
        time(&format!("{label}: render"), started);
        let started = Instant::now();
        let _ = doc.execute("table.geometry", serde_json::json!({}));
        time(&format!("{label}: table.geometry"), started);
        let started = Instant::now();
        let _ = doc.execute("text.read", serde_json::json!({}));
        time(&format!("{label}: text.read"), started);
    }
    for round in 0..3 {
        let started = Instant::now();
        KitTools::text(&mut doc, "x", 1.0).unwrap();
        time(&format!("type {round}: text"), started);
        let started = Instant::now();
        let _ = doc.inspect();
        time(&format!("type {round}: inspect"), started);
        let started = Instant::now();
        let _ = doc.render(0.0, 0.0, 1.0, 1280, 760).unwrap();
        time(&format!("type {round}: render"), started);
    }
    // A keystroke in the middle, with what the editor asks after it (its overlays' reads).
    let middle = String::from_utf8_lossy(&bytes).find("## Section 1000").map_or(0, |at| String::from_utf8_lossy(&bytes)[..at].chars().count());
    doc.execute("select.set", serde_json::json!({ "anchor": middle })).unwrap();
    for round in 0..3 {
        let started = Instant::now();
        KitTools::text(&mut doc, "y", 1.0).unwrap();
        time(&format!("middle {round}: text"), started);
        let started = Instant::now();
        let _ = doc.revision();
        time(&format!("middle {round}: revision"), started);
        for read in ["code.blocks", "diagram.view", "property.view", "table.geometry", "image.wanted", "select.get"] {
            let started = Instant::now();
            let _ = doc.execute(read, serde_json::json!({ "top": 0, "bottom": 3000 }));
            time(&format!("middle {round}: {read}"), started);
        }
        let started = Instant::now();
        let _ = doc.inspect();
        time(&format!("middle {round}: inspect"), started);
        let started = Instant::now();
        let _ = doc.render(0.0, 0.0, 1.0, 1280, 760).unwrap();
        time(&format!("middle {round}: render"), started);
        let started = Instant::now();
        let _ = KitTools::state(&mut doc, 700.0, 300.0, ToolMods::default(), 1.0);
        time(&format!("middle {round}: tool state"), started);
        let started = Instant::now();
        let _ = doc.render(0.0, 0.0, 2.0, 2560, 1520).unwrap();
        time(&format!("middle {round}: render at 2x"), started);
        let started = Instant::now();
        for _ in 0..10 {
            let _ = doc.body().len();
        }
        time(&format!("middle {round}: body x10"), started);
        let started = Instant::now();
        let _ = doc.dirty();
        time(&format!("middle {round}: dirty"), started);
    }
}
