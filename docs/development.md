# Developing Kition Note

How to build Note from source, run its tests, and find your way around the repository. To use
Note in your own app, start with the [README](../README.md).

## Repository layout

| Path | What |
|---|---|
| `crates/` | The engine (Rust), in groups: `kdm`, `engine`, `formats`, `view` ([Layout](#layout) lists each crate) |
| `bindings/` | The engine's two outlets: `wasm` (the editor's engine) and `headless` (the `kition-note` executable) |
| `web/` | The kit's editor (React, on kition-sdk's `@kition/kit-editor`) |
| `agent/` | The agent guide and the check catalogue |
| `conformance/` | One fixture per check code, and the agent conformance fixtures |
| `cases/` | The test cases of the feature catalogue (`features.yaml`, beside `kit.json`, the kit manifest) |
| `docs/` | This file, the design, the writing format, gaps, roadmap, the quality reference, attribution; `features/` (the golden suite, performance), `parity/` |
| `assets/` | `readme/`: the pictures in the README |
| `tools/` | `xtask`: `cargo xtask ci`, `wasm`, `manifest`, `corpus`, `golden`, `perf` |

## Quick start

Requirements and fonts are below. On a clean clone:

```sh
scripts/fetch-fonts.sh ~/kition-note-fonts         # see "Fonts"
export KITION_FONTS_DIR=~/kition-note-fonts

# The headless kit, and the note tools for any MCP client
cargo build --release -p kition-note               # target/release/kition-note
cargo install --git https://github.com/KitionAI/kition-sdk kition-kits --bin kition-kits-mcp
kition-kits-mcp --workspace <folder of notes> --kit target/release/kition-note

# The editor, in a development host
cd web
pnpm install
pnpm run engine                                    # the engine (WebAssembly) and fonts into public/
pnpm dev                                           # http://localhost:5190/dev.html
```

## Requirements

- Rust 1.95 or newer, with the `wasm32-unknown-unknown` target for the page's engine.
- [wasm-pack](https://rustwasm.github.io/wasm-pack/) to build the engine for the editor. Some
  dependencies compile C for WebAssembly; on macOS use Homebrew's `llvm` (the build script finds
  it).
