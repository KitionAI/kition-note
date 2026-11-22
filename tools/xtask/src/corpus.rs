//! `cargo xtask corpus` and `cargo xtask round-trip`: the round-trip suite (`docs/features/round-trip.md`).
//!
//! The corpus is real, public Markdown files (`tools/xtask/corpus.json`), fetched by commit into
//! `corpus/` (not tracked). The suite
//! opens each through `NoteDocument`, saves it without an edit (must be byte-identical), and makes
//! one-place edits that must change exactly one line of the saved file. The synthetic edges
//! (`kition_note_kdm::file::bench::EDGE_FILES`) and an invalid UTF-8 file run with it.

use std::path::PathBuf;
use std::process::Command;

use kition_kit::KitDocument;
use kition_note_kdm::file::bench::{EDGE_FILES, INVALID_UTF8_FILE};
use kition_note_kit::NoteDocument;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::root;

struct Entry {
    name: String,
    url: String,
    sha256: String,
}

fn entries() -> Result<Vec<Entry>, String> {
    let path = root().join("tools").join("xtask").join("corpus.json");
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let manifest: Value = serde_json::from_str(&text).map_err(|error| format!("corpus.json: {error}"))?;
    let files = manifest.get("files").and_then(Value::as_array).ok_or("corpus.json has no files")?;
    files
        .iter()
        .map(|file| {
            let field = |key: &str| file.get(key).and_then(Value::as_str).map(str::to_owned);
            Ok(Entry {
                name: field("name").ok_or("a corpus entry has no name")?,
                url: field("url").ok_or("a corpus entry has no url")?,
                sha256: field("sha256").ok_or("a corpus entry has no sha256")?,
            })
        })
        .collect()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn corpus_dir() -> PathBuf {
    root().join("corpus")
}

/// Fetches the corpus files by commit.
pub fn corpus() -> Result<(), String> {
    let dir = corpus_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries()? {
        let target = dir.join(&entry.name);
        if std::fs::read(&target).is_ok_and(|bytes| sha256(&bytes) == entry.sha256) {
            continue;
        }
        let url = &entry.url;
        let status = Command::new("curl").args(["-fsSL", "-o"]).arg(&target).arg(url).status().map_err(|error| format!("curl: {error}"))?;
        if !status.success() {
            return Err(format!("curl {url}: {status}"));
        }
        let bytes = std::fs::read(&target).map_err(|error| format!("{}: {error}", target.display()))?;
        if sha256(&bytes) != entry.sha256 {
            return Err(format!("{}: the hash differs from corpus.json", entry.name));
        }
    }
    println!("corpus: all files in {}", dir.display());
    Ok(())
}

/// Lines of a file's bytes, for counting what an edit changed.
fn lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split(|&byte| byte == b'\n').collect()
}

/// Whether `after` differs from `before` in exactly one line (`replaced`) or by exactly one added
/// line (`!replaced`).
fn one_line(before: &[u8], after: &[u8], replaced: bool) -> bool {
    let (a, b) = (lines(before), lines(after));
    if replaced {
        a.len() == b.len() && a.iter().zip(&b).filter(|(x, y)| x != y).count() == 1
    } else {
        let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        b.len() == a.len() + 1 && a.get(prefix..) == b.get(prefix + 1..)
    }
}

/// Where the one-place edits go: scalar offsets of line ends in the note's text, outside
/// frontmatter and fenced code.
struct Places {
    heading_end: Option<usize>,
    paragraph_end: Option<usize>,
    list_item_end: Option<usize>,
}

fn places(text: &str) -> Places {
    let mut found = Places { heading_end: None, paragraph_end: None, list_item_end: None };
    let total = text.lines().count();
    let mut offset = 0usize;
    let mut fence: Option<&str> = None;
    let mut in_frontmatter = text.starts_with("---\n");
    for (index, line) in text.split('\n').enumerate() {
        let end = offset + line.chars().count();
        let trimmed = line.trim_start();
        if in_frontmatter {
            if index > 0 && (line == "---" || line == "...") {
                in_frontmatter = false;
            }
        } else if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(if trimmed.starts_with("```") { "```" } else { "~~~" });
        } else if line.starts_with('#') && line.chars().nth(1).is_some_and(|c| c == ' ' || c == '#') {
            found.heading_end.get_or_insert(end);
        } else if (line.starts_with("- ") || line.starts_with("* ")) && !line.starts_with("- [") && found.list_item_end.is_none() {
            found.list_item_end = Some(end);
        } else if index >= total / 2 && found.paragraph_end.is_none() && line.chars().next().is_some_and(char::is_alphabetic) && !line.contains('|') {
            found.paragraph_end = Some(end);
        }
        offset = end + 1;
    }
    found
}

#[derive(Default)]
struct Tally {
    done: usize,
    clean: usize,
    failed: Vec<String>,
}

impl Tally {
    fn add(&mut self, ok: bool, what: String) {
        self.done += 1;
        if ok {
            self.clean += 1;
        } else {
            self.failed.push(what);
        }
    }
}

