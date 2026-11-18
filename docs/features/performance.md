# Performance

`cargo xtask perf` measures the note on about 1 MB of generated Markdown (headings, prose with CJK
and emoji, lists, GFM tables, code) in a release build, and prints the median of 5 to 21 runs of
each measurement as a table:

| Measurement | What it times |
|---|---|
| Open the file | Decode, a new replica (the text inserted in 16 KB pieces), page layout |
| Save the file | Export of the stored text |
| Page layout | Reading and laying out every page, what a host's `height` costs after a change |
| First render after a change | The pixel index of every page, then one thumbnail |
| Export PDF / HTML | `file.export` of the whole note, Letter pages for the PDF |
| One keystroke | `text.insert`: the transaction and the view refresh |
| External merge, whole | Merging an external change of the file into a note with unsaved edits: 1 % of the characters changed outside, 0.5 % locally, scattered; diff, merge and the replica transaction |
| The same, diff and merge alone | Without the replica transaction |
| External merge, disjoint scattered edits | One external edit about every 67 characters |
| Single-character inserts in one `system` transaction | 500, 2,000 and 8,000 inserts, to show how a transaction grows with its number of edits |
| The same inserts in transactions of 500 | The same 8,000 inserts split into several transactions |

## External merge budget

Merging an external change of the file into a note with unsaved edits must take at most
**100 ms on 1 MB**, on the merge case above.

## How to read the numbers

- Measure on an idle machine. Other builds running at the same time inflate every number and make
  runs differ by several times; compare only runs made under the same load.
- The numbers are native. The editor runs the engine as WebAssembly in a worker, which is slower.
- The pages are laid out when a host first asks for them after a change (`height`, `render`,
  `inspect`), not on every edit, and their pixel index is built on the first draw. Typing never
  waits for the layout, but a large note pays one layout per relayed change in the worker.
- A replica transaction should cost about linearly in its number of edits: four times the inserts,
  about four times the time. If it grows faster, the cost is in the replication engine
  (kition-sdk's `kition-kdm`), not in the note.
