# Note kit design: schema `kit.note` v1, commands, `note_check`, editor, export

## 1. Overview

This is the design reference of the Note kit: the KDM schema, the commands with their parameters,
the `note_check` catalogue, the editor on the engine, and previews and export.

- The kit is a Rust engine implementing kition-sdk's `KitDocument` over `kition_kit::session`
  (engine contract 2), compiled to WebAssembly for the page and built as the headless
  `kition-note` executable for agents. The editor is the kit's own `web/` on
  `@kition/sdk/editor`, shown by the host in a frame.
- **The stored text is the file.** Open and save without an edit is byte-identical; an edit
  changes only the bytes it is about. Markdown is never regenerated from a parse.
- **Comments are not written into the `.md`.** They are KDM objects of the replica the host stores.
- Kits reach other files only through handles the host lends; requests carry no path-shaped
  strings. An agent never replaces an existing file except through a draft the person merges.

Names: package `kition-note-ui`, crate `kition-note-kit`, module `kition_note_kit`, class
`NoteDocument`, headless executable `kition-note`.

## 2. KDM schema `kit.note` v1

### 2.1 Schema

```json
{
  "kdm": 1,
  "kit": "kit.note",
  "schemaVersion": 1,
  "root": "Note",
  "types": {
    "Note": {
      "fields": {
        "body": { "kind": "text" },
        "eol":  { "kind": "enum", "values": ["lf", "crlf", "mixed"], "optional": true },
        "bom":  { "kind": "boolean", "optional": true }
      }
    },
    "CommentThread": "<kition_kdm::comment::COMMENT_THREAD_TYPE_JSON>",
    "Comment":       "<kition_kdm::comment::COMMENT_TYPE_JSON>"
  }
}
```

The two comment types are inserted verbatim from kition-sdk `crates/kition-kdm/src/comment.rs`, the shared comment types of every kit; `model/schema.json` is generated from the Rust constants and a test keeps them equal.

| Field | Kind | Meaning | Who writes it |
|---|---|---|---|
| `Note.body` | `text`, no marks | The whole file as text, without a byte order mark. Line endings are `\n` when the file used one style; a file with mixed endings or a lone CR keeps every `\r` exactly where it was | Every text command; import as a `system` change |
| `Note.eol` | `enum` `lf` \| `crlf` \| `mixed`, optional (absent = `lf`) | How `\n` in `body` is written: `lf` as is, `crlf` as `\r\n`, `mixed` as is (the `\r` characters are in `body`) | Import (from the file), `file.setLineEndings` |
| `Note.bom` | `boolean`, optional (absent = `false`) | Write a UTF-8 byte order mark first | Import, `file.setLineEndings` |
| `CommentThread`, `Comment` | reserved types | Comment threads beside the content (section 2.4) | `comment.*` commands |

Exporting the file is `bom ? EF BB BF : ""` followed by `body` with `\n` → `\r\n` when `eol` is `crlf`. Nothing else is derived; there is no serialiser. `eol` and `bom` are independent: any combination is valid.

### 2.2 Why the file stays one text field (no block objects)

| Option | Round trip | Concurrent editing | Cost |
|---|---|---|---|
| **One `text` field holding the file (chosen)** | Byte-identical by construction: export writes the stored characters back | Sequence merge of characters; two people typing in one paragraph both keep their text | Structure is a derived index: the engine re-parses changed regions (source-positioned) |
| Block objects (`Paragraph`, `Heading`, `Table`, `Cell` …) with marks | Every save regenerates Markdown from objects: spacing, list markers, table padding, YAML style, escapes and HTML must all be reproduced; byte identity needs a lossless concrete-syntax tree in the KDM | Structural merges (split a paragraph while another person edits it) need conflict rules per block kind | A serialiser and a fidelity suite |
| Text plus a stored index (heading ids, table objects kept in sync) | Byte-identical | The stored index can disagree with the text after a merge and needs repair rules | Two sources of truth for one fact |

Structure is therefore never stored. Headings, sections, tables, properties, links, tasks and block ids are what the Markdown index (`crates/engine/markdown`) finds in `body`; commands edit the exact source span of what they address (section 3).

### 2.3 Line style and bytes

| | `kit.note` | Reason |
|---|---|---|
| Line style | `eol`, `bom` registers | A co-editor, a headless agent or a restored checkpoint that never read the file must write it in the file's style |
| Final newline | Not recorded | It is a character of `body` |
| Mixed endings, lone CR | Kept in `body` | Byte identity for every file |
| Invalid UTF-8 | Refused for editing: the note opens read-only and `check` reports `note.invalid-utf8` | Never write bytes the person did not have |
| Comment types | kition-sdk's `kition_kdm::comment` constants | One definition for every kit |