/// Inserts `text` at `at` in a fresh copy of the note and answers the saved bytes.
fn edited(name: &str, bytes: &[u8], at: usize, text: &str) -> Result<Vec<u8>, String> {
    let mut doc = NoteDocument::open(name, bytes)?;
    doc.execute("text.insert", json!({ "at": at, "text": text }))?;
    doc.save(None)
}

/// Runs `command` on a fresh copy of the note and answers the saved bytes.
fn commanded(name: &str, bytes: &[u8], command: &str, params: Value) -> Result<Vec<u8>, String> {
    let mut doc = NoteDocument::open(name, bytes)?;
    doc.execute(command, params).map_err(|error| format!("{name}: {command}: {error}"))?;
    doc.save(None)
}

/// The first body cell of the first table that has a body row: `(table, text)`.
fn first_cell(doc: &mut NoteDocument) -> Result<Option<(usize, String)>, String> {
    let tables = doc.execute("table.read", json!({}))?;
    for entry in tables.get("list").and_then(Value::as_array).into_iter().flatten() {
        let number = entry.get("table").and_then(Value::as_u64).unwrap_or(0);
        let table = doc.execute("table.read", json!({ "table": number }))?;
        if let Some(cell) = table.pointer("/rows/0/0").and_then(Value::as_str) {
            return Ok(Some((usize::try_from(number).unwrap_or(0), cell.to_owned())));
        }
    }
    Ok(None)
}

/// The first property of `kind` (`string` or `list`) the engine may rewrite, with its value.
fn first_property(doc: &mut NoteDocument, kind: &str) -> Result<Option<(String, Value)>, String> {
    let listed = doc.execute("property.list", json!({}))?;
    Ok(listed.get("properties").and_then(Value::as_array).into_iter().flatten().find_map(|property| {
        let editable = property.get("complex").is_none() && property.get("type").and_then(Value::as_str) == Some(kind);
        editable.then(|| {
            (property.get("key").and_then(Value::as_str).unwrap_or_default().to_owned(), property.get("value").cloned().unwrap_or(Value::Null))
        })
    }))
}

/// Runs the suite: the corpus must be complete (`cargo xtask corpus`).
pub fn round_trip() -> Result<(), String> {
    let mut round_trip = Tally::default();
    let (mut paragraph, mut heading, mut list) = (Tally::default(), Tally::default(), Tally::default());
    let (mut cell, mut property, mut list_property) = (Tally::default(), Tally::default(), Tally::default());
    let mut exports = Tally::default();
    let mut gaps = Gaps::default();
    for entry in entries()? {
        let path = corpus_dir().join(&entry.name);
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error} (run `cargo xtask corpus` first)", path.display()))?;
        if sha256(&bytes) != entry.sha256 {
            return Err(format!("{}: the hash differs from corpus.json (run `cargo xtask corpus`)", entry.name));
        }
        let mut doc = NoteDocument::open(&entry.name, &bytes)?;
        round_trip.add(doc.save(None)? == bytes, entry.name.clone());
        exports.add(gaps.export(&mut doc)?, entry.name.clone());
        let found = places(doc.text());
        if let Some(at) = found.paragraph_end {
            paragraph.add(one_line(&bytes, &edited(&entry.name, &bytes, at, " probe")?, true), entry.name.clone());
        }
        if let Some(at) = found.heading_end {
            heading.add(one_line(&bytes, &edited(&entry.name, &bytes, at, " probe")?, true), entry.name.clone());
        }
        if let Some(at) = found.list_item_end {
            list.add(one_line(&bytes, &edited(&entry.name, &bytes, at, "\n- probe")?, false), entry.name.clone());
        }
        let mut probe = NoteDocument::open(&entry.name, &bytes)?;
        if let Some((table, text)) = first_cell(&mut probe)? {
            let new = if text.is_empty() { "probe".to_owned() } else { format!("{text} probe") };
            let after =
                commanded(&entry.name, &bytes, "table.setCell", json!({ "table": table, "row": 1, "column": 0, "text": new, "expected": text }))?;
            cell.add(one_line(&bytes, &after, true), entry.name.clone());
        }
        if let Some((key, value)) = first_property(&mut probe, "string")? {
            let after = commanded(
                &entry.name,
                &bytes,
                "property.set",
                json!({ "key": key, "value": format!("{} probe", value.as_str().unwrap_or_default()) }),
            )?;
            // It must also read back as the new value.
            let reread = NoteDocument::open(&entry.name, &after)?.execute("property.list", json!({}))?;
            let back = reread
                .get("properties")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|found| found.get("key").and_then(Value::as_str) == Some(key.as_str()))
                .and_then(|found| found.get("value"))
                .cloned();
            let wanted = json!(format!("{} probe", value.as_str().unwrap_or_default()));
            property.add(one_line(&bytes, &after, true) && back == Some(wanted), entry.name.clone());
        }
        if let Some((key, Value::Array(mut items))) = first_property(&mut probe, "list")? {
            items.push(json!("probe"));
            let after = commanded(&entry.name, &bytes, "property.set", json!({ "key": key, "value": items }))?;
            list_property.add(one_line(&bytes, &after, false) || one_line(&bytes, &after, true), entry.name.clone());
        }
    }
    let mut edges = Tally::default();
    for (label, bytes) in EDGE_FILES {
        let doc = NoteDocument::open("edge.md", bytes)?;
        edges.add(doc.save(None)? == *bytes, (*label).to_owned());
        if !bytes.is_empty() {
            // One character typed at the start changes only the first line.
            let after = edited("edge.md", bytes, 0, "x")?;
            edges.add(one_line(bytes, &after, true), format!("{label}: edit"));
        }
    }
    let invalid = NoteDocument::open("broken.md", INVALID_UTF8_FILE)?;
    edges.add(invalid.is_read_only() && invalid.save(None)? == INVALID_UTF8_FILE, "invalid UTF-8".to_owned());

    println!("Round trip (corpus: {} files)\n", round_trip.done);
    println!("| Check | Clean |");
    println!("|---|---|");
    for (label, tally) in [
        ("Open and save, no edit (byte-identical)", &round_trip),
        ("One paragraph edit: one line changed", &paragraph),
        ("One heading edit: one line changed", &heading),
        ("One list item added: one line added", &list),
        ("table.setCell on the first body cell: one line changed", &cell),
        ("property.set on the first string property: one line changed, reads back", &property),
        ("property.set adding an item to the first list property: one line", &list_property),
        ("Synthetic edges (round trip, one edit) and invalid UTF-8", &edges),
        ("Exports: a well-formed PDF (letter) and HTML file, unchanged note", &exports),
    ] {
        println!("| {label} | {}/{} |", tally.clean, tally.done);
    }
    gaps.print();
    let failed: Vec<String> = [&round_trip, &paragraph, &heading, &list, &cell, &property, &list_property, &edges, &exports]
        .iter()
        .flat_map(|tally| tally.failed.clone())
        .collect();
    let listed = entries()?.len();
    if round_trip.done != listed {
        return Err(format!("the corpus has {} files, not {listed}", round_trip.done));
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("round-trip: not clean: {}", failed.join(", "))) }
}

