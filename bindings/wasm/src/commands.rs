//! The note's command catalogue (`docs/design.md` section 3.2): every command with its title, what
//! it does (`KitCommand.description`), its parameters as notation and as a `ParameterSchemaSpec`
//! (KHP A5, A6), what it answers as a `ValueSchemaSpec` (`KitCommand.output_schema`, KHP A1), and
//! whether it changes the document. `run.rs`, `export.rs` and `editor.rs` run them;
//! `tests/commands.rs` checks every command's real answers against what it declares.
//!
//! Parameter names never use the words the Kit Process Protocol reads as file access (`path`,
//! `target`, `file`, ...; kition-headless `FILE_PARAMS`): a heading is named by `heading` (its
//! path from the top) or `line`, and a link's destination is `destination`.

use kition_kit::KitCommand;
use serde_json::{Map, Value, json};

const UNIT: &str = "How positions are counted: Unicode characters (scalar, the default), or UTF-16 code units as the editor counts them.";
const EXPECTED: &str = "The text there now; the command is refused when it differs.";
const HEADING: &str = "The heading's path from the top, each element a heading's plain text, for example [\"Plan\", \"Risks\"]; or give line.";
const HEADING_LINE: &str = "The heading's line, from 1, instead of heading.";
const TABLE: &str = "The table's index in the note, from 0 (document order).";
const SECTION_ID: &str = "The section's id from outline.read (\"section:3\"), instead of heading or line: it names the same section after a rename or a move, for as long as the note stays open.";

fn integer(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

fn one_of(values: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": values, "description": description })
}

fn strings(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": description })
}

fn any(description: &str) -> Value {
    json!({ "type": "json", "description": description })
}

fn required(mut spec: Value) -> Value {
    if let Some(object) = spec.as_object_mut() {
        object.insert("required".into(), Value::Bool(true));
    }
    spec
}

fn unit() -> Value {
    json!({ "type": "string", "enum": ["scalar", "utf16"], "default": "scalar", "description": UNIT })
}

fn schema(fields: &[(&str, Value)]) -> Value {
    Value::Object(fields.iter().map(|(name, spec)| ((*name).to_owned(), spec.clone())).collect::<Map<_, _>>())
}

fn heading_fields() -> Vec<(&'static str, Value)> {
    vec![("heading", strings(HEADING)), ("line", integer(HEADING_LINE)), ("section", string(SECTION_ID))]
}

fn with_heading(mut fields: Vec<(&'static str, Value)>) -> Value {
    let mut all = heading_fields();
    all.append(&mut fields);
    schema(&all)
}

// What a command answers (KHP A1 `output.schema`): one `ValueSchemaSpec`. Objects are closed
// (`additionalProperties: false`) and a field every answer has is `required`, so a schema says
// exactly what comes back.

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn array(items: Value, description: &str) -> Value {
    json!({ "type": "array", "items": items, "description": description })
}

/// An object with exactly `fields`.
fn object(fields: &[(&str, Value)]) -> Value {
    json!({ "type": "object", "additionalProperties": false, "properties": schema(fields) })
}

fn described(mut node: Value, description: &str) -> Value {
    if let Some(object) = node.as_object_mut() {
        object.insert("description".into(), Value::String(description.into()));
    }
    node
}

/// `node`, or `null`.
fn or_null(node: Value, description: &str) -> Value {
    json!({ "oneOf": [node, { "type": "null" }], "description": description })
}

/// The answer of a command that answers nothing more: `{}`.
fn nothing() -> Value {
    described(object(&[]), "An empty object: the command answers nothing more.")
}

/// The answer of an edit command: whether the text changed.
fn edited() -> Value {
    object(&[("changed", required(boolean("Whether the note's text changed (false when it already read so).")))])
}

/// The answer of `text.insert`, `text.delete` and `text.replace`.
fn new_length() -> Value {
    object(&[("length", required(integer("The note's length after the edit, in the request's unit.")))])
}

fn thread_only() -> Value {
    object(&[("thread", required(string("The thread's id.")))])
}

fn new_comment() -> Value {
    object(&[("thread", required(string("The thread's id."))), ("comment", required(string("The new comment's id.")))])
}

/// The answer of the fold commands.
fn folds() -> Value {
    object(&[("folded", required(integer("How many headings and list items are folded now.")))])
}

/// `edit.undo` and `edit.redo`: whether a step was taken, and what it made stale on screen.
fn history() -> Value {
    object(&[
        ("done", required(boolean("Whether there was a step to take; false changes nothing."))),
        (
            "damage",
            required(described(
                object(&[
                    ("kind", required(one_of(&["none", "full", "rects"], "Nothing, the whole view, or the rectangles in rects."))),
                    (
                        "rects",
                        array(
                            array(json!({ "type": "integer" }), "[x0, y0, x1, y1]"),
                            "With kind rects: the areas to draw again, in document pixels.",
                        ),
                    ),
                ]),
                "What the step made stale on screen.",
            )),
        ),
    ])
}

/// What printed pages could not draw as it is (`page.layout`, `file.export` of a PDF).
fn unsupported() -> Value {
    described(
        object(&[
            ("missingGlyphs", required(integer("Characters drawn as boxes: no page font has them."))),
            ("missingSample", required(string("Up to 24 of those characters, in order of first use."))),
            ("images", required(integer("Images drawn as labelled boxes."))),
            ("embeds", required(integer("Embeds drawn as labelled boxes."))),
            ("math", required(integer("Math drawn as its source."))),
            ("mermaid", required(integer("Mermaid diagrams drawn as their source."))),
            ("html", required(integer("HTML blocks drawn as their source."))),
        ]),
        "What the pages could not draw as the editor shows it.",
    )
}

fn text_read_output() -> Value {
    object(&[
        ("text", required(string("The text between from and to (the whole note by default), front matter included."))),
        ("length", required(integer("The whole note's length, in the request's unit."))),
    ])
}

fn outline_output() -> Value {
    object(&[
        (
            "headings",
            required(array(
                object(&[
                    ("path", required(strings("The heading's path: the plain texts of the headings above it and its own."))),
                    ("level", required(integer("1 to 6."))),
                    ("line", required(integer("The heading's line, from 1."))),
                    ("from", required(integer("Where its section starts (Unicode characters)."))),
                    ("to", required(integer("Where its section ends, subsections included (exclusive)."))),
                    ("words", required(integer("The words under it, subsections included."))),
                ]),
                "This page's headings, in document order.",
            )),
        ),
        ("total", required(integer("How many headings the note has."))),
        ("next", string("The cursor of the next page; absent on the last page.")),
    ])
}

/// `section.read` and `block.read`: Markdown and where it is.
fn span_output() -> Value {
    object(&[
        ("markdown", required(string("The Markdown, as the file has it."))),
        ("from", required(integer("Where it starts, in the request's unit."))),
        ("to", required(integer("Where it ends (exclusive)."))),
        ("line", required(integer("Its first line, from 1."))),
    ])
}

fn property_list_output() -> Value {
    object(&[
        (
            "properties",
            required(array(
                object(&[
                    ("key", required(string("The key."))),
                    (
                        "type",
                        required(one_of(
                            &["string", "number", "boolean", "null", "date", "datetime", "list", "other", "unknown"],
                            "What the value reads as; other is YAML the note does not type, unknown a value it could not read.",
                        )),
                    ),
                    ("value", required(any("The value: a string (dates too), number, boolean, list of strings, or null."))),
                    (
                        "style",
                        required(one_of(
                            &["empty", "plain", "single-quoted", "double-quoted", "flow-list", "block-list", "complex"],
                            "How it is written; property.set keeps the style. A complex one is only replaced whole.",
                        )),
                    ),
                    ("lines", required(array(json!({ "type": "integer" }), "Its first and last line, from 1."))),
                    ("complex", string("Why it is complex, when it is.")),
                ]),
                "The front matter's top-level properties, in order; empty without front matter.",
            )),
        ),
        ("invalid", object(&[("line", required(integer("The line, from 1."))), ("message", required(string("What is wrong.")))])),
        ("duplicates", strings("Keys written more than once.")),
    ])
}

fn table_read_output() -> Value {
    json!({
        "oneOf": [
            object(&[
                ("tables", required(integer("How many tables the note has."))),
                (
                    "list",
                    required(array(
                        object(&[
                            ("table", required(integer(TABLE))),
                            ("line", required(integer("Its first line, from 1."))),
                            ("rows", required(integer("Its rows, the header row included."))),
                            ("columns", required(integer("Its columns (the header's cells)."))),
                        ]),
                        "Each table.",
                    )),
                ),
            ]),
            object(&[
                ("table", required(integer(TABLE))),
                ("header", required(strings("The header cells' text."))),
                ("align", required(array(one_of(&["left", "center", "right", "none"], "A column's alignment."), "Each column's alignment."))),
                ("rows", required(array(strings("A body row's cells."), "The body rows, top to bottom (row 1 first)."))),
                ("lines", required(array(json!({ "type": "integer" }), "Its first and last line, from 1."))),
            ]),
        ],
        "description": "Without table: how many tables there are and where; with table: that table.",
    })
}

fn links_output() -> Value {
    let link = object(&[
        (
            "kind",
            required(one_of(
                &["wikilink", "markdown", "embed", "image", "url"],
                "The link's kind; markdown includes reference links and <autolinks>.",
            )),
        ),
        ("line", required(integer("Its line, from 1."))),
        ("index", required(integer("Which link of the line, from 0: what link.update takes."))),
        ("destination", required(string("The destination as written."))),
        (
            "state",
            required(one_of(
                &["resolved", "missing", "missing-anchor", "external", "unknown"],
                "Whether its target exists: a file the host has, a heading or block of this note, a web address, or unknown when the host cannot say.",
            )),
        ),
        ("text", string("The text shown (a wikilink's alias).")),
        ("anchor", string("The part after the file: #heading or #^block.")),
    ]);
    let tag = object(&[
        ("kind", required(one_of(&["tag"], "A tag."))),
        ("line", required(integer("Its line, from 1."))),
        ("name", required(string("The tag without #."))),
        ("property", boolean("True when it is written in the tags property.")),
    ]);
    let block = object(&[
        ("kind", required(one_of(&["blockId"], "A block id."))),
        ("line", required(integer("Its line, from 1."))),
        ("id", required(string("The id without the caret."))),
    ]);
    object(&[("links", required(array(json!({ "oneOf": [link, tag, block] }), "Links in document order, then tags, then block ids.")))])
}

fn projection_output() -> Value {
    object(&[(
        "objects",
        required(array(
            object(&[
                ("id", required(string("The object's id this session: section:3, task:2, table:1, property:1, link:4."))),
                ("kind", required(one_of(&["section", "task", "table", "property", "link"], "Its kind."))),
                (
                    "label",
                    required(string(
                        "How it reads: a heading path joined by ›, a task's text, a table's header cells, a property key, a link's destination.",
                    )),
                ),
                ("line", required(integer("Its first line, from 1."))),
                ("path", strings("A section's heading path.")),
                ("done", boolean("Whether a task is checked.")),
            ]),
            "The objects, sections first, each kind in document order.",
        )),
    )])
}

fn tasks_output() -> Value {
    object(&[(
        "tasks",
        required(array(
            object(&[
                ("line", required(integer("The task's line, from 1: what task.toggle takes."))),
                ("done", required(boolean("Checked."))),
                ("text", required(string("Its text, without the box and block id."))),
                ("blockId", required(or_null(json!({ "type": "string" }), "Its block id, or null."))),
            ]),
            "The tasks, in document order.",
        )),
    )])
}

fn find_output() -> Value {
    object(&[
        ("count", required(integer("How many matches are answered."))),
        ("more", required(boolean("Whether there are more than limit."))),
        (
            "matches",
            required(array(
                object(&[
                    ("from", required(integer("Where it starts, in the request's unit."))),
                    ("to", required(integer("Where it ends (exclusive)."))),
                    ("line", required(integer("Its line, from 1."))),
                    ("text", required(string("The text matched."))),
                ]),
                "The matches, in document order.",
            )),
        ),
    ])
}

fn layout_output() -> Value {
    object(&[
        ("pages", required(integer("How many pages the note prints on."))),
        (
            "pageSize",
            required(described(
                object(&[("width", required(number("Points."))), ("height", required(number("Points.")))]),
                "The page size in points.",
            )),
        ),
        (
            "blocks",
            required(array(
                object(&[
                    ("page", required(integer("The page, from 1."))),
                    ("line", required(integer("The block's first line, from 1."))),
                    ("kind", required(string("What the block is, for example paragraph, heading, list, quote, code, table, math or rule."))),
                    ("x", required(number("Its left edge on the page, in points."))),
                    ("y", required(number("Its top edge on the page, in points."))),
                    ("width", required(number("Points."))),
                    ("height", required(number("Points."))),
                ]),
                "Where each block (or the given page's blocks) lands, at most 500.",
            )),
        ),
        (
            "overflow",
            required(array(
                object(&[
                    ("page", required(integer("The page, from 1."))),
                    ("line", required(integer("The block's first line, from 1."))),
                    ("kind", required(string("code or table."))),
                    ("width", required(number("The width it needs, in points."))),
                ]),
                "Blocks wider than the page's column: a code line wrapped, a table squeezed.",
            )),
        ),
        ("unsupported", required(unsupported())),
    ])
}

fn export_output() -> Value {
    object(&[
        ("format", required(one_of(&["html", "pdf"], "The format written."))),
        ("data", required(string("The file's bytes, base64."))),
        ("bytes", required(integer("Its size in bytes."))),
        ("name", required(string("A file name for it: the note's, ending in .html or .pdf."))),
        ("mime", required(one_of(&["text/html", "application/pdf"], "Its media type."))),
        ("pages", integer("A PDF's page count.")),
        ("unsupported", unsupported()),
    ])
}

fn threads_output() -> Value {
    let comment = object(&[
        ("id", required(string("The comment's id."))),
        ("author", required(string("The author's principal id."))),
        ("name", required(or_null(json!({ "type": "string" }), "The author's display name, or null."))),
        ("body", required(string("The comment's text."))),
        ("created", required(string("When, ISO 8601 UTC."))),
    ]);
    object(&[(
        "threads",
        required(array(
            object(&[
                ("id", required(string("The thread's id: what comment.reply and comment.resolve take."))),
                ("resolved", required(boolean("Resolved."))),
                ("comments", required(array(comment, "Its comments in order; the first opened it."))),
                ("quote", string("The text it is anchored to.")),
                ("from", integer("Where that text is now (Unicode characters, or the request's unit).")),
                ("to", integer("Where it ends (exclusive).")),
                ("line", integer("Its line, from 1.")),
                ("gone", boolean("True when the text it was anchored to is no longer in the note.")),
            ]),
            "The note's comment threads; one without quote is on the whole note.",
        )),
    )])
}

fn template_output() -> Value {
    object(&[("markdown", required(string("The template with its variables filled in.")))])
}

fn find_replace_output() -> Value {
    object(&[("replaced", required(integer("How many matches were replaced."))), ("changed", required(boolean("Whether the note's text changed.")))])
}

fn task_toggle_output() -> Value {
    object(&[("done", required(boolean("Whether the task is checked now."))), ("changed", required(boolean("Whether the note's text changed.")))])
}

fn table_create_output() -> Value {
    object(&[("table", required(integer("The new table's index, from 0."))), ("changed", required(boolean("Whether the note's text changed.")))])
}

fn retarget_output() -> Value {
    object(&[
        ("links", required(integer("How many links and embeds were rewritten."))),
        ("changed", required(boolean("Whether the note's text changed."))),
    ])
}

fn line_endings_output() -> Value {
    object(&[
        ("changed", required(boolean("Whether the file changed."))),
        ("eol", required(one_of(&["lf", "crlf"], "The line endings the file has now."))),
        ("bom", required(boolean("Whether the file starts with a byte order mark."))),
    ])
}

/// Where a character is drawn, in document pixels.
fn caret_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("x", required(number("Across, in document pixels."))),
        ("y", required(number("The line's top, in document pixels."))),
        ("height", required(number("The caret's height, in document pixels."))),
    ]
}

