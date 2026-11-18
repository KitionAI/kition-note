//! Regressions of the note's write commands: each test is the probe of one case under
//! `cases/regressions/`, and names it.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use kition_kit::session::KitSessionDocument;
use kition_note_kit::NoteDocument;
use serde_json::{Value, json};

fn open(text: &str) -> NoteDocument {
    NoteDocument::open("note.md", text.as_bytes()).unwrap()
}

fn run(doc: &mut NoteDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

/// `cases/regressions/issue-setext-heading-to-atx`: an underlined heading whose text spans lines
/// keeps all of it when it becomes an ATX heading. The lines are joined with one space each, the
/// way Markdown reads the line breaks inside a heading, so the heading's text is unchanged.
#[test]
fn setext_heading_to_atx_keeps_every_line() {
    let text = "# Plan\n\nTitle part\ncontinued here\n---\n\nBody.\n";
    let mut doc = open(text);
    let before = run(&mut doc, "outline.read", json!({}));
    assert_eq!(before["headings"][1]["path"], json!(["Plan", "Title part continued here"]));
    run(&mut doc, "heading.setLevel", json!({ "line": 3, "level": 3, "style": "atx" }));
    assert_eq!(doc.text(), "# Plan\n\n### Title part continued here\n\nBody.\n");
    let after = run(&mut doc, "outline.read", json!({}));
    assert_eq!(after["headings"][1]["path"], before["headings"][1]["path"], "the heading's text is the same");

    // Indented lines and Windows line endings: the first line's indentation stays.
    let text = "Intro\n\n  One\r\n Two\r\n=====\r\n\r\nBody.\r\n";
    let mut doc = open(text);
    run(&mut doc, "heading.setLevel", json!({ "line": 3, "level": 4, "style": "atx" }));
    assert_eq!(doc.text(), "Intro\n\n  #### One Two\r\n\r\nBody.\r\n");
}

/// `cases/regressions/issue-block-replace-ids`: a block id the replacement sets is checked like
/// every other id write (`block.setId`): one another block carries is refused. And a tight list
/// item named by its id keeps its list marker and checkbox when only its text is given.
#[test]
fn block_replace_checks_new_ids_and_keeps_the_list_marker() {
    let text =
        "# Plan\n\nIntro text. ^intro\n\n- [ ] Draft ^draft\n- [x] Book the room ^book\n- Plain item ^plain\n  - Nested ^nested\n\n1. First ^first\n";
    let mut doc = open(text);
    let error = doc.execute("block.replace", json!({ "blockId": "intro", "markdown": "New intro ^book" })).unwrap_err();
    assert!(error.starts_with("refused:"), "{error}");
    assert_eq!(doc.text(), text, "nothing is written");
    run(&mut doc, "block.replace", json!({ "blockId": "intro", "markdown": "Same id again. ^intro" }));
    assert!(doc.text().contains("\nSame id again. ^intro\n"), "{}", doc.text());
    run(&mut doc, "block.replace", json!({ "blockId": "intro", "markdown": "A new id. ^lead" }));
    assert!(doc.text().contains("\nA new id. ^lead\n"), "{}", doc.text());

    let mut doc = open(text);
    run(&mut doc, "block.replace", json!({ "blockId": "book", "markdown": "Booked" }));
    assert!(doc.text().contains("\n- [x] Booked ^book\n"), "the marker and the checkbox stay:\n{}", doc.text());
    run(&mut doc, "block.replace", json!({ "blockId": "plain", "markdown": "Changed\n\nSecond paragraph" }));
    assert!(doc.text().contains("\n- Changed\n\n  Second paragraph ^plain\n  - Nested ^nested\n"), "{}", doc.text());
    run(&mut doc, "block.replace", json!({ "blockId": "nested", "markdown": "Inner" }));
    assert!(doc.text().contains("\n  - Inner ^nested\n"), "{}", doc.text());
    run(&mut doc, "block.replace", json!({ "blockId": "first", "markdown": "One" }));
    assert!(doc.text().ends_with("\n1. One ^first\n"), "{}", doc.text());
    // A replacement that starts with its own marker is written as given.
    let mut doc = open(text);
    run(&mut doc, "block.replace", json!({ "blockId": "draft", "markdown": "- [x] Drafted" }));
    assert!(doc.text().contains("\n- [x] Drafted ^draft\n"), "{}", doc.text());
}

/// `cases/regressions/issue-block-set-id-line`: `block.setId` by line marks a Markdown block, so a
/// line of the front matter, of a code block or of a table is refused as an invalid target; giving
/// a line the id it already has changes nothing and is not refused.
#[test]
fn block_set_id_by_line_takes_only_block_lines() {
    let text = "---\ntitle: Plan\n---\n# Plan\n\nIntro text. ^intro\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| - | - |\n| 1 | 2 |\n";
    for line in [1, 2, 3, 8, 9, 10, 12, 13, 14] {
        let mut doc = open(text);
        let error = doc.execute("block.setId", json!({ "line": line, "id": "t" })).unwrap_err();
        assert!(error.starts_with("invalid-params:"), "line {line}: {error}");
        assert_eq!(doc.text(), text, "line {line}");
    }
    let mut doc = open(text);
    let answer = run(&mut doc, "block.setId", json!({ "line": 6, "id": "intro" }));
    assert_eq!(doc.text(), text, "{answer}");
    run(&mut doc, "block.setId", json!({ "line": 6, "id": "lead" }));
    assert!(doc.text().contains("Intro text. ^lead\n"), "{}", doc.text());
    // Another block's id is still refused.
    let error = doc.execute("block.setId", json!({ "line": 4, "id": "lead" })).unwrap_err();
    assert!(error.starts_with("refused:"), "{error}");
}

/// `cases/regressions/issue-task-toggle-block-id`: `task.toggle` by block id toggles the task the
/// id marks, wherever on the task the id stands: on a continuation line, or on its own line after
/// a one-task list.
#[test]
fn task_toggle_by_block_id_finds_the_task_the_id_marks() {
    let text = "# Tasks\n\n- [ ] Draft the post\n  with a second line ^draft\n- [ ] Other\n\nText.\n\n- [ ] Alone\n\n^alone\n";
    let mut doc = open(text);
    let answer = run(&mut doc, "task.toggle", json!({ "blockId": "draft" }));
    assert_eq!(answer["done"], true);
    assert!(doc.text().contains("- [x] Draft the post\n"), "{}", doc.text());
    assert!(doc.text().contains("- [ ] Other\n"), "{}", doc.text());
    run(&mut doc, "task.toggle", json!({ "blockId": "alone" }));
    assert!(doc.text().contains("- [x] Alone\n"), "{}", doc.text());
}

/// `cases/regressions/issue-table-create-line`: `table.create` by line inserts before a line that
/// starts a block; a line inside a code block, the front matter or a table is refused instead of
/// writing the table into the code, the properties or the other table.
#[test]
fn table_create_by_line_refuses_lines_inside_other_blocks() {
    let text = "---\ntitle: Plan\n---\n# Plan\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\nEnd.\n";
    for line in [1, 2, 3, 7, 8, 11, 12] {
        let mut doc = open(text);
        let error = doc.execute("table.create", json!({ "line": line, "columns": 2 })).unwrap_err();
        assert!(error.starts_with("invalid-params:"), "line {line}: {error}");
        assert_eq!(doc.text(), text, "line {line}");
    }
    // Before the fence, before the other table and before a paragraph it lands as before.
    for (line, before) in [(6, "```rust"), (10, "| A | B |"), (14, "End.")] {
        let mut doc = open(text);
        run(&mut doc, "table.create", json!({ "line": line, "columns": 2, "header": ["X", "Y"] }));
        let at = doc.text().find("| X | Y |").unwrap();
        assert!(doc.text()[at..].contains(&format!("\n\n{before}")), "line {line}:\n{}", doc.text());
        assert!(doc.text().contains("```rust\nfn main() {}\n```\n"), "line {line}:\n{}", doc.text());
    }
}

/// `cases/regressions/issue-link-escaping`: `link.update` and `links.retarget` write what they are
/// given as Markdown reads it back: a destination with spaces, parentheses or angle brackets in an
/// angle-bracket destination (escaped inside it), or percent-encoded where the link is written so;
/// link text with brackets escaped. A wikilink cannot escape `]]` or `|`, so they are refused.
#[test]
fn link_writes_escape_what_they_write() {
    let destination = |doc: &mut NoteDocument, line: u64| {
        let links = run(doc, "links.list", json!({}));
        links["links"].as_array().unwrap().iter().find(|link| link["line"] == line).cloned().unwrap()
    };
    let text = "See [the site](https://example.com/site).\n\nA [[timeline]] link and ![chart](chart.png).\n\nAlso [doc](my%20doc.md).\n";

    let mut doc = open(text);
    run(&mut doc, "link.update", json!({ "line": 1, "index": 0, "destination": "notes/a (draft).md", "text": "the x](y site" }));
    let link = destination(&mut doc, 1);
    assert_eq!(link["destination"], "notes/a (draft).md", "{}", doc.text());
    // links.list answers the text as written.
    assert_eq!(link["text"], "the x\\](y site", "{}", doc.text());
    assert!(doc.text().ends_with("link and ![chart](chart.png).\n\nAlso [doc](my%20doc.md).\n"), "{}", doc.text());

    let mut doc = open(text);
    run(&mut doc, "link.update", json!({ "line": 1, "index": 0, "destination": "a <b>.md" }));
    // links.list answers the destination as written; the file it names is read back unescaped.
    assert_eq!(destination(&mut doc, 1)["destination"], "a \\<b\\>.md", "{}", doc.text());
    assert_eq!(run(&mut doc, "links.retarget", json!({ "from": "a <b>.md", "to": "c.md" }))["links"], 1, "{}", doc.text());
    assert!(doc.text().starts_with("See [the site](<c.md>)."), "{}", doc.text());

    let mut doc = open(text);
    let error = doc.execute("link.update", json!({ "line": 3, "index": 0, "destination": "timeline", "text": "a]]b" })).unwrap_err();
    assert!(error.starts_with("invalid-params:"), "{error}");
    assert_eq!(doc.text(), text);

    let mut doc = open(text);
    let answer = run(&mut doc, "links.retarget", json!({ "from": "chart.png", "to": "img/a)b.png" }));
    assert_eq!(answer["links"], 1);
    let images: Vec<Value> =
        run(&mut doc, "links.list", json!({}))["links"].as_array().unwrap().iter().filter(|link| link["kind"] == "image").cloned().collect();
    assert_eq!(images.len(), 1, "the image is still an image:\n{}", doc.text());
    assert_eq!(images[0]["destination"], "img/a)b.png", "{}", doc.text());

    let mut doc = open(text);
    run(&mut doc, "links.retarget", json!({ "from": "my doc.md", "to": "your (new) doc.md" }));
    assert!(doc.text().contains("[doc](your%20%28new%29%20doc.md)"), "percent-encoded as written:\n{}", doc.text());
    assert_eq!(destination(&mut doc, 5)["kind"], "markdown", "{}", doc.text());

    let mut doc = open(text);
    let error = doc.execute("links.retarget", json!({ "from": "timeline", "to": "a|b" })).unwrap_err();
    assert!(error.starts_with("invalid-params:"), "{error}");
    assert_eq!(doc.text(), text);

    // link.insert writes its text the same way: escaped, not stripped of its brackets.
    let mut doc = open("Text.\n");
    run(&mut doc, "link.insert", json!({ "at": 5, "destination": "a <b>.md", "text": "x]y", "style": "markdown" }));
    let link = destination(&mut doc, 1);
    assert_eq!((link["destination"].as_str(), link["text"].as_str()), (Some("a \\<b\\>.md"), Some("x\\]y")), "{}", doc.text());
}

/// `cases/regressions/issue-comment-author`: a comment's author is the caller. An agent (or any
/// caller with a principal of its own) that names another author is refused, and its own principal
/// is accepted; only the page's own person, whose principal the host passes in `author`, sets it.
#[test]
fn comment_author_is_the_caller() {
    let text = "# Plan\n\nShip the beta.\n";
    let agent = Origin::agent("agent:me");
    let mut doc = open(text);
    let error = doc.execute_as("comment.add", json!({ "quote": "beta", "body": "Which?", "author": "person:bo" }), &agent).unwrap_err();
    assert!(error.starts_with("invalid-params:"), "{error}");
    assert!(run(&mut doc, "comment.threads", json!({}))["threads"].as_array().unwrap().is_empty(), "no thread was added");

    let added = doc.execute_as("comment.add", json!({ "quote": "beta", "body": "Which?", "author": "agent:me" }), &agent).unwrap();
    let thread = added["thread"].as_str().unwrap().to_owned();
    let error = doc.execute_as("comment.reply", json!({ "thread": thread, "body": "Mine.", "author": "person:bo" }), &agent).unwrap_err();
    assert!(error.starts_with("invalid-params:"), "{error}");
    doc.execute_as("comment.reply", json!({ "thread": thread, "body": "Mine." }), &agent).unwrap();
    let threads = run(&mut doc, "comment.threads", json!({}));
    let authors: Vec<&Value> = threads["threads"][0]["comments"].as_array().unwrap().iter().map(|comment| &comment["author"]).collect();
    assert_eq!(authors, [&json!("agent:me"), &json!("agent:me")]);

    // The page's person: the host's principal for them comes in author.
    run(&mut doc, "comment.reply", json!({ "thread": thread, "body": "From the page.", "author": "person:ann" }));
    let threads = run(&mut doc, "comment.threads", json!({}));
    assert_eq!(threads["threads"][0]["comments"][2]["author"], "person:ann");
}

fn select(doc: &mut NoteDocument, anchor: usize, focus: usize) {
    run(doc, "selection.set", json!({ "anchor": anchor, "focus": focus }));
}

/// `cases/regressions/issue-editor-list-front-matter`: the editor's list, quote and heading
/// commands change body lines only; the front matter's fences and properties stay as written.
#[test]
fn editor_line_commands_leave_the_front_matter() {
    let text = "---\ntitle: Plan\n---\n# Plan\n\nText.\n";
    for command in ["list.bullet", "list.numbered", "list.task", "format.quote", "format.heading2"] {
        // A note opens with the caret at its start, on the front matter's opening fence.
        let mut doc = open(text);
        let _ = doc.execute(command, json!({}));
        assert!(doc.text().starts_with("---\ntitle: Plan\n---\n"), "{command}:\n{}", doc.text());
        // A selection from the front matter into the body changes the body's lines only.
        let mut doc = open(text);
        select(&mut doc, 4, text.find("Plan\n\n").unwrap() + 2);
        let _ = doc.execute(command, json!({}));
        assert!(doc.text().starts_with("---\ntitle: Plan\n---\n"), "{command}:\n{}", doc.text());
    }
}

/// `cases/regressions/issue-editor-clear-images`: Clear formatting keeps an image (or embed) as it
/// is, takes a link's brackets away as before, and never pairs one link's `[` with another's `](`.
#[test]
fn clear_formatting_keeps_images() {
    let text = "See [the site](https://example.com) and ![chart](chart.png) **now**.\n";
    let mut doc = open(text);
    select(&mut doc, 0, text.len() - 1);
    run(&mut doc, "format.clear", json!({}));
    assert_eq!(doc.text(), "See the sitehttps://example.com and ![chart](chart.png) now.\n");

    let text = "- [ ] see [a](b) and ![[pic.png]] [c](d)\n";
    let mut doc = open(text);
    select(&mut doc, 0, text.len() - 1);
    run(&mut doc, "format.clear", json!({}));
    assert_eq!(doc.text(), "- [ ] see ab and ![[pic.png]] cd\n");
}

/// `cases/regressions/issue-editor-cut-text`: Cut's clipboard text is exactly the text it removes,
/// also when the deletion takes the opener of a span whose closer is cut.
#[test]
fn cut_answers_the_text_it_removes() {
    for (text, from, to) in [("a **b** c", 4, 9), ("a **b** c", 2, 9), ("plain text", 0, 5)] {
        let mut doc = open(text);
        select(&mut doc, from, to);
        let cut = run(&mut doc, "edit.cut", json!({}));
        let removed = text.len() - doc.text().len();
        let start = (0..=text.len()).find(|at| !doc.text().starts_with(&text[..*at + 1]) || *at == text.len()).unwrap();
        assert_eq!(cut["text"].as_str().unwrap(), &text[start..start + removed], "{text:?} {from}..{to}: left {:?}", doc.text());
    }
}

/// `cases/regressions/issue-editor-local-date`: Insert date and Insert date and time write the
/// local date and time: the page's time zone, or the offset given in `utcOffset`, not UTC.
#[test]
fn insert_date_uses_the_local_time() {
    let date = |offset: i64| {
        let mut doc = open("Today: \n");
        select(&mut doc, 7, 7);
        run(&mut doc, "insert.date", json!({ "utcOffset": offset }));
        doc.text().trim_start_matches("Today: ").trim_end().to_owned()
    };
    // Fourteen hours ahead of UTC and twelve behind are always on different dates.
    let (east, west) = (date(14 * 60), date(-12 * 60));
    assert_eq!((east.len(), west.len()), (10, 10), "{east} {west}");
    assert!(east > west, "{east} is after {west}");
    let stamp = |offset: i64| {
        let mut doc = open("At \n");
        select(&mut doc, 3, 3);
        run(&mut doc, "insert.timestamp", json!({ "utcOffset": offset }));
        doc.text().trim_start_matches("At ").trim_end().to_owned()
    };
    let (utc, plus_ninety) = (stamp(0), stamp(90));
    let minutes = |stamp: &str| stamp[11..13].parse::<i64>().unwrap() * 60 + stamp[14..16].parse::<i64>().unwrap();
    assert_eq!((minutes(&plus_ninety) - minutes(&utc)).rem_euclid(24 * 60), 90, "{utc} {plus_ninety}");
    // Out of range offsets are refused.
    let mut doc = open("x\n");
    let error = doc.execute("insert.date", json!({ "utcOffset": 100_000 })).unwrap_err();
    assert!(error.starts_with("invalid-params:"), "{error}");
}

/// `cases/regressions/issue-comment-anchor-moves`: a comment thread's anchor moves with the
/// note's writes, so a long insertion before the quoted text no longer makes the thread jump to
/// an equal word nearer its old place (here the front matter's tag).
#[test]
fn comment_anchors_move_with_writes() {
    let text = "---\ntitle: Launch plan\ntags: [beta, launch]\n---\n\n# Launch plan\n\nIntro text with a link and beta. ^intro\n\nMore.\n";
    let quoted = text.rfind("beta").unwrap();
    let threads_line = |doc: &mut NoteDocument| run(doc, "comment.threads", json!({}))["threads"][0]["line"].clone();
    let agent = Origin::agent("agent:me");
    let mut doc = open(text);
    doc.execute_as("comment.add", json!({ "from": quoted, "to": quoted + 4, "body": "Which beta?" }), &agent).unwrap();
    assert_eq!(threads_line(&mut doc), 8);
    let long = "x".repeat(80);
    doc.execute_as("text.insert", json!({ "at": 63, "text": format!("{long}\n\n") }), &agent).unwrap();
    assert_eq!(threads_line(&mut doc), 10, "the thread stays on the text it quotes:\n{}", doc.text());
    let thread = &run(&mut doc, "comment.threads", json!({}))["threads"][0];
    let from = thread["from"].as_u64().unwrap() as usize;
    assert_eq!(doc.text().chars().skip(from).take(4).collect::<String>(), "beta");
    // The other writes carry it too: a section write and the whole note written again.
    run(&mut doc, "section.insert", json!({ "heading": ["Launch plan"], "position": "start", "markdown": &format!("{long} beta") }));
    let whole = doc.text().replacen("# Launch plan", &format!("# Launch plan\n\n{long}"), 1);
    run(&mut doc, "note.write", json!({ "markdown": whole }));
    let thread = &run(&mut doc, "comment.threads", json!({}))["threads"][0];
    let from = thread["from"].as_u64().unwrap() as usize;
    let line = thread["line"].as_u64().unwrap() as usize;
    assert_eq!(doc.text().lines().nth(line - 1).unwrap(), "Intro text with a link and beta. ^intro", "{}", doc.text());
    assert_eq!(doc.text().chars().skip(from).take(4).collect::<String>(), "beta");
}

/// `cases/regressions/issue-find-replace-front-matter`: find and find and replace act on the
/// body; the front matter's properties change through `property.set`, so a match there is neither
/// found nor counted.
#[test]
fn find_and_replace_skip_the_front_matter() {
    let text = "---\ntitle: Plan\ntags: [beta, launch]\n---\n# Plan\n\nShip the beta soon.\n\nThe beta ends in May.\n";
    let found = run(&mut open(text), "text.find", json!({ "find": "beta" }));
    assert_eq!(found["count"], 2, "{found}");
    assert_eq!(found["matches"][0]["line"], 7, "{found}");
    assert_eq!(run(&mut open(text), "text.find", json!({ "find": "launch" }))["count"], 0);
    let mut doc = open(text);
    let replaced = run(&mut doc, "text.findReplace", json!({ "find": "beta", "replace": "pilot" }));
    assert_eq!(replaced["replaced"], 1);
    assert!(doc.text().contains("tags: [beta, launch]"), "the tags property is untouched:\n{}", doc.text());
    assert!(doc.text().contains("Ship the pilot soon."), "the first match in the body is replaced:\n{}", doc.text());

    let mut doc = open(text);
    run(&mut doc, "text.findReplace", json!({ "find": "beta", "replace": "pilot", "occurrence": 2 }));
    assert!(doc.text().contains("The pilot ends in May."), "the second match counts in the body:\n{}", doc.text());

    let mut doc = open(text);
    let replaced = run(&mut doc, "text.findReplace", json!({ "find": "beta", "replace": "pilot", "occurrence": "all" }));
    assert_eq!(replaced["replaced"], 2);
    assert!(doc.text().contains("tags: [beta, launch]"), "{}", doc.text());

    // A word only the front matter holds is not found.
    let mut doc = open(text);
    let error = doc.execute("text.findReplace", json!({ "find": "launch", "replace": "x" })).unwrap_err();
    assert!(error.starts_with("not-found:"), "{error}");
    assert_eq!(doc.text(), text);
}
