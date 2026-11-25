//! Ported from Kition's `documentText.spec.ts` (replica creation), `documentComments.spec.ts`
//! (comment repair) and the replica half of `documentMerge.spec.ts`.

use std::time::Instant;

use kition_kit::kdm::comment::{AnchorRange, CommentAnchor, NewComment, add_thread, delete_comment, delete_thread, reply, threads};
use kition_kit::kdm::comment::{COMMENT_THREAD_TYPE, COMMENT_THREAD_TYPE_JSON, COMMENT_TYPE, COMMENT_TYPE_JSON};
use kition_kit::session::{KitSession, MemoryBlobs};

use super::*;

const QUARTER: &str = "\u{5b63}\u{5ea6}";
const BOM_CHAR: &str = "\u{feff}";

fn person() -> Origin {
    Origin::person("user:test")
}

fn import(bytes: &[u8]) -> KitSession<NoteModel> {
    KitSession::<NoteModel>::import("note.md", bytes, Box::new(MemoryBlobs::default()), ActorId::random()).unwrap()
}

#[test]
fn the_schema_is_kit_note_v1_with_the_shared_comment_types() {
    let schema = Schema::parse(SCHEMA).unwrap();
    assert_eq!((schema.kit.as_str(), schema.schema_version, schema.root.as_str()), (KIT, 1, NOTE));
    let file: Value = serde_json::from_str(SCHEMA).unwrap();
    let thread: Value = serde_json::from_str(COMMENT_THREAD_TYPE_JSON).unwrap();
    let comment: Value = serde_json::from_str(COMMENT_TYPE_JSON).unwrap();
    assert_eq!(file["types"]["CommentThread"], thread);
    assert_eq!(file["types"]["Comment"], comment);
    assert_eq!(file["types"]["Note"]["fields"]["eol"]["values"], json!(["lf", "crlf", "mixed"]));
}

#[test]
fn a_file_imports_with_its_style_and_nothing_to_undo() {
    let raw = format!("{BOM_CHAR}# {QUARTER} \u{1F600}\r\nBody\r\n");
    let session = import(raw.as_bytes());
    assert_eq!(session.model().body(), format!("# {QUARTER} \u{1F600}\nBody\n"));
    assert_eq!(session.model().style(), LineStyle { eol: Eol::Crlf, bom: true });
    assert_eq!(registers(session.replica()), json!({ "eol": "crlf", "bom": true }));
    assert!(!session.can_undo());
    assert_eq!(session.export("note.md").unwrap(), raw.as_bytes());
}

#[test]
fn a_plain_file_writes_no_registers() {
    let session = import(b"# Plan\n");
    assert_eq!(session.replica().get(&root_id(), EOL).unwrap_or(Value::Null), Value::Null);
    assert_eq!(session.replica().get(&root_id(), BOM).unwrap_or(Value::Null), Value::Null);
    assert_eq!(registers(session.replica()), json!({ "eol": "lf", "bom": false }));
}

#[test]
fn a_co_editor_that_never_read_the_file_saves_it_in_its_style() {
    let session = import(b"a\r\nb\r\n");
    let other = KitSession::<NoteModel>::open_snapshot(&session.snapshot().unwrap(), Box::new(MemoryBlobs::default()), ActorId::random()).unwrap();
    assert_eq!(other.export("note.md").unwrap(), b"a\r\nb\r\n");
}

#[test]
fn invalid_utf8_has_no_replica_form() {
    let error = KitSession::<NoteModel>::import("note.md", b"ok\xFF", Box::new(MemoryBlobs::default()), ActorId::random()).err().unwrap();
    assert!(error.to_string().starts_with("note.invalid-utf8"), "{error}");
}

#[test]
fn later_edits_undo_but_never_the_imported_body() {
    let mut session = import(b"abc");
    session.execute(&person(), |_, tx| Ok(tx.text_insert(&root_id(), BODY, 3, "d")?)).unwrap();
    assert!(session.undo().unwrap().0);
    assert_eq!(session.model().body(), "abc");
    assert!(!session.can_undo());
}

#[test]
fn a_large_body_is_written_in_pieces_and_survives_a_snapshot() {
    let lines: Vec<String> = (0..3000).map(|index| format!("Line {index}: {QUARTER} \u{1F600} words")).collect();
    let raw = format!("{}\r\n{}\r\n{}\r\n", lines.join("\r\n"), "\u{1F600}\u{1F44D}".repeat(6000), lines.join("\r\n"));
    let session = import(raw.as_bytes());
    let body = raw.replace("\r\n", "\n");
    assert!(session.model().body() == body);
    let loaded = KitSession::<NoteModel>::open_snapshot(&session.snapshot().unwrap(), Box::new(MemoryBlobs::default()), ActorId::random()).unwrap();
    assert!(loaded.model().body() == body);
    assert!(loaded.export("note.md").unwrap() == raw.as_bytes());
}

