# Changelog

Changes to Kition Note, newest first. A version is the kit package version in `kit.json`.

## 0.1.5

- The same kit as 0.1.4, built on kition-sdk 6672663 (the replica's undo passes over an undo group whose changes came to nothing): the 0.1.4 package was built on kition-sdk 6b4d471.

## 0.1.4

- A host asks a note file for its document id (`document.identity`) and copies a note as a new
  document (`document.fork`): a `.md` file is the note's text and embeds no replica, so it carries
  no id and its fork is the same bytes (Kit Process Protocol section 5.6, kition-sdk 6b4d471).
- The headless program speaks Kit Process Protocol version 1 (`headless.protocol` is
  `kition-kit/1`), as hosts on kition-sdk from bb1b07b require.
- Undo after a step whose changes came to nothing (Enter, then Backspace) takes back the typing
  before it; before, that Undo changed nothing and the typing stayed.
- kition-sdk 6672663: the replica's undo passes over a group whose changes came to nothing, as
  it did before 4b81569, so the editor again skips such a step on its own (empty when the text is
  the same at its end) and the Undo after it takes back the typing before.

## 0.1.3

- Math after a coloured highlight keeps the text's colour; before, the formula drawn away from
  the caret could take the highlight's colour, in the light and the dark palette.
- In a dark host the first frame is already dark: the editor no longer paints light frames
  before it follows the host's color scheme.
- `kit.json` declares the engine's editor state as `engine.editorState`, the same list as the
  engine's `EDITOR_STATE`, so a host that does not run the engine reads `inspect()` without it.
- The README connects an agent with `npm install -g @kition/cli` and `kition agent add`, uses
  the standard kit link row, and links KHP at `kitharness/kitharness`.

## 0.1.2

- The same kit as 0.1.1, released again so that its release carries prebuilt headless
  executables for macOS, Linux and Windows, which `kition kit install` downloads.

## 0.1.1

- The editor follows the host's color scheme, live: in a dark host the note is painted in a dark
  palette after Obsidian's default dark theme, with the same lengths as the light one, so nothing
  moves on a switch. The overlays, the canvas backdrop and Mermaid diagrams follow it. Export,
  print and the agent's preview stay light (`note.platform.color-scheme`).

## 0.1.0

The first public release, under the Apache License 2.0, published as the `package-v0.1.0`
Release of this repository. A published tag and its assets are never deleted, moved or reused; a
change is published under a new version. It contains:

- The Rust engine (`kition-note-kit`): Markdown notes kept byte for byte, minimal-span edits for
  text, properties, sections, blocks, tasks, tables and links, find and replace, `note_check`
  with fix hints, comment threads beside the file, live co-editing on a KDM replica, and export
  to HTML, PDF and pictures.
- The headless kit (`kition-note`) for agents over the Kit Protocol, with the object model of
  the note's sections, tasks, tables, properties and links, and writes with a `basis` that are
  refused when someone else changed what they rest on.
- An edit through the object model (`model/batch`: extensions, workspace scripts) is a change of
  the note as a command's is: the revision moves, so the editor draws it, the note has unsaved
  changes, and the selection follows the text (`KitDocument::batch_applied`).
- The kit's editor (`web/`): the engine's canvas in kit-editor's frame, with the live preview,
  tables, comments, find, outline, drafts, history and exports.
- A clean note whose file another program changed is read again into the same session: the
  file's difference is taken in as an outside change, so the person's and the agent's undo steps
  stay and undo leaves the other program's change; the selection, folds and comment threads stay
  too.
- The editor state (`editorState`): the selection and the folds, moved along the document's
  changes when put back.
- NOTICE: Loro (MIT), bundled by the editor and linked by the engine.
- Quality: a missing private tool (kition-server) is a skip, not a failure.
- Quality vocabulary: cases run in the layers `unit`, `gate`, `corpus` and `eval`
  (`cargo xtask eval`, run by kition-sdk's `tools/eval-cases.mjs`); cases name their `dimension`
  and their `source` names its `license`; only a maintainer approves expected-result changes.
  `cargo xtask golden` stays the round-trip suite against stored files.
- Toolchain: Rust 1.95, Node 22.23.4, pnpm 10.34.9. Linking sibling checkouts in `web/` is
  opt-in (KITION_LINK=1).
- `scripts/fetch-fonts.sh <folder>`: downloads the fonts the build needs from their upstream
  sources and checks each file's SHA-256, so a source build needs no private repository.
