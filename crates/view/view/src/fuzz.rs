//! The view over random and adversarial Markdown (deterministic seeds): styling, layout, edits
//! (each a fresh `update` over the changed text), carets, hits, selections, and a frame never
//! panic, and every caret and hit stays in the text.

use super::*;

/// A small deterministic generator (xorshift).
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

/// Markdown pieces: block starts, inline marks, and text in several scripts.
const PIECES: &[&str] = &[
    "# ",
    "## ",
    "###### ",
    "> ",
    "> [!note] ",
    "- ",
    "* ",
    "1. ",
    "- [ ] ",
    "- [x] ",
    "    ",
    "\t",
    "```",
    "```rust",
    "```python",
    "```sql",
    "~~~",
    "$$",
    "$",
    "| a | b |",
    "| --- | :-: |",
    "|",
    "---",
    "***",
    "**",
    "*",
    "_",
    "__",
    "~~",
    "==",
    "`",
    "``",
    "[",
    "]",
    "(",
    ")",
    "[[",
    "]]",
    "![[",
    "![",
    "](https://kition.ai)",
    "<b>",
    "</b>",
    "<span style=\"color: red\">",
    "</span>",
    "<!--",
    "-->",
    "&amp;",
    "#tag",
    "[^1]",
    "[^1]: ",
    "^[note]",
    "\\",
    "%%",
    "text",
    "word ",
    "中文",
    "😀",
    "é",
    "\u{200b}",
    "\u{a0}",
    "\r",
    " ",
    "  ",
    "\n",
    "\n\n",
    "\n- ",
    "\n> ",
    "\n```\n",
    "\n|",
    "---\ntitle: x\n---\n",
    "1",
    "<",
    ">",
    "&",
    "\"",
    "'",
];

fn random_text(random: &mut Random, pieces: usize) -> String {
    let mut text = String::new();
    for _ in 0..pieces {
        text.push_str(PIECES[random.below(PIECES.len())]);
    }
    text
}

/// Texts no random draw is likely to make.
fn adversarial() -> Vec<String> {
    let scale = if cfg!(debug_assertions) { 1 } else { 10 };
    vec![
        String::new(),
        "\n".repeat(500 * scale),
        "> ".repeat(200 * scale),
        "- ".repeat(100) + "x",
        "    ".repeat(100) + "- deep",
        "```\n".to_string() + &"code\n".repeat(500 * scale),
        "| a |".repeat(200) + "\n" + &"| - |".repeat(200),
        "*".repeat(2000 * scale),
        "[".repeat(2000 * scale),
        "$".repeat(1001),
        "中".repeat(3000 * scale),
        "😀".repeat(1000 * scale),
        "word ".repeat(4000 * scale),
        "<b>".repeat(500),
        "[[".repeat(500) + &"]]".repeat(500),
        "#".repeat(5000),
        "\r\n".repeat(300),
    ]
}

/// Lays out `text` and walks what the view answers about it.
fn exercise(text: &str) {
    let mut view = View::default();
    view.update(text, 0..text.len());
    view.realize(0.0, 1.0e7);
    let len = text.len();
    let mut at = 0;
    while at <= len {
        let clamped = view.clamp(at);
        assert!(clamped <= len, "clamp past the end");
        let caret = view.caret(clamped);
        assert!(caret.x.is_finite() && caret.top.is_finite(), "a caret off the page at {at}");
        let hit = view.hit(caret.x, caret.top + caret.height / 2.0);
        assert!(hit <= len, "a hit past the end");
        let _ = view.vertical(clamped, caret.x, 1);
        let _ = view.row_edges(clamped);
        // Every character boundary on short texts; a sample on long ones.
        at += if len < 400 { 1 } else { len / 200 + 1 };
        while at < len && !text.is_char_boundary(at) {
            at += 1;
        }
    }
    let _ = view.selection(0, len);
    let _ = view.hit(-50.0, -50.0);
    let _ = view.hit(1.0e6, 1.0e7);
    let _ = view.frame(0.0, 0.0, 1.0, 64, 64);
}

