//! Every parameter of every write says what it is to the premise guard: an object by id (`targets`,
//! `objects`, typed), a place in the note's text (`places`), or a value the write puts in or a
//! setting, which names nothing (listed here with why). Read from each handler in `src/run.rs`; the
//! declarations are in `src/premise.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitCommand;
use kition_note_kit::{KitDocument, NoteDocument};

/// Parameters that name nothing, by command (`*`: every command), with why.
const VALUES: &[(&str, &str, &str)] = &[
    ("*", "unit", "how the write counts positions"),
    ("*", "expected", "the text the write checks its place holds; the place is named by its place parameters"),
    ("*", "text", "the text the write puts in"),
    ("*", "markdown", "the Markdown the write puts in"),
    ("text.findReplace", "replace", "the text each match becomes"),
    ("text.findReplace", "caseSensitive", "how `find` matches"),
    ("text.findReplace", "regex", "how `find` matches"),
    ("section.replace", "withHeading", "whether the heading line is replaced too"),
    ("heading.setLevel", "level", "the level written"),
    ("heading.setLevel", "style", "the heading style written"),
    ("block.setId", "id", "the new block id written; it names no block before the write"),
    ("task.toggle", "done", "the state written"),
    ("property.set", "value", "the value written"),
    ("property.rename", "to", "the new key written; it names no property before the write"),
    ("table.add", "columns", "the size of the new table"),
    ("table.add", "rows", "the size of the new table"),
    ("table.add", "header", "the header cells written"),
    ("table.insertRow", "cells", "the cells written"),
    ("table.insertColumn", "header", "the header cell written"),
    ("table.insertColumn", "align", "the alignment written"),
    ("table.align", "align", "the alignment written"),
    ("link.insert", "destination", "a file or address the link points to, outside the note"),
    ("link.insert", "embed", "whether the link is an embed"),
    ("link.insert", "style", "the link syntax written"),
    ("link.update", "destination", "a file or address the link points to, outside the note"),
    ("link.retarget", "to", "the new destination written"),
    ("comment.add", "body", "the comment's text"),
    ("comment.add", "author", "a principal id of the workspace, not an object of the note"),
    ("comment.add", "name", "the author's display name"),
    ("comment.add", "created", "a time"),
    ("comment.reply", "body", "the comment's text"),
    ("comment.reply", "author", "a principal id of the workspace, not an object of the note"),
    ("comment.reply", "name", "the author's display name"),
    ("comment.reply", "created", "a time"),
    ("comment.resolve", "resolved", "the state written"),
    ("file.setLineEndings", "eol", "the line endings written"),
    ("file.setLineEndings", "bom", "whether the file starts with a byte order mark"),
    ("edit.undo", "scope", "whose steps the undo takes back; names no object"),
    ("edit.redo", "scope", "whose steps the redo puts back; names no object"),
];

/// Parameter names that say they name something (an id, a thread, a comment, a heading, a key).
fn names_something(name: &str) -> bool {
    matches!(name, "id" | "ids" | "thread" | "comment" | "heading" | "toHeading" | "key" | "blockId") || name.ends_with("Id") || name.ends_with("Ids")
}

/// The writes an agent's step or a host can run with params (the editor's own commands, which act
/// on its selection, are not offered to agents and take no place).
fn writes() -> Vec<KitCommand> {
    let doc = NoteDocument::create("a.md", 1, 1).unwrap();
    doc.commands()
        .into_iter()
        .filter(|command| command.mutates && command.schema.is_some())
        .filter(|command| {
            ![
                "selection.",
                "edit.cut",
                "edit.indent",
                "edit.outdent",
                "edit.move",
                "edit.delete",
                "edit.sort",
                "edit.trim",
                "format.",
                "list.",
                "insert.",
            ]
            .iter()
            .any(|prefix| command.id.starts_with(prefix))
        })
        .collect()
}

fn marked(command: &KitCommand, name: &str) -> bool {
    command.targets == Some(name)
        || command.objects.contains(&name)
        || command.places.contains(&name)
        || command.name_params.iter().any(|(param, _)| *param == name)
}

#[test]
fn the_sdk_scanner_finds_no_unmarked_integer_id_parameter() {
    // The note has no integer ids (its objects are the root and comments, by string ids). The
    // SDK's scanner reads integer parameters whose name or description says id: before the
    // declarations it found `block.setId`'s `occurrence` ("which block with that id"), a place.
    kition_kit::testing::check_id_params(&writes(), &[]).unwrap();
}

#[test]
fn every_parameter_of_every_write_is_an_object_a_place_or_a_listed_value() {
    let mut problems = Vec::new();
    let mut counts = (0, 0, 0);
    for command in writes() {
        for name in command.schema.as_ref().unwrap().as_object().unwrap().keys() {
            let value = VALUES.iter().any(|(id, param, _)| (*id == command.id || *id == "*") && param == name);
            match (marked(&command, name), value) {
                (true, true) => problems.push(format!("{}.{name} is marked and listed as a value", command.id)),
                (true, false) if command.places.contains(&name.as_str()) => counts.1 += 1,
                (true, false) => counts.0 += 1,
                (false, true) => counts.2 += 1,
                (false, false) => problems.push(format!("{}.{name} is neither marked nor listed", command.id)),
            }
        }
    }
    println!("objects {}, places {}, values {}", counts.0, counts.1, counts.2);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_parameter_that_names_something_is_marked_or_allowed_with_a_reason() {
    let allowed: &[(&str, &str)] = &[("block.setId", "id")];
    let mut problems = Vec::new();
    for command in writes() {
        for name in command.schema.as_ref().unwrap().as_object().unwrap().keys().filter(|name| names_something(name)) {
            if !marked(&command, name) && !allowed.contains(&(command.id.as_str(), name.as_str())) {
                problems.push(format!("{}.{name}", command.id));
            }
        }
    }
    assert!(problems.is_empty(), "parameters naming something, unmarked: {problems:?}");
}

#[test]
fn ids_name_their_types() {
    for command in writes() {
        for name in command.targets.iter().copied().chain(command.objects.iter().copied()) {
            let types = command.object_types.iter().find(|(param, _)| *param == name).map(|(_, types)| *types);
            let expected: &[&str] = match name {
                "thread" => &["CommentThread"],
                "comment" => &["Comment"],
                other => panic!("{}.{other}: an id parameter this test does not know", command.id),
            };
            assert_eq!(types, Some(expected), "{}.{name}", command.id);
        }
    }
}