### 2.4 Comments: where they live

- Threads are KDM objects of the replica, so they converge with the text and travel through sync like any change; a commenter role may write only them.
- **The `.md` never holds them.** The replica is stored by the host and reached by the editor through the Kit UI protocol's `replicaRead`/`replicaWrite`. A note opened without its replica has no comments.
- Anchors are `{ object: <Note id>, field: "body", range: { start, end }, quote }`. All comment commands run in the Rust engine, so stored ranges are always **Unicode scalar** offsets; the editor converts for display. Ranges are hints: on read the engine relocates each thread by its `quote` near the stored range.
- **A host that keeps threads itself** (Kit UI protocol capability `comment-anchors`) gets none of these: the editor lists `comment-anchors`, a selection's Comment (selection bar, context menu, Mod+Alt+M) sends `host/service` `comment.create` with the anchor `{ ids: ["note"], range: { start, end }, quote }` (scalar offsets of `body`, the quote at most 500 characters), and the kit's own comments panel is hidden. The editor marks each open thread of `comments/show` (a tint over its text from `presenceLocate`, a marker right of the column) and sends `comments/activated` on a marker's click. The host stores an anchor as it was made, so the editor keeps it on the text (`web/src/commentAnchors.ts`): while the note is open each range moves through every change (the common start and end of the text before and after), and an arriving anchor is found by its quote nearest to its range; `comments/locate` answers from the same ranges. `comment.create` is served only to a person with comment rights, so a viewer is offered no Comment; a read-only person selects with the mouse through `select.set` (`web/src/readOnlySelect.ts`), since kit-editor refuses the text tool's gestures there.
- Headless: the executable serves the replica methods (`model.open`, `model.import`, `model.export`, `model.save`), so a host that stores the replica can give the agent the same threads.

### 2.5 Identity, open and save

| Step | Behaviour |
|---|---|
| Open a file with no stored replica | `import`: a new replica (new document UUID) created with `body`, `eol`, `bom` in its creating `system` change; the body is inserted in pieces of about 16 KB, last first, for edit cost |
| Open a file with its stored replica | Load the replica, then merge what the file holds that the replica does not as one `system` change (patience line diff refined per line, with a work budget; `crates/formats/markdown-file/src/merge.rs`); a file changed on disk while unsaved edits exist merges the same way and never overwrites them |
| Save | Export as in 2.1; the host writes the `.md` and stores the replica |
| `model.open` (headless, used by drafts) | Implemented; never falls back to an empty document |
| Invalid UTF-8 | The engine keeps the original bytes, shows a lossy text read-only, refuses every mutating command with `read-only`, and saves the original bytes unchanged |

### 2.6 Positions

Positions are Unicode scalars in Rust and UTF-16 code units in TypeScript. Agents and the engine count Unicode characters, so **every command that takes or returns a text position takes `unit`**: `"scalar"` (default) or `"utf16"` (for hosts and editors that count UTF-16). Results report positions in the unit of the request. A position inside a character (a split surrogate pair in `utf16`) is `out-of-range`. Agents are steered away from raw offsets towards addresses (heading path, block id, property key, table coordinates, quote, line, object id).

### 2.7 Live co-editing

The note co-edits through the Kit UI protocol's `replica` and `presence` capabilities, so any host that relays replica changes gets live co-editing. Nothing here is note-specific in the protocol; what the note adds is how its one text field behaves. The KDM is a plain object store with an ordered change log (kition-sdk data model 6): a replica takes another's changes only in order, one line of changes after the other, and refuses changes that do not follow its version (`stale`); concurrent edits are not merged (Kition Desktop is for one person; collaboration comes later with server-ordered changes).

