//! `note_check` on the conformance fixtures (`conformance/`, listed in `agent/checks.json`): each
//! triggers its code. Its own test binary, because it lends files to the engine through
//! `kition_kit::host`, which is process-wide.

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;
use serde_json::Value;

#[test]
fn the_conformance_fixtures_trigger_their_codes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance").canonicalize().unwrap();
    let catalogue: Value = serde_json::from_str(include_str!("../../../agent/checks.json")).unwrap();
    kition_kit::host::set_root(Some(root.clone()));
    kition_kit::host::set_folder("");
    for entry in catalogue.as_array().unwrap() {
        let (code, fixture) = (entry["code"].as_str().unwrap(), entry["fixture"].as_str().unwrap());
        let bytes = std::fs::read(root.join(fixture)).unwrap_or_else(|error| panic!("{fixture}: {error}"));
        let doc = NoteDocument::open(fixture, &bytes).unwrap();
        let found = doc.check();
        kition_kit::diagnostics::check_diagnostics(&found).unwrap();
        let codes: Vec<&str> = found.iter().map(|found| found.code.as_str()).collect();
        assert!(codes.contains(&code), "{fixture}: {codes:?}");
        if code != "note.truncated" {
            assert!(codes.iter().all(|found| *found == code), "{fixture} reports more than {code}: {codes:?}");
        }
        // The check changes nothing.
        assert_eq!(doc.revision(), 0);
        assert!(!doc.dirty());
    }
    // `basic.md`, the document the conformance suite edits, has nothing to report.
    let basic = NoteDocument::open("basic.md", &std::fs::read(root.join("basic.md")).unwrap()).unwrap();
    assert!(basic.check().is_empty(), "basic.md: {:?}", basic.check().iter().map(|found| &found.code).collect::<Vec<_>>());
    kition_kit::host::set_root(None);
}