fn selection_output() -> Value {
    let mut caret = caret_fields();
    caret.push(("remote", boolean("True when another person's change put the caret there.")));
    object(&[
        ("anchor", required(integer("Where the selection started, in Unicode characters."))),
        ("focus", required(integer("Where it ends: the caret."))),
        ("from", required(integer("The selection's start."))),
        ("to", required(integer("Its end (exclusive)."))),
        ("text", required(string("The selected text."))),
        ("line", required(integer("The caret's line, from 1."))),
        ("column", required(integer("The caret's column, from 1, in Unicode characters."))),
        ("before", required(string("The caret's line before it, at most 80 characters (for completions)."))),
        ("caret", required(described(object(&caret), "Where the caret is drawn."))),
    ])
}

fn where_output() -> Value {
    object(&caret_fields())
}

fn cut_output() -> Value {
    object(&[("text", required(string("The text cut, for the clipboard; empty when nothing was selected.")))])
}

fn table_geometry_output() -> Value {
    object(&[(
        "tables",
        required(array(
            object(&[
                ("table", required(integer(TABLE))),
                ("columns", required(array(json!({ "type": "number" }), "The column edges across, in document pixels."))),
                (
                    "rows",
                    required(array(
                        object(&[
                            ("top", required(number("Document pixels."))),
                            ("bottom", required(number("Document pixels."))),
                            ("line", required(integer("The row's line, from 1."))),
                        ]),
                        "Each row, the header first.",
                    )),
                ),
            ]),
            "The tables drawn between top and bottom.",
        )),
    )])
}

fn code_blocks_output() -> Value {
    object(&[(
        "blocks",
        required(array(
            object(&[
                ("top", required(number("Document pixels."))),
                ("bottom", required(number("Document pixels."))),
                ("left", required(number("Its box's left edge, in document pixels."))),
                ("right", required(number("Its box's right edge, in document pixels."))),
                ("language", required(string("The fence's language, or empty."))),
                ("callout", required(boolean("Inside a callout (a copy button only, no language label)."))),
                ("from", required(integer("Where its code starts, in Unicode characters."))),
                ("to", required(integer("Where its code ends (exclusive)."))),
            ]),
            "The fenced code blocks drawn between top and bottom.",
        )),
    )])
}

