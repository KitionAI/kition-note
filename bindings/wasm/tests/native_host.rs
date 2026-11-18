use kition_note_kit::{KitDocument, NoteDocument};

#[test]
fn native_host_imports() {
    let bytes = b"---\ncontact_id: expert\n---\n\n# Expert\n\nA native host.\n";
    let doc = NoteDocument::open("expert.md", bytes).unwrap();
    assert_eq!(doc.save(None).unwrap(), bytes);
}

#[test]
fn native_host_system_edits_stay_outside_person_undo() {
    use kition_note_kit::{KitSessionDocument, Origin};
    let mut doc = NoteDocument::open("expert.md", b"# Expert\n").unwrap();
    doc.execute_as("text.replace", serde_json::json!({"from":9,"to":9,"text":"Background\n"}), &Origin::system("system:contacts")).unwrap();
    assert!(!doc.undo().unwrap().0);
    assert!(String::from_utf8(doc.save(None).unwrap()).unwrap().contains("Background"));
}