// ---- Merging into a replica ----------------------------------------------------------------

/// Merges through a real replica holding `current`; answers whether it changed and the text.
fn merge_on_replica(base: &str, incoming: &str, current: &str) -> (bool, String) {
    let mut session = import(current.as_bytes());
    let (changed, _) = session.execute(&Origin::system("import"), |model, tx| merge_external(tx, base, incoming, model.body())).unwrap();
    (changed, session.model().body().to_owned())
}

#[test]
fn merging_into_a_replica_keeps_both_sides() {
    assert_eq!(merge_on_replica("a\nb\n", "a\nb\n", "a\nB\nlocal\n"), (false, "a\nB\nlocal\n".into()));
    assert_eq!(merge_on_replica("a\n", "a\nb\n", "a\nb\n"), (false, "a\nb\n".into()));
    assert_eq!(merge_on_replica("a\nb\n", "a\nchanged\nb\nc\n", "a\nb\n"), (true, "a\nchanged\nb\nc\n".into()));
    let base = "# Plan\n\nFirst point.\nSecond point.\nThird point.\n";
    let current = "# Plan\n\nFirst point, edited here.\nSecond point.\nThird point.\n";
    let incoming = "# Plan\n\nFirst point.\nSecond point.\nThird point, edited outside.\n";
    assert_eq!(merge_on_replica(base, incoming, current).1, "# Plan\n\nFirst point, edited here.\nSecond point.\nThird point, edited outside.\n");
}

#[test]
fn a_merge_refuses_a_current_text_that_is_not_the_notes() {
    let mut session = import(b"abc");
    assert!(session.execute(&Origin::system("import"), |_, tx| merge_external(tx, "abc", "abcd", "ab")).is_err());
    assert_eq!(session.model().body(), "abc");
}

#[test]
fn write_body_changes_only_what_differs() {
    let mut session = import(b"one\ntwo\nthree\n");
    let (changed, _) = session.execute(&person(), |model, tx| write_body(tx, model.body(), "one\n2\nthree\n")).unwrap();
    assert!(changed);
    assert_eq!(session.model().body(), "one\n2\nthree\n");
    let (unchanged, _) = session.execute(&person(), |model, tx| write_body(tx, model.body(), "one\n2\nthree\n")).unwrap();
    assert!(!unchanged);
}

/// The external-merge benchmark of Kition's `documentKdm.bench.spec.ts` (1 % external and 0.5 %
/// local scattered edits on about 1 MB of Markdown), timed whole: diff, merge, and the replica
/// transaction. The budget is 100 ms in a release build (`docs/features/performance.md`); a debug test
/// build only checks the result and prints the time.
#[test]
fn a_one_megabyte_merge_keeps_every_edit() {
    let (base, current, incoming, expected) = kition_note_markdown_file::bench::merge_case(1_000_000);
    let mut session = import(current.as_bytes());
    let started = Instant::now();
    let (changed, _) = session.execute(&Origin::system("import"), |model, tx| merge_external(tx, &base, &incoming, model.body())).unwrap();
    let elapsed = started.elapsed();
    assert!(changed);
    assert!(session.model().body() == expected);
    println!("merge_external on {} characters: {elapsed:?} (debug build)", base.chars().count());
}

// ---- Comment repair -------------------------------------------------------------------------

fn said(body: &str) -> NewComment {
    NewComment { author: "user:ada".into(), name: None, body: body.into(), created: "2026-01-01T00:00:00.000Z".into() }
}

/// A note with one thread of `count` comments.
fn with_thread(count: usize) -> KitSession<NoteModel> {
    let mut session = import(b"# Plan\n\nGoals\n");
    let anchor = CommentAnchor {
        field: Some(BODY.into()),
        range: Some(AnchorRange { start: 8, end: 13 }),
        quote: Some("Goals".into()),
        ..CommentAnchor::object(&root_id())
    };
    let ((thread, _), _) = session.execute(&person(), |_, tx| Ok(add_thread(tx, &anchor, &said("Which quarter?"))?)).unwrap();
    for index in 1..count {
        session.execute(&person(), |_, tx| Ok(reply(tx, &thread, &said(&format!("Reply {index}")))?)).unwrap();
    }
    session
}