fn diagrams_output() -> Value {
    let kind = || one_of(&["mermaid", "html"], "A Mermaid diagram or an HTML block.");
    object(&[
        (
            "wanted",
            required(array(
                object(&[("kind", required(kind())), ("source", required(string("Its source: the key diagram.set sizes it by.")))]),
                "The blocks away from the caret the editor has not sized yet.",
            )),
        ),
        ("column", required(number("The text column's width, in document pixels."))),
        (
            "diagrams",
            required(array(
                object(&[
                    ("kind", required(kind())),
                    ("source", required(string("Its source."))),
                    ("top", required(number("Document pixels."))),
                    ("left", required(number("Document pixels."))),
                    ("width", required(number("Document pixels."))),
                    ("height", required(number("Document pixels."))),
                    ("code", required(integer("Where its source starts, in Unicode characters."))),
                ]),
                "The blocks drawn between top and bottom.",
            )),
        ),
    ])
}

fn diagram_set_output() -> Value {
    object(&[("changed", required(boolean("Whether the size differed from the one the editor had.")))])
}

fn properties_view_output() -> Value {
    let row = object(&[
        ("key", required(string("The key."))),
        ("kind", required(one_of(&["tags", "aliases", "list", "checkbox", "number", "date", "datetime", "text"], "How the row shows its value."))),
        ("values", required(strings("The value, as text: one item, or each item of a list."))),
        ("y", required(number("The row's top, in document pixels."))),
        ("height", required(number("Document pixels."))),
        ("value", required(integer("Where the value starts in the text, in Unicode characters."))),
    ]);
    or_null(
        object(&[
            ("top", required(number("Document pixels."))),
            ("bottom", required(number("Document pixels."))),
            ("left", required(number("Document pixels."))),
            ("width", required(number("Document pixels."))),
            ("end", required(integer("Where the front matter's text ends, in Unicode characters."))),
            ("rows", required(array(row, "One row per property."))),
            ("collapsed", required(boolean("Folded to its heading."))),
        ]),
        "The properties block, or null when the note shows none (no front matter, an empty one, or one edited as its source).",
    )
}

fn image_wanted_output() -> Value {
    object(&[
        ("images", required(strings("Pictures the note's picture lines name that are neither loaded nor known missing, by name."))),
        ("drawn", required(strings("Pictures drawn now, by name."))),
    ])
}

fn image_set_output() -> Value {
    object(&[("drawn", required(boolean("Whether the bytes were a picture; otherwise it is recorded as missing.")))])
}

fn view_width_output() -> Value {
    object(&[
        ("changed", required(boolean("Whether the width or height differed, so the page is laid out again."))),
        ("width", required(integer("The page's width now, in document pixels."))),
    ])
}

fn view_scheme_output() -> Value {
    object(&[
        ("changed", required(boolean("Whether the palette differed, so the note is painted again."))),
        ("scheme", required(string("The color scheme the note paints with now."))),
    ])
}

fn block_at_output() -> Value {
    object(&[
        ("callout", required(boolean("Whether the point is in a callout."))),
        ("active", required(boolean("Whether the caret is in that callout too (shown as its source)."))),
    ])
}

fn fold_properties_output() -> Value {
    object(&[("folded", required(boolean("Whether the properties block is folded now.")))])
}

/// One command of the catalogue.
pub struct Spec {
    pub id: &'static str,
    pub title: &'static str,
    /// What it does, when to choose it, and what it changes or answers (`KitCommand.description`).
    pub description: &'static str,
    pub params: &'static str,
    pub schema: fn() -> Value,
    /// What it answers (`KitCommand.output_schema`, KHP A1).
    pub output: fn() -> Value,
    pub mutates: bool,
    /// Listed to agents (`note_commands`); the others are the editor's or the host's
    /// (kit.json `agent.exclude`).
    pub agent: bool,
    pub menu: Option<&'static [&'static str]>,
    pub shortcut: Option<&'static str>,
}

const fn spec(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    params: &'static str,
    schema: fn() -> Value,
    output: fn() -> Value,
    mutates: bool,
) -> Spec {
    Spec { id, title, description, params, schema, output, mutates, agent: true, menu: None, shortcut: None }
}

const fn private(mut spec: Spec) -> Spec {
    spec.agent = false;
    spec
}

/// One of the editor's commands, in a menu, with a shortcut; it takes no parameters.
const fn editor_item(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    menu: &'static [&'static str],
    shortcut: Option<&'static str>,
    mutates: bool,
    output: fn() -> Value,
) -> Spec {
    Spec { id, title, description, params: "{}", schema: no_params, output, mutates, agent: false, menu: Some(menu), shortcut }
}

/// An Insert date command: the local time, in the page's time zone or the one `utcOffset` gives.
const fn with_utc_offset(mut spec: Spec) -> Spec {
    spec.params = "{ utcOffset?: number }";
    spec.schema = utc_offset_schema;
    spec
}

fn utc_offset_schema() -> Value {
    schema(&[("utcOffset", integer("Minutes local time is ahead of UTC (negative behind); the page's time zone by default."))])
}