| Concern | How the note does it |
|---|---|
| Local edits as KDM changes | Every edit (typing, a command, a section move) is text inserts and deletes on `Note.body` (scalar indices), `eol`/`bom` register writes, or comment objects; kit-editor sends `replica/changed` after each, and the host pulls them with `replica/export` |
| Remote changes, live | `replica/import` goes to the engine worker (`import_changes`); kit-editor repaints the canvas and re-asks the text tool's overlays, so the other person's text shows at once |
| Text runs, blocks, marks | All are Markdown source in `body`: a paragraph is a run of characters, a block boundary is `\n`, a mark is its delimiters (`**`, `[[`, `|`). A change is text inserts and deletes at character positions; a block move (`section.move`) is a delete plus an insert of the section's text |
| The file stays what people wrote | The text is the file: nothing is regenerated, so only edited spans differ from the original bytes |
| Caret, selection, folds | `Editor::follow_remote` diffs the text before and after an import (common start and end skipped, the middle diffed with the file merge's line-then-character diff) and moves every byte offset the editor keeps through those edits: the caret, the anchor, folds, the open typing step's ranges, the undo records' selections and the selection history. Text inserted at the caret goes after it (each person's typing run stays whole); an offset inside replaced text goes to the replacement's end |
| Undo | The person's Undo takes back only their own changes (KDM undo scope). The replica's undo group ends where an import changes the same text, so the editor ends its typing step there too, and a run that another person's change interrupted undoes in two steps (each restoring its own selection) |
| Scrolling | A caret moved by another person's change is not scrolled to (`caretRect`'s `remote`): one in view stays where it is on screen, one out of view is left alone |
| Presence | The engine shares `{ caret, anchor }` as `{ object: "note", field: "body", offset }` (Unicode characters, the same on every replica) and `["note"]` as the selection when text is selected (`presenceCaret`); the editor sends it with `presence/set`. Each peer's caret and selection are placed in this person's layout (`presenceLocate`) and drawn over the canvas in the peer's color with their name (`web/src/Presence.tsx`) |

Verified by the editor's co-editing tests (`bindings/wasm/src/coedit_tests.rs`, `note.collab.live/editor-follows-remote`) and two editors in a browser (`web/tests/workspace.mjs`, `note.collab.live/two-editors-in-browser`).

## 3. Commands

### 3.1 Conventions (every command)

- **One command, one transaction**, with the caller's origin: `person` from the editor, `agent` from `note_run`, `system` for imports, merges and the host's rename retargeting.
- **Positions** take `unit` (`"scalar"` default, `"utf16"`), section 2.6. Ranges are `from`, `to` (end exclusive).
- **Addresses** (preferred by agents):

  | Address | Type | Meaning |
  |---|---|---|
  | `heading` | array of strings | Heading path from the top, each element a heading's plain text (inline Markdown removed, trimmed): `["Plan", "Risks"]`. Two sibling headings with the same text answer `ambiguous` with their lines; use `line` then |
  | `line` | integer, from 1 | A line of the current text (after the change that produced it) |
  | `section`, `task` | string | An object id from `projection.read` (`section:3`, `task:2`), which follows its object through edits while the note is open |
  | `blockId` | string | An Obsidian block id (`^risk-1`), without the caret |
  | `key` | string | A top-level frontmatter key |
  | `table` | integer, from 0 | A table's index in document order, as `table.read`, `note_inspect` and `task.list` number them |
  | `row`, `column` | integers, from 0 | `row` 0 is the header row, 1… the body rows (the delimiter row is not addressable); `column` from the left |
  | `quote` | string | Text to find; the first occurrence unless `occurrence` (from 1) is given |

- **Stale-range guard**: every command that edits a range or a table cell from positions computed earlier takes `expected` (the source text the caller believes is there) and refuses with `stale` when the text differs, so a stale position fails instead of changing the wrong text.
- **Minimal span**: a command rewrites only the source span of what it addresses and keeps the surrounding style (table padding and outer pipes, list markers, YAML list style, quoting, line endings). Formatting the whole construct is always a separate, explicit command (`table.format`).
- **Errors** are typed reasons: `invalid-params`, `not-found`, `ambiguous`, `out-of-range`, `stale`, `read-only`, `refused` (with a `code` such as `note.property-complex` when a check code describes why). An empty `params` answers a typed error and changes nothing.
- **Schemas**: every command carries a `ParameterSchemaSpec`, including the read-only ones; descriptions are one English sentence.
- **No file-like parameter names.** kition-headless treats a parameter named `path`, `paths`, `folder`, `dir`, `directory`, `output`, `outputDir`, `file`, `files` or `target` as file access and hides the command from agents (`FILE_PARAMS`). So the heading address is `heading` (not `path`), a link's destination is `destination` (not `target`), `section.move` names its target heading `toHeading` or `toLine`, and `table.add` takes `line`. A test (`every_agent_command_has_a_schema_and_no_file_parameter`) keeps it so.

### 3.2 Document commands

`R` = read-only (`mutates: false`). Agent: **yes** = listed by `note_commands` and run by `note_run`; **no** = the editor or host only (manifest `agent.exclude`). `bindings/wasm/src/commands.rs` lists every command with its schema; `run.rs` runs them on `kition-note-edit` and `export.rs` runs the exports.