/// What the corpus's exports could not draw (`docs/design.md` 6.1), summed.
#[derive(Default)]
struct Gaps {
    pages: u64,
    files_missing_glyphs: u64,
    missing_glyphs: u64,
    images: u64,
    math: u64,
    mermaid: u64,
    html: u64,
}

impl Gaps {
    /// Exports `doc` as PDF and HTML; answers whether both are well formed and the note is unchanged.
    fn export(&mut self, doc: &mut NoteDocument) -> Result<bool, String> {
        let before = doc.text().to_owned();
        let pdf = doc.export_pdf("letter")?;
        let html = doc.export_html(true);
        let answer = doc.execute("file.export", json!({ "format": "pdf" }))?;
        let count = |key: &str| answer.get("unsupported").and_then(|gaps| gaps.get(key)).and_then(Value::as_u64).unwrap_or(0);
        self.pages += answer.get("pages").and_then(Value::as_u64).unwrap_or(0);
        self.missing_glyphs += count("missingGlyphs");
        self.files_missing_glyphs += u64::from(count("missingGlyphs") > 0);
        self.images += count("images") + count("embeds");
        self.math += count("math");
        self.mermaid += count("mermaid");
        self.html += count("html");
        Ok(pdf.starts_with(b"%PDF-1.4\n")
            && pdf.ends_with(b"%%EOF\n")
            && html.starts_with("<!doctype html>")
            && html.ends_with("</html>\n")
            && doc.text() == before
            && !doc.dirty())
    }

    fn print(&self) {
        println!(
            "\nExports drew {} pages. Not drawn as they are: {} glyphs without a page font in {} files (boxes), {} images and embeds (labels), {} math spans, {} Mermaid diagrams and {} HTML blocks (source).",
            self.pages, self.missing_glyphs, self.files_missing_glyphs, self.images, self.math, self.mermaid, self.html
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_tells_replacements_and_additions() {
        assert!(one_line(b"a\nb\nc\n", b"a\nB\nc\n", true));
        assert!(!one_line(b"a\nb\nc\n", b"A\nB\nc\n", true));
        assert!(one_line(b"a\nb\n", b"a\nx\nb\n", false));
        assert!(!one_line(b"a\nb\n", b"a\nx\ny\nb\n", false));
    }

    #[test]
    fn places_skip_frontmatter_and_code() {
        let text = "---\ntitle: x\n---\n# Title\n```\n# not a heading\n```\n- item\n\nA paragraph here.\n";
        let found = places(text);
        let at = |line: usize| text.split('\n').take(line + 1).map(|line| line.chars().count() + 1).sum::<usize>() - 1;
        assert_eq!(found.heading_end, Some(at(3)));
        assert_eq!(found.list_item_end, Some(at(7)));
        assert_eq!(found.paragraph_end, Some(at(9)));
    }
}
