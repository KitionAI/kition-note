# Note writing format

How a model writes Kition Note documents: the steps it sends, how it names what it changes,
what a new document starts with, the named values commands accept, and worked examples.
The package ships the short version for models as [agent/guide.md](../agent/guide.md); this file is
the complete reference it is drawn from. Everything below comes from the engine itself
(its commands and their parameter schemas, and what `inspect` reports) and from the guide.

## What a model writes

A model never writes the file bytes. It calls the kit's agent tools (prefix `note`), and the
engine applies each change, keeps it in undo history and saves the file:

| Tool | Use it to |
| --- | --- |
| `note_inspect {path}` | See the note: outline (heading lines), properties, tables, tasks, links, comment threads, word count, and a preview image |
| `note_commands {path, query}` | Find commands by keywords (`task`, `table cell`, `section`, `property`, `text`). It answers at most 12; use more specific words when it says "showing 12" |
| `note_run {path, steps}` | Run 1 to 50 steps `{command, params}` in order. It stops at the first failure and saves after the last step |
| `note_query` / `note_batch` | Read or set objects of the object model (`table`, `property`, `comment`) |
| `note_check {path}` | List problems: broken links and anchors, unclosed fences, bad front matter, skipped heading levels, tables with uneven columns |
| `note_create {path}` | Make a new, empty note |

Edits are live unless a call passes `draft: true`; then they go to a draft the person reviews, and
`note_draft_ready` hands it to them.

## Syntax

A step is one engine command with its parameters, as JSON:

```json
{"command": "<command id>", "params": {"<name>": <value>}}
```

`note_run` takes a list of steps and runs them in order, stopping at the first failure. Parameter
names, types and required fields come from each command's schema; `note_commands` returns them.
Commands by namespace (the part of the id before the first dot):

| Namespace | Commands | Change the document | Open to the agent | Examples |
| --- | ---: | ---: | ---: | --- |
| `block` | 4 | 3 | 4 | `block.read`, `block.replace`, `block.append` |
| `callout` | 1 | 1 | 0 | `callout.insert` |
| `code` | 1 | 0 | 0 | `code.blocks` |
| `codeBlock` | 1 | 1 | 0 | `codeBlock.insert` |
| `comment` | 6 | 5 | 4 | `comment.threads`, `comment.add`, `comment.reply` |
| `date` | 1 | 1 | 0 | `date.insert` |
| `diagram` | 2 | 0 | 0 | `diagram.view`, `diagram.set` |
| `edit` | 12 | 10 | 2 | `edit.selectAll`, `edit.cut`, `edit.indent` |
| `embed` | 1 | 1 | 0 | `embed.insert` |
| `file` | 2 | 1 | 2 | `file.export`, `file.setLineEndings` |
| `footnote` | 1 | 1 | 0 | `footnote.insert` |
| `format` | 20 | 20 | 0 | `format.math`, `format.highlightColor`, `format.clear` |
| `heading` | 1 | 1 | 1 | `heading.setLevel` |
| `image` | 3 | 1 | 0 | `image.wanted`, `image.set`, `image.insert` |
| `link` | 4 | 3 | 3 | `link.list`, `link.insert`, `link.update` |
| `list` | 4 | 4 | 0 | `list.bullet`, `list.numbered`, `list.task` |
| `mathBlock` | 1 | 1 | 0 | `mathBlock.insert` |
| `note` | 1 | 1 | 1 | `note.write` |
| `outline` | 1 | 0 | 1 | `outline.read` |
| `page` | 1 | 0 | 1 | `page.layout` |
| `property` | 5 | 3 | 4 | `property.list`, `property.set`, `property.delete` |
| `rule` | 1 | 1 | 0 | `rule.insert` |
| `section` | 5 | 4 | 5 | `section.read`, `section.replace`, `section.insert` |
| `select` | 5 | 2 | 0 | `select.get`, `select.set`, `select.replace` |
| `table` | 13 | 11 | 11 | `table.read`, `table.add`, `table.setCell` |
| `tag` | 1 | 1 | 0 | `tag.insert` |
| `task` | 2 | 1 | 2 | `task.list`, `task.toggle` |
| `template` | 1 | 0 | 1 | `template.expand` |
| `text` | 6 | 4 | 6 | `text.read`, `text.find`, `text.insert` |
| `timestamp` | 1 | 1 | 0 | `timestamp.insert` |
| `view` | 10 | 0 | 0 | `view.width`, `view.hoverOut`, `view.blockAt` |
| `wikilink` | 1 | 1 | 0 | `wikilink.insert` |

The 71 commands in `kit.json` `agent.exclude` are editor commands the agent tools do not
offer.

### How things are addressed

- **Lines count from 1.** `task.toggle {line}`, `heading.setLevel {line}`, and the outline in
  `note_inspect` all use the line as the file shows it. Lines move when you insert text above them:
  read them again (`task.list`, `text.find`) after an edit instead of adding up.
- **Text positions count from 0**, in Unicode scalar values by default (`unit: "scalar"`). Get them
  from `text.find {find}`, which answers `from`, `to` and `line` for each match; do not count
  characters yourself.
- **Headings are paths** of their plain text from the top: `["Launch plan", "Tasks"]` is the
  `## Tasks` under `# Launch plan`. Use the path, or `line`, in `section.*`, `block.append`,
  `task.list`, `table.add`.