#[test]
fn repair_leaves_whole_threads_alone() {
    let mut first = with_thread(2);
    let (repaired, _) = first.execute(&Origin::system("repair"), |_, tx| Ok(repair_comment_threads(tx)?)).unwrap();
    assert!(!repaired);
    assert_eq!(threads(first.replica()).unwrap()[0].comments.len(), 2);
}

fn agent() -> Origin {
    Origin::agent("agent:test")
}

/// A draft of `session` (an agent's, made now) and the version it was made at.
fn draft_of(session: &KitSession<NoteModel>) -> (KitSession<NoteModel>, kition_kit::kdm::Version) {
    (session.fork(ActorId::random(), Box::new(MemoryBlobs::default())).unwrap(), session.version())
}

/// The thread's id and its comments' ids.
fn thread_ids(session: &KitSession<NoteModel>) -> (ObjectId, Vec<ObjectId>) {
    let found = threads(session.replica()).unwrap();
    let thread = &found[0];
    (thread.id.clone(), thread.comments.iter().map(|comment| comment.id.clone()).collect())
}

#[test]
fn a_draft_and_the_person_deleting_the_two_comments_of_a_thread_leave_no_empty_thread() {
    // An agent's draft deletes the first comment while the person deletes the second: each leaves
    // the thread one comment, and the list merge (data model 6.6, D6) takes both removals in. The
    // repair after the merge deletes the thread left with none.
    let mut doc = with_thread(2);
    let (thread, comments) = thread_ids(&doc);
    let (mut draft, since) = draft_of(&doc);
    draft.execute(&agent(), |_, tx| Ok(delete_comment(tx, &thread, &comments[0])?)).unwrap();
    doc.execute(&person(), |_, tx| Ok(delete_comment(tx, &thread, &comments[1])?)).unwrap();
    assert_eq!(thread_ids(&doc), (thread.clone(), vec![comments[0].clone()]), "the person's delete leaves the thread");
    let changes = draft.export_changes(&since).unwrap();
    doc.merge_changes(&changes, &agent()).unwrap();
    assert!(threads(doc.replica()).unwrap().is_empty());
    assert_eq!(doc.replica().objects_of_type(COMMENT_THREAD_TYPE), Vec::<ObjectId>::new(), "no empty thread");
    assert_eq!(doc.replica().objects_of_type(COMMENT_TYPE), Vec::<ObjectId>::new());
    assert_eq!(doc.model().body(), "# Plan\n\nGoals\n");
}

#[test]
fn a_draft_replying_to_a_thread_the_person_deleted_meanwhile_is_refused() {
    // The reply would be a comment in no thread: the merge refuses it (D8, the draft writes the
    // thread's list; the document deleted the thread) and changes nothing.
    let mut doc = with_thread(1);
    let (thread, _) = thread_ids(&doc);
    let (mut draft, since) = draft_of(&doc);
    draft.execute(&agent(), |_, tx| Ok(reply(tx, &thread, &said("Next quarter."))?)).unwrap();
    doc.execute(&person(), |_, tx| Ok(delete_thread(tx, &thread)?)).unwrap();
    let before = doc.state_hash().unwrap();
    let changes = draft.export_changes(&since).unwrap();
    let error = doc.merge_changes(&changes, &agent()).unwrap_err().to_string();
    assert_eq!(
        error,
        format!(
            "stale: the draft collides with the document's changes since it was made: {thread} (the draft changes it; the document deleted it; rule D8)"
        )
    );
    assert_eq!(doc.state_hash().unwrap(), before);
    assert_eq!(doc.replica().objects_of_type(COMMENT_THREAD_TYPE), Vec::<ObjectId>::new());
    assert_eq!(doc.replica().objects_of_type(COMMENT_TYPE), Vec::<ObjectId>::new(), "no orphan reply");
}

#[test]
fn undoing_a_thread_takes_the_reply_an_agent_added_since_with_it() {
    // The person's Undo of the step that opened the thread deletes it, and the agent's later reply
    // would be a comment in no thread: the repair after the undo deletes it.
    let mut doc = with_thread(1);
    let (thread, _) = thread_ids(&doc);
    doc.execute(&agent(), |_, tx| Ok(reply(tx, &thread, &said("An agent's reply"))?)).unwrap();
    assert_eq!(thread_ids(&doc).1.len(), 2);
    assert!(doc.undo().unwrap().0);
    assert!(threads(doc.replica()).unwrap().is_empty());
    assert_eq!(doc.replica().objects_of_type(COMMENT_THREAD_TYPE), Vec::<ObjectId>::new());
    assert_eq!(doc.replica().objects_of_type(COMMENT_TYPE), Vec::<ObjectId>::new(), "no orphan reply");
    assert_eq!(doc.model().body(), "# Plan\n\nGoals\n");
}