| Command | Parameters | R | Agent | Notes |
|---|---|:-:|:-:|---|
| **Read** | | | | |
| `text.read` | `from?`, `to?`, `unit?` | R | yes | The text or a range; `length` in the request's unit |
| `outline.read` | `limit?`, `cursor?` | R | yes | Headings with `path`, `level`, `line`, `range`, word count of the section; paged |
| `section.read` | `heading` or `line`, `withHeading?` (default `true`) | R | yes | The section's Markdown and its range |
| `block.read` | `blockId` | R | yes | The block's Markdown, line and range |
| `property.list` | — | R | yes | Typed values (`string`, `number`, `boolean`, `date`, `datetime`, `list`), each key's line range and style, `complex: true` for keys the engine will not rewrite |
| `table.read` | `table` | R | yes | Header, alignment, rows as cell text, line range; `tables: n` without `table` |
| `link.list` | `kinds?` (`wikilink`, `markdown`, `embed`, `tag`, `blockId`) | R | yes | Every link with `line`, `index` (in the line), target, alias, and resolution state (`resolved`, `missing`, `missing-anchor`, `unknown` without host access) |
| `task.list` | `heading?` or `line?` | R | yes | Task items with `line`, `done`, text, block id |
| `text.find` | `find`, `regex?` (default `false`), `caseSensitive?`, `unit?`, `limit?` | R | yes | Matches with ranges and lines |
| `projection.read` | `kinds?` | R | yes | The note's sections, tasks, tables, properties and links with their ids, label and first line |
| `comment.threads` | — | R | yes | Threads with their relocated range now |
| `template.expand` | `markdown`, `title?`, `date?` (ISO 8601) | R | yes | Expands `{{title}}`, `{{date}}`, `{{time}}` and `{{date:FORMAT}}`; pure, no file access |
| `page.layout` | `pageSize?` (`letter`, `a4`), `page?` | R | yes | Where each block lands on the pages, blocks too wide for the column, and what the pages cannot draw (section 6) |
| `file.export` | `format` (`html`, `pdf`), `standalone?` (HTML; default `true`: the stylesheet inlined; `false`: only the `<main>` element), `pageSize?` (PDF: `letter`, `a4`) | R | yes | Answers the file as base64 `data` with `format`, `bytes`, `name`, `mime`, and for a PDF `pages` and `unsupported`; `note_export` writes it as a new file, and the editor takes the bytes through the `exportHtml` and `exportPdf` extras. One-inch margins. Section 6 |
| **Text** | | | | |
| `text.insert` | `at`, `text`, `unit?` | | yes | |
| `text.delete` | `from`, `to`, `unit?`, `expected?` | | yes | |
| `text.replace` | `from`, `to`, `text`, `unit?`, `expected?` | | yes | |
| `text.findReplace` | `find`, `replace`, `occurrence?` (integer from 1, or `"all"`; default 1), `regex?`, `caseSensitive?` | | yes | Answers the count replaced |
| `note.write` | `markdown` | | yes | The whole new text, applied as a line diff (never a whole replace), so concurrent edits elsewhere survive |
| **Structure** | | | | |
| `section.replace` | `heading`, `line` or `section`, `markdown`, `withHeading?` (default `false`: the body under the heading) | | yes | |
| `section.insert` | `heading`, `line` or `section`, `position` (`before`, `after`, `start`, `end`: inside the section), `markdown` | | yes | Adds blank lines only where Markdown needs them |
| `section.delete` | `heading`, `line` or `section` | | yes | Heading and body, up to the next heading of the same or a higher level |
| `section.move` | `heading`, `line` or `section`; `toHeading`, `toLine` or `toSection`; `position` (`before`, `after`, `start`, `end`) | | yes | Keyboard "move section", agents |
| `heading.setLevel` | `heading`, `line` or `section`, `level` (1–6) | | yes | Changes only the marker (`##`), or turns a Setext heading into ATX only when asked (`style: "atx"`) |
| `block.replace` | `blockId`, `markdown` | | yes | Keeps the `^id` unless `markdown` sets one |
| `block.append` | `heading?`, `line?` or `section?` (end of section; end of note without), `markdown` | | yes | |
| `block.setId` | `line`, or `blockId` with `occurrence`; `id` | | yes | Fix for `note.duplicate-block-id` |
| `task.toggle` | `line`, `blockId` or `task`, `done?` (default: flip) | | yes | Changes the one character `[ ]` ↔ `[x]` |
| **Properties** | | | | |
| `property.set` | `key`, `value` (`json`: string, number, boolean, null, or a list of strings) | | yes | Replaces exactly the key's lines in the key's existing style (scalar, flow list, block list); a new key goes last in the block; creates the frontmatter when there is none. Refuses `note.property-complex` keys |
| `property.delete` | `key`, `occurrence?` | | yes | `occurrence` removes one of duplicated keys (fix for `note.frontmatter-duplicate-key`) |
| `property.rename` | `key`, `to` | | yes | Changes only the key token |
| **Tables** | | | | |
| `table.add` | `line` (insert before it) or `heading` with `position`, `columns`, `rows?`, `header?` (array of strings) | | yes | Written in the note's dominant table style, or `\| a \| b \|` with `---` when it has none |
| `table.setCell` | `table`, `row`, `column`, `text`, `expected?` | | yes | Rewrites only the cell's span; pads only when that row's cells were padded; escapes `\|` |
| `table.insertRow` | `table`, `at` (row index), `cells?` | | yes | |
| `table.insertColumn` | `table`, `at` (column index), `header?`, `align?` | | yes | Touches each row once, keeping each row's spacing |
| `table.deleteRow` | `table`, `row` | | yes | The header row cannot be deleted |
| `table.deleteColumn` | `table`, `column` | | yes | |
| `table.moveRow` | `table`, `from`, `to` | | yes | |
| `table.moveColumn` | `table`, `from`, `to` | | yes | |
| `table.align` | `table`, `column`, `align` (`left`, `center`, `right`, `none`) | | yes | Changes only that column's delimiter cell |
| `table.format` | `table` | | yes | The one command that rewrites a whole table (pads and aligns); fix for `note.table-column-mismatch` |
| **Links** | | | | |
| `link.insert` | `at`, `unit?`, `destination` (a link text, for example `Plan` or `docs/plan.md#Risks`), `text?`, `style?` (`wikilink` default, `markdown`), `embed?` | | yes | `destination` is document content, not a file request (no handle needed) |
| `link.update` | `line`, `index` (from 0), `destination`, `text?` | | yes | Changes one link's destination; fix for `note.broken-anchor` |
| `link.retarget` | `from`, `to` (old and new link targets) | | no | Run by the host as a `system` change on every linking note when a file is renamed; rewrites wikilinks, Markdown links and embeds, keeping aliases and anchors |
| **Comments** (manifest `collaboration.comments.commands`) | | | | |
| `comment.add` | `body`, then one of: `from`+`to` (+`unit?`), `quote` (+`occurrence?`), `heading` or `line` (the heading line), nothing (the whole note); `author?`, `name?`, `created?` | | yes | An agent's comment is written as the agent's principal |
| `comment.reply` | `thread`, `body`, `author?`, `name?` | | yes | |
| `comment.resolve` | `thread`, `resolved?` (default `true`) | | yes | |
| `comment.move` | `thread`, `quote` or `from`+`to` (+`unit?`) | | no | A person's gesture in the editor |
| `comment.delete` | `thread`, `comment` | | no | Agents do not delete people's comments |
| **File and history** | | | | |
| `file.setLineEndings` | `eol` (`lf`, `crlf`), `bom?` | | yes | Rewrites `\r` in `body` and sets the registers in one transaction; fix for `note.mixed-line-endings` |
| `edit.undo` | `scope?` (`person`, `agent`) | | yes | In the undo scope `scope` names, else the scope of the origin it runs as: the person's Undo keeps the agent's steps, and an agent's undo takes back only its own |
| `edit.redo` | `scope?` (`person`, `agent`) | | yes | Like `edit.undo` |