pub const SPECS: &[Spec] = &[
    // Read
    spec(
        "text.read",
        "Read the text",
        "Answers the note's raw Markdown (front matter included) and its length, or only the part between `from` and `to` (text positions from 0). Use it to read exact text before editing by position.",
        "{ from?: number, to?: number, unit?: \"scalar\" | \"utf16\" } (the whole text by default)",
        || {
            schema(&[
                ("from", integer("Where to start; 0 by default.")),
                ("to", integer("Where to stop (exclusive); the end by default.")),
                ("unit", unit()),
            ])
        },
        text_read_output,
        false,
    ),
    spec(
        "outline.read",
        "Read the outline",
        "Lists the headings with their level, line (from 1), heading path, section range and word count, a page at a time (`limit`, then `cursor` from `next`). Use it to find the path or line that section commands take.",
        "{ limit?: number, cursor?: string }",
        || {
            schema(&[
                ("limit", integer("The most headings to answer (default 200, at most 1000).")),
                ("cursor", string("Where the previous page ended (its next value).")),
            ])
        },
        outline_output,
        false,
    ),
    spec(
        "projection.read",
        "List the note's objects and their ids",
        "Lists the note's sections, tasks, tables, properties and links with the ids this session gives them (`section:3`, `task:2`), their label and first line, in document order by kind; `kinds` limits the list. Section commands take a section's id as `section`, `task.toggle` a task's as `task`, and receipts and `others` name what changed by these ids. An id follows its object through edits (a section renamed or moved keeps it) while the note stays open; it is never written into the file.",
        "{ kinds?: (\"section\" | \"task\" | \"table\" | \"property\" | \"link\")[] }",
        || {
            schema(&[(
                "kinds",
                json!({ "type": "array", "items": { "type": "string", "enum": ["section", "task", "table", "property", "link"] }, "description": "Only these kinds; all by default." }),
            )])
        },
        projection_output,
        false,
    ),
    spec(
        "section.read",
        "Read a section",
        "Answers the Markdown of one section (the heading and everything under it), picked by heading path or line, with its range and first line; `withHeading: false` leaves the heading line out.",
        "{ heading?: string[], line?: number, section?: string, withHeading?: boolean, unit?: \"scalar\" | \"utf16\" }",
        || {
            with_heading(vec![
                ("withHeading", json!({ "type": "boolean", "default": true, "description": "Include the heading line." })),
                ("unit", unit()),
            ])
        },
        span_output,
        false,
    ),
    spec(
        "block.read",
        "Read a block",
        "Answers the Markdown of the block carrying the block id `blockId` (`^id`), with its range and first line.",
        "{ blockId: string, unit?: \"scalar\" | \"utf16\" }",
        || schema(&[("blockId", required(string("The block id, without the caret (^)."))), ("unit", unit())]),
        span_output,
        false,
    ),
    spec(
        "property.list",
        "List the properties",
        "Lists the front-matter properties with key, value, type, writing style and lines, and reports duplicated keys and YAML it cannot read. Use it before `property.set`, `property.delete` or `property.rename`.",
        "{}",
        no_params,
        property_list_output,
        false,
    ),
    spec(
        "table.read",
        "Read a table",
        "Answers one table (`table`, from 0) as its header, body rows, column alignment and lines; without `table` it answers how many tables the note has and where each is.",
        "{ table?: number } (without table: how many tables there are)",
        || schema(&[("table", integer(TABLE))]),
        table_read_output,
        false,
    ),
    spec(
        "link.list",
        "List links, tags and block ids",
        "Lists the note's wikilinks, Markdown links, embeds, images, URLs, tags and block ids with their line and, for links, whether the target exists; `kinds` limits the list.",
        "{ kinds?: (\"wikilink\" | \"markdown\" | \"embed\" | \"image\" | \"url\" | \"tag\" | \"blockId\")[] }",
        || {
            schema(&[(
                "kinds",
                json!({ "type": "array", "items": { "type": "string", "enum": ["wikilink", "markdown", "embed", "image", "url", "tag", "blockId"] }, "description": "Only these kinds; all by default." }),
            )])
        },
        links_output,
        false,
    ),
    spec(
        "task.list",
        "List tasks",
        "Lists the task items (`- [ ]`, `- [x]`) with line, text, done and block id, in the whole note or one section. Use it to get the line `task.toggle` takes.",
        "{ heading?: string[], line?: number, section?: string } (all tasks by default)",
        || with_heading(Vec::new()),
        tasks_output,
        false,
    ),
    spec(
        "text.find",
        "Find text",
        "Finds text (or a regular expression with `regex: true`) in the note's body (the front matter is not searched: read properties with `property.list`) and answers each match's `from`, `to`, line and text. The way to get text positions for `text.insert`, `text.replace` and `comment.add`.",
        "{ find: string, regex?: boolean, caseSensitive?: boolean, unit?: \"scalar\" | \"utf16\", limit?: number }",
        || {
            schema(&[
                ("find", required(string("The text, or a regular expression with regex."))),
                ("regex", boolean("Read find as a regular expression (Rust syntax).")),
                ("caseSensitive", json!({ "type": "boolean", "default": true, "description": "Match case." })),
                ("unit", unit()),
                ("limit", integer("The most matches to answer (default 100).")),
            ])
        },
        find_output,
        false,
    ),
    spec(
        "page.layout",
        "Read the page layout",
        "Answers how the note prints on `letter` or `a4` pages: the page count, where each block lands (page, x, y, width, height in points), blocks too wide for the column, and what the pages cannot draw. Use it to check page breaks before exporting a PDF.",
        "{ pageSize?: \"letter\" | \"a4\", page?: number } (every page by default)",
        || {
            schema(&[
                ("pageSize", json!({ "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "The page size." })),
                ("page", integer("Only the blocks placed on this page, from 1.")),
            ])
        },
        layout_output,
        false,
    ),
    spec(
        "file.export",
        "Export",
        "Writes the note as a standalone HTML file (the reading view with its stylesheet; `standalone: false` only the `<main>` element) or prints it to a PDF on `letter` or `a4` pages, and answers the file as base64 `data` with its size, a file name and, for a PDF, its page count and what the pages could not draw. Does not change the note.",
        "{ format: \"html\" | \"pdf\", standalone?: boolean, pageSize?: \"letter\" | \"a4\" }",
        || {
            schema(&[
                ("format", required(json!({ "type": "string", "enum": ["html", "pdf"], "description": "The file's format." }))),
                (
                    "standalone",
                    json!({ "type": "boolean", "default": true, "description": "HTML: a whole file with its stylesheet; false writes the <main> element only." }),
                ),
                ("pageSize", json!({ "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "PDF: the page size." })),
            ])
        },
        export_output,
        false,
    ),
    spec(
        "comment.threads",
        "List comment threads",
        "Lists the comment threads with their comments, resolved state, and the text each is anchored to with where it is now (or that it is gone). Use it to get the thread ids the other comment commands take.",
        "{ unit?: \"scalar\" | \"utf16\" }",
        || schema(&[("unit", unit())]),
        threads_output,
        false,
    ),
    spec(
        "template.expand",
        "Expand a template",
        "Answers `markdown` with the template variables (`{{title}}`, `{{date}}`, `{{time}}`, `{{date:FORMAT}}`) filled in, without changing the note; pass the result to `note.write` or `section.insert`.",
        "{ markdown: string, title?: string, date?: string }",
        || {
            schema(&[
                ("markdown", required(string("Template text with {{title}}, {{date}}, {{time}} and {{date:FORMAT}} (YYYY, MM, DD, HH, mm, ss)."))),
                ("title", string("What {{title}} becomes.")),
                ("date", string("The date and time to use, ISO 8601 (2026-10-23 or 2026-10-23T14:05); now by default.")),
            ])
        },
        template_output,
        false,
    ),
    // Text
    spec(
        "text.insert",
        "Insert text",
        "Inserts `text` at a text position (from 0) and answers the note's new length. Get positions from `text.find`; inserting at 0 puts text above the front matter.",
        "{ at: number, text: string, unit?: \"scalar\" | \"utf16\", expected?: string }",
        || {
            schema(&[
                ("at", required(integer("Where to insert."))),
                ("text", required(string("The Markdown to insert."))),
                ("unit", unit()),
                ("expected", string("What the empty range at `at` holds: \"\"; any other text fails the insert.")),
            ])
        },
        new_length,
        true,
    ),
    spec(
        "text.delete",
        "Delete text",
        "Deletes the text between `from` and `to` and answers the note's new length; pass `expected` (the text you mean to delete) so a stale position fails instead of deleting the wrong text.",
        "{ from: number, to: number, unit?: \"scalar\" | \"utf16\", expected?: string }",
        || {
            schema(&[
                ("from", required(integer("Where the range starts."))),
                ("to", required(integer("Where the range ends (exclusive)."))),
                ("unit", unit()),
                ("expected", string(EXPECTED)),
            ])
        },
        new_length,
        true,
    ),
    spec(
        "text.replace",
        "Replace text",
        "Replaces the text between `from` and `to` with `text` and answers the note's new length; pass `expected` with the current text so a stale range fails.",
        "{ from: number, to: number, text: string, unit?: \"scalar\" | \"utf16\", expected?: string }",
        || {
            schema(&[
                ("from", required(integer("Where the range starts."))),
                ("to", required(integer("Where the range ends (exclusive)."))),
                ("text", required(string("The Markdown to put there."))),
                ("unit", unit()),
                ("expected", string(EXPECTED)),
            ])
        },
        new_length,
        true,
    ),
    spec(
        "text.findReplace",
        "Find and replace",
        "Replaces matches of `find` in the note's body with `replace` (matches in the front matter are not counted: change properties with `property.set`): the first by default, the n-th with `occurrence`, or every one with `occurrence: \"all\"`; `regex` and `caseSensitive` as in `text.find`. Answers how many were replaced.",
        "{ find: string, replace: string, occurrence?: number | \"all\", regex?: boolean, caseSensitive?: boolean }",
        || {
            schema(&[
                ("find", required(string("The text, or a regular expression with regex."))),
                ("replace", required(string("The replacement; with regex, $1 names a group."))),
                ("occurrence", any("Which match to replace: a number from 1, or \"all\"; 1 by default.")),
                ("regex", boolean("Read find as a regular expression (Rust syntax).")),
                ("caseSensitive", json!({ "type": "boolean", "default": true, "description": "Match case." })),
            ])
        },
        find_replace_output,
        true,
    ),
    spec(
        "note.write",
        "Write the whole note",
        "Replaces the whole note with `markdown` as one undo step, rewriting only the lines that differ. Use it to write a new note; prefer section, block and text commands to change part of one.",
        "{ markdown: string }",
        || schema(&[("markdown", required(string("The note's whole new Markdown; only the lines that differ are changed.")))]),
        edited,
        true,
    ),
    // Structure
    spec(
        "section.replace",
        "Replace a section",
        "Replaces the body of the section at a heading path or line (subsections included) with `markdown`, keeping its heading unless `withHeading: true` (then `markdown` includes the heading).",
        "{ heading?: string[], line?: number, section?: string, markdown: string, withHeading?: boolean }",
        || {
            with_heading(vec![
                ("markdown", required(string("The section's new Markdown."))),
                ("withHeading", json!({ "type": "boolean", "default": false, "description": "Replace the heading line too." })),
            ])
        },
        edited,
        true,
    ),
    spec(
        "section.insert",
        "Insert into a section",
        "Inserts `markdown` as a new block before the heading, after the section and its subsections, right under the heading (`start`), or at the end of the section's own text before its subsections (`end`).",
        "{ heading?: string[], line?: number, section?: string, position: \"before\" | \"after\" | \"start\" | \"end\", markdown: string }",
        || {
            with_heading(vec![
                (
                    "position",
                    required(one_of(
                        &["before", "after", "start", "end"],
                        "Before the heading, after the section, or inside it at its start or end.",
                    )),
                ),
                ("markdown", required(string("The Markdown to insert."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "section.delete",
        "Delete a section",
        "Deletes the section at a heading path or line: its heading and everything under it, subsections included.",
        "{ heading?: string[], line?: number, section?: string }",
        || with_heading(Vec::new()),
        edited,
        true,
    ),
    spec(
        "section.move",
        "Move a section",
        "Moves the section at a heading path or line, with its subsections, before, after, or into the start or end of another section (`toHeading` or `toLine`); a section cannot move into itself.",
        "{ heading?: string[], line?: number, section?: string, toHeading?: string[], toLine?: number, toSection?: string, position: \"before\" | \"after\" | \"start\" | \"end\" }",
        || {
            with_heading(vec![
                ("toHeading", strings("The heading to move it next to, by path.")),
                ("toLine", integer("The heading to move it next to, by line, instead of toHeading.")),
                ("toSection", string("The section to move it next to, by its id from outline.read, instead of toHeading or toLine.")),
                ("position", required(one_of(&["before", "after", "start", "end"], "Where, relative to that heading's section."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "heading.setLevel",
        "Set a heading's level",
        "Sets the level (1 to 6) of the heading at a path or line by rewriting only its `#` run (an underlined heading's underline); its subsections keep their own levels.",
        "{ heading?: string[], line?: number, section?: string, level: number, style?: \"atx\" }",
        || {
            with_heading(vec![
                ("level", required(json!({ "type": "integer", "minimum": 1, "maximum": 6, "description": "The new level, 1 to 6." }))),
                ("style", one_of(&["atx"], "Rewrite an underlined heading as a # heading.")),
            ])
        },
        edited,
        true,
    ),
    spec(
        "block.replace",
        "Replace a block",
        "Replaces the block carrying the block id `blockId` with `markdown`, keeping its `^id` unless `markdown` sets one.",
        "{ blockId: string, markdown: string }",
        || {
            schema(&[
                ("blockId", required(string("The block id, without the caret (^)."))),
                ("markdown", required(string("The block's new Markdown; its ^id is kept unless this sets one."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "block.append",
        "Append a block",
        "Appends `markdown` as a new block at the end of the section at a heading path or line, or at the end of the note without one.",
        "{ heading?: string[], line?: number, section?: string, markdown: string } (the end of the note without a heading)",
        || with_heading(vec![("markdown", required(string("The Markdown to add as a block of its own.")))]),
        edited,
        true,
    ),
    spec(
        "block.setId",
        "Set a block id",
        "Gives a block a block id (`^id`) so links and `block.*` commands can reach it: the block ending on `line`, or the one already carrying `blockId` (renaming its id).",
        "{ line?: number, blockId?: string, occurrence?: number, id: string }",
        || {
            schema(&[
                ("line", integer("The block's last line, from 1.")),
                ("blockId", string("The block's current id, instead of line.")),
                ("occurrence", integer("Which of the blocks with that id, from 1.")),
                ("id", required(string("The new id: letters, digits and hyphens, without the caret."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "task.toggle",
        "Check or uncheck a task",
        "Checks or unchecks the task at `line` (from 1) or carrying `blockId`; `done` sets it, otherwise it flips. Answers whether it is checked now. Get lines from `task.list`.",
        "{ line?: number, blockId?: string, task?: string, done?: boolean }",
        || {
            schema(&[
                ("line", integer("The task's line, from 1.")),
                ("blockId", string("The task's block id, instead of line.")),
                ("task", string("The task's id from task.list (\"task:2\"), instead of line or blockId.")),
                ("done", boolean("Checked or not; flips it by default.")),
            ])
        },
        task_toggle_output,
        true,
    ),
    // Properties
    spec(
        "property.set",
        "Set a property",
        "Sets a front-matter property to a string, number, boolean, null or list of strings in the key's own writing style, adding the property (and the front matter) when it is missing.",
        "{ key: string, value: string | number | boolean | null | string[] }",
        || {
            schema(&[
                ("key", required(string("A top-level property key."))),
                ("value", required(any("A string, number, boolean, null, or a list of strings."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "property.delete",
        "Delete a property",
        "Deletes the front-matter property `key` and its lines; `occurrence` picks one when the key is written more than once.",
        "{ key: string, occurrence?: number }",
        || schema(&[("key", required(string("A top-level property key."))), ("occurrence", integer("Which of a key written twice, from 1."))]),
        edited,
        true,
    ),
    spec(
        "property.rename",
        "Rename a property",
        "Renames the front-matter property `key` to `to`, changing only the key and keeping its value and place.",
        "{ key: string, to: string }",
        || schema(&[("key", required(string("The property's key now."))), ("to", required(string("The new key.")))]),
        edited,
        true,
    ),
    // Tables
    spec(
        "table.add",
        "Insert a table",
        "Inserts a new table of `columns` columns (with optional header cells and `rows` empty body rows) before a line, at a section position, or at the end of the note, and answers its table index.",
        "{ line?: number, heading?: string[], position?: \"before\" | \"after\" | \"start\" | \"end\", columns: number, rows?: number, header?: string[] } (the end of the note by default)",
        || {
            schema(&[
                ("line", integer("Insert before this line, from 1.")),
                ("heading", strings("Or a heading's path, with position.")),
                ("position", one_of(&["before", "after", "start", "end"], "Where, relative to the heading's section; end by default.")),
                ("columns", required(json!({ "type": "integer", "minimum": 1, "maximum": 64, "description": "How many columns." }))),
                ("rows", integer("How many empty body rows (default 1).")),
                ("header", strings("The header cells.")),
            ])
        },
        table_create_output,
        true,
    ),
    spec(
        "table.setCell",
        "Set a table cell",
        "Sets one cell's text: `table` and `column` count from 0, `row` 0 is the header and body rows count from 1. Pass `expected` with the current text so a stale cell fails.",
        "{ table: number, row: number, column: number, text: string, expected?: string }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("row", required(integer("The row, 0 for the header row."))),
                ("column", required(integer("The column, from 0."))),
                ("text", required(string("The cell's new text (inline Markdown, one line)."))),
                ("expected", string("The cell's text now; the command is refused when it differs.")),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.insertRow",
        "Insert a table row",
        "Inserts a body row before body row `at` (1 is the first; one past the last appends) with optional `cells`.",
        "{ table: number, at: number, cells?: string[] }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("at", required(integer("Where the new row goes: 1 before the first body row; one past the last appends."))),
                ("cells", strings("The new row's cells.")),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.insertColumn",
        "Insert a table column",
        "Inserts a column at `at` (from 0; one past the last appends) with an optional header and alignment; every row gets an empty cell there.",
        "{ table: number, at: number, header?: string, align?: \"left\" | \"center\" | \"right\" | \"none\" }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("at", required(integer("Where the new column goes, from 0; one past the last appends."))),
                ("header", string("The new column's header.")),
                ("align", one_of(&["left", "center", "right", "none"], "The new column's alignment.")),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.deleteRow",
        "Delete a table row",
        "Deletes body row `row` (from 1) from a table; the header row cannot be deleted.",
        "{ table: number, row: number }",
        || schema(&[("table", required(integer(TABLE))), ("row", required(integer("The body row, from 1.")))]),
        edited,
        true,
    ),
    spec(
        "table.deleteColumn",
        "Delete a table column",
        "Deletes column `column` (from 0) from every row of a table, header included.",
        "{ table: number, column: number }",
        || schema(&[("table", required(integer(TABLE))), ("column", required(integer("The column, from 0.")))]),
        edited,
        true,
    ),
    spec(
        "table.moveRow",
        "Move a table row",
        "Moves body row `from` to position `to` within a table (body rows count from 1).",
        "{ table: number, from: number, to: number }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("from", required(integer("The body row to move, from 1."))),
                ("to", required(integer("Where it ends up, from 1."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.moveColumn",
        "Move a table column",
        "Moves column `from` to position `to` within a table (from 0), header and alignment included.",
        "{ table: number, from: number, to: number }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("from", required(integer("The column to move, from 0."))),
                ("to", required(integer("Where it ends up, from 0."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.align",
        "Align a table column",
        "Sets a column's alignment (left, center, right or none) by rewriting only its cell of the delimiter row.",
        "{ table: number, column: number, align: \"left\" | \"center\" | \"right\" | \"none\" }",
        || {
            schema(&[
                ("table", required(integer(TABLE))),
                ("column", required(integer("The column, from 0."))),
                ("align", required(one_of(&["left", "center", "right", "none"], "The alignment."))),
            ])
        },
        edited,
        true,
    ),
    spec(
        "table.format",
        "Format a table",
        "Rewrites a table with padded cells so its columns line up in the Markdown source, keeping every cell's text.",
        "{ table: number }",
        || schema(&[("table", required(integer(TABLE)))]),
        edited,
        true,
    ),
    // Links
    spec(
        "link.insert",
        "Insert a link",
        "Inserts a link to `destination` at a text position, as a wikilink or a Markdown link, with optional display text, or as an embed with `embed: true`.",
        "{ at: number, unit?: \"scalar\" | \"utf16\", destination: string, text?: string, style?: \"wikilink\" | \"markdown\", embed?: boolean }",
        || {
            schema(&[
                ("at", required(integer("Where to insert."))),
                ("unit", unit()),
                ("destination", required(string("What the link points at, as the note writes it: Plan, Plan#Risks, docs/plan.md."))),
                ("text", string("The text shown (a wikilink's alias).")),
                (
                    "style",
                    json!({ "type": "string", "enum": ["wikilink", "markdown"], "default": "wikilink", "description": "[[wikilink]] or [text](markdown)." }),
                ),
                ("embed", boolean("Embed it (![[...]]) instead of linking.")),
            ])
        },
        edited,
        true,
    ),
    spec(
        "link.update",
        "Change a link",
        "Changes the destination (and the text, when given) of the `index`-th link on `line`, as `link.list` numbers them; nothing else of the link changes.",
        "{ line: number, index: number, destination: string, text?: string }",
        || {
            schema(&[
                ("line", required(integer("The link's line, from 1."))),
                ("index", required(integer("Which link of the line, from 0."))),
                ("destination", required(string("The new destination."))),
                ("text", string("The new text shown.")),
            ])
        },
        edited,
        true,
    ),
    private(spec(
        "link.retarget",
        "Retarget links to a renamed file",
        "Rewrites every link and embed to the file `from` so it points at `to`, as after a rename, and answers how many it rewrote; the host runs it.",
        "{ from: string, to: string }",
        || {
            schema(&[
                ("from", required(string("The old link destination, for example Plan or docs/plan.md."))),
                ("to", required(string("The new one."))),
            ])
        },
        retarget_output,
        true,
    )),
    // Comments
    spec(
        "comment.add",
        "Add a comment",
        "Starts a comment thread anchored to a text range (`from`/`to`), the `occurrence`-th match of `quote`, a heading, or the whole note without an anchor, and answers the thread's and the comment's ids. Comments are kept beside the note, never written into the file.",
        "{ body: string, from?: number, to?: number, unit?: \"scalar\" | \"utf16\", quote?: string, occurrence?: number, heading?: string[], line?: number, author?: string, name?: string, created?: string } (the whole note without an anchor)",
        || {
            schema(&[
                ("body", required(string("The comment's text."))),
                ("from", integer("Where the commented range starts.")),
                ("to", integer("Where it ends (exclusive).")),
                ("unit", unit()),
                ("quote", string("Or the text to comment on; its first occurrence unless occurrence is given.")),
                ("occurrence", integer("Which occurrence of quote, from 1.")),
                ("heading", strings("Or a heading's path: comments on the heading.")),
                ("line", integer("Or the heading's line, from 1.")),
                ("author", string("The caller's own principal id (the default); the editor's person passes theirs, and another is refused.")),
                ("name", string("The author's display name.")),
                ("created", string("When, ISO 8601 UTC; now by default.")),
            ])
        },
        new_comment,
        true,
    ),
    spec(
        "comment.reply",
        "Reply to a comment",
        "Adds a reply to the comment thread `thread` and answers the new comment's id.",
        "{ thread: string, body: string, author?: string, name?: string, created?: string }",
        || {
            schema(&[
                ("thread", required(string("The thread's id."))),
                ("body", required(string("The reply's text."))),
                ("author", string("The caller's own principal id (the default); the editor's person passes theirs, and another is refused.")),
                ("name", string("The author's display name.")),
                ("created", string("When, ISO 8601 UTC; now by default.")),
            ])
        },
        new_comment,
        true,
    ),
    spec(
        "comment.resolve",
        "Resolve a comment thread",
        "Marks the comment thread `thread` resolved, or open again with `resolved: false`.",
        "{ thread: string, resolved?: boolean }",
        || {
            schema(&[
                ("thread", required(string("The thread's id."))),
                ("resolved", json!({ "type": "boolean", "default": true, "description": "Resolved, or open again." })),
            ])
        },
        thread_only,
        true,
    ),
    private(spec(
        "comment.move",
        "Move a comment thread",
        "Anchors the comment thread `thread` to another text range, quote or heading, as the editor does when its anchor text is edited.",
        "{ thread: string, quote?: string, occurrence?: number, from?: number, to?: number, unit?: \"scalar\" | \"utf16\", heading?: string[], line?: number }",
        || {
            schema(&[
                ("thread", required(string("The thread's id."))),
                ("quote", string("The text to anchor it to.")),
                ("occurrence", integer("Which occurrence of quote, from 1.")),
                ("from", integer("Or where the range starts.")),
                ("to", integer("And where it ends (exclusive).")),
                ("unit", unit()),
                ("heading", strings("Or a heading's path: anchors it to the heading.")),
                ("line", integer("Or the heading's line, from 1.")),
            ])
        },
        thread_only,
        true,
    )),
    private(spec(
        "comment.delete",
        "Delete a comment",
        "Deletes one comment from a thread; deleting the last one removes the thread.",
        "{ thread: string, comment: string }",
        || schema(&[("thread", required(string("The thread's id."))), ("comment", required(string("The comment's id.")))]),
        thread_only,
        true,
    )),
    // File and history
    spec(
        "file.setLineEndings",
        "Set the line endings",
        "Sets the file's line endings to LF or CRLF and whether it starts with a byte order mark; the text is otherwise unchanged. Answers the line style the file has now.",
        "{ eol: \"lf\" | \"crlf\", bom?: boolean }",
        || {
            schema(&[
                ("eol", required(one_of(&["lf", "crlf"], "LF or CRLF for every line."))),
                ("bom", boolean("Write a UTF-8 byte order mark; unchanged by default.")),
            ])
        },
        line_endings_output,
        true,
    ),
    // The editor's commands: they act on the selection, which only the editor has (never an agent's).
    private(spec(
        "select.get",
        "Read the selection",
        "Answers the editor's selection: anchor and focus positions, the selected text, the caret's line and column, the line's text before it, and where the caret is drawn.",
        "{}",
        no_params,
        selection_output,
        false,
    )),
    private(spec(
        "select.set",
        "Set the selection",
        "Sets the editor's selection by anchor and focus positions, selects a line, puts the caret at a line and column, or at a point of the document.",
        "{ anchor?: number, focus?: number, line?: number, column?: number, x?: number, y?: number }",
        selection_schema,
        nothing,
        false,
    )),
    editor_item("edit.selectAll", "Select All", "Selects the whole note in the editor.", &["Edit"], Some("Cmd+A"), false, nothing),
    private(spec(
        "edit.cut",
        "Cut",
        "Removes the editor's selection from the note and answers its text for the clipboard.",
        "{}",
        no_params,
        cut_output,
        true,
    )),
    private(spec(
        "table.geometry",
        "Where the tables are drawn",
        "Answers where the editor draws each table's columns and rows between `top` and `bottom`, for the table widget's controls.",
        "{ top?: number, bottom?: number }",
        range_schema,
        table_geometry_output,
        false,
    )),
    private(spec(
        "code.blocks",
        "Where the code blocks are drawn",
        "Answers where the editor draws each fenced code block between `top` and `bottom`, with its language, for its copy and language controls.",
        "{ top?: number, bottom?: number }",
        range_schema,
        code_blocks_output,
        false,
    )),
    private(spec(
        "diagram.view",
        "The Mermaid diagrams to draw and where",
        "Answers the Mermaid diagrams and HTML blocks the editor still has to size, those drawn between `top` and `bottom` with their boxes, and the column width.",
        "{ top?: number, bottom?: number }",
        range_schema,
        diagrams_output,
        false,
    )),
    private(spec(
        "diagram.set",
        "A Mermaid diagram's drawn size",
        "Gives the editor a Mermaid diagram's or HTML block's drawn size, by its source, so the page reserves that space; answers whether the size changed.",
        "{ source: string, width: number, height: number }",
        diagram_schema,
        diagram_set_output,
        false,
    )),
    private(spec(
        "property.view",
        "The front matter as a properties block",
        "Answers the front matter as the editor's properties block (each row's key, kind, values and position), or null when the note shows none.",
        "{}",
        no_params,
        properties_view_output,
        false,
    )),
    private(spec(
        "image.wanted",
        "The pictures the note draws that are not loaded",
        "Answers the pictures the note draws that the editor has not loaded yet, and those drawn now, by name.",
        "{}",
        no_params,
        image_wanted_output,
        false,
    )),
    private(spec(
        "image.set",
        "Hand the note a picture's bytes",
        "Hands the editor a picture's bytes (`data`, base64) by name so the note draws it; bytes that are not a picture record it as missing.",
        "{ name: string, data: string }",
        picture_schema,
        image_set_output,
        false,
    )),
    private(spec(
        "select.replace",
        "Replace the selection",
        "Replaces the editor's selection with `text`; with `paste: true` it is pasted text, and an address pasted over a selection links it.",
        "{ text: string, paste?: boolean }",
        replace_schema,
        nothing,
        true,
    )),
    private(spec(
        "view.width",
        "Fit the page to the visible width",
        "Sets the page to the editor's visible width (and height) so lines wrap to it; answers whether that changed and the page's width.",
        "{ width: number, height?: number }",
        width_schema,
        view_width_output,
        false,
    )),
    private(spec(
        "view.scheme",
        "Paint the note in the host's color scheme",
        "Paints the note with the light or the dark palette, as the host's color scheme is; nothing moves. Answers whether that changed.",
        "{ scheme: 'light' | 'dark' }",
        scheme_schema,
        view_scheme_output,
        false,
    )),
    private(spec(
        "view.hoverOut",
        "The pointer left the note: nothing is hovered",
        "Tells the editor the pointer left the note, so no heading or item is drawn hovered.",
        "{}",
        no_params,
        nothing,
        false,
    )),
    private(spec(
        "select.where",
        "Where a character is drawn",
        "Answers where the character at text position `at` is drawn on the page (x, top and height, in document pixels).",
        "{ at: number }",
        where_schema,
        where_output,
        false,
    )),
    private(spec(
        "view.blockAt",
        "What block a point of the page is in",
        "Answers whether the page point `x`, `y` is in a callout, and whether the caret is in that callout too.",
        "{ x: number, y: number }",
        point_schema,
        block_at_output,
        false,
    )),
    private(spec(
        "view.foldProperties",
        "Fold the properties block to its heading, or open it",
        "Folds the properties block to its heading line in the editor, or opens it again; writes nothing and answers whether it is folded now.",
        "{}",
        no_params,
        fold_properties_output,
        false,
    )),
    private(spec(
        "format.math",
        "Inline math",
        "Toggles inline math (`$…$`) on the editor's selection, or on the word at the caret.",
        "{}",
        no_params,
        nothing,
        true,
    )),
    private(spec(
        "format.highlightColor",
        "Highlight in a colour",
        "Highlights the editor's selection in `color` (Obsidian's coloured highlight, `==🔴…==`); in a highlight of that colour the highlight goes, in another its colour changes.",
        "{ color: string }",
        highlight_color_schema,
        nothing,
        true,
    )),
    private(spec(
        "format.clear",
        "Clear formatting",
        "Removes bold, italic, strikethrough, highlight, inline code and inline math marks from the lines the editor's selection covers.",
        "{}",
        no_params,
        nothing,
        true,
    )),
    private(spec(
        "select.controlKey",
        "A macOS Control editing key",
        "Runs a macOS Control editing key (`key`, for example `a`, `e`, `k`, `t`, `o`) or Page Up and Page Down on the editor's caret, as macOS text fields do.",
        "{ key: string, shift?: boolean }",
        control_key_schema,
        nothing,
        true,
    )),
    editor_item(
        "format.bold",
        "Bold",
        "Toggles bold (`**…**`) on the editor's selection, or on the word at the caret.",
        &["Format"],
        Some("Cmd+B"),
        true,
        nothing,
    ),
    editor_item(
        "format.italic",
        "Italic",
        "Toggles italic (`*…*`) on the editor's selection, or on the word at the caret.",
        &["Format"],
        Some("Cmd+I"),
        true,
        nothing,
    ),
    editor_item(
        "format.strike",
        "Strikethrough",
        "Toggles strikethrough (`~~…~~`) on the editor's selection, or on the word at the caret.",
        &["Format"],
        Some("Cmd+Shift+K"),
        true,
        nothing,
    ),
    editor_item(
        "format.code",
        "Inline Code",
        "Toggles inline code (backticks) on the editor's selection, or on the word at the caret.",
        &["Format"],
        Some("Cmd+E"),
        true,
        nothing,
    ),
    editor_item(
        "format.highlight",
        "Highlight",
        "Toggles highlight (`==…==`) on the editor's selection, or on the word at the caret.",
        &["Format"],
        Some("Cmd+Shift+H"),
        true,
        nothing,
    ),
    editor_item(
        "format.link",
        "Link",
        "Turns the editor's selection into a Markdown link (`[text]()`) and puts the caret where its address goes (where its text goes, without a selection).",
        &["Format"],
        Some("Cmd+K"),
        true,
        nothing,
    ),
    editor_item(
        "format.paragraph",
        "Paragraph",
        "Turns the headings on the lines the editor's selection covers back into plain paragraphs.",
        &["Format", "Heading"],
        Some("Cmd+0"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading1",
        "Heading 1",
        "Turns the lines the editor's selection covers into level-1 headings.",
        &["Format", "Heading"],
        Some("Cmd+1"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading2",
        "Heading 2",
        "Turns the lines the editor's selection covers into level-2 headings.",
        &["Format", "Heading"],
        Some("Cmd+2"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading3",
        "Heading 3",
        "Turns the lines the editor's selection covers into level-3 headings.",
        &["Format", "Heading"],
        Some("Cmd+3"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading4",
        "Heading 4",
        "Turns the lines the editor's selection covers into level-4 headings.",
        &["Format", "Heading"],
        Some("Cmd+4"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading5",
        "Heading 5",
        "Turns the lines the editor's selection covers into level-5 headings.",
        &["Format", "Heading"],
        Some("Cmd+5"),
        true,
        nothing,
    ),
    editor_item(
        "format.heading6",
        "Heading 6",
        "Turns the lines the editor's selection covers into level-6 headings.",
        &["Format", "Heading"],
        Some("Cmd+6"),
        true,
        nothing,
    ),
    editor_item(
        "list.bullet",
        "Bulleted List",
        "Turns the selected lines into a bulleted list (`- `), or back into paragraphs when every line is one already.",
        &["Format", "List"],
        Some("Cmd+Shift+8"),
        true,
        nothing,
    ),
    editor_item(
        "list.numbered",
        "Numbered List",
        "Turns the selected lines into a numbered list (`1. `), or back into paragraphs when every line is one already.",
        &["Format", "List"],
        Some("Cmd+Shift+7"),
        true,
        nothing,
    ),
    editor_item(
        "list.task",
        "Task List",
        "Turns the selected lines into task items (`- [ ] `), or takes the task marker away when every line has it.",
        &["Format", "List"],
        Some("Cmd+Shift+9"),
        true,
        nothing,
    ),
    editor_item(
        "list.toggleCheckbox",
        "Toggle Checkbox",
        "Checks the task items in the editor's selection, or opens them when none is open; other lines go a step: text becomes a task, a list item gets a box.",
        &["Format", "List"],
        Some("Cmd+L"),
        true,
        nothing,
    ),
    editor_item(
        "edit.indent",
        "Indent",
        "Indents every line the editor's selection covers by a tab (inside a quote, after its `>`).",
        &["Format"],
        Some("Cmd+]"),
        true,
        nothing,
    ),
    editor_item(
        "edit.outdent",
        "Outdent",
        "Takes one tab, or up to four spaces, from the start of every line the editor's selection covers.",
        &["Format"],
        Some("Cmd+["),
        true,
        nothing,
    ),
    editor_item("edit.moveLineUp", "Move Line Up", "Moves the lines the editor's selection covers up by one line.", &["Edit"], None, true, nothing),
    editor_item(
        "edit.moveLineDown",
        "Move Line Down",
        "Moves the lines the editor's selection covers down by one line.",
        &["Edit"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "edit.deleteParagraph",
        "Delete Paragraph",
        "Deletes the lines the editor's selection covers (the caret's line without a selection).",
        &["Edit"],
        Some("Cmd+D"),
        true,
        nothing,
    ),
    editor_item(
        "format.comment",
        "Comment",
        "Wraps the editor's selection (or the word at the caret) in a Markdown comment (`%%…%%`) hidden when the note is read, or takes the comment away.",
        &["Format"],
        Some("Cmd+/"),
        true,
        nothing,
    ),
    editor_item(
        "format.quote",
        "Quote",
        "Turns the selected lines into a block quote (`> `), or back into paragraphs when every line is quoted already.",
        &["Format"],
        Some("Cmd+Shift+."),
        true,
        nothing,
    ),
    editor_item(
        "format.headingUp",
        "Promote Heading",
        "Raises the headings the editor's selection covers by one level (for example `###` to `##`); a plain line becomes a level-6 heading.",
        &["Format", "Heading"],
        Some("Cmd+Shift+["),
        true,
        nothing,
    ),
    editor_item(
        "format.headingDown",
        "Demote Heading",
        "Lowers the headings the editor's selection covers by one level (for example `##` to `###`); a level-6 heading becomes a paragraph.",
        &["Format", "Heading"],
        Some("Cmd+Shift+]"),
        true,
        nothing,
    ),
    editor_item(
        "view.fold",
        "Fold Heading",
        "Folds or opens the heading, or the list item with children, on the editor caret's line; answers how many are folded.",
        &["View"],
        Some("Cmd+Alt+["),
        false,
        folds,
    ),
    editor_item(
        "view.foldAll",
        "Fold All Headings",
        "Folds every heading and every list item with children in the editor; answers how many are folded.",
        &["View"],
        None,
        false,
        folds,
    ),
    editor_item("view.unfoldAll", "Unfold All", "Opens every fold in the editor; answers that none is folded.", &["View"], None, false, folds),
    editor_item(
        "view.foldMore",
        "Fold More",
        "Folds the innermost open heading or list item around the editor's caret; answers how many are folded.",
        &["View"],
        None,
        false,
        folds,
    ),
    editor_item(
        "view.foldLess",
        "Fold Less",
        "Opens the outermost folded heading or list item around the editor's caret; answers how many are folded.",
        &["View"],
        None,
        false,
        folds,
    ),
    editor_item(
        "edit.selectSection",
        "Select Section",
        "Selects the whole section under the heading above the editor's caret.",
        &["Edit"],
        None,
        false,
        nothing,
    ),
    editor_item(
        "edit.sortLines",
        "Sort Lines",
        "Sorts the lines the editor's selection covers alphabetically, ignoring case.",
        &["Edit"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "edit.trimWhitespace",
        "Trim Trailing Spaces",
        "Removes trailing spaces and tabs from the lines the editor's selection covers.",
        &["Edit"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "wikilink.insert",
        "Wikilink",
        "Wraps the editor's selection in a wikilink (`[[…]]`), or inserts an empty one to type a note name into.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "embed.insert",
        "Embed",
        "Wraps the editor's selection in an embed (`![[…]]`), or inserts an empty one.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "image.insert",
        "Image",
        "Inserts a Markdown image (`![…]()`) at the editor's caret, the selection as its alt text.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item("tag.insert", "Tag", "Inserts a tag sign (`#`) before the editor's selection to type a tag into.", &["Insert"], None, true, nothing),
    editor_item(
        "table.insert",
        "Table",
        "Inserts an empty two-column table (header, delimiter and one body row) at the editor's caret.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "mathBlock.insert",
        "Math Block",
        "Puts the lines the editor's selection covers between `$$` fences, or inserts an empty math block.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "callout.insert",
        "Callout",
        "Puts the lines the editor's selection covers in a note callout (`> [!NOTE]`), or inserts a new one with a title and contents.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    editor_item(
        "footnote.insert",
        "Footnote",
        "Inserts a footnote reference at the editor's caret and its definition at the end of the note, the caret there.",
        &["Insert"],
        None,
        true,
        nothing,
    ),
    with_utc_offset(editor_item(
        "date.insert",
        "Date",
        "Puts today's local date (YYYY-MM-DD) in place of the editor's selection.",
        &["Insert"],
        None,
        true,
        nothing,
    )),
    with_utc_offset(editor_item(
        "timestamp.insert",
        "Date and Time",
        "Puts the current local date and time (YYYY-MM-DD HH:mm) in place of the editor's selection.",
        &["Insert"],
        None,
        true,
        nothing,
    )),
    editor_item(
        "codeBlock.insert",
        "Code Block",
        "Puts the lines the editor's selection covers between ``` fences, or inserts an empty code block.",
        &["Insert"],
        Some("Cmd+Shift+C"),
        true,
        nothing,
    ),
    editor_item("rule.insert", "Horizontal Rule", "Inserts a horizontal rule (`---`) at the editor's caret.", &["Insert"], None, true, nothing),
    // Undo scopes (KitModel 1.1 M12a, engine rule E14a): an agent's undo takes back only its own steps.
    Spec {
        menu: Some(&["Edit"]),
        shortcut: Some("Cmd+Z"),
        ..spec(
            "edit.undo",
            "Undo",
            "Takes back the caller's last edit (the person's or, with `scope: \"agent\"`, the agent's) without touching the other's edits; answers whether there was one.",
            "{ scope?: \"person\" | \"agent\" }",
            undo_schema,
            history,
            true,
        )
    },
    Spec {
        menu: Some(&["Edit"]),
        shortcut: Some("Cmd+Shift+Z"),
        ..spec(
            "edit.redo",
            "Redo",
            "Applies again the edit the caller's last undo took back, in the same scope as `edit.undo`; answers whether there was one.",
            "{ scope?: \"person\" | \"agent\" }",
            redo_schema,
            history,
            true,
        )
    },
];

/// No parameters.
fn no_params() -> Value {
    schema(&[])
}

fn selection_schema() -> Value {
    schema(&[
        ("anchor", integer("Where the selection starts, in Unicode characters; or give line, or x and y.")),
        ("focus", integer("Where it ends (the caret); anchor by default.")),
        ("line", integer("Or a line, from 1: selects it, or with column puts the caret there.")),
        ("column", integer("With line: the column, from 1, in Unicode characters (at most the line's end).")),
        ("x", json!({ "type": "number", "description": "Or a point of the document (with y), in document pixels: the caret goes there." })),
        ("y", json!({ "type": "number", "description": "With x: the point's y, in document pixels." })),
    ])
}

fn picture_schema() -> Value {
    schema(&[("name", required(string("The picture's name as the note writes it."))), ("data", required(string("Its bytes, base64.")))])
}

fn diagram_schema() -> Value {
    schema(&[
        ("source", required(string("The diagram's source: the code between its fences."))),
        ("width", required(json!({ "type": "number", "description": "Its drawn width in document pixels." }))),
        ("height", required(json!({ "type": "number", "description": "Its drawn height in document pixels." }))),
    ])
}

fn range_schema() -> Value {
    schema(&[
        ("top", json!({ "type": "number", "description": "Only tables below this document height." })),
        ("bottom", json!({ "type": "number", "description": "Only tables above this document height." })),
    ])
}

fn width_schema() -> Value {
    schema(&[
        ("width", required(json!({ "type": "number", "description": "The visible width in document pixels." }))),
        ("height", json!({ "type": "number", "description": "The visible height in document pixels: the room below the note is half of it." })),
    ])
}

fn scheme_schema() -> Value {
    schema(&[("scheme", required(one_of(&["light", "dark"], "The host's color scheme.")))])
}

fn highlight_color_schema() -> Value {
    schema(&[("color", required(one_of(&["red", "orange", "yellow", "green", "blue", "purple"], "The highlight's colour.")))])
}

fn control_key_schema() -> Value {
    schema(&[
        ("key", required(string("The letter (a, e, b, f, p, n, v, d, h, k, o, or t), or pageUp or pageDown."))),
        ("shift", boolean("With Shift (Page Up and Page Down extend the selection).")),
    ])
}

fn point_schema() -> Value {
    schema(&[
        ("x", required(json!({ "type": "number", "description": "Across, in document pixels." }))),
        ("y", required(json!({ "type": "number", "description": "Down, in document pixels." }))),
    ])
}

fn where_schema() -> Value {
    schema(&[("at", required(integer("The character, from 0, in Unicode characters.")))])
}

fn replace_schema() -> Value {
    schema(&[
        ("text", required(string("The text that replaces the selection."))),
        ("paste", boolean("Plain text the person pasted: an address over a selection links it.")),
    ])
}

/// Whether the editor runs `id` itself, on its selection.
pub fn is_editor(id: &str) -> bool {
    id.starts_with("select.")
        || id.starts_with("format.")
        || id.starts_with("list.")
        || matches!(
            id,
            "wikilink.insert"
                | "embed.insert"
                | "image.insert"
                | "tag.insert"
                | "table.insert"
                | "mathBlock.insert"
                | "callout.insert"
                | "footnote.insert"
                | "date.insert"
                | "timestamp.insert"
                | "codeBlock.insert"
                | "rule.insert"
                | "edit.selectAll"
                | "edit.cut"
                | "edit.selectSection"
                | "edit.sortLines"
                | "edit.trimWhitespace"
                | "edit.indent"
                | "edit.outdent"
                | "edit.moveLineUp"
                | "edit.moveLineDown"
                | "edit.deleteParagraph"
                | "code.blocks"
                | "diagram.view"
                | "diagram.set"
                | "property.view"
                | "table.geometry"
                | "view.width"
                | "view.scheme"
                | "view.hoverOut"
                | "view.fold"
                | "view.foldAll"
                | "view.unfoldAll"
                | "view.foldMore"
                | "view.foldLess"
                | "view.blockAt"
                | "view.foldProperties"
                | "image.wanted"
                | "image.set"
        )
}

/// `edit.undo`'s parameters: the undo scope, else the scope of the origin it runs as.
fn undo_schema() -> Value {
    schema(&[(
        "scope",
        one_of(
            &["person", "agent"],
            "Whose last step to undo: the person's or the agent's. By default the scope of whoever runs it (an agent undoes only its own steps).",
        ),
    )])
}

fn redo_schema() -> Value {
    schema(&[(
        "scope",
        one_of(&["person", "agent"], "Whose last undone step to redo: the person's or the agent's. By default the scope of whoever runs it."),
    )])
}

/// The commands only the editor or the host runs (kit.json `agent.exclude`).
pub fn excluded() -> Vec<&'static str> {
    SPECS.iter().filter(|spec| !spec.agent).map(|spec| spec.id).collect()
}

/// Every command, with whether it can run now.
pub fn list(can_undo: bool, can_redo: bool, read_only: bool) -> Vec<KitCommand> {
    SPECS
        .iter()
        .map(|spec| {
            // Undo and Redo stay enabled: kit-editor's editor also undoes steps the engine does not hold
            // (a merged draft is a script step of its model), and an undo with nothing to take back
            // answers `done: false`.
            let _ = (can_undo, can_redo);
            let enabled = !(spec.mutates && read_only);
            let addresses = crate::premise::addresses(spec.id);
            KitCommand {
                id: spec.id.into(),
                title: spec.title.into(),
                menu: spec.menu.map(|menu| menu.iter().map(|item| (*item).to_owned()).collect()),
                shortcut: spec.shortcut.map(str::to_owned),
                params: Some(spec.params.into()),
                schema: Some((spec.schema)()),
                description: spec.description.into(),
                output_schema: Some((spec.output)()),
                keywords: Vec::new(),
                mutates: spec.mutates,
                enabled,
                targets: addresses.and_then(|addresses| addresses.targets),
                objects: addresses.map_or(&[], |addresses| addresses.objects),
                object_types: addresses.map_or(&[], |addresses| addresses.object_types),
                places: addresses.map_or(&[], |addresses| addresses.places),
                ..Default::default()
            }
        })
        .collect()
}

pub fn is_known(id: &str) -> bool {
    SPECS.iter().any(|spec| spec.id == id)
}

pub fn mutates(id: &str) -> bool {
    SPECS.iter().any(|spec| spec.id == id && spec.mutates)
}
