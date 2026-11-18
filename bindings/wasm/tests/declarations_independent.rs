//! Every write command's premise declaration compared with an account of its code: for each
//! command the kit offers that changes the note (`mutates`), what its handler
//! (`bindings/wasm/src/run.rs`, `editor.rs`, `lib.rs`) and the engine edit behind it
//! (`crates/engine/*`) resolve, read, check and write, written down from that code alone, apart
//! from the declarations (`premise.rs`, the premise callbacks of `lib.rs`, the `reads`/`names` a
//! plan fills), one row per code path ([`READINGS`]), on the note [`FIXTURE`]. The test checks
//! that the declarations hold what the rows say each command reads and writes.
//!
//! A row: `ROW <command> | sel=<the editor's selection> | params=<JSON> | resolves=<how each
//! parameter names its place> | reads=<regions> | checks=<what decides whether it writes at all> |
//! assigns=<what it writes> | state=<editor or session state> | note=<the code path>`. Regions:
//! `L19` a whole line (from 1), `L19[2..5]` columns of it (from 0, Unicode scalars), `L17-L25`
//! lines, `gap@L22` the start of a line, `gap@L21$` the end of one, `end` the end of the note,
//! `fm` the front matter (L1-L5), `all` the whole body, `eol` and `bom` the root's registers,
//! `thread:A{fields}` and `comment:A1{fields}` the comment objects (thread A on "beta" of L9,
//! thread B, resolved, on "Beta build" of L30), `files` the workspace's files, `clock` the time.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use kition_note_kit::commands::SPECS;

/// The note every row is read on.
pub const FIXTURE: &str = r#"---
title: Launch plan
status: draft
tags: [beta, launch]
---

# Launch plan

Intro text with a [[timeline]] link and beta. ^intro

## Goals

- Ship the beta
- Collect feedback
  - From ten teams

## Tasks

- [ ] Draft the announcement ^draft
- [x] Book the review meeting ^book
- [ ] Draft the invite ^draft

1. First step
2. Second step

## Timeline

| Week | Milestone |
| --- | :-: |
| 1 | Beta build |
| 2 | Feedback round |

