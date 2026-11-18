# Round-trip golden suite

The note's promise is that a file comes back exactly as it was read, plus the edits made to it
(`docs/design.md` 2.2). `cargo xtask golden` checks it on real files and on synthetic edges.

## Corpus

`tools/xtask/corpus.json` lists public Markdown files, each pinned by commit and SHA-256:

| Group | Source |
|---|---|
| Obsidian Help vault | `obsidianmd/obsidian-help` at `cd6c9f3c`, English pages and two Chinese ones |
| GitHub READMEs | react, vscode, rust, ohmyzsh, vue core, ollama, uv, excalidraw, logseq, joplin, tensorflow, gitignore |

`cargo xtask corpus` downloads the files by commit and checks each SHA-256. Files land in
`corpus/`, which is not tracked: corpus files are never committed. `cargo xtask golden` reads them
from there.

## What the suite checks

- **Open and save without an edit**: the saved bytes equal the file's, for every file.
- **One-place edits** through the engine's commands, each in a fresh copy: `text.insert` appends
  ` probe` to a paragraph line in the second half of the file and to the first heading, and adds
  a list item after the first bullet; `table.setCell` appends ` probe` to the first body cell of
  the first table with a body row (with `expected`); `property.set` appends ` probe` to the first
  string property (which must read back as the new value) and adds an item `probe` to the first
  list property. Each must change exactly one line of the saved file (an added list item or
  property item: add exactly one line). A file without a matching place (no paragraph line in the
  second half, no bullet, no table) is skipped for that edit.
- **Exports**: each file exports as a PDF and a standalone HTML file through the engine, both
  well formed, and the note stays unchanged; the suite prints what the pages could not draw
  (`docs/design.md` 6.1).
- **Synthetic edges** (`kition_note_kdm::file::bench::EDGE_FILES`): empty, LF, CRLF, CRLF with a
  byte order mark, a byte order mark only, two byte order marks, no final newline, an embedded NUL,
  mixed endings, a lone CR, only lone CRs, trailing white space. Each saves back byte for byte and
  takes a one-character edit in its first line only. A file that is not valid UTF-8 opens
  read-only and saves its bytes unchanged.

The suite prints a table of each check with the files that passed, and fails when any check is not
clean. Flow-list property edits are covered by unit tests, since the corpus's list properties are
block lists.