The editor has further commands of its own that act on the selection or the view (`select.*`, `format.*`, `list.*`, the `*.insert` commands of the Insert menu such as `callout.insert`, most `edit.*`, `view.*`, and the table and diagram geometry it draws from); `kit.json` excludes them from agents. Tags, callouts and plain lists have no document commands of their own: tags are listed by `link.list` (`kind: tag`, also the `tags` property) and counted with callouts by `note_inspect`; list items are added with `block.append` or `section.insert` and checked with `task.toggle`.

Not commands: selection, viewport and fold state (the editor's own state, reported to the host as text ranges); link resolution for embeds (the binding asks the host for a handle and the engine reads it through the lent file); the links answer for the host's link index (`linksOf()`, an engine extra declared in `kit.json` `extras`).

### 3.3 Parameter schema examples

```json
{
  "text.replace": {
    "from":     { "type": "integer", "required": true, "description": "Where the range starts." },
    "to":       { "type": "integer", "required": true, "description": "Where the range ends (exclusive)." },
    "text":     { "type": "string",  "required": true, "description": "The Markdown to put there." },
    "unit":     { "type": "string",  "enum": ["scalar", "utf16"], "default": "scalar", "description": "How positions are counted: Unicode characters, or UTF-16 code units." },
    "expected": { "type": "string",  "description": "The text the range holds now; the command is refused when it differs." }
  },
  "section.replace": {
    "heading":     { "type": "array", "items": { "type": "string" }, "description": "Heading path from the top, for example [\"Plan\", \"Risks\"]." },
    "line":        { "type": "integer", "description": "The heading's line, from 1, instead of heading." },
    "markdown":    { "type": "string", "required": true, "description": "The new Markdown of the section." },
    "withHeading": { "type": "boolean", "default": false, "description": "Replace the heading line too." }
  },
  "table.setCell": {
    "table":    { "type": "integer", "required": true, "description": "The table's index in the note, from 0." },
    "row":      { "type": "integer", "required": true, "description": "The row, 0 for the header row." },
    "column":   { "type": "integer", "required": true, "description": "The column, from 0." },
    "text":     { "type": "string",  "required": true, "description": "The cell's new text (Markdown inline)." },
    "expected": { "type": "string",  "description": "The cell's text now; the command is refused when it differs." }
  },
  "property.set": {
    "key":   { "type": "string", "required": true, "description": "A top-level frontmatter key." },
    "value": { "type": "json",   "required": true, "description": "A string, number, boolean, null, or a list of strings." }
  }
}
```

A command that takes `heading` or `line` (one of the two) declares both optional and answers `invalid-params` when neither or both are given.

### 3.4 Agent surface

Manifest (`kit.json`, excerpt):

```json
"agent": {
  "prefix": "note",
  "tools": "document",
  "drafts": true,
  "objectModel": true,
  "layout": true,
  "export": { "command": "file.export", "data": "data" },
  "exclude": ["link.retarget", "comment.move", "comment.delete", "select.get", "…"]
},
"headless": {
  "executable": "kition-note",
  "protocol": "kition-kit/1",
  "features": ["view.describe", "document.check", "document.identity", "model.open", "model.import", "model.export", "model.save", "model.describe", "model.batch"]
}
```

| Tool | What note answers |
|---|---|
| `note_inspect` | Outline (heading paths, levels, lines; paged), properties typed, link, embed, tag and block-id counts with unresolved ones listed, tables (index, size, line), task counts, words, `eol`/`bom`, open and resolved comment threads; `preview: true` renders the note's first page (or `target`: a heading path or a page) through the engine; `layout: true` answers pages with placed blocks and overflow (section 6) |
| `note_commands` | Section 3.2, agent-facing commands, with schemas |
| `note_run` | Steps of section 3.2 commands; structured reads go here |
| `note_query`, `note_batch` | The object model: `Note`, `CommentThread`, `Comment`, and the projection's objects computed from the body. `eol` and `bom` are read-only in the object model, so an agent changes them only through `file.setLineEndings`, which also rewrites the `\r` characters |
| `note_check` | Section 4 |
| `note_create`, `note_export` | `creates` `note` (`.md`, "Untitled note"); `exports` `html`, `pdf` through `file.export` |
| `note_draft_ready`, `note_draft_reopen` | Drafts are the way an agent changes an existing note |

`VOLATILE` `inspect()` keys (kept out of `view.describe` across a save): `selection`, `view`.

## 4. `note_check`

### 4.1 Rules

- `check()` reads the note and changes nothing (revision, dirty state, history unchanged); errors first; at most 100 diagnostics per code, the rest counted in one `note.truncated`.
- Link checks need to know whether targets exist. Link targets are asked of the host's lent files (`kition_kit::host`: headless `kition-note` lends the workspace, the host's kit worker does in the page): a missing file is a host answer of "not found"; no host, or any other answer, is `unknown` and raises **no** link diagnostic (never a false error).
- A wikilink without a folder is looked up beside the note and at the workspace root only; Obsidian finds a note of that name in any folder, so a link to a note elsewhere can be reported.
- `target` is `{ objectId: "note", path: "body:<line>:<column>" }` (both from 1, the column in Unicode characters) and the message names the line and the heading path above it.
- Fix hints are commands `note_run` accepts (agent-facing commands of 3.2) with concrete params; a hint is given only when the fix is unambiguous.
- The package ships `agent/checks.json` (`[{ code, severity, summary, fixture }]`); each `fixture` is `conformance/check-<code>.md`, which triggers exactly that code (`bindings/wasm/tests/checks.rs`).