- A current Node.js (22 or newer) and pnpm 10 for the editor.
- The four fonts the engine draws text with, in a folder named by `KITION_FONTS_DIR` (see
  [Fonts](#fonts)). They are not in this repository; the headless build and the editor's engine
  build stop with an error naming the missing file until they are there.

The Rust crates of the [Kition SDK](https://github.com/KitionAI/kition-sdk) and Kition Tessera are
git dependencies pinned by commit in `Cargo.toml`; Cargo fetches them.

## Fonts

`scripts/fetch-fonts.sh <folder>` downloads each file below, with its license, from its upstream
source into `<folder>`, laid out as shown, and checks its SHA-256; then set `KITION_FONTS_DIR` to
that folder (the default is a `kition-fonts` folder beside this repository, the maintainers'
private font checkout, which has the same layout and the same files). `bindings/headless/build.rs` builds the faces into `kition-note`, and
`web/scripts/build-engine.sh` copies them, with their license files, into the editor's package.

| Path under `$KITION_FONTS_DIR` | Font | License | Download |
|---|---|---|---|
| `fonts/inter/Inter-Variable.ttf` | Inter (prose) | OFL-1.1, `fonts/inter/OFL.txt` | <https://github.com/google/fonts/raw/45e3d5269e4a17d823f6d84470bc084dc5ae46f6/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf>, license `ofl/inter/OFL.txt` at the same commit |
| `fonts/jetbrains-mono/JetBrainsMono-Variable.ttf` | JetBrains Mono (code) | OFL-1.1, `fonts/jetbrains-mono/OFL.txt` | <https://github.com/google/fonts/raw/45e3d5269e4a17d823f6d84470bc084dc5ae46f6/ofl/jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf>, license `ofl/jetbrainsmono/OFL.txt` at the same commit |
| `fonts/droid-sans-fallback/DroidSansFallbackFull.ttf` | Droid Sans Fallback (Chinese and Japanese) | Apache-2.0, `fonts/droid-sans-fallback/NOTICE` | <https://android.googlesource.com/platform/frameworks/base/+/cac6285b48e88e118f69992d5c97d56163b6274c/data/fonts/DroidSansFallbackFull.ttf>, license: the `NOTICE` file in the same folder |
| `fonts/noto-color-emoji/Noto-COLRv1.ttf` | Noto Color Emoji (emoji, COLRv1) | OFL-1.1, `fonts/noto-color-emoji/OFL.txt` | <https://github.com/googlefonts/noto-emoji/raw/1ffdd21391dd1f25c081fa93a9dea0c7c029442b/2D/fonts/Noto-COLRv1.ttf>, license `2D/fonts/LICENSE` at the same commit |

The files are renamed to the paths in the first column. Their SHA-256 sums:

```text
29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031  fonts/inter/Inter-Variable.ttf
48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda  fonts/jetbrains-mono/JetBrainsMono-Variable.ttf
acb6440a713d880a13a21b468ba7cd43f5a2b2934972e51be791c880730777b8  fonts/droid-sans-fallback/DroidSansFallbackFull.ttf
b8e25ea68db82f9e4d0aee921f4420be2be39887bd5c893a2ad98710531f9d0c  fonts/noto-color-emoji/Noto-COLRv1.ttf
```

The math face (STIX Two Math) is in the repository, in the vendored latex-rust
(`vendor/latex-rust/fonts/stix-two-math/`).

## Build the engine and run the tests

```sh
cargo build --release -p kition-note   # the headless kit: target/release/kition-note
cargo xtask corpus                     # fetch the public round-trip corpus into corpus/ (docs/features/golden.md)
cargo test                             # every crate's tests, including kition-sdk's contract check
cargo xtask ci                         # fmt, clippy -D warnings, tests, the WebAssembly clippy
cargo xtask golden                     # byte-identical round trip and one-place edits on the corpus
cargo xtask perf                       # timings on 1 MB (docs/features/performance.md)
```

Some tests read every file in `corpus/`, so fetch it once before `cargo test`. Building
`kition-note` needs the fonts ([Fonts](#fonts)). `cargo xtask manifest` checks `kit.json` with
kition-sdk's `tools/check-manifest.mjs`; it needs a kition-sdk checkout beside this repository (or
`KITION_SDK_DIR`).

Read [CONTRIBUTING.md](../.github/CONTRIBUTING.md), [AGENTS.md](../AGENTS.md) and
[quality.md](quality.md) before a change: tests are the spec, every bug fix starts with a failing
case under `cases/regressions/`, and every feature has an entry in `features.yaml`. `cargo xtask ci`
is the check every change passes.

## Run the headless kit for an agent

`kition-note` serves one note at a time over the Kit Protocol; kition-sdk's `kition-kits-mcp`
turns kits into MCP tools (`note_inspect`, `note_run`, `note_check`, …) on a workspace folder for
any MCP client:

```sh
cargo build --release -p kition-note
cargo install --git https://github.com/KitionAI/kition-sdk kition-kits --bin kition-kits-mcp
kition-kits-mcp --workspace <folder> --kit target/release/kition-note
```

`--kit` takes the executable; the server reads the `kit.json` nearest above it, this
repository's. How an agent should use the tools is in [agent/guide.md](../agent/guide.md); the
commands and their parameters are in [writing-format.md](writing-format.md) and
[design.md](design.md).

## Build and run the editor

```sh
cd web
pnpm install
pnpm run engine      # builds bindings/wasm into web/public/engine and copies the fonts
pnpm dev             # http://localhost:5190/dev.html: the editor in a development host
pnpm test            # unit tests (Vitest)
pnpm build           # type check and the package in web/dist
pnpm run test:flow   # browser flows in Chromium and WebKit (Playwright; run with no HTTP proxy set)
```

The editor installs the published `@kition/*` packages of kition-sdk and Kition Tessera from npm
and the committed lockfile, so it needs no other checkout. To work against local checkouts
instead, clone `kition-sdk` and `kition-tessera` next to this repository (or set `KITION_SDK` and
`KITION_TESSERA`), run `pnpm install` in each of them, then run `KITION_LINK=1 pnpm install` here:
`web/.pnpmfile.cjs` links their packages, and the Vite configuration reads kition-sdk's package
sources in place. The linked install rewrites `web/pnpm-lock.yaml`; do not commit that.

The README's screenshots are taken by [`web/scripts/readme-shots.mjs`](../web/scripts/readme-shots.mjs)
from the development host; it is a tool, not a test. `assets/readme/made-1.png` … `made-4.png` are
the `thumb.png` of four kition-showcase samples (`note/mandarin-study-notes`,
`note/mechanics-revision-formulas`, `note/parcel-tracking-api-reference`,
`note/sourdough-starter-guide`), copied as they are.

## Publish

Kition ships the kit's package. Publish it from a Kition checkout with
`scripts/publish-kit.sh note` (see `docs/kits.md` there).

## Native hosts

`bindings/wasm` also builds as a native Rust library. Depend on this kit's SDK crate and
import its re-exported `KitDocument` contract; a host does not need to match a separate
`kition-sdk` Git revision.
The SDK constrains the Loro facade and internal implementation to their compatible versions,
because a consuming workspace does not inherit the kit's lockfile.

```rust,ignore
use kition_note_kit::{KitDocument, NoteDocument};
let document = NoteDocument::open("person.md", b"# Person\n")?;
let bytes = document.save(None)?;
```

## Layout

| Path | What |
|---|---|
| `kit.json` | The kit manifest (KHP 1.0 and Kition platform fields) |
| `crates/kdm/kdm-model` | `kition-note-kdm`: the schema, the materialised view, comment repair, merges |
| `crates/formats/markdown-file` | `kition-note-markdown-file`: bytes to text and back (BOM, CRLF, mixed endings), the external-change merge |
| `crates/engine/markdown` | `kition-note-markdown`: the source-positioned Markdown index (pulldown-cmark and an Obsidian scan) and the property block |
| `crates/engine/edit` | `kition-note-edit`: minimal-span edits for properties, tables, sections, blocks, tasks, links and find and replace |
| `crates/engine/check` | `kition-note-check`: `note_check`, diagnostics with fix hints |
| `crates/view/*` | The engine's own view: layout, painting, fonts, code highlighting |
| `crates/formats/export` | HTML, PDF and picture export |
| `bindings/wasm` | `kition-note-kit`: `NoteDocument` (`KitDocument`, contract 2 session members, extras) |
| `bindings/headless` | `kition-note`: the same document over the Kit Protocol |
| `web/` | The kit's editor on `@kition/kit-editor`, relayed to the engine in a Worker |
| `assets/readme/` | The pictures in the README |
| `agent/` | The agent guide and the check catalogue |
| `cases/`, `features.yaml`, `conformance/` | The feature catalogue, its test cases, and one fixture per check code |
| `tools/xtask/` | `cargo xtask ci`, `wasm`, `manifest`, `corpus`, `golden`, `perf` |
| `docs/` | Design (`design.md`), the writing format; the golden suite and performance in `docs/features/` |

## How it fits together

- **One engine.** The engine is Rust. It implements the Kition SDK's `KitDocument` contract on
  the SDK's Kit Data Model: a note is one `Note` object whose `body` is the whole `.md` file
  (`crates/kdm/kdm-model/model/schema.json`, schema `kit.note` v1).
- **In the page.** The editor (`web/`) is built on the SDK's `@kition/kit-editor`. The engine
  runs as WebAssembly in a Web Worker and draws the note; hosts embed the editor in a sandboxed
  frame over the Kit UI protocol.
- **Headless.** `kition-note` speaks the Kit Protocol of the
  [Kit Harness Protocol (KHP)](https://github.com/kitharness/khp); the SDK's
  `kition-kits-mcp` serves it to agents, and its `@kition/script` drives it from code.
- **Comments** are objects of the replica the host stores, beside the file.