See [the site](https://example.com/site) and ![chart](chart.png).

### Details

```rust
fn main() {}
```

> [!note] Remember
> Keep it short.

## Budget

| Item | Cost |
| --- | --- |
| Venue | 100 |
"#;

/// The rows, one per code path (see the module docs), in four parts (`reader` A to D).
const READINGS: &str = r########"
# ======== reader A
# Part A. Fixture: FIXTURE (631 scalars, LF, ASCII so bytes = scalars).
# Id mapping checked in bindings/wasm/src/projection.rs: project() gives sections in ix.headings order (L7,L11,L17,L26,L35,L44 -> section:1..6),
# tasks in ix.tasks order (L19,L20,L21 -> task:1..3). Only three id params exist (ID_PARAMS): section->line, toSection->toLine, task->line;
# resolve() replaces the id with Item.line of the snapshot now (heading line / task marker line); id plus heading/line (or line/blockId) -> invalid-params; unknown id -> not-found.
# Tables have ids table:1/2 in the projection but no command here takes a table id; `table` params elsewhere are 0-based numbers.
# Common to every row (lib.rs execute_as 670-733): checks=command known; params an object; schema refuse_unknown (unknown key -> invalid-params); read-only (file not UTF-8) -> refused.
# Common state: origin (undo scope of session.execute; ends the person's typing run via stop_typing); selection is NOT read by any of these commands (not commands::is_editor);
# after the write editor.follow maps caret/selection/folds (written, not read). Body = whole file incl. front matter (kdm-model NoteModel::body).
# Common defect (all body writes): no command here assigns thread:A{anchor}; the stored scalar range 118..122 stays put and relocate() (run.rs 453) picks the occurrence of the quote "beta"
#  nearest the stale start. Concrete: text.insert at=63 of 80 chars -> "beta" of L9 is now at 198 (distance 80) while L4[8..12] "beta" is at 44 (distance 74): comment.threads and the tint now anchor thread:A on the front-matter tag.
# ---- bindings/wasm/src/run.rs text_command 1001 / text_edit 638; positions.rs
ROW text.insert | sel=none | params={"at":118,"text":"the "} | resolves=at:scalar offset (unit default scalar; Unit::to_scalar, > 631 -> out-of-range) | reads=none (no index; only text length) | checks=at <= len(all) (out-of-range); text present and a string | assigns=gap@L9[40] (inserted text, CR/CRLF normalized to the note eol unless mixed) | state=origin (undo scope) | note=run.rs 647-649 + 1001-1008; answer length computed from unit; no check of what is at the gap; thread:A not moved (see common defect: stale anchor now quotes nothing new but relocate finds beta at 122 nearest, ok here; larger inserts above L9 jump it to L4)
ROW text.insert | sel=none | params={"at":118,"text":"the ","expected":""} | resolves=at:scalar offset | reads=gap@L9[40] (empty range compared) | checks=at <= len; expected must equal "" (any other text -> stale) | assigns=gap@L9[40] | state=origin | note=run.rs 660-665: for insert from==to so the only passing expected is ""; it proves nothing about the neighbourhood
ROW text.insert | sel=none | params={"at":118,"text":"the ","unit":"utf16"} | resolves=at:UTF-16 code units -> scalar (Unit::Utf16.to_scalar walks all chars; inside a surrogate pair or past end -> out-of-range) | reads=all[0..at] (chars before at, to convert units) | checks=at on a scalar boundary and <= utf16 length | assigns=gap@L9[40] | state=origin | note=positions.rs 35-51; answer length in utf16
ROW text.delete | sel=none | params={"from":116,"to":122} | resolves=from,to:scalar offsets | reads=none | checks=from,to <= len (out-of-range); from <= to (invalid-params) | assigns=L9[38..44] (deleted) | state=origin | note=run.rs 651-654; without expected nothing about the content is checked, a stale position deletes whatever is there; deleting the quoted "beta" orphans/relocates thread:A to L4 or L13 beta (nearest to 118: L13[11..15] at 153, distance 35 vs L4 distance 74 -> L13)
ROW text.delete | sel=none | params={"from":116,"to":122,"expected":"d beta"} | resolves=from,to:scalar offsets | reads=L9[38..44] (compared to expected) | checks=range bounds; from<=to; L9[38..44] == expected else stale | assigns=L9[38..44] (deleted) | state=origin | note=run.rs 660-665
ROW text.delete | sel=none | params={"from":116,"to":122,"unit":"utf16"} | resolves=from,to:utf16 units -> scalars | reads=all[0..122] (unit conversion) | checks=bounds/boundaries in utf16; from<=to | assigns=L9[38..44] | state=origin | note=positions.rs to_scalar
ROW text.replace | sel=none | params={"from":118,"to":122,"text":"pilot"} | resolves=from,to:scalar offsets | reads=none | checks=bounds; from<=to; text required | assigns=L9[40..44] (replaced) | state=origin | note=run.rs 655-658; thread:A quote "beta" gone at its range -> relocate picks L13 "beta" (distance 35); no expected -> stale positions overwrite whatever is there
ROW text.replace | sel=none | params={"from":118,"to":122,"text":"pilot","expected":"beta"} | resolves=from,to:scalar offsets | reads=L9[40..44] (compared) | checks=bounds; from<=to; L9[40..44]=="beta" else stale | assigns=L9[40..44] | state=origin | note=run.rs 660-665
ROW text.replace | sel=none | params={"from":118,"to":122,"text":"pilot","unit":"utf16","expected":"beta"} | resolves=from,to:utf16 -> scalar | reads=all[0..122] (conversion); L9[40..44] (expected) | checks=bounds/boundaries; from<=to; expected | assigns=L9[40..44] | state=origin | note=same path, utf16 unit
# ---- run.rs plan "text.findReplace" 688-711; crates/engine/edit/src/find.rs replace 45
ROW text.findReplace | sel=none | params={"find":"beta","replace":"pilot"} | resolves=occurrence:default Nth(1) = first match in the whole file (front matter, code and links included); regex default false (escaped); caseSensitive default true | reads=all (every match scanned in document order, front matter first) | checks=find non-empty and <= 4096 bytes; pattern compiles; >= 1 non-empty match else not-found | assigns=L4[7..11] (first "beta" is the front-matter tag, NOT L9) | state=origin | note=find.rs 45-77; an agent meaning the prose "beta" edits the tags property; answer replaced=1
ROW text.findReplace | sel=none | params={"find":"beta","replace":"pilot","occurrence":3} | resolves=occurrence:3rd non-empty match counted over all (L4, L9, L13 -> L13) | reads=all (matches 1..3 counted) | checks=find valid; occurrence integer >= 1 (0, negative, non-integer, other string -> invalid-params); match #3 exists else not-found ("does not occur 3 times") | assigns=L13[11..15] | state=origin | note=run.rs 692-698; find.rs 50-56
ROW text.findReplace | sel=none | params={"find":"beta","replace":"pilot","occurrence":"all"} | resolves=occurrence:"all" | reads=all | checks=find valid; at least one match | assigns=L4[7..11]; L9[40..44]; L13[11..15] (each match, capped at 10000) | state=origin | note=find.rs 65; replaces inside front matter too; thread:A quote "beta" is gone from the note -> thread orphaned (comment.threads gone=true), not resolved or moved
ROW text.findReplace | sel=none | params={"find":"beta","replace":"pilot","occurrence":"all","caseSensitive":false} | resolves=case-insensitive match | reads=all | checks=as above | assigns=L4[7..11]; L9[40..44]; L13[11..15]; L30[8..12] ("Beta" inside table 0 cell; thread:B quote "Beta build" now gone) | state=origin | note=find.rs 30
ROW text.findReplace | sel=none | params={"find":"^- \\[ \\] (.*)$","replace":"- [x] $1","regex":true,"occurrence":"all"} | resolves=regex, multi_line on (^ $ per line), $1 expanded by captures.expand | reads=all | checks=pattern compiles (size limit 1 MiB); non-empty matches only | assigns=L19; L21 (whole matched lines rewritten) | state=origin | note=find.rs 59-61; a regex may span lines (\n in the pattern) and match inside code blocks or front matter; no structural check
# ---- run.rs write "note.write" 918-921; kdm-model write_body (line diff refined to chars)
ROW note.write | sel=none | params={"markdown":"---\ntitle: Launch plan\n...whole new note..."} | resolves=none | reads=all (diffed against the new text); eol (register: CR/CRLF in markdown normalized to \n unless mixed) | checks=markdown present and a string (empty string allowed: deletes everything) | assigns=only the differing spans of all (diff_text) | state=origin | note=run.rs 918-921; no expected/basis check, a whole-note overwrite rests on nothing; thread anchors not moved
# ---- run.rs write "file.setLineEndings" 922-943
ROW file.setLineEndings | sel=none | params={"eol":"crlf"} | resolves=bom:default from the current register | reads=all (every \r scanned); eol; bom | checks=eol is "lf" or "crlf" (else invalid-params) | assigns=eol register (set only when it differs); text: every "\r\n" loses its \r, every lone \r becomes \n (none in the fixture) | state=origin | note=run.rs 922-943; changed true when edits or style differ
ROW file.setLineEndings | sel=none | params={"eol":"lf","bom":true} | resolves=bom:explicit | reads=all (\r scan); eol; bom | checks=eol valid; bom a boolean | assigns=bom register (false->true); eol unchanged (lf) | state=origin | note=same path, explicit bom; "mixed" cannot be requested
# ---- run.rs plan "section.replace" 712-717; edit/src/section.rs replace 67; lib.rs find_heading 202, content_end 233, insert_block 170, minimal 133
ROW section.replace | sel=none | params={"heading":["Goals"],"markdown":"- Ship v1"} | resolves=heading:path; exact full-path match over all headings first (["Goals"] has none), else unique heading whose path ends with it -> L11; two -> ambiguous (lines listed); none -> not-found; [] -> invalid | reads=L7,L11,L17,L26,L35,L44 (heading texts/paths for the match); L11 (level, range end); L17 (next heading of same or higher level: section end); L12-L16 (trailing blank lines trimmed by content_end; leading blank lines kept; old text compared for the minimal edit) | checks=heading found and unambiguous; markdown not blank after trimming newlines (else invalid "use section.delete") | assigns=L13-L15 (body replaced, minimal span; L12 blank lead and L16 separation kept) | state=origin | note=section.rs 67-86, withHeading default false; subsections are part of the body
ROW section.replace | sel=none | params={"line":11,"markdown":"- Ship v1"} | resolves=line:1-based -> the indexed (top-level, not in quote/list) heading starting on that line; non-heading line -> not-found | reads=L11 (heading on the line); L17 (section end); L12-L16 | checks=a heading starts on L11; markdown not blank | assigns=L13-L15 | state=origin | note=HeadingRef::Line path of find_heading
ROW section.replace | sel=none | params={"section":"section:4","markdown":"Moved to Q3."} | resolves=section:id -> line 26 via projection snapshot now (Item.line) -> HeadingRef::Line(26) | reads=projection ids; L26; L44 (next level<=2 heading: section end); L27-L43 (body incl. subsection Details, trailing blank trimmed) | checks=id well-formed and present now (not-found); not given with heading/line; heading on L26; markdown not blank | assigns=L28-L42 (table 0, link paragraph, ### Details, code, callout replaced by the text; L27 lead blank kept) | state=origin; projection ids (session) | note=a section's body includes its subsections: Details disappears
ROW section.replace | sel=none | params={"heading":["Launch plan","Goals"],"markdown":"## Aims\n\n- Ship v1","withHeading":true} | resolves=heading:full path match -> L11 | reads=headings (path match); L11; L17; L11-L16 | checks=heading found; markdown not blank | assigns=L11-L15 (heading and body, minimal span) | state=origin | note=section.rs 69: no check that markdown starts with a heading: withHeading with "- Ship v1" silently drops the heading and merges the list into section:1
ROW section.replace | sel=none | params={"line":26,"markdown":"x"} on a heading with an empty body (e.g. "## A\n## B") | resolves=line -> heading | reads=heading line; next heading; body (blank) | checks=heading; markdown | assigns=gap after heading line (insert_block with blank lines from blank_before/blank_at of the neighbours) | state=origin | note=section.rs 76-77 empty-body branch; not reachable on the fixture (every section has a body)
# ---- run.rs plan "section.insert" 718-723; section.rs insert 89, position_at 50, own_end 40
ROW section.insert | sel=none | params={"heading":["Goals"],"position":"before","markdown":"## Context"} | resolves=heading:path (suffix-unique) -> L11; position required | reads=headings (path match); L11 (heading start); L10 (blank_before: no blank added); L11 (blank_at: not blank, so a blank line follows) | checks=heading found/unambiguous; position one of before/after/start/end (missing -> invalid); markdown not blank | assigns=gap@L11 (new block "## Context\n\n") | state=origin | note=section.rs 52; inserts a heading that now owns nothing (Goals stays a sibling)
ROW section.insert | sel=none | params={"heading":["Goals"],"position":"after","markdown":"Note."} | resolves=heading -> L11 | reads=headings; L11 level; L17 (section end = next level<=2 heading); L16 (blank_before); L17 (blank_at) | checks=heading; position; markdown | assigns=gap@L17 ("Note.\n\n" before ## Tasks) | state=origin | note=section.rs 53 (heading.section.end)
ROW section.insert | sel=none | params={"heading":["Goals"],"position":"start","markdown":"Note."} | resolves=heading -> L11 | reads=headings; L11 (range end = start of L12; line before not blank -> leading blank line); L12 (blank_at) | checks=heading; position; markdown | assigns=gap@L12 ("\nNote.\n") | state=origin | note=section.rs 54
ROW section.insert | sel=none | params={"line":26,"position":"end","markdown":"Note."} | resolves=line:26 -> heading ## Timeline | reads=L26; L35 (first deeper heading inside the section = own_end); L44 (section end); L27-L34 (content_end trims trailing blank L34); L33 (blank_before); L34 (blank_at) | checks=heading on L26; position; markdown | assigns=gap@L34 ("\nNote.\n": after the link paragraph, before ### Details) | state=origin | note=section.rs 55 + own_end 40-47
ROW section.insert | sel=none | params={"section":"section:1","position":"end","markdown":"Note."} | resolves=section:id -> line 7 (projection) | reads=projection ids; L7; L11 (first deeper heading = own_end); L8-L10; L9 (blank_before); L10 (blank_at) | checks=id present; heading; position; markdown | assigns=gap@L10 ("\nNote.\n" after the intro paragraph) | state=origin; projection ids | note=id path
# ---- run.rs plan "section.delete" 724-729; section.rs delete 98
ROW section.delete | sel=none | params={"heading":["Timeline"]} | resolves=heading:path suffix-unique -> L26 | reads=headings (path match); L26 (level 2); L35 (level 3: inside); L44 (level 2: section end) | checks=heading found/unambiguous | assigns=L26-L43 (heading, table 0, link paragraph, subsection Details, trailing blank line: whole range deleted) | state=origin | note=section.rs 98-101; no check of what is inside (subsections, tables, threads); thread:B quote "Beta build" gone -> orphaned
ROW section.delete | sel=none | params={"line":44} | resolves=line -> ## Budget | reads=L44; headings after (none: section ends at end) | checks=heading on L44 | assigns=L44-L48 to end (deleted; L43 blank stays, note ends "\n\n") | state=origin | note=line path
ROW section.delete | sel=none | params={"section":"section:5"} | resolves=section:id -> line 35 | reads=projection ids; L35 (level 3); L44 (level 2 <= 3: section end) | checks=id present; heading | assigns=L35-L43 (deleted) | state=origin; projection ids | note=id path
# ---- run.rs plan "section.move" 730-738; section.rs move_to 104
ROW section.move | sel=none | params={"heading":["Budget"],"toHeading":["Goals"],"position":"before"} | resolves=heading:path -> L44; toHeading:path -> L11; position required | reads=headings (both path matches); L44 section (to end); L11 (target start); L10 (blank_before at place); L11 (blank_at at place); L44-L48 (moving text, trailing blanks trimmed) | checks=both headings found/unambiguous; position valid; target heading not inside the moving section (refused "cannot move into itself"); place == moving section start or end -> no edits (changed false) | assigns=L44-L48 (deleted whole); gap@L11 (moved text: Budget heading through its table, plus a blank line) | state=origin | note=section.rs 104-118; blank-line decisions use the original text around place, not the text after the cut; moving the last section leaves the note ending "\n\n"
ROW section.move | sel=none | params={"line":26,"toLine":11,"position":"after"} | resolves=line,toLine:1-based heading lines | reads=L26 section (L26-L43 incl. Details); L11 section end = gap@L17; L16, L17 (blank lines at place) | checks=headings on both lines; L11 not inside L26..L43; place (L17 start) not equal to the moving section's start/end | assigns=L26-L43 (deleted); gap@L17 (Timeline with Details inserted) | state=origin | note=line path
ROW section.move | sel=none | params={"section":"section:5","toSection":"section:3","position":"start"} | resolves=section:id -> line 35; toSection:id -> toLine 17 (projection) | reads=projection ids; L35 section (L35-L43); L17 (range end = gap@L18); L17 (blank_before: heading, adds blank); L18 (blank_at) | checks=ids present; not both id and heading/line; headings; target outside moving section; no-op check | assigns=L35-L43 (deleted); gap@L18 ("\n### Details ... > Keep it short.\n") | state=origin; projection ids | note=id path; Details (level 3) now sits under Tasks, the level is not adjusted
ROW section.move | sel=none | params={"heading":["Timeline"],"toHeading":["Details"],"position":"end"} | resolves=heading -> L26; toHeading suffix -> L35 | reads=L26 section L26-L43; L35 | checks=target L35 inside L26..L44 -> refused | assigns=none | state=origin | note=refusal path section.rs 108-110
ROW section.move | sel=none | params={"heading":["Tasks"],"toHeading":["Goals"],"position":"after"} | resolves=heading -> L17; toHeading -> L11 | reads=L17 section start; L11 section end (= gap@L17) | checks=place == moving section start -> no edits | assigns=none (changed false) | state=origin | note=no-op path section.rs 111-113
# ---- run.rs plan "heading.setLevel" 739-750; section.rs set_level 122
ROW heading.setLevel | sel=none | params={"heading":["Goals"],"level":3} | resolves=heading:path suffix-unique -> L11; level:u8 (>255 -> invalid) | reads=headings (path match); L11[0..2] (the # run, marker) | checks=level integer; style absent or "atx" (else invalid); heading found/unambiguous; 1<=level<=6 (engine, invalid); ATX heading | assigns=L11[0..2] (# run; minimal: one "#" inserted at L11[2]) | state=origin | note=section.rs 127-129; subsections keep their levels, so section extents change (Goals at 3 now ends at ## Tasks still; at level 1 it would swallow Tasks..Budget); style "atx" is ignored for an ATX heading
ROW heading.setLevel | sel=none | params={"line":35,"level":2} | resolves=line -> ### Details | reads=L35[0..3] | checks=level; heading on L35 | assigns=L35[0..3] (minimal: one # deleted) | state=origin | note=line path
ROW heading.setLevel | sel=none | params={"section":"section:2","level":1} | resolves=section:id -> line 11 | reads=projection ids; L11[0..2] | checks=id present; level; heading | assigns=L11[0..2] | state=origin; projection ids | note=id path
ROW heading.setLevel | sel=none | params={"line":N,"level":1} on a Setext heading ("Title\n-----") | resolves=line -> heading whose first (text) line is N | reads=underline line (marker, its width in chars) | checks=level 1..2 without atx | assigns=underline line (all "=" of the same width) | state=origin | note=section.rs 130-133; not on the fixture
ROW heading.setLevel | sel=none | params={"line":N,"level":3} on a Setext heading, no style | resolves=line -> heading | reads=heading (setext flag) | checks=setext and level>2 without atx -> refused | assigns=none | state=origin | note=section.rs 134-139
ROW heading.setLevel | sel=none | params={"line":N,"level":3,"style":"atx"} on a Setext heading | resolves=line -> heading | reads=heading lines (first line text, underline) | checks=level; heading | assigns=heading lines N..underline (rewritten "### <first line>") | state=origin | note=section.rs 141-144 DEFECT: only the first text line is kept; a Setext heading whose text spans lines ("Title part\ncontinued\n---") becomes "### Title part" and "continued" is lost
# ---- run.rs plan "block.replace" 751-756; edit/src/block.rs find 10, replace 38; markdown block_ranges (lib.rs 585)
ROW block.replace | sel=none | params={"blockId":"intro","markdown":"Intro with the [[timeline]] link."} | resolves=blockId:^id over all indexed block ids (not in front matter/code); exactly one else ambiguous; none -> not-found | reads=all block ids (^intro, ^draft x2, ^book: uniqueness); L9 (^intro token inline, holder = innermost block containing it = paragraph L9; block = L9 through the end of the id line) | checks=one block carries ^intro; markdown not empty after trimming newlines | assigns=L9 (minimal span; " ^intro" re-appended because markdown has no trailing id) | state=origin | note=block.rs 38-52; thread:A quote "beta" removed -> relocates to L4/L13 beta
ROW block.replace | sel=none | params={"blockId":"book","markdown":"Booked"} | resolves=blockId -> ^book L20 (unique) | reads=block ids; L20 (holder = list item L20, which starts at column 0) | checks=unique; markdown non-empty | assigns=L20[0..35] (whole item incl. "- [x] " marker rewritten to "Booked ^book") | state=origin | note=DEFECT-ish: the block of an inline id in a tight list item is the item (bullet and checkbox included), so the task stops being a list item unless markdown repeats "- [x] "; in a loose list the innermost holder is the paragraph instead, so the same call keeps the bullet -> behaviour depends on list tightness
ROW block.replace | sel=none | params={"blockId":"draft","markdown":"x"} | resolves=blockId -> two blocks (L19, L21) | reads=block ids | checks=ambiguous (no occurrence parameter on block.replace) | assigns=none | state=origin | note=block.rs 15; there is no way to address either ^draft block with block.replace
ROW block.replace | sel=none | params={"blockId":"intro","markdown":"New intro ^book"} | resolves=blockId -> L9 | reads=block ids; L9 | checks=unique; non-empty; ends_with_id -> markdown used as is | assigns=L9 (^intro replaced by ^book) | state=origin | note=block.rs 44-45 DEFECT: the id markdown sets is not checked for uniqueness (block.setId refuses duplicates): now two ^book blocks, and task.toggle/block.replace by "book" become ambiguous; links to ^intro dangle
# ---- run.rs plan "block.append" 757-764; block.rs append 55
ROW block.append | sel=none | params={"markdown":"Done."} | resolves=no heading -> end of the note | reads=end; L48 (blank_before: not blank -> leading blank line); end (blank_at) | checks=markdown not blank; heading/line not both | assigns=end ("\nDone.\n") | state=origin | note=block.rs 64
ROW block.append | sel=none | params={"heading":["Timeline"],"markdown":"Done."} | resolves=heading:path -> L26 | reads=headings; L26 level; L44 (section end); L27-L43 (content_end of the whole section incl. Details, trailing blank trimmed); L42 (blank_before); L43 (blank_at) | checks=heading found/unambiguous; markdown not blank | assigns=gap@L43 ("\nDone.\n" after the callout) | state=origin | note=block.rs 60-63 DEFECT: place = end of the section including its subsections, so a block appended to Timeline lands under ### Details (it now belongs to Details' section), unlike section.insert end
ROW block.append | sel=none | params={"line":11,"markdown":"- Ask legal"} | resolves=line -> ## Goals | reads=L11; L17 (section end); L12-L16 (content_end); L15 (blank_before not blank); L16 (blank_at blank) | checks=heading on L11; markdown | assigns=gap@L16 ("\n- Ask legal\n": a separate list after a blank line, not a 3rd item) | state=origin | note=line path
ROW block.append | sel=none | params={"section":"section:6","markdown":"Total 100"} | resolves=section:id -> line 44 | reads=projection ids; L44; end; L48 (blank_before) | checks=id present; heading; markdown | assigns=end ("\nTotal 100\n") | state=origin; projection ids | note=id path
# ---- run.rs plan "block.setId" 765-780; block.rs set_id_on_line 70, set_id 90
ROW block.setId | sel=none | params={"line":13,"id":"ship"} | resolves=line:1-based line (any line, not a block) | reads=all block ids (uniqueness of "ship"); L13 (blank check, existing token check, trimmed end) | checks=id non-empty ASCII letters/digits/hyphens; no block anywhere already has ^ship (refused); line exists (out-of-range); line not blank (invalid) | assigns=gap@L13$ (" ^ship" after the trimmed line) | state=origin | note=block.rs 70-87
ROW block.setId | sel=none | params={"line":9,"id":"lead"} | resolves=line 9 | reads=block ids; L9 (^intro token found inside the line) | checks=id valid; ^lead unused; line exists, not blank | assigns=L9[46..52] (^intro -> ^lead, minimal) | state=origin | note=block.rs 83-85; DEFECT: {"line":9,"id":"intro"} is refused "another block already ends with ^intro" (the line's own id counts), while the blockId path allows id == old
ROW block.setId | sel=none | params={"line":2,"id":"t"} | resolves=line 2 (front matter) | reads=block ids; L2 | checks=id valid; unused; line exists; not blank | assigns=gap@L2$ (" ^t") | state=origin | note=DEFECT: no check that the line is a Markdown block: in front matter it changes the title property to "Launch plan ^t"; on L38 (inside the code fence) it writes " ^t" into the code; on L30 it appends to a table row
ROW block.setId | sel=none | params={"blockId":"book","id":"booking"} | resolves=blockId:^book, occurrence absent -> must be unique | reads=block ids (find + uniqueness of "booking"); L20[30..35] (token) | checks=id valid; ^book exactly one (ambiguous/not-found); ^booking not used by another block (unless equal to old) | assigns=L20[30..35] (token rewritten, minimal) | state=origin | note=block.rs 90-97
ROW block.setId | sel=none | params={"blockId":"draft","occurrence":2,"id":"invite"} | resolves=blockId:^draft, occurrence:2 = 2nd in document order -> L21 | reads=block ids; L21[23..29] | checks=id valid; occurrence n>=1 and <= count (0 or 3 -> out-of-range); ^invite unused | assigns=L21[23..29] (^draft -> ^invite) | state=origin | note=block.rs 16-20
ROW block.setId | sel=none | params={"blockId":"draft","id":"x"} | resolves=blockId without occurrence | reads=block ids | checks=two ^draft -> ambiguous | assigns=none | state=origin | note=error path; giving both line and blockId (or neither) -> invalid-params "give line or blockId"; occurrence given with line is silently ignored
# ---- run.rs plan "task.toggle" 781-797; block.rs task_on_line 100, toggle 112
ROW task.toggle | sel=none | params={"line":19} | resolves=line:1-based -> the task whose [ ] marker starts on that line (whole line incl. ending) | reads=L19 (line range); L19[2..5] (marker, done flag) | checks=line exists (out-of-range); a task marker on the line (not-found) | assigns=L19[3..4] (" " -> "x") | state=origin | note=toggle path: done = !task.done; answer done=true
ROW task.toggle | sel=none | params={"line":20,"done":true} | resolves=line -> task L20 | reads=L20; L20[2..5] (already [x]) | checks=line exists; task on line; done == current -> no edits | assigns=none (changed false) | state=origin | note=explicit done path, block.rs 114-116
ROW task.toggle | sel=none | params={"line":20,"done":false} | resolves=line -> task L20 | reads=L20; L20[2..5] | checks=line; task | assigns=L20[3..4] ("x" -> " "; "[X]" also counts done) | state=origin | note=explicit done differing
ROW task.toggle | sel=none | params={"task":"task:3"} | resolves=task:id -> line 21 (projection Item.line of the marker line) | reads=projection ids; L21; L21[2..5] | checks=id well-formed/present; not with line/blockId; task on L21 | assigns=L21[3..4] | state=origin; projection ids | note=id path
ROW task.toggle | sel=none | params={"blockId":"book"} | resolves=blockId:^book unique -> line of the ^id TOKEN (not of the block start) -> L20 | reads=block ids (uniqueness); L20[30..35] (token position); L20; L20[2..5] | checks=^book exactly one; a task marker on the token's line | assigns=L20[3..4] | state=origin | note=run.rs 784-790 DEFECT: the line taken is the id token's line: a task whose id stands on a continuation line ("- [ ] Draft\n  more ^x") or on its own line after the list resolves to a line with no task -> not-found, although the id names that task's block
ROW task.toggle | sel=none | params={"blockId":"draft"} | resolves=blockId -> two blocks | reads=block ids | checks=ambiguous (no occurrence parameter) | assigns=none | state=origin | note=neither ^draft task can be toggled by id
ROW task.toggle | sel=none | params={"blockId":"intro"} | resolves=blockId -> ^intro token on L9 | reads=block ids; L9 | checks=no task on L9 -> not-found | assigns=none | state=origin | note=error path; line+blockId together or neither -> invalid-params
# ======== reader B
# Part B. Fixture: FIXTURE (LF, ends with a line break: 49 lines, L49 empty; body = whole file incl. front matter, 631 scalars).
# Scalar line starts used below: L4=37 L9=78 L13=142 L14=158 L19=207 L26=340 L28=353 L30=388 L33=431 L46=585 L48=615 end=631.
# Projection check (projection.rs project/Tracked::open): sections section:1..6 = L7,L11,L17,L26,L35,L44; tasks task:1..3 = L19,L20,L21; tables table:1 = L28-L31, table:2 = L46-L48 (1-based ids) while every table.* command takes a 0-based integer `table` index into ix.tables (table 0 = L28-L31, 1 = L46-L48); properties property:1..3 = title,status,tags; links link:1..3 = [[timeline]], [the site], ![chart]. projection::resolve only rewrites `section`, `toSection`, `task` (ID_PARAMS), and none of the commands below declares those keys in its schema, so no projection id (section:N, table:N, property:N, link:N) is accepted by any of them (refuse_unknown, or number() refuses "table:1").
# Common to every row (lib.rs execute_as 670): command known; params an object; keys outside the command schema refused (invalid-params); refused when the file is read-only (invalid UTF-8); stop_typing() ends the person's open typing run (editor state) before the write; the write runs in one KDM transaction with the caller's origin (an undo step in the origin's scope). Body writes go through run.rs write 913 -> plan 671 -> apply 580 (normalize: sort, refuse overlap; inserted CR/CRLF become LF unless the eol register is mixed).
# ---- property.* (run.rs plan 798; crates/engine/edit/src/props.rs; crates/engine/markdown/src/frontmatter.rs scan/classify)
ROW property.set | sel=none | params={"key":"status","value":"ready"} | resolves=key:front-matter key, first entry whose unquoted key equals it (fm.entries.find) | reads=fm(L1 and closing fence L5 located: block exists); L2-L4(whole block parsed by saphyr: YAML validity, duplicate keys); L3(entry lines, value span L3[8..13], style Plain); eol | checks=key non-empty single line; value is string/number/bool/null/list of strings; key not duplicated in fm(L2-L4); fm(L2-L4) valid YAML (an error under any other key refuses too); entry style not Complex (L3) | assigns=L3[8..13] (value "draft"->"ready", minimal span; plain kept, double-quoted when the new string is not plain-safe e.g. "yes", "a: b", "1") | state=origin | note=props.rs set 165, branch (style,value) scalar 219-239 with non-empty value; Value::Null writes the word null in place of the value; Number written as JSON text
ROW property.set | sel=none | params={"key":"tags","value":["beta","launch","ga"]} | resolves=key:first entry named tags (L4) | reads=fm; L2-L4(YAML validity, duplicates); L4(value span L4[6..20] "[beta, launch]": separator ", " or ",", first item's quote style, inner padding) | checks=as above; style FlowList (L4) | assigns=L4[19..19] (insert ", ga" into the flow list; minimal span) | state=origin | note=props.rs set 204-213 FlowList+List; items rewritten in the first item's style; an empty list writes []
ROW property.set | sel=none | params={"key":"tags","value":"beta"} | resolves=key:first entry named tags (L4) | reads=fm; L2-L4(YAML validity, duplicates); L4(entry lines, style FlowList, line ending) | checks=as above | assigns=L4[6..20] (entry rewritten as "tags: beta"; the whole entry line minus its ending is the replaced region, minimal span) | state=origin | note=props.rs set 214-218 kind change list->scalar; same path for scalar->list e.g. {"key":"status","value":["a","b"]} rewrites L3 as "status: [a, b]" (flow style, plain items)
ROW property.set | sel=none | params={"key":"owner","value":"Ana"} | resolves=key:no entry named owner -> new key | reads=fm(body end = start of L5); L2-L4(YAML validity; duplicate list); L5(closing fence line: its line ending) | checks=key non-empty single line; value type; fm valid YAML; owner not in duplicates | assigns=gap@L5 (new line "owner: Ana" before the closing fence; key double-quoted when not plain-safe or holding # or :) | state=origin | note=props.rs set 172-176 new key last in block; null value writes "owner:"
ROW property.set | sel=none | params={"key":"owner","value":"Ana"} | resolves=key:new | reads=L1(first line is not ---: no block) | checks=key non-empty single line; value type | assigns=gap@L1 (new block "---", "owner: Ana", "---" inserted at byte 0) | state=origin | note=props.rs set 169-171, path for a note without front matter (not reachable on this fixture, which has one)
ROW property.set | sel=none | params={"key":"aliases","value":["a","b"]} | resolves=key:first entry named aliases | reads=fm; block YAML; entry header + item lines (prefix "  - ", each item's text, first item's quote style) | checks=as above; style BlockList | assigns=item lines of the entry (each changed item line rewritten, unchanged ones kept; empty list replaces value..last item with " []") | state=origin | note=props.rs set 179-203 BlockList+List; not reachable on this fixture (no block list); also the Empty-style path 221-229 ("key:" gets " value" appended on its header line; null writes nothing) is not reachable here
ROW property.delete | sel=none | params={"key":"status"} | resolves=key:all entries named status; exactly one required | reads=fm(block exists); L3(entry lines incl. line ending) | checks=fm exists (not-found); key present (not-found); key written once (ambiguous otherwise) | assigns=L3 (whole line incl. its line break deleted) | state=origin | note=props.rs delete 259; no duplicate/invalid-YAML/Complex refusal on this path (a Complex or invalid entry can be deleted)
ROW property.delete | sel=none | params={"key":"status","occurrence":1} | resolves=key:occurrence-th entry named status (from 1) | reads=fm; L3 | checks=fm exists; key present; occurrence within count (0 or 2 -> out-of-range) | assigns=L3 (whole line deleted) | state=origin | note=props.rs delete 259 / occurrence_of 244 Some(n) branch; the way to remove one copy of a duplicated key
ROW property.rename | sel=none | params={"key":"status","to":"state"} | resolves=key:the single entry named status | reads=fm; L2-L4(YAML validity; duplicates; every key compared with to); L3(key token L3[0..6]) | checks=to non-empty single line; fm exists; key not duplicated; fm valid YAML; no other entry already named to (refused note.frontmatter-duplicate-key); key present and unique | assigns=L3[0..6] (key token only, "status"->"state"; quoted when not plain-safe) | state=origin | note=props.rs rename 266; Complex style is not refused (only the key changes)
ROW property.rename | sel=none | params={"key":"status","to":"status"} | resolves=key:entry L3 | reads=fm; L2-L4(validity, duplicates); L3 | checks=as above | assigns=none (changed:false) | state=origin | note=props.rs rename 276 to==key no-op after all checks
ROW property.rename | sel=none | params={"key":"status","to":"title"} | resolves=key:entry L3; to collides with L2 | reads=fm; L2(key title); L3 | checks=refused: note already has title | assigns=none (refused) | state=origin | note=props.rs rename 272
# ---- table.create (run.rs plan 813; table.rs create 364; lib.rs insert_block; section.rs position_at)
ROW table.create | sel=none | params={"columns":2} | resolves=place:end of the note (text.len, after L48's line break) | reads=L48(line before: blank? decides a leading blank line); end(blank at?); L28,L46(tables starting before the place: answer index) | checks=columns 1..64; rows <=1000; header cells one line | assigns=end (inserted "\n" + header "Column 1/Column 2" row, delimiter row " --- ", one empty body row, "\n") | state=origin | note=run.rs 832 (None,None) -> text.len(); answers table 2; rows default 1; header cells beyond columns silently dropped
ROW table.create | sel=none | params={"line":22,"columns":2,"rows":2,"header":["Step","Owner"]} | resolves=line:1-based line -> start of that line (gap@L22, "insert before this line") | reads=L21(blank before? no -> leading blank line added); L22(blank at? yes -> no trailing blank); L28,L46(tables before place: answer 0) | checks=line >= 1 (invalid); line <= line count 49 (out-of-range); columns 1..64; rows <=1000; header cells one line | assigns=gap@L22 ("\n" + 4-line table) | state=origin | note=run.rs 817-824; `position` is accepted by the schema but ignored with line. DEFECT: no check that the line is a block boundary: {"line":38,"columns":2} writes the table text inside the ```rust fence (L37-L39) and answers table 1 although no table exists; {"line":30,...} splits table 0 (L28-L29 stay a header-only table, L30-L31 become paragraph text); {"line":15,...} cuts the nested list item
ROW table.create | sel=none | params={"heading":["Timeline"],"columns":2} | resolves=heading:path, exact full path first, else unique heading whose path ends with it (["Launch plan","Timeline"] -> L26); ambiguous with 2+ matches; position default end | reads=L26(heading, level 2); L27-L34(own text up to first subsection L35 Details; trailing blank lines trimmed -> gap@L34); L33(blank before? no -> leading blank); L34(blank at? yes); L28,L46(tables before place: answer 1) | checks=heading path non-empty, found, unambiguous; position one of before/after/start/end; columns; rows; header | assigns=gap@L34 ("\n" + table) | state=origin | note=run.rs 825-831 + section.rs position_at End; table 1 (Budget) becomes index 2
ROW table.create | sel=none | params={"heading":["Timeline"],"position":"before","columns":2} | resolves=heading:path -> L26; position before -> heading start gap@L26 | reads=L26(heading line, also blank-at check: not blank -> trailing blank line); L25(blank before: yes) ; L28,L46(answer 0) | checks=as above | assigns=gap@L26 (table + "\n\n") | state=origin | note=section.rs position_at Before
ROW table.create | sel=none | params={"heading":["Timeline"],"position":"after","columns":2} | resolves=heading:path -> L26; after -> section end incl. subsection Details = gap@L44 | reads=L26(level 2) and later headings L35(level 3, inside), L44(level 2: section end); L43(blank before); L44(not blank -> trailing blank); L28,L46(answer 1) | checks=as above | assigns=gap@L44 (table + "\n\n") | state=origin | note=section.rs position_at After
ROW table.create | sel=none | params={"heading":["Timeline"],"position":"start","columns":2} | resolves=heading:path -> L26; start -> end of the heading line gap@L27 | reads=L26(heading range; line before the place is L26: not blank -> leading blank); L27(blank at: yes); L28,L46(answer 0) | checks=as above | assigns=gap@L27 ("\n" + table) | state=origin | note=section.rs position_at Start
# ---- table.* on an existing table (run.rs plan 838-885; table.rs). `table` = 0-based index into ix.tables in document order; tables in quotes/lists count too.
ROW table.setCell | sel=none | params={"table":0,"row":1,"column":1,"text":"Beta release","expected":"Beta build"} | resolves=table:index 0 in document order (L28-L31); row:0 header, 1.. body rows (delimiter row not counted) -> L30; column:0-based cell of that row -> L30[5..17] | reads=L28-L31(every cell segment: padded? decides width keeping); L30[6..16](cell content, compared with expected); L30[5..17](segment: lead/trail spaces) | checks=table index exists (not-found / out-of-range); row exists (out-of-range); column < that row's cell count (out-of-range); expected equals the cell text (raw or with \pipe unescaped) else stale; text one line (checked after expected) | assigns=L30[11..16] ("build"->"release", minimal span of the cell segment; literal pipes in text escaped) | state=origin | note=table.rs set_cell 70; thread:B{anchor} (quote "Beta build") is not touched: after this write its quote is gone (orphaned). DEFECT (minor): a text ending in a backslash written into an unpadded cell with no trailing space (e.g. "a" in a row "a pipe b") escapes the following pipe and merges two cells
ROW table.setCell | sel=none | params={"table":1,"row":0,"column":1,"text":"Price"} | resolves=table:index 1 (L46-L48); row 0 header L46; column 1 -> L46[8..14] | reads=L46-L48(padded?); L46[9..13](current content); L46[8..14](segment) | checks=table/row/column exist; text one line; no expected check | assigns=L46[9..13] ("Cost"->"Price") | state=origin | note=table.rs set_cell without expected; header cells are writable; equal text -> no edit (changed:false)
ROW table.insertRow | sel=none | params={"table":0,"at":1,"cells":["0","Kickoff"]} | resolves=table:index 0; at:body position from 1 -> before row 1 (L30) | reads=L28-L31(padded?); L28(header cell count = columns); L30(template row: prefix before the row, leading/trailing pipe, cell widths when padded; its line ending) | checks=table exists; 1 <= at <= rows (3) (out-of-range, 0 is the header); cells one line | assigns=gap@L30 (new row line "pipe 0 pipe Kickoff pipe" + line break) | state=origin | note=table.rs insert_row 129, Some(before) branch; cells beyond the header's column count silently dropped
ROW table.insertRow | sel=none | params={"table":0,"at":3} | resolves=table:index 0; at=rows.len()=3 -> append after last row | reads=L28-L31(padded?); L28(columns); L30(template); L31(last line: ends with a line break? its ending) | checks=as above | assigns=gap@L32 (new empty row line) | state=origin | note=table.rs insert_row None branch (append); for a table that ends the note without a line break the row is written after "\n" at the line end
ROW table.insertColumn | sel=none | params={"table":0,"at":1,"header":"Owner","align":"left"} | resolves=table:index 0; at:0-based column, one past the last appends | reads=L28-L31(padded?; every row's cells, leading/trailing pipes); L28(columns 2) | checks=table exists; at <= columns (out-of-range); header one line; align one of left/center/right/none | assigns=L28[8] (" Owner " + pipe), L29[7] (" :---- " + pipe), L30[5], L31[5] (empty cell "  " + pipe) | state=origin | note=table.rs insert_column 183; align default none; rows shorter than at get the cell appended at their end (at.min(len)), shifting it into the wrong column
ROW table.insertColumn | sel=none | params={"table":0,"at":2} | resolves=table:index 0; at = columns -> append | reads=L28-L31(pipes, last cell end, padded?) | checks=as above | assigns=L28[20],L29[13],L30[18],L31[22] (after each trailing pipe: empty segment + pipe; delimiter " --- ") | state=origin | note=table.rs insert_segment trailing-pipe append; without a trailing pipe " pipe segment" goes after the last cell
ROW table.deleteRow | sel=none | params={"table":0,"row":1} | resolves=table:index 0; row:body row from 1 -> L30 | reads=L30(row line with its ending) | checks=table exists; row != 0 (refused: header); row exists (out-of-range) | assigns=L30 (whole line incl. line break deleted) | state=origin | note=table.rs delete_row 211; deleting the only body row of table 1 leaves header + delimiter
ROW table.deleteColumn | sel=none | params={"table":0,"column":1} | resolves=table:index 0; column:0-based | reads=L28(columns); L28-L31(each row's cell column 1 segment, cell count, trailing pipe) | checks=table exists; column < header columns (out-of-range); columns > 1 (refused) | assigns=L28[7..19], L29[6..12], L30[4..17], L31[4..21] (pipe before + the cell's segment, each row) | state=origin | note=table.rs delete_column 221 last-column branch; rows lacking the column are skipped
ROW table.deleteColumn | sel=none | params={"table":0,"column":0} | resolves=table:index 0; column 0 | reads=L28-L31(cell 0 segment and the pipe after it) | checks=as above | assigns=L28[1..8], L29[1..7], L30[1..5], L31[1..5] (segment + following pipe) | state=origin | note=table.rs delete_column first/middle branch
ROW table.moveRow | sel=none | params={"table":0,"from":1,"to":2} | resolves=table:index 0; from/to body rows from 1 -> L30, L31 | reads=L30(moving line text incl. ending); L31(target line: ends with a line break?) ; L28(header line ending, used only when the moving line has none) | checks=table exists; from,to != 0 (refused); from,to <= body rows 2 (out-of-range); from==to -> no edit | assigns=L30 (deleted) and gap@L32 (L30's text inserted after L31) | state=origin | note=table.rs move_row 245 to>from branch; when the moved row is the note's last line its preceding line ending is taken (row from-1 read)
ROW table.moveRow | sel=none | params={"table":0,"from":2,"to":1} | resolves=table:index 0; from L31, to L30 | reads=L31(moving line); L30(target start) | checks=as above | assigns=gap@L30 (L31's text inserted), L31 (deleted) | state=origin | note=table.rs move_row to<from branch
ROW table.moveColumn | sel=none | params={"table":0,"from":0,"to":1} | resolves=table:index 0; from/to 0-based columns | reads=L28(columns); L28-L31(every row's cell segments, first..last segment span) | checks=table exists; from,to < columns (out-of-range); from==to -> no edit | assigns=L28[1..19], L29[1..12], L30[1..17], L31[1..21] (segments reordered, joined by pipes; minimal span per row; delimiter cell, i.e. alignment, moves too) | state=origin | note=table.rs move_column 285; DEFECT (minor): a row with fewer cells than max(from,to)+1 is skipped silently, so its cells end up under the wrong headers
ROW table.align | sel=none | params={"table":0,"column":1,"align":"right"} | resolves=table:index 0; column:0-based cell of the delimiter row L29 | reads=L29[8..11](delimiter cell content ":-:", its width) | checks=table exists; column < delimiter-row cell count (not the header's) (out-of-range); align one of left/center/right/none | assigns=L29[8..9] (":-:"->"--:" minimal span; width kept, at least 3) | state=origin | note=table.rs align 311
ROW table.format | sel=none | params={"table":0} | resolves=table:index 0 | reads=L28-L31(every cell's content, max cell count, widths, quote prefix of the header row, header line ending); L29(column alignments as parsed) | checks=table exists | assigns=L28-L31 (whole table rewritten with padded aligned columns; minimal() trims the edit to L28[19]..L31[3]) | state=origin | note=table.rs format 324; a row with more cells than the header gets empty header cells; escaped pipes kept
# ---- links (run.rs plan 886-911; links.rs; markdown lib.rs link/finish_link_texts, obsidian.rs bare URLs)
ROW link.insert | sel=none | params={"at":176,"destination":"Feedback plan","text":"plan"} | resolves=at:scalar offset of the whole body (unit default scalar) -> L14[18] (end of "- Collect feedback"); style default wikilink | reads=none of the text content (offset->byte through the line table only) | checks=at <= text length and on a character (out-of-range); unit scalar/utf16; style wikilink/markdown; destination non-empty, one line, no "]]" and no pipe (wikilink); text one line | assigns=gap@L14[18] ("[[Feedback plan(pipe)plan]]"; alias only when text differs from destination) | state=origin | note=run.rs 886 + links.rs render 30. DEFECT: the alias text is not checked for "]]" or a pipe: {"at":176,"destination":"a","text":"x]]y"} writes "[[a(pipe)x]]y]]". DEFECT: no check of where it lands: {"at":100,...} inserts inside "[[timeline]]" (L9[22]); {"at":44,...} inside the front-matter flow list L4; inside the code fence L38
ROW link.insert | sel=none | params={"at":176,"destination":"docs/my plan.md","style":"markdown"} | resolves=at:scalar -> L14[18]; style markdown | reads=none | checks=as above (no ]] / pipe check for markdown) | assigns=gap@L14[18] ("[my plan.md](<docs/my plan.md>)": shown text = text or last path part, brackets stripped; destination in <> when it holds a space or parenthesis) | state=origin | note=links.rs render Markdown; a destination holding < or > and a space is left unwrapped (broken link)
ROW link.insert | sel=none | params={"at":631,"destination":"chart.png","embed":true} | resolves=at:scalar end of note | reads=none | checks=as first row | assigns=end ("![[chart.png]]") | state=origin | note=links.rs render embed (!-prefix), also with style markdown -> "![chart.png](chart.png)"
ROW link.insert | sel=none | params={"at":176,"unit":"utf16","destination":"Plan"} | resolves=at:UTF-16 offset -> byte (inside a surrogate pair -> out-of-range) | reads=none | checks=as first row | assigns=gap@L14[18] ("[[Plan]]") | state=origin | note=run.rs Positions::byte utf16 branch
ROW link.update | sel=none | params={"line":9,"index":0,"destination":"schedule"} | resolves=line:1-based line; index:0-based among links whose start lies on L9 (ix.links sorted by start, bare URLs included) -> [[timeline]] L9[18..30] | reads=L9(link list on the line; link kind Wikilink; destination span L9[20..28]) | checks=destination non-empty one line; line has a link (not-found); index < count (out-of-range); link has its destination in itself (reference links refused); wikilink destination without ]] and pipe | assigns=L9[20..27] ("timeline"->"schedule", minimal span) | state=origin | note=links.rs update 76 wiki branch; anchor (#...) part is replaced too since the whole destination is given
ROW link.update | sel=none | params={"line":9,"index":0,"destination":"timeline","text":"the timeline"} | resolves=as above | reads=L9[20..28](destination); alias span (none) | checks=as above; text one line | assigns=gap@L9[28] (inserts "(pipe)the timeline" after the destination) | state=origin | note=links.rs update (None,true) branch. DEFECT: text not checked for "]]" or a pipe (same as link.insert alias)
ROW link.update | sel=none | params={"line":33,"index":0,"destination":"https://example.com/plan","text":"the plan"} | resolves=line 33 index 0 -> [the site](...) L33[4..40] | reads=L33(links on the line in order); L33[15..39](destination as written: starts with <?); L33[5..13](link text span) | checks=as first link.update row | assigns=L33[35..39] ("site"->"plan"), L33[9..13] ("site"->"plan") | state=origin | note=links.rs update markdown branch: destination wrapped in <> when written so or when it has a space/parenthesis. DEFECT: text not checked for ] or [: {"text":"x](y"} breaks the link (render strips brackets, update does not)
ROW link.update | sel=none | params={"line":33,"index":1,"destination":"img/chart.png"} | resolves=line 33 index 1 -> ![chart](chart.png) L33[45..64] | reads=L33(link order); L33[54..63](destination) | checks=as above | assigns=L33[54..54] (insert "img/" before chart.png, minimal span) | state=origin | note=links.rs update image (Markdown branch); text untouched without `text`
ROW link.update | sel=none | params={"line":1,"index":0,"destination":"x"} | resolves=line 1 has no link | reads=L1 | checks=not-found | assigns=none | state=origin | note=links.rs find 63 empty. Not on fixture: reference links refused; autolinks/bare URLs take a destination but a given `text` is silently dropped ((None,false) branch) while the answer says changed
ROW links.retarget | sel=none | params={"from":"timeline","to":"schedule"} | resolves=from:file name compared case-folded with ./ and .md dropped, against each link's file part (before #) | reads=all(every link: kind, external?, file, destination span as written for its #anchor, <...> and %20); L9[20..28] matches | checks=from and to non-empty one line; external links (https:, autolinks, bare URLs), empty-file links ([[#x]]) and reference links skipped | assigns=L9[20..27] ("timeline"->"schedule"); answer links:1 | state=origin | note=links.rs retarget 116 wiki branch: .md kept/dropped per the old link's form
ROW links.retarget | sel=none | params={"from":"chart.png","to":"img/chart.png"} | resolves=from as above -> image L33 | reads=all(links); L33[54..63](written destination) | checks=as above | assigns=L33[54..54] (insert "img/"); answer links:1 | state=origin | note=links.rs retarget markdown/image branch: written with %20 -> spaces of to encoded; space in to -> wrapped in <>. DEFECT: to is not escaped for its link form: {"from":"timeline","to":"a(pipe)b"} writes [[a(pipe)b]] (b becomes an alias); {"from":"chart.png","to":"a)b"} writes ](a)b) and breaks the image
ROW links.retarget | sel=none | params={"from":"https://example.com/site","to":"x"} | resolves=from matches only an external link | reads=all(links) | checks=as above | assigns=none; answer links:0, changed:false | state=origin | note=links.rs retarget filter: external links never retargeted; a from with an #anchor never matches anything (compared with the file part only)
# ---- comments (run.rs write 944-989, comment_range 511, anchor_at 539, new_comment 549, thread_id 566; kition-sdk crates/kition-kdm/src/comment.rs)
ROW comment.add | sel=none | params={"body":"Which beta?","quote":"beta"} | resolves=quote:occurrence 1 (default; 0 treated as 1) of a case-sensitive non-overlapping search over the whole body including front matter -> L4[7..11] | reads=all(search for the quote); L4[7..11](quote text stored) | checks=body non-empty after trim; quote non-empty; occurrence found (not-found) | assigns=new CommentThread{anchor={object:note,field:body,range:44..48,quote:"beta"},resolved:false,comments:[new]}; new Comment{author,created,body} | state=origin.principal (author default); clock (created default) | note=run.rs 968 + comment_range quote branch. NOTE: the first "beta" is in the front matter (L4), not L9: thread:A's place is occurrence 2
ROW comment.add | sel=none | params={"body":"Which beta?","quote":"beta","occurrence":2,"author":"person:bo","name":"Bo","created":"2026-11-13T10:00:00Z"} | resolves=quote:2nd occurrence -> L9[40..44] | reads=all(search); L9[40..44] | checks=as above | assigns=new thread{anchor range 118..122 quote "beta",resolved:false,comments}; new Comment{author:"person:bo",name:"Bo",created,body} | state=none (author and created given) | note=new_comment 549. DEFECT: author overrides origin.principal unchecked (any caller can write as another principal); created is not validated as ISO 8601
ROW comment.add | sel=none | params={"body":"Wording","from":118,"to":122} | resolves=from/to:scalar offsets of the whole body (unit default scalar) -> L9[40..44] | reads=L9[40..44](quote stored) | checks=both from and to given; each within text and on a character; from <= to; body non-empty | assigns=new thread{anchor range 118..122, quote "beta"}; new Comment | state=origin.principal; clock | note=comment_range from/to branch; from/to win silently over quote and heading when several are given; occurrence ignored
ROW comment.add | sel=none | params={"body":"Dates?","heading":["Timeline"]} | resolves=heading:path, exact then unique suffix -> L26 | reads=L26(the whole heading line incl. "## ") | checks=heading path non-empty, found, unambiguous; body | assigns=new thread{anchor range 340..351, quote "## Timeline"}; new Comment | state=origin.principal; clock | note=comment_range heading branch
ROW comment.add | sel=none | params={"body":"Dates?","line":26} | resolves=line:1-based line that must be a heading line -> L26 | reads=L26 | checks=line is a heading (else not-found, e.g. line 9) | assigns=as heading row | state=origin.principal; clock | note=comment_range heading_ref Line
ROW comment.add | sel=none | params={"body":"Overall fine"} | resolves=no anchor parameter -> whole note | reads=none | checks=body non-empty | assigns=new thread{anchor={object:note,field:body} no range/quote,resolved:false}; new Comment | state=origin.principal; clock | note=comment_range Ok(None) -> anchor_at without range
ROW comment.reply | sel=none | params={"thread":"thread:A","body":"Agreed"} | resolves=thread:object id taken verbatim | reads=thread:A{type,comments} (length for the append) | checks=thread exists and is a CommentThread (not-found); body non-empty | assigns=new Comment{author,created,body}; thread:A{comments} (appended at the end) | state=origin.principal; clock | note=run.rs 944 + comment.rs reply 149; resolved is not read
ROW comment.reply | sel=none | params={"thread":"thread:B","body":"Reopen?","author":"person:bo"} | resolves=thread:B | reads=thread:B{type,comments} | checks=as above | assigns=new Comment{author:"person:bo",...}; thread:B{comments} appended | state=clock | note=a resolved thread takes replies and stays resolved; author unchecked as in comment.add
ROW comment.resolve | sel=none | params={"thread":"thread:A"} | resolves=thread:A; resolved default true | reads=thread:A{type} | checks=thread exists and is a CommentThread; resolved boolean | assigns=thread:A{resolved}=true | state=origin | note=run.rs 950 + comment.rs set_resolved 158; current value not read (already-resolved B is set again)
ROW comment.resolve | sel=none | params={"thread":"thread:B","resolved":false} | resolves=thread:B | reads=thread:B{type} | checks=as above | assigns=thread:B{resolved}=false | state=origin | note=reopen path
ROW comment.move | sel=none | params={"thread":"thread:A","quote":"beta","occurrence":3} | resolves=thread verbatim; quote 3rd occurrence -> L13[11..15] | reads=thread:A{type}; all(search); L13[11..15](quote) | checks=thread exists and is a CommentThread; a range given (quote, from+to, heading or line; else invalid); quote non-empty and found | assigns=thread:A{anchor} replaced whole: {object:note,field:body,range:153..157,quote:"beta"} (any extra anchor keys dropped) | state=origin | note=run.rs 975 + plan 680; resolved and comments untouched
ROW comment.move | sel=none | params={"thread":"thread:B","from":390,"to":391} | resolves=from/to scalar -> L30[2..3] | reads=thread:B{type}; L30[2..3] | checks=as above; from <= to, in range | assigns=thread:B{anchor} {range 390..391, quote "1"} | state=origin | note=from/to path; a resolved thread may be moved
ROW comment.move | sel=none | params={"thread":"thread:A","heading":["Goals"]} | resolves=heading path -> L11 | reads=thread:A{type}; L11 | checks=heading found and unambiguous | assigns=thread:A{anchor} {range 132..140, quote "## Goals"} | state=origin | note=heading path; {"line":11} same through the line path
ROW comment.move | sel=none | params={"thread":"thread:A"} | resolves=no range | reads=thread:A{type} | checks=invalid-params (quote, or from and to, is required) | assigns=none | state=origin | note=run.rs plan 682
ROW comment.delete | sel=none | params={"thread":"thread:A","comment":"comment:A1"} | resolves=thread and comment ids verbatim | reads=thread:A{type,comments}; comment:A1 exists | checks=thread exists and is a CommentThread; comment object exists (any type); comment is in this thread's list (KDM not-found otherwise) | assigns=thread:A{comments} (A1 removed); comment:A1 deleted; thread:A deleted (A1 was its only comment) | state=origin | note=run.rs 955 + comment.rs delete_comment 164; no author check (anyone deletes anyone's comment)
ROW comment.delete | sel=none | params={"thread":"thread:A","comment":"comment:A2"} | resolves=as above, after a reply A2 exists | reads=thread:A{type,comments}; comment:A2 | checks=as above | assigns=thread:A{comments} (A2 removed); comment:A2 deleted; thread kept | state=origin | note=delete_comment with comments left
ROW comment.delete | sel=none | params={"thread":"thread:B","comment":"comment:A1"} | resolves=comment of another thread | reads=thread:B{type,comments}; comment:A1 exists | checks=refused: A1 not in thread:B's list | assigns=none | state=origin | note=membership check in comment.rs, not in run.rs
# ---- edit.undo / edit.redo (lib.rs execute_as 707-714, undo_in 764, redo_in 780, caret_after_redo 799; kition-sdk session/mod.rs undo_in 225, history_scope 363, settle 336; kition-kdm replica.rs undo_or_redo 616; kdm-model repair.rs)
ROW edit.undo | sel=none | params={} | resolves=scope:none given -> the caller's origin scope (agent) | reads=undo stack (agent scope): the last agent step = task.toggle on L19, i.e. L19[3..4] ("x" it inserted, " " it deleted); registers written by the person or other replicas after it | checks=a step exists in that scope (else done:false); scope param valid | assigns=L19[3..4] ("x"->" ", the inverse of exactly the characters that step wrote; text other writers added since is kept); system commit re-setting registers another scope/replica wrote later (keep-later-changes); repair transaction may delete comment threads left empty | state=origin (scope); undo stack (agent); person's typing run ended; selection not restored (person records untouched) | note=scenario: agent ran task.toggle {line:19}, then edit.undo as agent. lib.rs undo_in with UndoScope::Agent; skip_empty/next_record only for person scope. No check that L19 still holds what the step wrote (the CRDT inverse acts on the step's own characters)
ROW edit.undo | sel=none | params={"scope":"person"} | resolves=scope:person, given explicitly by an agent caller | reads=undo stack (person scope): the person's last step (e.g. typing "s" at L13[15]); editor record of that step (selection before) when it is still the replica's last | checks=scope valid; a person step exists | assigns=the person's step reverted (e.g. L13[15..16] deleted); editor selection reset to the record's before-selection; editor done/undone lists moved | state=origin ignored for scope; undo stack (person); editor records; selection (written) | note=scenario: person typed at end of L13, agent sends edit.undo scope person. DEFECT/observation: any caller (an agent) can take back the person's steps by passing scope:"person"; there is no check of origin against scope
ROW edit.undo | sel=none | params={} | resolves=scope: origin person (Cmd+Z) | reads=editor record of the last step: empty (a typing run that changed nothing) | checks=record.empty | assigns=none in the note (record moved from done to undone; answers done:true, damage none) | state=undo records; typing run ended | note=lib.rs undo_in skip_empty path: no replica undo
ROW edit.undo | sel=none | params={} | resolves=scope: origin person | reads=undo stack (person) empty | checks=no step | assigns=none (done:false, damage none) | state=undo stack (person); typing run ended | note=nothing-to-undo path
ROW edit.undo | sel=none | params={} | resolves=scope: agent origin | reads=undo stack (agent): last agent step = comment.resolve thread:A; thread:A{resolved} current value and who wrote it last | checks=step exists | assigns=thread:A{resolved} back to false, unless the person or another replica set it later (then kept by the keep-later-changes system commit) | state=origin; undo stack (agent) | note=scenario: agent resolved thread:A then undid; a comment step is undone like a text step (same replica undo)
ROW edit.redo | sel=none | params={} | resolves=scope: agent origin | reads=redo stack (agent): the undone task.toggle on L19 (L19[3..4]); the body before the redo (to find the changed span) | checks=a redone step exists (else done:false) | assigns=L19[3..4] (" "->"x" again); person's caret and selection moved to the end of the changed span (L19[4]) | state=origin; redo stack (agent); caret/selection (written) | note=lib.rs redo_in: for a non-person scope restore_selection is skipped and caret_after_redo moves the editor caret. DEFECT: an agent's redo moves the person's caret/selection to the agent's change (an agent's undo does not)
ROW edit.redo | sel=none | params={} | resolves=scope: origin person | reads=redo stack (person); editor record (selection after) | checks=step exists; empty-record skip as in undo | assigns=the person's undone step re-applied; selection set to the record's after-selection, or caret to the end of the changed span when no record matches | state=redo stack (person); editor records; selection (written) | note=lib.rs redo_in person path
ROW edit.redo | sel=none | params={"scope":"robot"} | resolves=scope invalid | reads=none | checks=invalid-params (scope is person or agent) | assigns=none | state=none | note=history_scope 363 refuses before any undo; execute_as has already ended the typing run
# ======== reader C
# Part C. Fixture: FIXTURE. Columns 0-based scalars.
# Shorthand used in rows:
#   W(a..b,"x") = NoteDocument::transact (editor.rs:897): clamps a,b to body length; executes text.replace
#     {from,to (scalars),text} through execute_as (lib.rs:670: known-command, refuse_unknown params, read-only
#     refusal; editor.inside so typing run is not stopped) -> run.rs:917 text_command (1001): text_edit checks
#     from<=to and to<=len ("out-of-range"; no `expected` is passed), normalize_inserted reads eol (\r\n and \r
#     -> \n unless eol=mixed), apply_text_edits (delete then insert in the body). Then renumber_changed
#     (editor.rs:925): reads the changed lines plus the next line; if any is an ordered item, renumber_from walks
#     previous siblings upward (previous_sibling/sibling_step until a parent/paragraph-after-blank) and next
#     siblings downward, skips a line the view styles mark as code, and rewrites wrong digit runs.
#   Undo state used by every write: editor.join, editor.batch/batch_before (grouped), editor.step, done/undone
#     records (take_step, editor.rs:1013), session undo group; selections.edited set.
#   Thread anchors: no editor command reads or assigns thread fields; stored scalar ranges go stale and run.rs:453
#     relocate() re-finds the quote nearest the old range on read. So thread:A{anchor} is NOT assigned by any row.
#   No "pending format" state exists in this editor (no stored marks): every format command acts on the text now.
#   Ids: these commands take no ids (selection-based); projection.rs ids are not consulted (assumption untested).
# ---- bindings/wasm/src/editor.rs run_editor (2343) and helpers
ROW edit.cut | sel=L9:40-44 | params={} | resolves=none (selection only) | reads=L9[40..44](selected_text answered); L9(orphan_start: ** * ~~ == ` spans on the line of from); L8-L10(renumber check) ; eol | checks=from!=to (else no write, answers ""); read-only refusal (lib.rs:682) | assigns=L9[40..44] deleted via W(from..to,"") | state=selection(anchor,focus ordered); undo step; view styles (renumber) | note=editor.rs:2664 plain cut. Answers text=from..to.
ROW edit.cut | sel=L9:0..L11:3 | params={} | resolves=none | reads=L9-L11(selected text); L9(orphan_start spans on from's line); L9-L12(renumber neighbours); eol | checks=from!=to | assigns=L9[0..]..L11[..3] deleted (lines merged into "Goals") | state=selection; undo step | note=editor.rs:2664 multi-line cut; orphan_start only looks at the line of `from`.
ROW edit.cut | sel=L13:0 | params={} | resolves=none | reads=none | checks=from==to -> nothing written | assigns=none | state=selection(caret) | note=editor.rs:2667 caret: answers {"text":""}, no edit, no undo step.
ROW edit.cut | sel=(input "a **b** c", select "b** c") | params={} | resolves=none | reads=line of from(mark_spans: span.open.1==from and close inside selection) | checks=from!=to | assigns=orphan_start..to i.e. "**b** c" deleted | state=selection | note=editor.rs:743 orphan path. DEFECT: answer text is from..to ("b** c") but the deletion starts at the orphaned opener (orphan_start), so the clipboard text differs from what was removed ("**" lost). No such span in the fixture.
ROW edit.cut | sel=L30:6-10 | params={} | resolves=none | reads=L30[6..10]; L30(spans) | checks=from!=to | assigns=L30[6..10] deleted (cell text "Beta"; table padding not realigned) | state=selection | note=editor.rs:2664 in a table cell: no table awareness; selection L30:0..L31:3 would cut pipes/row break with no check.
ROW selection.replace | sel=L9:40-44 | params={"text":"gamma"} | resolves=none | reads=eol(normalize_inserted in run.rs); L8-L10(renumber) | checks=text required (invalid-params); read-only refusal | assigns=L9[40..44]="gamma" via W | state=selection; editor.join set Fresh (own undo step) | note=editor.rs:2459 plain replace (paste absent/false): insert used verbatim by the editor, \r normalized only by run.rs per eol.
ROW selection.replace | sel=L13:15 | params={"text":"\n- Write docs"} | resolves=none | reads=eol; L13-L15(renumber check: no ordered item) | checks=text required | assigns=gap@L13$ new line "- Write docs" | state=caret; join Fresh | note=editor.rs:2459 caret insert; no list continuation logic (unlike Enter).
ROW selection.replace | sel=L9:40-44 | params={"text":"https://example.com/beta","paste":true} | resolves=none | reads=L9[40..44](selected text as link label); eol | checks=paste==true; from!=to; is_address(text: http(s)/mailto, no whitespace, len>8); selection has no \n (type_paired) | assigns=L9[40..44]="[beta](https://example.com/beta)" (replace select=true: new text selected) | state=selection; join Fresh | note=editor.rs:2467 -> type_paired 1244 address branch; insert normalized \r\n->\n before.
ROW selection.replace | sel=L13:2..L14:9 | params={"text":"https://example.com","paste":true} | resolves=none | reads=L13-L14(selection contains \n); eol | checks=paste; is_address true; selection has \n -> type_paired returns false (multi-char) | assigns=L13[2..]..L14[..9] replaced by the address verbatim | state=selection | note=editor.rs:2471 address over a multi-line selection falls back to plain replace.
ROW selection.replace | sel=L13:2-6 | params={"text":"a\r\nb","paste":true} | resolves=none | reads=L13-L14(renumber) | checks=paste; not an address | assigns=L13[2..6]="a\nb" (editor turns \r\n and \r into \n for paste) | state=selection | note=editor.rs:2466 paste normalization independent of the note's eol (in a crlf note the editor's text has no \r anyway).
ROW selection.replace | sel=L30:6-10 | params={"text":"x\ny","paste":true} | resolves=none | reads=eol | checks=text required | assigns=L30[6..10]="x\ny" (splits the table row) | state=selection | note=editor.rs:2471 no table-cell guard: a pasted line break inside a cell breaks table 0 (L28-L31).
ROW selection.controlKey | sel=L9:20 | params={"key":"a"} | resolves=key:lowercased name | reads=view row_edges(row of caret) | checks=key required (invalid-params); read-only refusal (spec mutates=true) | assigns=none (caret -> row start) | state=caret; view layout (wrapped rows); selections history (record_selection); folds/front matter (unfold_at_caret, leave_front_matter) | note=editor.rs:2071 move_to. DEFECT: spec marks controlKey mutating, so in a read-only (invalid UTF-8) note even a/e/b/f/p/n/v caret moves are refused by lib.rs:682 though control_key guards only k/o/t itself.
ROW selection.controlKey | sel=L9:20 | params={"key":"e"} | resolves=key | reads=view row_edges | checks=key required | assigns=none (caret -> row end) | state=caret; view layout; selection history; folds | note=editor.rs:2075.
ROW selection.controlKey | sel=L30:7 | params={"key":"e"} | resolves=key | reads=view row_edges in a table row = the cell's stops | checks=key required | assigns=none (caret -> cell text end) | state=caret; view table layout | note=editor.rs:2075 table branch of row_edges (view lib.rs:1087).
ROW selection.controlKey | sel=L9:20 | params={"key":"b"} | resolves=key b/f/p/n/v -> ArrowLeft/ArrowRight/ArrowUp/ArrowDown/PageDown without modifiers | reads=L9 graphemes; view snap/vertical/caret x; fold spans; table cells (cell_at) | checks=key required | assigns=none (caret moves; a selection collapses to its edge for b/f) | state=selection; goal column; row_end; folds; pairs cleared | note=editor.rs:2079-2083 -> navigate 1969.
ROW selection.controlKey | sel=L9:20 | params={"key":"pageDown","shift":true} | resolves=key pageup/pagedown; shift extends | reads=view vertical (20 rows), caret x, fold spans, cells | checks=key required | assigns=none (focus moves, anchor kept with shift) | state=selection; goal; folds; view height/layout | note=editor.rs:2409 page branch, then unfold_at_caret.
ROW selection.controlKey | sel=L9:20 | params={"key":"d"} | resolves=key d -> delete forward | reads=L9[20..21](next grapheme); cells (cell_at); fold state at L9$ | checks=read-only (delete returns early); table-cell end guard; from==to after computing -> nothing | assigns=L9[20..21] deleted via W | state=caret; folds; view tables | note=editor.rs:2084 -> delete(true) 1452 plain forward.
ROW selection.controlKey | sel=L11:8 (L11 folded) | params={"key":"d"} | resolves=key d | reads=L11$ next char \n; view is_hidden over L12-L16 | checks=forward, caret, next char \n, next byte hidden | assigns=L12-L16 (hidden section content up to before L17) deleted | state=caret; editor.folded; view hidden ranges | note=editor.rs:1473 Delete at a folded line end takes the folded content.
ROW selection.controlKey | sel=L30:15 | params={"key":"d"} | resolves=key d | reads=view index_tables cell content of table 0 row 2 col 1 (L30) | checks=caret in a cell and from>=content.end -> no-op | assigns=none | state=caret; view table index | note=editor.rs:1460 cell guard (forward at cell text end).
ROW selection.controlKey | sel=L13:2-6 | params={"key":"d"} | resolves=key d | reads=L13(orphan_start spans); L13-L14(renumber) | checks=from!=to | assigns=orphan_start..to (L13[2..6]) deleted | state=selection | note=editor.rs:1490 selection branch of delete.
ROW selection.controlKey | sel=L9:20 | params={"key":"h"} | resolves=key h -> Backspace | reads=L9[19..21](char before/after: empty-pair check); previous grapheme; cells; fold spans | checks=read-only; cell start guard; fold end check | assigns=L9[19..20] deleted (or both chars of an empty pair "()" / "[]") | state=caret; editor.folded | note=editor.rs:2085 -> delete(false) plain; pair branch editor.rs:1501.
ROW selection.controlKey | sel=L16:0 (L11 folded, caret at fold end) | params={"key":"h"} | resolves=key h | reads=fold_spans (folded line end, hidden extent) | checks=caret == a fold span's end | assigns=L11$..fold end: the whole hidden section content L12-L15 deleted | state=caret; editor.folded; view hidden | note=editor.rs:1484 Backspace at a fold's end.
ROW selection.controlKey | sel=L30:6 | params={"key":"h"} | resolves=key h | reads=view index_tables (cell content start) | checks=caret <= cell content.start -> no-op | assigns=none | state=caret; view tables | note=editor.rs:1460 cell guard (backward).
ROW selection.controlKey | sel=L9:30 | params={"key":"groupH"} | resolves=key grouph (Ctrl+Alt+H) | reads=L9[..30](delete_group: characters of one group on the line, graphemes) | checks=read-only; cell start guard | assigns=L9[group..30] deleted | state=caret | note=editor.rs:2087 delete(false, alt).
ROW selection.controlKey | sel=L13:2 after an edit | params={"key":"undoSelection"} | resolves=key undoselection | reads=selection history (selections.edited, done) | checks=edited or history empty -> runs edit.undo | assigns=whatever the last person undo step changed (edit.undo in Person scope) | state=selection history; undo stack (person scope) | note=editor.rs:2142 undo_selection(true) edit branch.
ROW selection.controlKey | sel=L13:2 after a move | params={"key":"undoSelection"} | resolves=key | reads=selection history done | checks=history non-empty and not edited | assigns=none (selection restored, clamped) | state=selection history | note=editor.rs:2148 selection branch.
ROW selection.controlKey | sel=L13:2 | params={"key":"redoSelection"} | resolves=key redoselection | reads=selection history undone | checks=undone empty -> runs edit.redo | assigns=what the redo step brings back (or none) | state=selection history; undo stack (person scope) | note=editor.rs:2150 redo branch; otherwise pops undone selection only.
ROW selection.controlKey | sel=L9:20 | params={"key":"k"} | resolves=key k | reads=view row_edges(row end); L9[20..row end] | checks=not read-only; to>from | assigns=L9[20..row end] deleted (whole rest of the row) | state=caret; view layout (wrapped rows) | note=editor.rs:2089 kill to row end.
ROW selection.controlKey | sel=L9:52 | params={"key":"k"} | resolves=key k | reads=view row_edges (end==caret); L9$ next grapheme | checks=end<=focus -> next grapheme | assigns=gap@L9$ line break deleted (L9 and L10 joined) | state=caret | note=editor.rs:2094 at a row end the next grapheme (the \n) goes.
ROW selection.controlKey | sel=L13:2-6 | params={"key":"k"} | resolves=key k | reads=none beyond selection | checks=from!=to | assigns=L13[2..6] deleted (no orphan_start here, unlike d/h) | state=selection | note=editor.rs:2091 selection kill.
ROW selection.controlKey | sel=L30:16 | params={"key":"k"} | resolves=key k | reads=view row_edges in a table row (cell stop end); next grapheme | checks=none for tables | assigns=L30[16..17] deleted (the padding space before the closing pipe; repeated: the pipe itself) | state=caret; view table layout | note=editor.rs:2089. DEFECT: Ctrl-K has no table-cell guard (Delete/Ctrl-D does, editor.rs:1460): at a cell's text end it deletes the padding then the `|`, breaking table 0.
ROW selection.controlKey | sel=L9:20 | params={"key":"o"} | resolves=key o | reads=eol | checks=not read-only | assigns=gap@L9[20] line break inserted (selection replaced if any); caret stays before it | state=selection; join Fresh | note=editor.rs:2100. No table guard: on L30 it splits the table row.
ROW selection.controlKey | sel=L9:20 | params={"key":"t"} | resolves=key t | reads=L9 (line edges); L9[19..21](graphemes around caret) | checks=not read-only; caret collapsed; caret not at line start; line length>=2 | assigns=L9[19..21] two graphemes swapped | state=caret | note=editor.rs:2107 mid-line transpose.
ROW selection.controlKey | sel=L11:8 | params={"key":"t"} | resolves=key t | reads=L11[6..8] (two graphemes before the caret at line end) | checks=caret at line end -> middle = previous grapheme | assigns=L11[6..8] swapped ("Goasl") | state=caret | note=editor.rs:2113 line-end transpose.
ROW selection.controlKey | sel=L11:0 or L13:2-6 | params={"key":"t"} | resolves=key t | reads=L11 edges | checks=caret at line start or a selection -> no-op | assigns=none | state=selection | note=editor.rs:2107 guard.
ROW selection.controlKey | sel=L9:20 | params={"key":"z"} | resolves=key unknown | reads=none | checks=none (unknown names fall to `_ => {}`) | assigns=none | state=engaged flag set; unfold_at_caret | note=editor.rs:2120 silently ignored, no error.
ROW format.bold | sel=L9:40 (inside "beta") | params={} | resolves=none | reads=L9(wrap_at_caret: rfind/find "**" around caret, word run [A-Za-z0-9_] around caret); eol; L8-L10 renumber | checks=no enclosing ** pair around caret; word non-empty | assigns=L9[40..44] replaced by "**beta**" (word deleted and reinserted with marks) | state=caret | note=editor.rs:2672 -> wrap 2802 -> wrap_at_caret 3072 word branch; caret kept in the word (+2 at word end: after closing mark). thread:A{anchor} not updated (relocated by quote on read).
ROW format.bold | sel=L9:44 ("beta" then ".") | params={} | resolves=none | reads=L9 around caret | checks=word ends at caret -> caret after closing mark | assigns=L9[40..44]="**beta**" | state=caret | note=editor.rs:3124 caret==end branch.
ROW format.bold | sel=L12:0 (blank line) | params={} | resolves=none | reads=L12 | checks=no word at caret | assigns=gap@L12 "****" inserted, caret between | state=caret | note=editor.rs:3132 empty-pair branch.
ROW format.bold | sel=(caret inside "**beta**" after it was bolded, L9:44) | params={} | resolves=none | reads=L9(opening "**" before caret not after a letter/"*", followed by non-space; closing "**" after caret preceded by non-space, not followed by letter; no "**" between) | checks=both marks found and open+2<=caret | assigns=L9 closing "**" and opening "**" deleted (grouped, one undo step) | state=caret | note=editor.rs:3096 unwrap at caret.
ROW format.bold | sel=L9:40-44 | params={} | resolves=none | reads=L9[40..44](selection; spaces at its ends trimmed out); L9(mark_spans "**"/"__" runs of 2 or 3) | checks=no span exactly matching; no span overlapped | assigns=L9[40..44] replaced by "**beta**"; selection = inner text | state=selection | note=editor.rs:2865 plain wrap.
ROW format.bold | sel=(L9 after bolding, select "beta" inside "**beta**") | params={} | resolves=none | reads=L9 spans | checks=selection equals a span's inner text (or the span with its marks) | assigns=both marks deleted (grouped) | state=selection | note=editor.rs:2829 unwrap of an exact span.
ROW format.bold | sel=(line "**b** c", select "b** c") | params={} | resolves=none | reads=whole line of from (all ** spans overlapped by the selection) | checks=touched spans non-empty | assigns=union range replaced by "**" + text without the touched marks + "**" ("**b c**") | state=selection | note=editor.rs:2844 merge overlapped spans. A partial selection inside one span ("**abc**", select "b") rewrites the identical text (replace with same content: a no-op edit that still opens an undo step).
ROW format.bold | sel=L13:2..L14:9 | params={} | resolves=none | reads=L13-L14 (selection, trimmed of edge spaces/tabs) | checks=to > line end of from -> multi-line branch (no span/toggle check) | assigns=L13[2..]..L14[..9] replaced by "**Ship the beta\n- Collect**" | state=selection | note=editor.rs:2820 marks around the whole multi-line selection; never toggles off across lines; markup spans a list boundary (bold not rendered).
ROW format.bold | sel=L30:6-16 | params={} | resolves=none | reads=L30 spans | checks=as single-line wrap | assigns=L30[6..16]="**Beta build**" (table padding not realigned) | state=selection | note=editor.rs:2865 table cell: no table logic.
ROW format.italic | sel=L9:40 | params={} | resolves=none | reads=L9 around caret ("*" marks; mark_spans "*"/"_" runs of 1 or 3) | checks=same branches as bold (unwrap at caret / word / empty pair; selection exact span / overlap / plain / multi-line) | assigns=L9[40..44]="*beta*" | state=caret or selection | note=editor.rs:2673 wrap("*"); identical code paths to format.bold rows above.
ROW format.italic | sel=L13:2..L14:9 | params={} | resolves=none | reads=L13-L14 | checks=multi-line | assigns=L13[2..]..L14[..9] wrapped in "*" | state=selection | note=editor.rs:2820 multi-line branch.
ROW format.strike | sel=L9:40-44 | params={} | resolves=none | reads=L9 ("~~" runs of exactly 2) | checks=as wrap | assigns=L9[40..44]="~~beta~~" | state=selection | note=editor.rs:2674 wrap("~~"); same paths as bold (caret word / unwrap / empty pair / overlap / multi-line).
ROW format.code | sel=L9:40 | params={} | resolves=none | reads=L9 ("`" runs of 1) | checks=as wrap_at_caret | assigns=L9[40..44]="`beta`" | state=caret | note=editor.rs:2675 wrap("`"); marks inside the code text are not escaped; same paths as bold.
ROW format.code | sel=L37:0..L39:3 | params={} | resolves=none | reads=L37-L39 | checks=multi-line | assigns=L37-L39 replaced by "`" + fenced block + "`" | state=selection | note=editor.rs:2820 no check for a fenced code block: wraps the fences in backticks.
ROW format.math | sel=L48:8 (in "100") | params={} | resolves=none | reads=L48 ("$" runs of 1) | checks=as wrap_at_caret | assigns=L48[8..11]="$100$" | state=caret | note=editor.rs:2681 wrap("$"); same paths as bold. Table cell: no table logic.
ROW format.math | sel=L9:40-44 | params={} | resolves=none | reads=L9 spans "$" | checks=as wrap | assigns=L9[40..44]="$beta$" | state=selection | note=editor.rs:2681 selection branch.
ROW format.highlight | sel=L9:40-44 | params={} | resolves=none | reads=L9("==" spans on the line of from: selection inside one?) | checks=not inside a highlight -> wrap("==") | assigns=L9[40..44]="==beta==" | state=selection | note=editor.rs:2676 -> highlight_color(None) 2908 -> wrap("==") (all wrap paths: caret word, empty pair, exact-span unwrap, overlap merge, multi-line).
ROW format.highlight | sel=(L9 "==🟡beta==", caret in beta) | params={} | resolves=none | reads=L9 "==" spans; chars after open mark (HIGHLIGHT_COLORS emoji) | checks=inside a span and a colour emoji follows the opening marks | assigns=closing "==" and opening "==" + emoji deleted (grouped) | state=selection | note=editor.rs:2932 coloured highlight removed.
ROW format.highlight | sel=(L9 "==beta==", caret in beta) | params={} | resolves=none | reads=L9 spans | checks=inside, no colour -> wrap("==") at caret -> unwrap branch | assigns=both "==" marks deleted | state=caret | note=editor.rs:2944 -> wrap_at_caret unwrap. With a partial selection inside ("e" of beta) wrap's overlap branch rewrites the same text: no-op edit (highlight not removed).
ROW format.highlightColor | sel=L9:40-44 | params={"color":"red"} | resolves=color:name -> index in [red,orange,yellow,green,blue,purple] -> HIGHLIGHT_COLORS emoji | reads=L9 "==" spans | checks=color required and known (invalid-params "no highlight colour"); not inside a highlight | assigns=L9[40..44]="==<red emoji>beta==" (wrap then emoji inserted after the opening marks, grouped) | state=selection | note=editor.rs:2677 -> highlight_color (None,Some) branch 2976.
ROW format.highlightColor | sel=L12:0 | params={"color":"green"} | resolves=color | reads=L12 | checks=no word -> wrap inserts "====", empty-pair test | assigns=gap@L12 "==<green emoji>==" | state=caret | note=editor.rs:2984 empty branch.
ROW format.highlightColor | sel=(caret in "==<red>beta==") | params={"color":"red"} | resolves=color | reads=L9 spans; emoji after open | checks=inside and same colour | assigns=closing "==" and opening "=="+emoji deleted | state=selection | note=editor.rs:2950 same colour removes.
ROW format.highlightColor | sel=(caret in "==<red>beta==" or "==beta==") | params={"color":"blue"} | resolves=color | reads=L9 spans; emoji after open | checks=inside; other colour or none | assigns=the emoji after the opening marks replaced (or inserted) by the blue emoji | state=selection | note=editor.rs:2962 colour change.
ROW format.highlightColor | sel=(select "==beta==" including marks) | params={"color":"red"} | resolves=color | reads=L9 spans | checks=inside requires open.1<=from: false -> (None,Some) branch -> wrap finds the exact span (open.0==from, close.1==to) | assigns=both "==" deleted; no emoji inserted | state=selection | note=editor.rs:2976. DEFECT: a selection that includes the highlight marks removes the highlight instead of colouring it.
ROW format.highlightColor | sel=L13:2..L14:9 | params={"color":"red"} | resolves=color | reads=L13-L14 | checks=multi-line: wrap multi-line branch, then no "==" pair on one line | assigns=selection wrapped in "==" only | state=selection | note=editor.rs:2976. DEFECT: across lines the colour is silently dropped (plain highlight written).
ROW format.highlightColor | sel=L9:40-44 | params={"color":"pink"} | resolves=color unknown | reads=none | checks=schema enum / not in list -> invalid-params | assigns=none | state=none | note=editor.rs:2911 error before any read.
ROW format.clear | sel=L9:40 | params={} | resolves=none | reads=none | checks=from==to -> nothing | assigns=none | state=caret | note=editor.rs:3008 caret no-op.
ROW format.clear | sel=L33:0-65 | params={} | resolves=none | reads=L33 (mark spans for ** * ~~ == ` $; link "[" "](" ")" triples; heading #s) | checks=only tokens entirely inside the selection; none -> no write | assigns=L33 "[" "](" ")" of the link and of the image embed deleted -> "See the sitehttps://example.com/site and !chartchart.png." (grouped) | state=selection (anchor/focus shifted) | note=editor.rs:3005. DEFECT: the image embed "![chart](chart.png)" is mangled into "!chartchart.png"; link destinations are fused into the label text.
ROW format.clear | sel=L11:0-8 | params={} | resolves=none | reads=L11 (heading "#"s + space) | checks=token "## " fully in selection | assigns=L11[0..3] deleted -> "Goals" (paragraph) | state=selection | note=editor.rs:3043 heading marks; needs the selection to start at the line start.
ROW format.clear | sel=L37:0..L39:3 | params={} | resolves=none | reads=L37-L39 | checks=tokens inside selection | assigns=none for the fixture (but any `*`, `$`, `[..](..)` or "# " inside a fenced block would be removed) | state=selection | note=editor.rs:3014. DEFECT (code): no code-block/inline-code awareness: `# comment` lines in a fence and marks inside `code` are stripped.
ROW format.clear | sel=(line "- [ ] see [a](b)", whole line) | params={} | resolves=none | reads=line (first "[" is the task box, then the link's "](" and ")") | checks=tokens in selection | assigns="[" of the checkbox, "](" and ")" deleted -> "-  ] see [ab" | state=selection | note=editor.rs:3027. DEFECT: link scan pairs the task checkbox's "[" with a later link's "](", destroying the task.
ROW format.clear | sel=L9:0..L13:15 | params={} | resolves=none | reads=L9-L13 (every line from from's line start to to's line end) | checks=tokens fully inside selection | assigns=heading marks "## " of L11; no other tokens here | state=selection | note=editor.rs:3014 multi-line.
ROW format.link | sel=L9:40-44 | params={} | resolves=none | reads=L9[40..44](label) | checks=none | assigns=L9[40..44]="[beta]()"; caret between the parentheses | state=selection | note=editor.rs:2683.
ROW format.link | sel=L9:40 | params={} | resolves=none | reads=none | checks=none | assigns=gap@L9[40] "[]()"; caret between the brackets | state=caret | note=editor.rs:2690 empty label branch (no word-at-caret, unlike bold).
ROW format.link | sel=L9:0..L11:5 | params={} | resolves=none | reads=L9-L11 (label) | checks=none (no multi-line or "]" check) | assigns=L9..L11[..5] replaced by "[<text with blank line>]()" | state=selection | note=editor.rs:2683. DEFECT: a multi-line or "]"-containing label yields a broken link; no escaping.
ROW format.paragraph | sel=L11:4 | params={} | resolves=level 0 from command name | reads=L11 (leading "#" run and following space or end) | checks=hashes>0 and rest starts with " " or is empty | assigns=L11[0..3] "## " removed -> "Goals" | state=selection (shifted) | note=editor.rs:2693 -> prefix_lines 3140/3144, grouped.
ROW format.paragraph | sel=L9:5 | params={} | resolves=level 0 | reads=L9 | checks=not a heading -> used 0, prefix "" | assigns=none in content, but W(L9 start..start,"") is still executed (empty text.replace: undo step opened, renumber check) | state=selection; undo | note=editor.rs:3157. Empty edit not skipped.
ROW format.heading1 | sel=L9:5 | params={} | resolves=level 1 from command name | reads=L9 prefix | checks=no heading prefix -> used 0 | assigns=gap@L9 "# " -> "# Intro text ..." | state=selection | note=editor.rs:2693 prefix_lines.
ROW format.heading2 | sel=L35:4 | params={} | resolves=level 2 | reads=L35 ("###" + space) | checks=heading prefix | assigns=L35[0..4] "### " -> "## " | state=selection | note=editor.rs:2693. Same level (heading2 on L11) rewrites "## " with "## " (no-op edit, no toggle off).
ROW format.heading3 | sel=L19:8 | params={} | resolves=level 3 | reads=L19 prefix | checks=no list/task/quote/table/code check | assigns=gap@L19 "### " -> "### - [ ] Draft the announcement ^draft" | state=selection | note=editor.rs:2693. DEFECT: the task (task:1) becomes heading text; list items, tasks, numbered items (L23), callout lines (L41 -> "### > [!note] Remember"), code lines (L38) and table rows (L30 breaks table 0) are all prefixed blindly.
ROW format.heading4 | sel=L11:0..L13:4 | params={} | resolves=level 4 | reads=L11-L13 (each line's prefix) | checks=per line | assigns=L11[0..3]->"#### "; gap@L12 "#### " (blank line becomes an empty heading); gap@L13 "#### " | state=selection | note=editor.rs:3150 every line from from's line to to's line, blank lines included. DEFECT: blank line turned into empty heading.
ROW format.heading5 | sel=L13:0..L15:0 | params={} | resolves=level 5 | reads=L13-L15 | checks=per line | assigns=gap@L13, gap@L14, gap@L15 "##### " | state=selection | note=editor.rs:3148 a selection ending at column 0 still includes that line.
ROW format.heading6 | sel=L7:3 | params={} | resolves=level 6 | reads=L7 | checks=heading prefix "# " | assigns=L7[0..2] -> "###### " | state=selection | note=editor.rs:2693. A line "#" alone (rest empty) counts as heading; "####### x" (7) is replaced too; "#tag" is not.
ROW format.comment | sel=L9:41 | params={} | resolves=none | reads=L9 ("%%" positions on the line; word_at group run) | checks=caret not between a %% pair; word run all word chars | assigns=L9[40..44]="%%beta%%"; word selected | state=caret | note=editor.rs:2717 -> toggle_comment 1706 word branch.
ROW format.comment | sel=(L9 "%%beta%%", caret in beta) | params={} | resolves=none | reads=L9 %% marks paired in order | checks=open+2<=caret<=close | assigns=both "%%" deleted (grouped) | state=caret | note=editor.rs:1716 caret in comment. Multi-line %% comments are not detected (pairs per line only).
ROW format.comment | sel=L11:1 | params={} | resolves=none | reads=L11 (word_at: "##" is group Other) | checks=word not all word chars -> falls to selection path; not wrapped | assigns=gap@L11[1] "%%%%" -> "#%%%%# Goals" | state=caret | note=editor.rs:1745. DEFECT: breaks the heading marker; caret on punctuation inserts an empty comment inside markup.
ROW format.comment | sel=L9:40-44 | params={} | resolves=none | reads=L9[38..46] (two chars before and after the selection) | checks=not already wrapped by "%%" | assigns=L9[40..44]="%%beta%%"; selection kept on inner text | state=selection | note=editor.rs:1745.
ROW format.comment | sel=(select "beta" inside "%%beta%%") | params={} | resolves=none | reads=2 chars before from and after to | checks=both are "%%" | assigns=L9 "%%beta%%" replaced by "beta" (range replaced whole) | state=selection | note=editor.rs:1738 unwrap.
ROW format.comment | sel=L13:0..L15:16 | params={} | resolves=none | reads=L13-L15 | checks=not wrapped | assigns=L13..L15 replaced by "%%...%%" (multi-line comment) | state=selection | note=editor.rs:1745 multi-line.
ROW format.quote | sel=L9:5 | params={} | resolves=prefix "> " | reads=L9 (all-lines check: trimmed start has ">"?; list continuation) | checks=not all lines quoted -> add | assigns=gap@L9 "> " | state=selection | note=editor.rs:2718 -> toggle_prefix 3178 add branch.
ROW format.quote | sel=L41:0..L42:5 | params={} | resolves=prefix | reads=L41-L42 | checks=all lines start with ">" (blank lines count as yes) | assigns=L41[0..2] and L42[0..2] removed -> callout becomes "[!note] Remember" paragraph | state=selection | note=editor.rs:3189 remove branch ("> " or bare ">" after indentation).
ROW format.quote | sel=L13:4 | params={} | resolves=prefix | reads=L13 (continuation(): list marker length) | checks=not all quoted | assigns=L13[0..2] "- " replaced by "> " -> "> Ship the beta" | state=selection | note=editor.rs:3197. DEFECT: the "list marker is replaced, not doubled" rule meant for list toggles strips the bullet when quoting.
ROW format.quote | sel=L19:8 | params={} | resolves=prefix | reads=L19 (continuation: "- [ ] " = 6 bytes) | checks=not all quoted | assigns=L19[0..6] replaced by "> " -> "> Draft the announcement ^draft" | state=selection | note=editor.rs:3197. DEFECT: quoting a task deletes its checkbox (task:1 disappears; L20 would lose its done state).
ROW format.quote | sel=L23:4 | params={} | resolves=prefix | reads=L23 (continuation "1. "); L22-L26 (renumber walk: L24 "2." no longer has a sibling before it) | checks=not all quoted | assigns=L23[0..3] replaced by "> " -> "> First step"; L24 left as "2." | state=selection; view styles | note=editor.rs:3197 + renumber_changed. DEFECT as above (number marker dropped).
ROW format.quote | sel=L15:6 | params={} | resolves=prefix | reads=L15 (indent 2 kept, marker "- " used) | checks=not all | assigns=L15[0..4] replaced by "  > " -> "  > From ten teams" | state=selection | note=editor.rs:3197 nested item: indentation kept before ">", marker dropped.
ROW format.quote | sel=L9:0..L11:3 | params={} | resolves=prefix | reads=L9-L11 | checks=not all quoted | assigns=gap@L9, gap@L10 (blank line gets "> "), gap@L11 "> " -> "> ## Goals" | state=selection | note=editor.rs:3197 blank lines are prefixed in the add branch.
ROW format.quote | sel=L30:7 | params={} | resolves=prefix | reads=L30 | checks=none for tables | assigns=gap@L30 "> " (breaks table 0) | state=selection | note=editor.rs:3197 no table guard.
ROW format.headingUp | sel=L11:4 | params={} | resolves=none | reads=L11 ("#" run followed by space) | checks=heading -> level 2 -> 1 | assigns=L11[0..3] -> "# " | state=selection | note=editor.rs:2753 prefix_lines.
ROW format.headingUp | sel=L7:3 | params={} | resolves=none | reads=L7 | checks=level 1 -> stays 1 | assigns=L7[0..2] rewritten "# " (identical text replaced: no-op edit, undo step) | state=selection | note=editor.rs:2758 saturating_sub(1).max(1).
ROW format.headingUp | sel=L9:5 | params={} | resolves=none | reads=L9 | checks=not a heading -> level 7 -> 6 | assigns=gap@L9 "###### " (paragraph becomes H6) | state=selection | note=editor.rs:2758. Also applies to blank lines in a multi-line selection (L12 -> "###### "), list/task/table/code/callout lines, "#tag" and a bare "#" line (heading test needs a space, unlike format.heading* which accepts an empty rest).
ROW format.headingDown | sel=L11:4 | params={} | resolves=none | reads=L11 | checks=heading level 2 -> 3 | assigns=L11[0..3] -> "### " | state=selection | note=editor.rs:2753.
ROW format.headingDown | sel=(line "###### x") | params={} | resolves=none | reads=line | checks=level 6 -> 7 > 6 | assigns=line[0..7] "###### " removed (paragraph) | state=selection | note=editor.rs:2760.
ROW format.headingDown | sel=L9:5 | params={} | resolves=none | reads=L9 | checks=not a heading -> 8 > 6, used 0 | assigns=none in content; empty W still executed (undo step) | state=selection | note=editor.rs:2760 no-op edit not skipped.
ROW format.headingDown | sel=L37:0..L39:3 (fence with a "# x" line inside) | params={} | resolves=none | reads=each line's "#" run | checks=no code-block check | assigns="# x" inside the fence -> "## x" | state=selection | note=editor.rs:2753. DEFECT: code lines starting with "# " are treated as headings (also by format.heading*/clear).
# ======== reader D
# Part D: editor commands acting on the selection/caret.
# Common to every row (not repeated in each row):
#  - Entry: lib.rs execute_as -> commands::is_known; params must be an object; refuse_unknown against the
#    command's schema (editor_item specs take params "{}" / no_params: ANY key is refused, invalid-params);
#    is_read_only (file not valid UTF-8) && mutates -> "read-only" error. Then run_editor (editor.rs:2344).
#    No projection ids are taken by these commands (they have no parameters), so the section/task/table id
#    map in projection.rs is not consulted; nothing here depends on it.
#  - Text = the model body (whole file, front matter L1-L5 included). The selection is editor.anchor/focus
#    (byte offsets), ordered() = (min,max).
#  - Every write goes through NoteDocument::replace -> transact (editor.rs:890-919) -> execute("text.replace")
#    -> run.rs text_command: normalize_inserted reads the note's eol style (lf/crlf strip \r from the insert;
#    mixed keeps it) -> kdm apply_text_edits (delete then insert on the body text).
#  - After EVERY transact (not inside renumbering): renumber_changed (editor.rs:925) reads the changed lines
#    and the line after them; if any is an ordered item, renumber_from walks UP (previous_sibling via
#    sibling_step: blank lines and deeper/lazy lines are Inside; a parent, a bullet item at the same indent,
#    or a paragraph after a blank line Ends) and DOWN over siblings, rewriting their digits; lines styled as
#    code in the view are skipped (reads view styles).
#  - take_step / grouped read undo-step state: editor.join, the open step's ranges and its age (clock millis,
#    JOIN_DELAY), done records' revision. Listed as "undo step" in state=.
#  - Thread anchors (thread:A on L9[40..44], thread:B on L30 "Beta build") are never read by any of these
#    commands; no command checks whether it rewrites or deletes anchored text.
# Fixture ends with a line break, so there is an empty last line L49.
# ---- bindings/wasm/src/editor.rs : list.bullet / list.numbered / list.task -> toggle_prefix (3178) -> prefix_lines (3140/3144)
ROW list.bullet | sel=L9:5 | params={} | resolves=lines:every line from line_around(from) to line_around(to) | reads=L9(all-check: trim_start starts with "-"; continuation(): existing list/quote marker length) | checks=all-lines-have-prefix test over L9 (false -> add path) | assigns=L9[0..0] insert "- " (prefix replaces continuation-used bytes = 0 here) | state=selection,undo step | note=editor.rs:2708 -> toggle_prefix add path on a plain paragraph; selection shifted by prefix length
ROW list.bullet | sel=L13:4 | params={} | resolves=lines:line of caret | reads=L13(all-check: starts with "-" after trim) | checks=all-lines-have "-" (true -> remove path); rest starts with "- " (had=2) else starts with "-" (had=1) else line left | assigns=L13[0..2] removed ("- ") | state=selection,undo step | note=toggle_prefix remove path. Defect: the all-check and the removal accept a bare "-" (prefix.trim_end()), so any line starting with "-" counts as a bullet: caret on L1 ("---") -> all=true, rest starts with "-" -> L1[0..1] removed, front matter opener becomes "--"; a line "-5 degrees" loses its minus sign
ROW list.bullet | sel=L19:8 | params={} | resolves=lines:line of caret | reads=L19(all-check) | checks=all-check true ("- [ ] ..." starts with "-") -> remove path | assigns=L19[0..2] removed -> "[ ] Draft the announcement ^draft" | state=selection,undo step | note=remove path on a task: only "- " goes, the task box "[ ] " stays as literal text at line start (bullet toggled off a task leaves an orphan box)
ROW list.bullet | sel=L15:6 | params={} | resolves=lines:line of caret | reads=L15(indent 2, all-check) | checks=all-check true -> remove path | assigns=L15[2..4] removed, indentation L15[0..2] kept -> "  From ten teams" | state=selection,undo step | note=remove path on a nested item: keeps indentation (becomes lazy continuation text of L14's item)
ROW list.bullet | sel=L23:3 | params={} | resolves=lines:line of caret | reads=L23(all-check false; continuation used=3 "1. "),L24(renumber_changed: next line ordered),L22-L21(previous_sibling walk of L24: L23 now bullet -> End) | checks=all-check false -> replace path | assigns=L23[0..3] "1. " -> "- " | state=selection,undo step,view styles(code check in renumber) | note=replace path: an existing list marker is replaced, not doubled. L24 "2. Second step" is left numbered 2 (first item of its now one-item list keeps its number)
ROW list.bullet | sel=L11:3 | params={} | resolves=lines:line of caret | reads=L11(all-check; continuation None) | checks=all-check false | assigns=L11[0..0] insert "- " -> "- ## Goals" | state=selection,undo step | note=add path on a heading: no heading check; the heading becomes a list item containing "## Goals" (section:2 heading disappears)
ROW list.bullet | sel=L42:5 | params={} | resolves=lines:line of caret | reads=L42(all-check false; continuation(): quoted line -> used=2 "> ") | checks=all-check false | assigns=L42[0..2] "> " replaced by "- " -> "- Keep it short." | state=selection,undo step | note=Defect: toggle_prefix treats the quote marker as a list marker to replace (continuation returns the quote prefix as `used`), so the line leaves the callout/quote instead of becoming "> - Keep it short."; same on L41 "> [!note] Remember" -> "- [!note] Remember" (callout header destroyed)
ROW list.bullet | sel=L30:4 | params={} | resolves=lines:line of caret | reads=L30(all-check; continuation None) | checks=all-check false | assigns=L30[0..0] insert "- " -> "- " before the row text (pipe row) | state=selection,undo step | note=add path inside a table: no table check, the table (index 0) is cut after L29; thread:B anchor not read
ROW list.bullet | sel=L19:0..L24:5 | params={} | resolves=lines:L19..L24 (line_around of from and to) | reads=L19-L24(all-check per line, blank L22 ignored by the check; continuation per line),L25(renumber_changed next line) | checks=all-check false (L23 "1." does not start with "-") | assigns=L19[0..6],L20[0..6],L21[0..6] "- [ ] "/"- [x] " replaced by "- " (task boxes and checked state dropped); L22[0..0] insert "- " (blank line gets an empty item); L23[0..3],L24[0..3] replaced by "- " | state=selection,undo step | note=multi-line replace path: prefix_lines rewrites every line last-first, including blank lines (all-check skips blanks but the rewrite does not); continuation() counts the task box in `used`, so the box goes
ROW list.bullet | sel=L13:0..L15:3 | params={} | resolves=lines:L13..L15 | reads=L13-L15(all-check: all start with "-") | checks=all-check true -> remove on each | assigns=L13[0..2],L14[0..2],L15[2..4] removed | state=selection,undo step | note=multi-line remove path
ROW list.numbered | sel=L9:5 | params={} | resolves=lines:line of caret | reads=L9(all-check "1."; continuation None),L10(renumber_changed next line) | checks=all-check false | assigns=L9[0..0] insert "1. " | state=selection,undo step,view styles | note=editor.rs:2709 add path; renumber: previous_sibling of L9 walks L8 blank (Inside), L7 heading -> End, keeps 1
ROW list.numbered | sel=L23:4 | params={} | resolves=lines:line of caret | reads=L23(all-check: starts with "1."),L24,L22-L21(renumber of L24: previous sibling search -> L23 now text, L22 blank Inside... End) | checks=all-check true -> remove path (rest starts with "1. ") | assigns=L23[0..3] removed -> "First step" | state=selection,undo step | note=remove path; L24 keeps "2." (no previous sibling, first item keeps its number). Defect: the check only accepts a literal "1." so "1.5 kg is enough" counts as numbered and loses "1." (had=2)
ROW list.numbered | sel=L24:4 | params={} | resolves=lines:line of caret | reads=L24(all-check: "2." does not start with "1."; continuation used=3),L23(previous_sibling: number 1),L25 | checks=all-check false -> replace path | assigns=L24[0..3] "2. " -> "1. ", then renumber_changed L24[0..1] "1" -> "2" | state=selection,undo step,view styles | note=Defect: list.numbered cannot toggle off an item numbered other than 1: it re-prefixes "1. " and renumbering restores "2.", net no change but two writes in one undo step
ROW list.numbered | sel=L13:0..L14:3 | params={} | resolves=lines:L13..L14 | reads=L13-L14(all-check; continuation used=2),L15(next line, not ordered),L12-L10(previous_sibling walk: blank Inside, heading End),L15-L17(walk down: nested Inside, blank Inside, heading End) | checks=all-check false | assigns=L14[0..2]->"1. ", L13[0..2]->"1. ", then L14[0..1] "1"->"2" | state=selection,undo step,view styles | note=replace path over bullets; renumber gives 1., 2.; nested L15 stays a bullet
ROW list.numbered | sel=L19:0..L21:4 | params={} | resolves=lines:L19..L21 | reads=L19-L21(all-check; continuation used=6 incl. task box),L22-L24(renumber walk down: blank L22 Inside, L23/L24 ordered Siblings),L18-L17(walk up: blank, heading End) | checks=all-check false | assigns=L19[0..6],L20[0..6],L21[0..6] -> "1. "/"2. "/"3. " (task boxes and checked state of L20 dropped); L23[0..1] "1"->"4", L24[0..1] "2"->"5" | state=selection,undo step,view styles | note=renumbering treats the blank L22 as inside the list, so the separate list L23-L24 is merged and renumbered 4,5 (writes outside the selection)
ROW list.numbered | sel=L38:3 | params={} | resolves=lines:line of caret | reads=L38(all-check; continuation None),view styles(L38 is code -> renumber skipped) | checks=all-check false | assigns=L38[0..0] insert "1. " | state=selection,undo step,view styles | note=no code-block check before writing: code text is changed; only the renumbering skips code lines
ROW list.task | sel=L13:5 | params={} | resolves=lines:line of caret | reads=L13(all-check "- [ ]"; continuation used=2) | checks=all-check false | assigns=L13[0..2] "- " -> "- [ ] " | state=selection,undo step | note=editor.rs:2710 replace path on a bullet
ROW list.task | sel=L15:5 | params={} | resolves=lines:line of caret | reads=L15(indent 2; continuation used=4) | checks=all-check false | assigns=L15[0..4] -> "  - [ ] " (indent kept) | state=selection,undo step | note=nested item
ROW list.task | sel=L19:8 | params={} | resolves=lines:line of caret | reads=L19(all-check: starts with "- [ ]") | checks=all-check true -> remove path (rest starts with "- [ ] ", had=6) | assigns=L19[0..6] removed -> "Draft the announcement ^draft" | state=selection,undo step | note=remove path
ROW list.task | sel=L20:8 | params={} | resolves=lines:line of caret | reads=L20(all-check "- [x]" != "- [ ]"; continuation used=6) | checks=all-check false -> replace path | assigns=L20[0..6] "- [x] " -> "- [ ] " | state=selection,undo step | note=Defect: on a checked task list.task does not toggle off; it rewrites the prefix and unchecks the task (done state lost)
ROW list.task | sel=L19:0..L21:5 | params={} | resolves=lines:L19..L21 | reads=L19-L21(all-check: L20 fails) | checks=all-check false | assigns=L19[0..6],L21[0..6] rewritten with identical "- [ ] "; L20[0..6] -> "- [ ] " | state=selection,undo step | note=one checked task in the selection blocks the toggle-off and unchecks it
ROW list.task | sel=L23:3 | params={} | resolves=lines:line of caret | reads=L23(continuation used=3),L24(renumber: previous sibling L23 now bullet -> End) | checks=all-check false | assigns=L23[0..3] "1. " -> "- [ ] " | state=selection,undo step,view styles | note=numbered item becomes a bullet task; L24 stays "2."
ROW list.task | sel=L42:3 | params={} | resolves=lines:line of caret | reads=L42(continuation: quote prefix used=2) | checks=all-check false | assigns=L42[0..2] "> " -> "- [ ] " | state=selection,undo step | note=Defect (same as list.bullet): the quote marker is replaced, the line leaves the callout
# ---- editor.rs : list.toggleCheckbox -> toggle_checkbox (1353)
ROW list.toggleCheckbox | sel=L19:10 | params={} | resolves=lines:line of caret | reads=L19(quote prefix, indent, marker "- ", status char after "[" ) | checks=line non-blank task -> all_tasks true; check = any status == ' ' (true) | assigns=L19[3..4] ' ' -> 'x' | state=selection,undo step | note=editor.rs:1430 all-tasks path, checking
ROW list.toggleCheckbox | sel=L20:10 | params={} | resolves=lines:line of caret | reads=L20(marker, status 'x') | checks=all_tasks true; no open box -> uncheck | assigns=L20[3..4] 'x' -> ' ' | state=selection,undo step | note=all-tasks path, unchecking (a custom status such as "[-]" counts as checked and is opened the same way)
ROW list.toggleCheckbox | sel=L19:0..L21:3 | params={} | resolves=lines:L19..L21 | reads=L19-L21(marker, status) | checks=all three tasks; L19/L21 open -> check all | assigns=L21[3..4] 'x', L20[3..4] 'x'->'x' (rewritten with the same char), L19[3..4] 'x' | state=selection,undo step | note=multi-line all-tasks path: every task set to the same state; already-checked boxes are replaced with identical text
ROW list.toggleCheckbox | sel=L19:0..L23:3 | params={} | resolves=lines:L19..L23 | reads=L19-L23(marker, status per line; blank L22 skipped for multi-line) | checks=not all tasks (L23 numbered without box) -> per-line step | assigns=L23[3..3] insert "[ ] "; L21[2..6],L20[2..6],L19[2..6] "[ ] "/"[x] " removed | state=selection,undo step | note=mixed path: tasks lose their box (item stays), non-task items gain one; blank lines skipped only when more than one line is selected
ROW list.toggleCheckbox | sel=L13:3 | params={} | resolves=lines:line of caret | reads=L13(marker "- ", no box) | checks=not all tasks | assigns=L13[2..2] insert "[ ] " | state=selection,undo step | note=(Some marker, None status) path: list item gets a box
ROW list.toggleCheckbox | sel=L15:5 | params={} | resolves=lines:line of caret | reads=L15(indent 2, marker) | checks=not all tasks | assigns=L15[4..4] insert "[ ] " | state=selection,undo step | note=nested item gets a box after its marker
ROW list.toggleCheckbox | sel=L23:4 | params={} | resolves=lines:line of caret | reads=L23(digits + ". " marker, length 3) | checks=not all tasks | assigns=L23[3..3] insert "[ ] " -> "1. [ ] First step" | state=selection,undo step,view styles(renumber of L23/L24, no change) | note=numbered item gets a box
ROW list.toggleCheckbox | sel=L9:5 | params={} | resolves=lines:line of caret | reads=L9(no marker) | checks=not all tasks | assigns=L9[0..0] insert "- [ ] " | state=selection,undo step | note=(None, _) path: plain paragraph becomes a task item
ROW list.toggleCheckbox | sel=L11:4 | params={} | resolves=lines:line of caret | reads=L11(no marker) | checks=not all tasks | assigns=L11[0..0] insert "- [ ] " -> "- [ ] ## Goals" | state=selection,undo step | note=no heading check: section:2's heading becomes a task
ROW list.toggleCheckbox | sel=L42:4 | params={} | resolves=lines:line of caret | reads=L42(quote prefix 2, no marker) | checks=not all tasks | assigns=L42[2..2] insert "- [ ] " -> "> - [ ] Keep it short." | state=selection,undo step | note=quoted path: the task goes after the ">" (correct, unlike list.task); on L41 the callout header becomes "> - [ ] [!note] Remember" (header broken, no check)
ROW list.toggleCheckbox | sel=L22:0 | params={} | resolves=lines:the single blank line | reads=L22 | checks=single line: blank not skipped; tasks list empty -> not all tasks | assigns=L22[0..0] insert "- [ ] " | state=selection,undo step,view styles(renumber: L23 next line ordered, no change) | note=caret on a blank line makes an empty task
# ---- editor.rs : edit.indent / edit.outdent -> indent_selection (1441) -> indent_lines (1867) -> renumber_line (1917) -> renumber_after (1576)
ROW edit.indent | sel=L15:4 | params={} | resolves=lines:line of from .. line of to | reads=L15(quote prefix),L14-L13(renumber_line walk up only when the line is numbered: not here) | checks=none (no list check; is_read_only re-checked) | assigns=L15[0..0] insert "\t" -> "\t  - From ten teams" | state=selection,undo step | note=editor.rs:2712; always a tab at the line start (after any quote prefix), mixing tab and existing spaces
ROW edit.indent | sel=L24:5 | params={} | resolves=lines:line of caret | reads=L24(numbered, indent after tab = 1),L23(renumber_line walk up: indent 0 < 1 -> stop, wanted 1),L25(renumber_after walk down; renumber_line also for the line after the selection) | checks=none | assigns=L24[0..0] insert "\t"; L24[1..2] "2" -> "1" | state=selection,undo step,view styles | note=nested numbered item restarts at 1 (renumber_changed in transact first keeps 2, renumber_line then sets 1)
ROW edit.indent | sel=L13:0..L14:4 | params={} | resolves=lines:L13..L14 | reads=L13-L14,L15(renumber_line on line after; not numbered) | checks=none | assigns=L13[0..0] "\t", L14[0..0] "\t" | state=selection,undo step | note=multi-line: every touched line indented, last first; selection shifted
ROW edit.indent | sel=L9:5 | params={} | resolves=lines:line of caret | reads=L9 | checks=none | assigns=L9[0..0] insert "\t" | state=selection,undo step | note=no list/paragraph check: a tab-indented paragraph after a blank line is an indented code block in Markdown (L9 becomes code, ^intro block id inside code)
ROW edit.indent | sel=L11:2 | params={} | resolves=lines:line of caret | reads=L11 | checks=none | assigns=L11[0..0] insert "\t" | state=selection,undo step | note=a heading indented by a tab stops being a heading (section:2 gone); no check
ROW edit.indent | sel=L42:3 | params={} | resolves=lines:line of caret | reads=L42(quote_prefix 2) | checks=none | assigns=L42[2..2] insert "\t" | state=selection,undo step | note=quoted line: tab after "> "
ROW edit.indent | sel=L19:0..L22:0 | params={} | resolves=lines:L19..L22 (to at column 0 of L22 still includes L22) | reads=L19-L22,L23(renumber_line for the line after) | checks=none | assigns=L19[0..0],L20[0..0],L21[0..0],L22[0..0] "\t" (blank L22 gets a lone tab) | state=selection,undo step | note=a selection ending at a line start indents that line too; blank lines are not skipped
ROW edit.outdent | sel=L15:4 | params={} | resolves=lines:line of caret | reads=L15(leading spaces: 2) | checks=leading tab (remove 1) or spaces (remove min(n,4)); 0 -> line skipped | assigns=L15[0..2] removed -> "- From ten teams" | state=selection,undo step | note=editor.rs:2713 outdent path with spaces
ROW edit.outdent | sel=L13:3 | params={} | resolves=lines:line of caret | reads=L13(no leading whitespace) | checks=remove == 0 -> skip | assigns=none | state=selection | note=no-op path (no write, no undo step opened since transact not called)
ROW edit.outdent | sel=L24:5 (after edit.indent: "\t1. Second step") | params={} | resolves=lines:line of caret | reads=L24(leading tab),L23(renumber_line: same indent, number 1 -> wanted 2),L25 | checks=leading tab -> remove 1 | assigns=L24[0..1] removed; L24[0..1] "1" -> "2" | state=selection,undo step,view styles | note=outdent rejoins the parent list and renumbers
ROW edit.outdent | sel=L13:0..L15:4 | params={} | resolves=lines:L13..L15 | reads=L13-L15 | checks=per line remove count (L13/L14 0 -> skipped) | assigns=L15[0..2] removed | state=selection,undo step | note=multi-line: only lines with leading whitespace change
# ---- editor.rs : edit.moveLineUp / edit.moveLineDown -> move_lines (1319)
ROW edit.moveLineUp | sel=L14:3 | params={} | resolves=lines:block = lines from line_around(from) to line_around(to) (to excluded when the selection ends right after a line break) | reads=L14(block),L13(line above) | checks=start == 0 -> no-op | assigns=L13-L14 rewritten whole as "L14\nL13" | state=selection,undo step | note=editor.rs:1336 up path; selection shifted by the line above's length
ROW edit.moveLineUp | sel=L15:5 | params={} | resolves=lines:block L15 | reads=L15,L14 | checks=start != 0 | assigns=L14-L15 rewritten whole -> "  - From ten teams\n- Collect feedback" | state=selection,undo step | note=no list-structure check: the child moves above its parent and now hangs under L13
ROW edit.moveLineUp | sel=L24:4 | params={} | resolves=lines:block L24 | reads=L24,L23,L25(renumber_changed next line),L22-L21(previous_sibling of new L23: blank Inside, bullet End) | checks=start != 0 | assigns=L23-L24 rewritten whole -> "2. Second step\n1. First step"; then L24[0..1] "1" -> "3" | state=selection,undo step,view styles | note=renumbering after the move: first item keeps its number 2, so the result is "2. Second step / 3. First step"
ROW edit.moveLineUp | sel=L19:0..L21:5 | params={} | resolves=lines:block L19-L21 | reads=L19-L21,L18 | checks=start != 0 | assigns=L18-L21 rewritten whole (blank moves below the block) | state=selection,undo step | note=multi-line block
ROW edit.moveLineUp | sel=L19:0..L20:0 | params={} | resolves=lines:block L19 only (to-1 used because the text before `to` ends with a line break) | reads=L19,L18 | checks=start != 0 | assigns=L18-L19 rewritten whole | state=selection,undo step | note=selection ending at a line start excludes that line (unlike toggle_prefix/indent/delete which include it)
ROW edit.moveLineUp | sel=L1:1 | params={} | resolves=lines:block L1 | reads=none beyond line_around | checks=start == 0 -> no-op | assigns=none | state=selection | note=first line: nothing moves
ROW edit.moveLineUp | sel=L29:3 | params={} | resolves=lines:block L29 | reads=L29,L28 | checks=start != 0 | assigns=L28-L29 rewritten whole (alignment row above header) | state=selection,undo step | note=no table check: table index 0 is no longer a table; same for L6 up past "---" breaking front matter
ROW edit.moveLineDown | sel=L13:2 | params={} | resolves=lines:block L13 | reads=L13,L14(line below) | checks=end >= text.len -> no-op | assigns=L13-L14 rewritten whole -> "L14\nL13" | state=selection,undo step | note=editor.rs:1341 down path
ROW edit.moveLineDown | sel=L23:0..L24:5 | params={} | resolves=lines:block L23-L24 | reads=L23-L25,L26(renumber_changed next line) | checks=end < len | assigns=L23-L25 rewritten whole (blank L25 moves above) | state=selection,undo step,view styles | note=multi-line; numbers unchanged (walk up from new L24: blank L23, L22 blank, L21 bullet End)
ROW edit.moveLineDown | sel=L48:3 | params={} | resolves=lines:block L48 | reads=L48,L49(empty last line) | checks=end (L48 end) < len because of the final line break | assigns=L48-L49 rewritten whole -> empty line then the Venue row (final line break moves before the row) | state=selection,undo step | note=moving the last text line down swaps it with the empty last line: the table gets a blank line inside it at L48 and the note loses its trailing newline
ROW edit.moveLineDown | sel=L49:0 | params={} | resolves=lines:empty last line | reads=none | checks=end >= text.len -> no-op | assigns=none | state=selection | note=no line below
# ---- editor.rs : edit.deleteParagraph -> delete_paragraph (1682)
ROW edit.deleteParagraph | sel=L13:4 | params={} | resolves=lines:line_around(from)..line_around(to) | reads=L13(range),layout(view.vertical: caret one visual row down, for the new caret) | checks=end < len -> take the line with its line break | assigns=L13+break removed | state=selection,view layout (width/wrap via view.vertical),undo step | note=editor.rs:1682 main path; caret goes to the line below at the same x
ROW edit.deleteParagraph | sel=L19:2..L21:6 | params={} | resolves=lines:L19..L21 | reads=L19-L21,layout | checks=end < len | assigns=L19-L21 + break removed | state=selection,view layout,undo step | note=multi-line; caret stays at the start if the row below falls inside the deleted block
ROW edit.deleteParagraph | sel=L23:2 | params={} | resolves=lines:L23 | reads=L23,new L23 (renumber_changed: "2. Second step" previous_sibling: blank, bullet End),layout | checks=end < len | assigns=L23 + break removed | state=selection,view layout,undo step,view styles | note=remaining item keeps "2." (first item keeps its number)
ROW edit.deleteParagraph | sel=L9:41 | params={} | resolves=lines:L9 | reads=L9,layout | checks=end < len | assigns=L9 + break removed | state=selection,view layout,undo step | note=deletes the text thread:A is anchored on (and block ^intro) without reading or checking the thread: the open thread is left without anchor text
ROW edit.deleteParagraph | sel=L49:0 | params={} | resolves=lines:empty last line | reads=layout | checks=end == len -> take the line break before instead | assigns=L48$ line break removed | state=selection,view layout,undo step | note=last-line path (start-1..end)
# ---- editor.rs : edit.sortLines / edit.trimWhitespace (2765)
ROW edit.sortLines | sel=L19:0..L21:4 | params={} | resolves=lines:line_around(from).start..line_around(to).end | reads=L19-L21(lines, lowercased sort keys) | checks=rewritten != block (else no write) | assigns=L19-L21 rewritten whole -> "- [ ] Draft the announcement ^draft / - [ ] Draft the invite ^draft / - [x] Book the review meeting ^book"; selection set to the block | state=selection,undo step | note=editor.rs:2765 sort path; stable sort by to_lowercase (byte order)
ROW edit.sortLines | sel=L13:0..L15:5 | params={} | resolves=lines:L13..L15 | reads=L13-L15 | checks=rewritten != block | assigns=L13-L15 rewritten whole -> "  - From ten teams / - Collect feedback / - Ship the beta" | state=selection,undo step | note=no list awareness: the nested child (leading spaces sort first) moves above its parent
ROW edit.sortLines | sel=L28:0..L31:3 | params={} | resolves=lines:L28..L31 | reads=L28-L31 | checks=rewritten != block | assigns=L28-L31 rewritten whole -> alignment row first, then row "1", row "2", header "Week" last | state=selection,undo step | note=no table check: table index 0 is destroyed (header no longer first)
ROW edit.sortLines | sel=L20:0..L24:2 | params={} | resolves=lines:L20..L24 | reads=L20-L24,L25(renumber_changed) | checks=rewritten != block | assigns=L20-L24 rewritten whole (blank L22 sorts to the top; tasks then numbered items) | state=selection,undo step,view styles | note=blank lines take part in the sort
ROW edit.sortLines | sel=L23:0..L24:3 | params={} | resolves=lines:L23..L24 | reads=L23-L24 | checks=rewritten == block -> no write | assigns=none | state=selection | note=already-sorted no-op path (also any single-line selection or caret)
ROW edit.trimWhitespace | sel=L7:0..L15:3 | params={} | resolves=lines:L7..L15 | reads=L7-L15(trailing spaces/tabs) | checks=rewritten != block -> fixture has no trailing whitespace -> no write | assigns=none | state=selection | note=editor.rs:2774 no-op path on the fixture
ROW edit.trimWhitespace | sel=L9:3 (L9 with two trailing spaces typed first) | params={} | resolves=lines:L9 | reads=L9 | checks=rewritten != block | assigns=L9 rewritten whole (trailing " \t" removed); selection set to the line | state=selection,undo step | note=trim path. Removes Markdown hard line breaks (two trailing spaces) too; in a mixed-eol note a line ending "\r" keeps spaces before the \r (only ' ' and '\t' trimmed after split on '\n')
# ---- editor.rs : insert.wikilink / insert.embed (surround_to_end 3208), insert.image (surround 3127)
ROW insert.wikilink | sel=L9:4 | params={} | resolves=at:caret | reads=none (selection text empty) | checks=none | assigns=gap@L9[4] insert "[[]]" | state=selection,undo step | note=editor.rs:2719 caret path; caret between the brackets. No file list consulted
ROW insert.wikilink | sel=L9:40-44 | params={} | resolves=at:selection | reads=L9[40..44](selected text "beta") | checks=none | assigns=L9[40..44] replaced with "[[beta]]" | state=selection,undo step | note=selection path; caret at end inside. The replace deletes and reinserts the text thread:A is anchored on; the thread is not read (anchor survival depends on the CRDT, unchecked)
ROW insert.wikilink | sel=L13:2..L14:9 | params={} | resolves=at:selection | reads=L13[2..]-L14[..9](selected text incl. line break) | checks=none | assigns=L13[2..]-L14[..9] replaced with "[[Ship the beta\n- Collect]]" | state=selection,undo step | note=across lines: a line break goes inside the link (no check), producing a broken wikilink
ROW insert.embed | sel=L33:3 | params={} | resolves=at:caret | reads=none | checks=none | assigns=gap@L33[3] insert "![[]]" | state=selection,undo step | note=editor.rs:2720 caret path
ROW insert.embed | sel=L30:6-16 | params={} | resolves=at:selection | reads=L30[6..16]("Beta build") | checks=none | assigns=L30[6..16] replaced with "![[Beta build]]" | state=selection,undo step | note=selection path in a table cell; thread:B anchor text rewritten unchecked
ROW insert.image | sel=L33:4 | params={} | resolves=at:caret | reads=none | checks=none | assigns=gap@L33[4] insert "![]()" | state=selection,undo step | note=editor.rs:2721 caret path; caret between "![" and "]" (alt text), not in the address
ROW insert.image | sel=L9:5-9 | params={} | resolves=at:selection | reads=L9[5..9]("text") | checks=none | assigns=L9[5..9] replaced with "![text]()" | state=selection,undo step | note=selection path; selected text becomes the alt text and stays selected
# ---- editor.rs : insert.tag (2722)
ROW insert.tag | sel=L9:16 | params={} | resolves=at:from (selection start) | reads=none | checks=none | assigns=gap@L9[16] insert "#" | state=selection,undo step (may join the typing run) | note=caret path; a selection is kept as text, "#" goes before it and the selection collapses after the "#"
ROW insert.tag | sel=L11:0 | params={} | resolves=at:from | reads=none | checks=none | assigns=gap@L11[0] insert "#" -> "### Goals" | state=selection,undo step | note=Defect: no context check: at a heading's start the tag sign raises the heading level (section:2 becomes level 3)
ROW insert.tag | sel=L13:2-6 | params={} | resolves=at:from | reads=none | checks=none | assigns=gap@L13[2] insert "#" -> "- #Ship the beta" | state=selection,undo step | note=selection path: selection not replaced
# ---- editor.rs : insert.footnote (2728)
ROW insert.footnote | sel=L9:44 | params={} | resolves=label:count of "[^" in the whole text / 2 + 1; at:selection end (to) | reads=all(count of "[^"),end(does the text end with "\n") | checks=none | assigns=end insert "\n[^1]: " (tail "" since text ends with "\n"); gap@L9[44] insert "[^1]" | state=selection,undo step (grouped) | note=editor.rs:2728 caret path; caret after "[^1]: " at the end. Defect: the label is guessed by counting "[^" anywhere (code, front matter, links), not by reading existing footnote labels: a note holding one reference "[^1]" without definition counts 1 -> label [^1] again (collision); "[^" inside a code block shifts the number
ROW insert.footnote | sel=L9:40-44 | params={} | resolves=label:as above; at:to (end of selection) | reads=all,end | checks=none | assigns=end insert "\n[^1]: "; gap@L9[44] insert "[^1]" | state=selection,undo step | note=selection path: the selection is not replaced; the reference goes after it
ROW insert.footnote | sel=L49:0 | params={} | resolves=at:to == end | reads=all,end | checks=none | assigns=end insert "\n[^1]: " then gap at the old end insert "[^1]" (before the definition) | state=selection,undo step | note=caret-at-end path: reference and definition adjacent; caret placed after the definition
# ---- editor.rs : insert.mathBlock / insert.codeBlock -> fence_lines (3230)
ROW insert.mathBlock | sel=L22:0 | params={} | resolves=lines:line_around(from)..line_around(to) | reads=L22(blank test) | checks=lines.trim() empty -> empty-block path | assigns=L22 replaced with "$$\n\n$$" | state=selection,undo step,view styles(renumber L23: no change) | note=editor.rs:2745 empty path; caret on the middle line
ROW insert.mathBlock | sel=L9:5 | params={} | resolves=lines:L9 | reads=L9 | checks=not blank -> wrap path | assigns=L9 replaced whole with "$$\n<L9>\n$$" | state=selection,undo step | note=wrap path: whole lines go between fences, selection shifted
ROW insert.mathBlock | sel=L13:0..L15:4 | params={} | resolves=lines:L13..L15 | reads=L13-L15 | checks=not blank | assigns=L13-L15 replaced whole with fenced block | state=selection,undo step | note=multi-line wrap path
ROW insert.codeBlock | sel=L22:0 | params={} | resolves=lines:L22 | reads=L22 | checks=blank -> empty path | assigns=L22 replaced with "```\n\n```" | state=selection,undo step | note=editor.rs:2793 empty path
ROW insert.codeBlock | sel=L28:0..L31:2 | params={} | resolves=lines:L28..L31 | reads=L28-L31 | checks=not blank | assigns=L28-L31 replaced whole with "```\n...\n```" | state=selection,undo step | note=wrap path: table index 0 becomes code (no check), later tables shift index
ROW insert.codeBlock | sel=L38:4 | params={} | resolves=lines:L38 | reads=L38 | checks=not blank | assigns=L38 replaced with "```\nfn main() {}\n```" | state=selection,undo step | note=Defect: no in-code check: inside the existing ```rust block the inserted opening fence closes it, leaving L39's fence opening a new unterminated block that swallows the rest of the note
# ---- editor.rs : insert.callout -> insert_callout (3264)
ROW insert.callout | sel=L22:0 | params={} | resolves=lines:L22 | reads=L22(blank test) | checks=blank -> new-callout path | assigns=gap@L22[0] (selection from..to) insert "\n> [!NOTE] Title\n> Contents\n" | state=selection,undo step,view styles | note=editor.rs:2746 empty path; "NOTE" selected
ROW insert.callout | sel=L9:5 | params={} | resolves=lines:L9 | reads=L9 | checks=not blank -> wrap path | assigns=L9 replaced whole with "> [!NOTE]\n> <L9>" | state=selection,undo step | note=wrap path; whole line quoted
ROW insert.callout | sel=L19:0..L21:3 | params={} | resolves=lines:L19..L21 | reads=L19-L21 | checks=not blank | assigns=L19-L21 replaced whole with "> [!NOTE]\n> - [ ] ...\n> - [x] ...\n> - [ ] ..." | state=selection,undo step | note=multi-line wrap; tasks move into a callout (task:1-3 now quoted)
ROW insert.callout | sel=L42:3 | params={} | resolves=lines:L42 | reads=L42 | checks=not blank | assigns=L42 replaced whole with "> [!NOTE]\n> > Keep it short." | state=selection,undo step | note=inside an existing callout: no check, the header line "> [!NOTE]" lands inside the outer callout and the line is double-quoted
ROW insert.callout | sel=L11:2 | params={} | resolves=lines:L11 | reads=L11 | checks=not blank | assigns=L11 replaced whole -> "> [!NOTE]\n> ## Goals" | state=selection,undo step | note=heading quoted: section:2 heading moves into a callout (no longer a section heading)
# ---- editor.rs : insert.date / insert.timestamp (2747) ; clock.rs now()
ROW insert.date | sel=L9:44 | params={} | resolves=at:selection | reads=clock(UTC now via js Date.now / SystemTime) | checks=none | assigns=gap@L9[44] insert "YYYY-MM-DD" | state=selection,clock,undo step | note=editor.rs:2747 caret path. Defect: clock::now is UTC (from_millis, no time zone), so "today" is wrong for users east/west of UTC near midnight
ROW insert.date | sel=L9:5-9 | params={} | resolves=at:selection | reads=clock | checks=none | assigns=L9[5..9] replaced with the date | state=selection,clock,undo step | note=selection path: replaces the selection
ROW insert.timestamp | sel=L9:44 | params={} | resolves=at:selection | reads=clock | checks=none | assigns=gap@L9[44] insert "YYYY-MM-DD HH:mm" | state=selection,clock,undo step | note=caret path; time in UTC (same defect)
ROW insert.timestamp | sel=L13:2..L14:3 | params={} | resolves=at:selection | reads=clock | checks=none | assigns=L13[2..]-L14[..3] replaced with the timestamp (lines joined) | state=selection,clock,undo step | note=selection across lines: the line break is replaced
# ---- editor.rs : insert.rule / insert.table -> insert_at_caret (3250)
ROW insert.rule | sel=L22:0 | params={} | resolves=at:selection from..to | reads=L22[0..0](text before caret on the line: whitespace only -> lead "\n") | checks=none | assigns=gap@L22[0] insert "\n---\n" | state=selection,undo step,view styles(renumber L23 walk: "---" ends the list) | note=editor.rs:2794 lead "\n" path; caret after "---"
ROW insert.rule | sel=L9:52 (line end) | params={} | resolves=at:caret | reads=L9[0..52](text before caret non-blank -> lead "\n\n") | checks=none | assigns=gap@L9$ insert "\n\n---\n" | state=selection,undo step | note=lead "\n\n" path (blank line keeps "---" from making L9 a setext heading)
ROW insert.rule | sel=L9:5 | params={} | resolves=at:caret | reads=L9[0..5] | checks=none | assigns=gap@L9[5] insert "\n\n---\n" -> L9 split: "Intro", "", "---", " text with ..." | state=selection,undo step | note=mid-line: the line is cut and its rest follows the rule with a leading space
ROW insert.rule | sel=L13:2..L14:3 | params={} | resolves=at:selection | reads=L13[0..2] | checks=none | assigns=L13[2..]-L14[..3] replaced with "\n\n---\n" | state=selection,undo step | note=selection path: selected text deleted
ROW insert.rule | sel=L1:0 | params={} | resolves=at:caret | reads=L1[0..0] | checks=none | assigns=gap@L1[0] insert "\n---\n" | state=selection,undo step | note=no front-matter check: the note no longer starts with "---", so fm L1-L5 stops being front matter
ROW insert.table | sel=L22:0 | params={} | resolves=at:selection | reads=L22[0..0](lead "\n") | checks=none | assigns=gap@L22[0] insert "\n" + 3-line empty 2-column table block (header, delimiter, one row) | state=selection,undo step,view styles(renumber L23 walk through table lines as lazy/End) | note=editor.rs:2795 lead "\n" path; caret 1 byte into the first cell. New table becomes table index 1 (Budget's table shifts to index 2)
ROW insert.table | sel=L14:18 (line end) | params={} | resolves=at:caret | reads=L14[0..18](lead "\n\n") | checks=none | assigns=gap@L14$ insert "\n\n" + the empty table block | state=selection,undo step | note=lead "\n\n" path after a list item: table at column 0 ends the list; L15 (nested item) is left after the table
ROW insert.table | sel=L30:5 | params={} | resolves=at:caret | reads=L30[0..5] | checks=none | assigns=gap@L30[5] insert "\n\n" + the empty table block | state=selection,undo step | note=no in-table check: the existing table row is split, table index 0 broken
ROW insert.table | sel=L38:3 | params={} | resolves=at:caret | reads=L38[0..3] | checks=none | assigns=gap@L38[3] insert "\n\n" + the empty table block | state=selection,undo step | note=no in-code check: the table text lands inside the rust code block
"########;

/// The rows: (command, fields by name, with `reader` the letter of the part it is in).
fn rows() -> Vec<(String, Vec<(String, String)>)> {
    let mut reader = String::new();
    READINGS
        .lines()
        .filter_map(|line| {
            if let Some(letter) = line.strip_prefix("# ======== reader ") {
                reader = letter.trim().to_owned();
            }
            line.strip_prefix("ROW ").map(|row| (row, reader.clone()))
        })
        .map(|(row, reader)| {
            let mut parts = row.split(" | ");
            let command = parts.next().unwrap().trim().to_owned();
            let fields = parts
                .map(|part| {
                    let (name, value) = part.split_once('=').unwrap_or((part, ""));
                    (name.trim().to_owned(), value.trim().to_owned())
                })
                .chain(std::iter::once(("reader".to_owned(), reader)))
                .collect();
            (command, fields)
        })
        .collect()
}

#[test]
fn every_write_command_has_a_reading_of_every_part() {
    let rows = rows();
    const PARTS: [&str; 9] = ["sel", "params", "resolves", "reads", "checks", "assigns", "state", "note", "reader"];
    for (command, fields) in &rows {
        let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, PARTS, "{command}");
    }
    let read: BTreeSet<&str> = rows.iter().map(|(command, _)| command.as_str()).collect();
    let writes: BTreeSet<&str> = SPECS.iter().filter(|spec| spec.mutates).map(|spec| spec.id).collect();
    let unread: Vec<&&str> = writes.difference(&read).collect();
    let unknown: Vec<&&str> = read.difference(&writes).collect();
    assert!(unread.is_empty() && unknown.is_empty(), "unread {unread:?}, not writes {unknown:?}");
}

// ---- The comparison.
//
// Rules, the same for every row:
//
// - Text a row holds: the regions of `reads=`, `checks=` and `assigns=`, outside parentheses
//   (parentheses say why). A whole line (`L19`, `L17-L25`) includes its line break; `L19[a..b]`
//   is those columns, `L19[a]`, `gap@L19` (the line's start), `gap@L19$` (its end) and `end` are
//   points; `all` is the body, `all[a..b]` characters of it, `fm` lines 1 to 5.
// - Facts a row holds, anywhere in `reads=`, `checks=`, `assigns=`, `state=`: `eol`, `bom`,
//   `thread:X{fields}` (each field; `type` only that it is there), `comment:X`, `files` (the
//   workspace's files), and in `state=` the editor's `selection` or `caret`. The clock and the
//   caller's origin are no facts of the document.
// - What the declaration holds: `under` on the fixture (after an agent's `task.toggle` of L19 for
//   `edit.undo`, and its undo too for `edit.redo`): a premise on the root without fields, or with
//   `body`, holds the whole body; a place (`covers`) or content premise holds what
//   `premise::footprint_of` gives; the root's fields and the comment objects' as the premises name
//   them (`fields: []` only that the object is there).
// - Held less: a character or point of the row's text the declaration does not hold (a range
//   holds the characters from its start to its end, both ends included: the kit compares an edit
//   with what a write reads by touching, `premise::touches`), or a fact it does not hold. Held
//   more: a character or point the declaration holds that no region of the row holds, or a fact
//   no row of the command names.
//
// Parts B, C and D state once, in their headers, that every write of the body reads the root's
// `eol` (inserted line breaks take it); their rows that write text hold `eol`.

use kition_note_kit::premise::footprint_of;
use kition_note_kit::{KitDocument, KitSessionDocument, NoteDocument, Origin};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ops::Range;

/// Each line's (start, end without its break, end with it), in characters.
fn lines() -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in FIXTURE.split_inclusive('\n') {
        let length = line.chars().count();
        let bare = line.trim_end_matches('\n').chars().count();
        out.push((at, at + bare, at + length));
        at += length;
    }
    out
}

fn line(n: usize) -> (usize, usize, usize) {
    let all = lines();
    let total = FIXTURE.chars().count();
    all.get(n.wrapping_sub(1)).copied().unwrap_or((total, total, total))
}

/// `text` without what parentheses hold.
fn bare(text: &str) -> String {
    let mut depth = 0usize;
    text.chars()
        .filter(|c| {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => return depth == 0,
            }
            false
        })
        .collect()
}

/// The text regions of one field of a row, in characters (`start == end`: a point).
fn regions(field: &str) -> Vec<Range<usize>> {
    let total = FIXTURE.chars().count();
    let text = bare(field);
    let token = Regex::new(
        r"gap@L(\d+)(\$|\[(\d+)\])?|L(\d+)\[(\d+)(?:\.\.(\d+))?\]|L(\d+)\s*-\s*L(\d+)|L(\d+)|\ball\[(\d+)\.\.(\d+)\]|\ball\b|\bfm\b|\bend\b",
    )
    .unwrap();
    let mut out = Vec::new();
    for found in token.captures_iter(&text) {
        let n = |i: usize| found.get(i).map(|m| m.as_str().parse::<usize>().unwrap());
        let whole = found.get(0).unwrap().as_str();
        if let Some(l) = n(1) {
            let (start, end, _) = line(l);
            let at = match (found.get(2).map(|m| m.as_str()), n(3)) {
                (Some("$"), _) => end,
                (_, Some(column)) => start + column,
                _ => start,
            };
            out.push(at..at);
        } else if let Some(l) = n(4) {
            let (start, _, _) = line(l);
            let from = start + n(5).unwrap();
            out.push(from..n(6).map_or(from, |to| start + to));
        } else if let (Some(a), Some(b)) = (n(7), n(8)) {
            out.push(line(a).0..line(b).2);
        } else if let Some(l) = n(9) {
            let (start, _, with_break) = line(l);
            out.push(start..with_break);
        } else if let (Some(a), Some(b)) = (n(10), n(11)) {
            out.push(a..b);
        } else if whole == "all" {
            out.push(0..total);
        } else if whole == "fm" {
            out.push(0..line(5).2);
        } else if whole == "end" {
            out.push(total..total);
        }
    }
    out
}

/// The facts a row names (see the rules).
fn facts(fields: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let object = Regex::new(r"(thread|comment):([A-Za-z0-9]+)(?:\{([^}]*)\})?").unwrap();
    let word = Regex::new(r"\b(eol|bom|files)\b").unwrap();
    for name in ["reads", "checks", "assigns", "state"] {
        let text = fields.get(name).map_or("", String::as_str);
        for found in word.captures_iter(text) {
            out.insert(found[1].to_owned());
        }
        for found in object.captures_iter(text) {
            let id = format!("{}:{}", &found[1], &found[2]);
            match found.get(3) {
                Some(list) => {
                    for field in list.as_str().split(',').map(str::trim).filter(|field| !field.is_empty()) {
                        out.insert(if field == "type" { format!("{id}.*") } else { format!("{id}.{field}") });
                    }
                }
                None => {
                    out.insert(format!("{id}.*"));
                }
            }
        }
    }
    let state = fields.get("state").map_or("", String::as_str);
    if Regex::new(r"\b(selection|caret|sel)\b").unwrap().is_match(state) {
        out.insert("selection".to_owned());
    }
    out
}

/// What the declaration holds on the fixture for `command` with `params`: text and facts.
fn declared_of(command: &str, params: &Value) -> (Vec<Range<usize>>, BTreeSet<String>) {
    let mut doc = NoteDocument::open("fixture.md", FIXTURE.as_bytes()).unwrap();
    if matches!(command, "edit.undo" | "edit.redo") {
        doc.execute_as("task.toggle", json!({ "line": 19 }), &Origin::agent("agent-me")).unwrap();
        if command == "edit.redo" {
            doc.execute_as("edit.undo", json!({}), &Origin::agent("agent-me")).unwrap();
        }
    }
    let total = FIXTURE.chars().count();
    let (mut text, mut facts) = (Vec::new(), BTreeSet::new());
    let Some(under) = doc.under(command, params).ok().flatten() else { return (text, facts) };
    for premise in &under.premises {
        let id = premise.id.as_str().map_or_else(|| premise.id.to_string(), str::to_owned);
        if id == "note" {
            match &premise.fields {
                None => {
                    text.push(0..total);
                    facts.extend(["eol".to_owned(), "bom".to_owned()]);
                }
                Some(fields) => {
                    for field in fields {
                        match field.as_str() {
                            "body" => text.push(0..total),
                            other => {
                                facts.insert(other.to_owned());
                            }
                        }
                    }
                }
            }
        } else if id.starts_with("thread:") || id.starts_with("comment:") {
            facts.insert(format!("{id}.*"));
            match &premise.fields {
                None => {
                    facts.insert(format!("{id}.**"));
                }
                Some(fields) => facts.extend(fields.iter().map(|field| format!("{id}.{field}"))),
            }
        }
    }
    if under.covers.is_some() || !under.content.is_empty() || under.objects.iter().any(|object| object == "note") {
        text.extend(footprint_of(command, params, FIXTURE).unwrap_or_default());
    }
    (text, facts)
}

fn holds_point(ranges: &[Range<usize>], at: usize) -> bool {
    ranges.iter().any(|range| range.start <= at && at <= range.end)
}

fn holds_char(ranges: &[Range<usize>], at: usize) -> bool {
    ranges.iter().any(|range| range.start <= at && at < range.end)
}

/// The characters (and points) of `of` that `by` does not hold, as ranges.
fn outside(of: &[Range<usize>], by: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    for range in of {
        if range.is_empty() {
            if !holds_point(by, range.start) {
                out.push(range.clone());
            }
            continue;
        }
        for at in range.clone() {
            if !holds_char(by, at) {
                match out.last_mut() {
                    Some(last) if last.end == at && last.start < last.end => last.end = at + 1,
                    _ => out.push(at..at + 1),
                }
            }
        }
    }
    out
}

/// A fact declared is held by a row's fact when they name the same thing (`thread:A.**`, the
/// whole thread, holds each of its fields).
fn fact_held(fact: &str, by: &BTreeSet<String>) -> bool {
    by.contains(fact) || fact.rsplit_once('.').is_some_and(|(id, _)| by.contains(&format!("{id}.**")))
}

/// Where `ranges` are, as lines and columns.
fn spelled(ranges: &[Range<usize>]) -> String {
    let all = lines();
    let at = |c: usize| -> String {
        let n = all.iter().position(|(start, _, with_break)| c >= *start && c < *with_break).unwrap_or(all.len());
        let start = all.get(n).map_or(FIXTURE.chars().count(), |line| line.0);
        format!("L{}:{}", n + 1, c - start)
    };
    ranges
        .iter()
        .map(|range| if range.is_empty() { at(range.start) } else { format!("{}..{}", at(range.start), at(range.end - 1)) })
        .collect::<Vec<_>>()
        .join(",")
}

/// One row's differences: (held less, held more), in words.
fn differences(command: &str, fields: &BTreeMap<String, String>) -> Result<(Vec<String>, Vec<String>), String> {
    let params: Value =
        serde_json::from_str(fields["params"].split(" on ").next().unwrap_or_default()).map_err(|_| "params are not JSON".to_owned())?;
    let mut text = Vec::new();
    for name in ["reads", "checks", "assigns"] {
        text.extend(regions(&fields[name]));
    }
    let mut named = facts(fields);
    if ["B", "C", "D"].contains(&fields["reader"].as_str()) && !regions(&fields["assigns"]).is_empty() {
        named.insert("eol".to_owned());
    }
    let (held, declared) = declared_of(command, &params);
    let mut less: Vec<String> = Vec::new();
    let closed: Vec<Range<usize>> = held.iter().map(|range| range.start..range.end + 1).collect();
    let not_held = outside(&text, &closed);
    if !not_held.is_empty() {
        less.push(format!("text {}", spelled(&not_held)));
    }
    less.extend(named.iter().filter(|fact| !declared.contains(*fact) && !fact_held(fact, &declared)).map(|fact| format!("fact {fact}")));
    let mut more = Vec::new();
    let extra = outside(&held, &text);
    if !extra.is_empty() {
        more.push(format!("text {}", spelled(&extra)));
    }
    more.extend(declared.iter().filter(|fact| !fact.ends_with(".**") && !named.contains(*fact)).map(|fact| format!("fact {fact}")));
    Ok((less, more))
}

/// The commands offered to agents whose declarations hold less than their rows: why each is no
/// write the guard lets land wrongly, and the behaviour test that shows it
/// (`bindings/headless/tests/independent.rs`). The comparison holds only text ranges; the kit also
/// works a write out again on the text as it is now and compares where it lands
/// (`premise::classify`), which no range shows.
const REVIEWED: &[(&str, &str)] = &[
    (
        "block.append",
        "the section's end: where it lands among the headings (a_section_rests_on_the_heading_that_ends_it); the note's last line only decides a blank line",
    ),
    ("block.replace", "the whole note is searched for the block id: another block given the id makes the write fail"),
    ("block.setId", "the whole note is searched for the new id: another block given it makes the write fail"),
    ("comment.add", "the matches before the quoted occurrence: the write worked out again lands elsewhere (a_quote_rests_on_the_matches_it_counts)"),
    ("comment.reply", "a reply is added at the end of the thread's comments, whatever others added"),
    ("edit.redo", "the person's selection is written, not read; the step's own holds come from its footprint"),
    ("edit.undo", "the person's selection is written, not read; the step's own holds come from its footprint"),
    ("link.update", "the links before it on its line: worked out again it names another link (links_rest_on_the_links_they_count)"),
    ("property.delete", "the front matter as a whole: broken, the write is worked out elsewhere (a_property_rests_on_the_front_matter_it_is_in)"),
    ("property.rename", "the front matter as a whole: broken, the write is worked out elsewhere (a_property_rests_on_the_front_matter_it_is_in)"),
    ("property.set", "the front matter as a whole: broken, the write is worked out elsewhere (a_property_rests_on_the_front_matter_it_is_in)"),
    ("section.delete", "the heading that ends the section: its level is where it lands (a_section_rests_on_the_heading_that_ends_it)"),
    ("section.insert", "the heading that ends the section and the blank lines around the place, which only decide separators"),
    ("section.move", "the headings that end both sections: where they land (a_section_rests_on_the_heading_that_ends_it)"),
    (
        "section.replace",
        "the heading path searched among all headings, and the heading that ends the section (a_section_rests_on_the_heading_that_ends_it)",
    ),
    ("table.create", "the other tables, read for the number it answers, and the last line, which decides a blank line"),
    ("table.moveRow", "the header's line ending, read only when the moved row has none"),
    ("task.toggle", "a block id on a line with no task: the write fails"),
    ("text.delete", "UTF-16 units before the position: an edit of their count moves it (a_utf16_position_rests_on_the_units_before_it)"),
    ("text.findReplace", "the matches it counts: worked out again it lands elsewhere (a_find_and_replace_rests_on_the_matches_it_counts)"),
    ("text.insert", "UTF-16 units before the position: an edit of their count moves it (a_utf16_position_rests_on_the_units_before_it)"),
    ("text.replace", "UTF-16 units before the position: an edit of their count moves it (a_utf16_position_rests_on_the_units_before_it)"),
];

#[test]
fn the_declarations_compared_with_the_readings() {
    let mut by_command: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    let mut report = Vec::new();
    for (command, list) in rows() {
        let fields: BTreeMap<String, String> = list.into_iter().collect();
        let entry = by_command.entry(command.clone()).or_default();
        entry.0 += 1;
        match differences(&command, &fields) {
            Err(why) => {
                entry.3 += 1;
                report.push(format!("UNCOMPARED {command} {} : {why}", fields["params"]));
            }
            Ok((less, more)) => {
                if !less.is_empty() {
                    entry.1 += 1;
                    report.push(format!("LESS {command} {} : {}", fields["params"], less.join("; ")));
                }
                if !more.is_empty() {
                    entry.2 += 1;
                    report.push(format!("MORE {command} {} : {}", fields["params"], more.join("; ")));
                }
            }
        }
    }
    let differing = by_command.values().filter(|(_, less, more, _)| *less + *more > 0).count();
    let less = by_command.values().filter(|(_, less, _, _)| *less > 0).count();
    let more = by_command.values().filter(|(_, _, more, _)| *more > 0).count();
    for line in &report {
        println!("{line}");
    }
    println!("commands {} differing {differing} held-less {less} held-more {more}", by_command.len());
    let offered = |command: &str| SPECS.iter().any(|spec| spec.id == command && spec.agent);
    type Counts = (usize, usize, usize, usize);
    let agents: Vec<(&String, &Counts)> = by_command.iter().filter(|(command, _)| offered(command)).collect();
    println!(
        "offered to agents {} differing {} held-less {} held-more {}",
        agents.len(),
        agents.iter().filter(|(_, (_, less, more, _))| *less + *more > 0).count(),
        agents.iter().filter(|(_, (_, less, _, _))| *less > 0).count(),
        agents.iter().filter(|(_, (_, _, more, _))| *more > 0).count()
    );
    let less: Vec<&str> = agents.iter().filter(|(_, (_, less, _, _))| *less > 0).map(|(command, _)| command.as_str()).collect();
    let reviewed: Vec<&str> = REVIEWED.iter().map(|(command, _)| *command).collect();
    assert_eq!(less, reviewed, "a command offered to agents holds less than its rows: explain it in REVIEWED or widen its declaration");
}