### 4.2 Codes

| Code | Severity | Raised when | Fix hint | Triggering fixture (`conformance/check-<code>.md`) |
|---|---|---|---|---|
| `note.invalid-utf8` | error | The file is not valid UTF-8; the note is open read-only and saves its original bytes | — | Any short text with one byte `0xFF` in a line |
| `note.unclosed-fence` | error | A code fence (```` ``` ```` or `~~~`) or a `$$` math block is never closed, so the rest of the note renders as code | `text.insert { at: <end>, text: "\n```\n" }` (the opening fence's marker) | `# Setup\n\n```bash\nnpm install\n\nMore text that became code.\n` |
| `note.frontmatter-invalid` | error | The leading `---` block does not parse as YAML, so no property is read | — (edit the source) | `---\ntitle: [unclosed\n---\n# Note\n` |
| `note.frontmatter-duplicate-key` | error | A top-level key appears twice (Obsidian keeps one, readers disagree which) | `property.delete { key, occurrence: 2 }` | `---\ntags: [a]\ntags: [b]\n---\n` |
| `note.broken-link` | error | A wikilink or Markdown link names a file the host says does not exist, or its target is empty (`[[]]`, `[text]()`) | — | `See [[Missing note]] and [the plan](missing/plan.md).\n` |
| `note.broken-embed` | error | `![[…]]` or `![](…)` names a file that does not exist | — | `![[missing-diagram.png]]\n` |
| `note.broken-anchor` | error | A link's `#heading` or `#^block` part names no heading or block id in its target (this note when the link has no file part) | `link.update { line, index, destination }` when one heading matches ignoring case and punctuation | `# Plan\n\nSee [[#Risks]] and [[#^missing-id]].\n` |
| `note.duplicate-block-id` | error | The same `^id` ends two blocks, so links to it are ambiguous | `block.setId { blockId, occurrence, id: "<id>-<occurrence>" }` | `First ^a1\n\nSecond ^a1\n` |
| `note.table-column-mismatch` | warning | A table row has more or fewer cells than its header (cells are dropped or added when it renders) | `table.format { table }` | `\| a \| b \|\n\| - \| - \|\n\| 1 \| 2 \| 3 \|\n` |
| `note.heading-level-skip` | warning | A heading is more than one level below the previous heading (`#` then `###`), which breaks the outline and exported tables of contents | `heading.setLevel { line, level }` | `# Title\n\n### Detail\n` |
| `note.property-complex` | info | A key's value is a nested map, a block scalar, or carries comments; the engine reads it but will not rewrite it (`property.set` refuses it) | — | `---\nauthor:\n  name: Ann\nsummary: \|\n  Two\n  lines\n---\n` |
| `note.mixed-line-endings` | info | The file mixes line endings or has a lone CR; kept as is, new lines are written with `\n` | `file.setLineEndings { eol: "lf" }` | `one\r\ntwo\nthree\r\n` |
| `note.truncated` | info | More than 100 diagnostics of one code; the rest are counted, not listed | — | `conformance/check-truncated.md` |

The fixtures with bytes an editor cannot type (`0xFF`, CR) are written by `xtask`.

### 4.3 What is deliberately not a diagnostic

- Markdown style lint (list marker consistency, trailing spaces, line length): opinion, not breakage.
- Unresolved links when the host cannot answer: state `unknown` in `link.list`, no diagnostic.
- Obsidian settings in `.obsidian/` (attachment folder, link format): not read.
- Authoring errors of a command (bad table coordinates, ambiguous path): command errors (3.1), not check results.
- Blocks wider than the page column: reported by `page.layout` and `note_inspect` `layout: true`, not by `check`.

## 5. Editor on the engine

The engine lays out, draws, and edits the note; `web/` is kit-editor's frame around its canvas
(`KitDocumentCanvas`).

- **View** (`crates/view`). `kition-note-fonts` registers the faces the editor hands over through
  `addFont` (Inter, JetBrains Mono, Droid Sans Fallback; a web build embeds none), picks a face
  per character, shapes with harfrust, and caches skrifa outlines. `kition-note-view` styles every
  byte from pulldown-cmark's offset iterator, wraps each source line into rows whose pieces keep
  their source bytes, and draws with vello_cpu; lines are memoised, so an edit lays out only its
  line. Without fonts text draws as bars, so headless renders and the contract check still see
  the note.
- **Live preview.** The lines the selection touches show their Markdown; on the others the
  syntax that only shapes text is hidden (heading marks, emphasis and code marks, link brackets
  and destinations, quote marks, rules), bullets are dots, a task's marker is a box, and an
  image-only line draws its picture (bytes from the host through `image.wanted` / `image.set`).
- **Math and diagrams.** Math (`$…$`, `$$…$$`) is laid out in the engine (`crates/view/view/src/math.rs`)
  and drawn as formulas away from the caret. Mermaid diagrams and HTML blocks are drawn by the
  editor over the canvas (`web/src/Diagrams.tsx`): the engine asks for the blocks it has no size
  for (`diagram.view`) and keeps each one's room once it is measured (`diagram.set`).
- **Tables.** A GFM table is a grid: pipes and the delimiter row take no room, cells wrap in their
  columns, and caret stops carry a cell and a line offset. The editor puts the table controls
  over the canvas from `table.geometry` (append buttons on the edge rails, the column menu) and
  runs `table.*`.
- **Editing** (`bindings/wasm/src/editor.rs`). The selection is view state (byte offsets that
  follow other people's edits). One `text` tool: click, drag, double click, keys, typed and
  pasted text, all through `text.replace` as the local person; a typing run is one undo group,
  multi-line edits are one step. The editor's own commands act on the selection and are kept
  from agents (`select.*`, `format.*`, `list.*`, the Insert menu's `*.insert`, `edit.*`).
- **Line separator.** A `mixed` note keeps its `\r` characters in `body`; they draw nothing, the
  caret at a line's end stays before a trailing `\r`, and Enter there breaks after it.
- **Units.** The editor's commands count Unicode characters (the engine's unit); `utf16` stays
  available for hosts that count that way.
- **Pointer contract.** A click on a link drawn as a link opens it (`openLink` request, the host's
  `host/open`); Cmd/Ctrl+click opens one shown as Markdown. Copy and cut take the engine's
  selection; paste arrives as typed text.
- **Strings.** Kit strings are localised in the kit (`en-US`, `zh-CN`).

## 6. Previews, layout and export

`kition-note-export` (`crates/formats/export`) holds the reading view used for page previews,
`page.layout` and `file.export`: `doc.rs` reads the note into blocks (pulldown-cmark with the
note's options, plus the Obsidian syntax: `%%comments%%` dropped, highlights, tags, block ids,
callouts; the frontmatter is not drawn), `layout.rs` lays them out on Letter or A4 pages with
one-inch margins, `raster.rs` draws the pixels, `pdf.rs` writes a PDF 1.4 file, and `html.rs` the
HTML file. The pixels and the PDF draw the same marks, so a preview and an export agree. Printing
hands the PDF to the host.

`layout: true` in `note_inspect` and `page.layout` answer each page's placed blocks with their
source `line` and bounds, and the blocks wider than the column.

### 6.1 What the export draws

Drawn: headings, paragraphs with bold, italic, strikethrough, inline code, highlights, tags, links (URI annotations in the PDF), wikilinks (styled, not resolved), footnotes (numbered, listed at the end), bullet, numbered and task lists (nested), quotes and callouts, tables (alignment, wrapped cells, column widths by content), code blocks (wrapped by characters, across pages), rules. Text wraps at spaces and between wide characters; pages break between lines and table rows; a heading keeps its next line.

Not drawn yet, and how it shows (counted in every `page.layout` and PDF `file.export` answer as `unsupported`, and listed in the export dialog before the person exports):

| Missing | Shows as |
|---|---|
| Glyphs outside Windows-1252 (Chinese, Japanese, Korean, emoji, most symbols) | An outlined box per character, counted with a sample |
| Text in the pixels | Bars over each run's x-height (greeked); boxes and rules are exact |
| Fonts other than PDF's standard Helvetica and Courier | Helvetica and Courier, not embedded; bold-italic as Helvetica-BoldOblique |
| Images and embeds (`![](…)`, `![[…]]`) | A labelled line, `[Image: …]` or `[Embed: …]` |
| Math (`$…$`, `$$…$$`) | Its source, in monospace |
| Mermaid diagrams | Their source in a code box |
| Raw HTML blocks | Their source (kept as HTML in the HTML export) |
| Syntax highlighting of code; heading anchors and an outline (bookmarks) in the PDF; internal links between notes in the PDF | Plain code; no PDF outline; wikilinks styled only |

The HTML export renders the same blocks with the Obsidian syntax (wikilinks as relative `.md` links, embeds, callouts, highlights, tags, block ids as element ids, footnotes), drops `%%comments%%` and the frontmatter, inlines a stylesheet, and never runs a script (math and Mermaid stay their source; `javascript:` links become `#`); raw HTML in the note is kept. Its expected file is `conformance/export.html` for `conformance/export.md`.

## 7. Known limitations

- Heading paths are ambiguous for repeated headings (`## Notes` under every day): the command
  answers `ambiguous` with the lines, and `note_inspect` lists lines with paths; use `line` or a
  section id.
- A property whose YAML style the lossless editor does not know (nested maps, block scalars,
  comments, flow maps, anchors, tags) is read but never rewritten: `property.set` refuses it with
  `note.property-complex`.
- Wikilinks without a folder are resolved beside the note and at the workspace root only
  (section 4.1).
- The PDF and the preview pixels draw with the standard PDF fonts and no images, math or diagrams
  (section 6.1).
