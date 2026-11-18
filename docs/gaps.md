# Gaps

> **Last updated:** 2026-11-16 · **Source:** `features.yaml` (updated 2026-11-16) and the engine's command list

Every known shortfall of Kition Note, from the feature catalogue: features that are not yet `has`,
then the differences recorded on features that work. Update the feature in `features.yaml` when you
land work, then this list. The comparison with the target application is in
[parity/target-app-parity.md](parity/target-app-parity.md); the order of work is in [roadmap.md](roadmap.md).

## Features not done

- `note.code-diagrams` (P1, partial): Code blocks and diagrams. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `note.comments` (P1, partial): Comment threads: add, reply, resolve, move, delete. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `note.editing` (P1, partial): Editor commands: selection, cut, indent, move lines, delete paragraph, sort, trim, undo and redo. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `note.export` (P1, partial): Export to HTML and PDF, page layout, line endings. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `note.images` (P1, partial): Pictures the note refers to: find missing ones, set their files. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `note.view` (P2, partial): Reading width, folding and block lookup. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.

## Differences on working features

- `note.collab.live`: A typing run is one undo step only until another person's change to the text comes in (the replica's undo group ends at an import that touches the same text); the run then undoes in two steps. Peers' carets are placed in this person's own layout (their active line's Markdown syntax is not shown here).
- `note.format`: The other formats (highlight and its colour, clear, link, paragraph, the other heading levels, comment, lists) are covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.
- `note.insert`: insert.image, insert.tag, insert.date, insert.timestamp and template.expand are covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.
- `note.links`: insert.wikilink and insert.embed are covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.
- `note.properties`: properties.view (the editor's properties panel) is covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.
- `note.structure`: section.delete and block.setId are covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.
- `note.tables`: table.deleteRow, table.deleteColumn, table.moveColumn and table.geometry are covered by the command and editor tests, not by the round trip in bindings/wasm/src/round_trip_tests.rs.

## Catalogue coverage

The catalogue has 20 features, and 119 of the engine's 119 commands are claimed
by one of them.
