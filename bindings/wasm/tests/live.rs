//! The open editor's path, as far as it runs without `web/`: the note in the page is this same
//! document, driven by the person's editor commands, peers' changes (`import_changes`) and the
//! file merged in (`merge_file`), and asked by the guard through the same hooks. What only the
//! page and the host can show (kit-editor's guard callbacks, the host's replica store) is not
//! tested here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::{ContentAsk, ContentPremise, CoveredAsk, CoveredContent, HistoricalBlobs};
use kition_note_kit::{KitDocument, NoteDocument};
use serde_json::{Value, json};

const NOTE: &str = "# Launch plan\n\nIntro text.\n\n## Tasks\n\n- [ ] Draft the announcement\n- [ ] Book the review meeting\n\n## Notes\n\nSome closing notes, long enough that a change set shows them only in part.\n";

fn open() -> NoteDocument {
    NoteDocument::open("live.md", NOTE.as_bytes()).unwrap()
}

fn ids(doc: &mut NoteDocument, kind: &str) -> Vec<(String, String)> {
    let read = doc.execute("projection.read", json!({ "kinds": [kind] })).unwrap();
    read["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|object| (object["label"].as_str().unwrap().to_owned(), object["id"].as_str().unwrap().to_owned()))
        .collect()
}

/// The person types: `text` over the selection from `from` to `to` (characters).
fn typed(doc: &mut NoteDocument, from: usize, to: usize, text: &str) {
    doc.execute("selection.set", json!({ "anchor": from, "focus": to })).unwrap();
    doc.execute("selection.replace", json!({ "text": text })).unwrap();
}

fn at(doc: &NoteDocument, find: &str) -> usize {
    let text = doc.text();
    text[..text.find(find).unwrap()].chars().count()
}

struct NoHistory;

impl HistoricalBlobs for NoHistory {
    fn read(&self, _hash: &str, _offset: usize, _length: usize) -> Result<Vec<u8>, String> {
        Err("unavailable".into())
    }
}

/// Ids follow what the person types: a paragraph typed above, a heading renamed by typing over
/// it, a task checked by its box.
#[test]
fn ids_follow_the_persons_typing() {
    let mut doc = open();
    let sections = ids(&mut doc, "section");
    let tasks = ids(&mut doc, "task");
    let intro = at(&doc, "Intro text.");
    typed(&mut doc, intro, intro, "A paragraph typed first.\n\n");
    let notes = at(&doc, "Notes");
    typed(&mut doc, notes, notes + 5, "Closing");
    let book = at(&doc, "[ ] Book") + 1;
    typed(&mut doc, book, book + 1, "x");
    let now = ids(&mut doc, "section");
    assert_eq!(now[2], ("Launch plan › Closing".to_owned(), sections[2].1.clone()), "{now:?}");
    assert_eq!(now[..2], sections[..2]);
    let checked = doc.execute("projection.read", json!({ "kinds": ["task"] })).unwrap();
    assert_eq!(checked["objects"][1]["id"], json!(tasks[1].1));
    assert_eq!(checked["objects"][1]["done"], json!(true));
}

/// The file changed while the note is open: the editor merges it (`merge_file`). The replica
/// keeps its threads (they never left it), and the ids follow the merge as a change made in the
/// file: a renamed section keeps its id.
#[test]
fn a_file_merged_into_the_open_note_keeps_threads_and_ids() {
    let mut doc = open();
    let added = doc.execute("comment.add", json!({ "quote": "Book the review meeting", "body": "When?" })).unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    let notes = ids(&mut doc, "section")[2].1.clone();
    doc.merge_file(NOTE.replace("## Notes", "## Closing").replace("Intro text.", "Intro text.\n\nMore from the file.").as_bytes()).unwrap();
    let threads = doc.execute("comment.threads", json!({})).unwrap();
    assert_eq!(threads["threads"][0]["id"], json!(thread));
    assert_eq!(threads["threads"][0]["from"], json!(at(&doc, "Book the review meeting")));
    assert_eq!(ids(&mut doc, "section")[2], ("Launch plan › Closing".to_owned(), notes));
}

/// The hooks the guard asks answer in the page's document as headless: a write by id is covered
/// with the body at its basis; a write by no place compares the body it is given whole, and
/// cannot say from an excerpt of a body this process never held (the page holds its own bodies;
/// kition-sdk `1b9d5ec`'s editor reader does not ask `content_at` at all, see the report).
#[test]
fn the_guard_hooks_answer_in_the_open_note() {
    let mut doc = open();
    let tasks = ids(&mut doc, "section")[1].1.clone();
    let before = doc.text().to_owned();
    let notes = at(&doc, "Some closing");
    typed(&mut doc, notes, notes + 4, "Our");
    let write = json!({ "section": tasks, "position": "end", "markdown": "- [ ] Mine" });
    let under = doc.under("section.insert", &write).unwrap().unwrap();
    assert!(under.covers.is_some(), "{under:?}");
    let ask = CoveredAsk { id: json!("note"), before: serde_json::Map::from_iter([("body".to_owned(), json!(before))]), ..CoveredAsk::default() };
    let covered = doc.covered("section.insert", &write, &[ask]).unwrap().unwrap();
    assert_eq!((covered.covered[0].covered, covered.covered[0].content), (Some(true), Some(CoveredContent::Same)), "{covered:?}");
    // By no place: the whole body says it; an excerpt this process never kept cannot.
    let append = json!({ "markdown": "The end." });
    let region = doc.under("block.append", &append).unwrap().unwrap().content[0].region.clone();
    let premise = ContentPremise { id: json!("note"), field: "body".into(), region };
    let whole = ContentAsk { premise: premise.clone(), before: Value::String(before.clone()), step: Default::default() };
    assert_eq!(doc.content_at(&whole, &NoHistory).unwrap(), Some(false));
    let excerpt = ContentAsk { premise, before: json!({ "excerpt": "…never seen…", "length": 300 }), step: Default::default() };
    assert_eq!(doc.content_at(&excerpt, &NoHistory).unwrap(), None);
}

/// `edit.undo` by an agent while the person edits in the open note (kition-sdk undo footprints): it
/// holds what the agent's step wrote, the note's body, whole. (Headless, another agent's step
/// refuses an agent's undo outright, and a file changed by another program is read again with no
/// steps to undo.)
#[test]
fn an_agent_undo_holds_what_its_step_wrote() {
    use kition_note_kit::{KitSessionDocument, Origin};
    let mut doc = open();
    assert!(doc.under("edit.undo", &json!({})).unwrap().unwrap().premises.is_empty());
    doc.execute_as("text.findReplace", json!({ "find": "Intro text", "replace": "Opening" }), &Origin::agent("agent-me")).unwrap();
    let under = doc.under("edit.undo", &json!({})).unwrap().unwrap();
    let held: Vec<(Value, Option<Vec<String>>)> = under.premises.iter().map(|premise| (premise.id.clone(), premise.fields.clone())).collect();
    assert_eq!(held, [(json!("note"), Some(vec!["body".to_owned()]))], "{under:?}");
    // The person's own Undo scope is not the agent's.
    assert!(doc.under("edit.undo", &json!({ "scope": "person" })).unwrap().unwrap().premises.is_empty());
}