#[test]
fn the_view_takes_any_markdown() {
    if !crate::tests::fonts() {
        return;
    }
    let mut random = Random(0x2545_f491_4f6c_dd1d);
    // `FUZZ_ROUNDS` runs more (and `FUZZ_SEED` another seed) by hand.
    let rounds = std::env::var("FUZZ_ROUNDS").ok().and_then(|rounds| rounds.parse().ok()).unwrap_or(if cfg!(debug_assertions) { 40 } else { 300 });
    if let Some(seed) = std::env::var("FUZZ_SEED").ok().and_then(|seed| seed.parse().ok()) {
        random = Random(seed);
    }
    for round in 0..rounds {
        let pieces = 1 + random.below(60);
        let text = random_text(&mut random, pieces);
        let run = std::panic::catch_unwind(|| exercise(&text));
        assert!(run.is_ok(), "round {round}: the view panicked on {text:?}");
        // An edit in it, styled again from the changed text.
        let mut edited = text.clone();
        let mut cut = random.below(edited.len() + 1);
        while !edited.is_char_boundary(cut) {
            cut -= 1;
        }
        edited.insert_str(cut, PIECES[random.below(PIECES.len())]);
        let run = std::panic::catch_unwind(|| exercise(&edited));
        assert!(run.is_ok(), "round {round}: the view panicked on {edited:?}");
    }
    for text in adversarial() {
        let started = std::time::Instant::now();
        let run = std::panic::catch_unwind(|| exercise(&text));
        let shown: String = text.chars().take(40).collect();
        assert!(run.is_ok(), "the view panicked on {shown:?}… ({} bytes)", text.len());
        assert!(started.elapsed().as_secs() < 30, "{shown:?}… took {:?}", started.elapsed());
    }
}

#[test]
fn styling_takes_any_markdown() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    for _ in 0..2000 {
        let pieces = 1 + random.below(80);
        let text = random_text(&mut random, pieces);
        let styles = style::Styles::of(&text);
        assert_eq!(styles.bytes.len(), text.len(), "a style byte per byte of {text:?}");
    }
    for text in adversarial() {
        let styles = style::Styles::of(&text);
        assert_eq!(styles.bytes.len(), text.len());
    }
}

/// The notes in the repository: the samples, the conformance notes, and the code corpora.
fn repository_notes() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut out = Vec::new();
    for folder in ["conformance", "web/tests/samples", "web/public/sample", "crates/view/view/fixtures"] {
        let Ok(entries) = std::fs::read_dir(root.join(folder)) else { continue };
        let mut paths: Vec<std::path::PathBuf> =
            entries.flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|ext| ext == "md")).collect();
        paths.sort();
        for path in paths {
            if let Ok(bytes) = std::fs::read(&path) {
                out.push(String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    out
}

#[test]
fn one_parse_styles_and_indexes_as_two_do() {
    let notes = repository_notes();
    assert!(notes.len() > 40, "the repository's notes are read ({})", notes.len());
    let mut random = Random(std::env::var("FUZZ_SEED").ok().and_then(|seed| seed.parse().ok()).unwrap_or(0x1dea_5eed));
    let rounds: usize =
        std::env::var("FUZZ_ROUNDS").ok().and_then(|rounds| rounds.parse().ok()).unwrap_or(if cfg!(debug_assertions) { 200 } else { 2000 });
    let mut texts = notes;
    texts.extend(adversarial());
    texts.extend((0..rounds).map(|_| {
        let pieces = 1 + random.below(60);
        random_text(&mut random, pieces)
    }));
    let named: std::collections::BTreeMap<String, String> =
        [("r".to_owned(), "x.md".to_owned()), ("1".to_owned(), "y.md".to_owned())].into_iter().collect();
    for text in &texts {
        for labels in [std::collections::BTreeMap::new(), named.clone()] {
            let (styles, code, index) = style::Styles::unlit_indexed(text, &labels, true);
            let (alone, alone_code) = style::Styles::unlit(text, &labels);
            assert_eq!(styles.bytes, alone.bytes, "{text:?}");
            assert_eq!(styles.lines, alone.lines, "{text:?}");
            assert_eq!(styles.starts, alone.starts, "{text:?}");
            assert_eq!(code, alone_code, "{text:?}");
            assert_eq!(format!("{index:?}"), format!("{:?}", kition_note_markdown::index(text)), "{text:?}");
        }
    }
}