- **Tables count from 0** in the note (`table: 0` is the first). Inside a table, `row` and `column`
  count from 0 and **row 0 is the header**, so the first body row is `row: 1`. `table.read` answers
  the header and the body rows separately; `table.insertRow {at}` uses the same numbering.
- **Properties** are front-matter keys: `property.set {key, value}`, where a list is an array of
  strings.

## Text writing format

Besides single commands, the engine takes whole text in one step:

- `note.write` `{ markdown: string }`: Replaces the whole note with `markdown` as one undo step. Use it to write a new note; prefer section, block and text commands to change part of one.

## Defaults

What a new, empty document starts with:

- File name `Untitled note.md`, size 612 × 792 (`kit.json` `creates`).

What `note_inspect` reports for a new document made at that size (generated from the engine; lists shortened to 20 entries):

```json
{
  "callouts": 0,
  "canRedo": false,
  "canUndo": false,
  "characters": 0,
  "comments": {
    "open": 0,
    "resolved": 0
  },
  "heading": null,
  "headings": 0,
  "lines": 0,
  "links": {
    "blockIds": 0,
    "embeds": 0,
    "markdown": 0,
    "tags": 0,
    "urls": 0,
    "wikilinks": 0
  },
  "openThreads": 0,
  "outline": [],
  "pages": 1,
  "properties": [],
  "readOnly": false,
  "registers": {
    "bom": false,
    "eol": "lf"
  },
  "tables": [],
  "tasks": {
    "done": 0,
    "open": 0
  },
  "words": 0
}
```

## Themes, layouts and other named values

Parameters that take one of a fixed list of names (themes, layouts, presets, styles, kinds, modes,
units and similar), from the command schemas. A parameter not listed here takes free values; its
schema says which.

| Parameter | Values | Used by |
| --- | --- | --- |
| `align` | `left`, `center`, `right`, `none` | `table.align`, `table.insertColumn` |
| `kinds` | `wikilink`, `markdown`, `embed`, `image`, `url`, `tag`, `blockId` | `link.list` |
| `style` | `atx` | `heading.setLevel` |
| `style` | `wikilink`, `markdown` | `link.insert` |
| `unit` | `scalar`, `utf16` | `block.read`, `comment.add`, `comment.move`, `comment.threads` and 7 more |

## Examples

### Example 1: add a task and check one off

The note has `## Tasks` with `- [x] Draft the announcement` and `- [ ] Book the review meeting`.

```json
{"tool": "note_run", "arguments": {"path": "plan.md", "steps": [
  {"command": "text.find", "params": {"find": "- [ ] Book the review meeting"}}
]}}
```

It answers `{"count": 1, "matches": [{"from": 292, "line": 20, "to": 321, ...}]}`. Insert after the
item, then check off line 20:

```json
{"tool": "note_run", "arguments": {"path": "plan.md", "steps": [
  {"command": "text.insert", "params": {"at": 321, "text": "\n- [ ] Send the invite to the ten teams"}},
  {"command": "task.toggle", "params": {"line": 20, "done": true}}
]}}
{"tool": "note_check", "arguments": {"path": "plan.md"}}
```

The list now reads `- [x] Draft the announcement`, `- [x] Book the review meeting`,
`- [ ] Send the invite to the ten teams`, and the check finds nothing wrong.

### Example 2: change a table and a property

```json
{"tool": "note_run", "arguments": {"path": "plan.md", "steps": [
  {"command": "table.read", "params": {"table": 0}}
]}}
```

It answers `{"header": ["Week", "Milestone"], "rows": [["1", "Beta build"], ["2", "Feedback round"]], ...}`.
The body rows are table rows 1 and 2 (row 0 is the header):

```json
{"tool": "note_run", "arguments": {"path": "plan.md", "steps": [
  {"command": "table.insertRow", "params": {"table": 0, "at": 3, "cells": ["3", "General availability"]}},
  {"command": "table.setCell", "params": {"table": 0, "row": 2, "column": 1, "text": "Feedback round (two weeks)", "expected": "Feedback round"}},
  {"command": "property.set", "params": {"key": "status", "value": "review"}}
]}}
{"tool": "note_check", "arguments": {"path": "plan.md"}}
```

The table gains `| 3 | General availability |`, the second row reads `Feedback round (two weeks)`,
the front matter says `status: review`, and the check finds nothing wrong.

## Workflow

1. `note_inspect` to see the structure.
2. `note_commands` with a few keywords to find the command and its parameters. Use only command ids
   it returned; an unknown id fails with the nearest commands listed.
3. `note_run` with a few steps. Pass `expected` to `table.setCell`, `text.replace` or `text.delete`
   when you know the current text, so a stale position fails instead of changing the wrong text.
4. `note_check`, then fix what it reports. A diagnostic with `fix` names the command and params that
   repair it.

## Pitfalls

- `section.insert {position: "end"}` adds a new block, separated by a blank line: a list item added
  that way starts a new list. To extend a list, `text.find` its last item and `text.insert` at its
  `to` with `"\n- item"`.
- `text.insert` at 0 puts text above the front matter, which then stops being front matter. Insert
  after the properties block (find the first heading or paragraph).
- Editor commands that act on a selection (`format.bold`, `list.task`, `edit.indent`, …) are not
  offered to agents; use the text, section, table, and property commands.
- A failed step names its index and the reason, for example `out-of-range: the note has 34 lines`.
  Read the note again before you retry; earlier steps in the same call were not run after it.
