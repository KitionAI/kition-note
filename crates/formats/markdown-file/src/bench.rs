//! Fixtures for tests, `cargo xtask golden` and `cargo xtask perf`: synthetic edge files, and
//! generated Markdown and scattered edits ported from Kition's `documentKdm.bench.spec.ts` and
//! `documentMerge.spec.ts`. Deterministic: the same seed gives the same text.

use crate::merge::{TextEdit, apply_edits};

/// Synthetic files every note must save back byte for byte without an edit (the edges of the
/// round-trip golden suite), all valid UTF-8.
pub const EDGE_FILES: &[(&str, &[u8])] = &[
    ("empty", b""),
    ("lf", b"# Plan\n\nBody\n"),
    ("crlf", b"# Plan\r\n\r\nBody\r\n"),
    ("crlf with a byte order mark", b"\xEF\xBB\xBF# Plan\r\nBody\r\n"),
    ("a byte order mark only", b"\xEF\xBB\xBF"),
    ("two byte order marks", b"\xEF\xBB\xBF\xEF\xBB\xBFa\n"),
    ("no final newline", b"# Plan\n\nBody"),
    ("an embedded NUL", b"a\x00b\n\x00\n"),
    ("mixed endings", b"one\r\ntwo\nthree\r\n"),
    ("a lone CR", b"one\rtwo\r\nthree\n"),
    ("only lone CRs", b"a\rb\rc"),
    ("trailing white space and tabs", b"a  \n\tb\t\n\n\n"),
];

/// A file that is not valid UTF-8: it opens read-only and saves unchanged.
pub const INVALID_UTF8_FILE: &[u8] = b"# Note\n\xFF\xFE broken\n";

/// A small deterministic generator (xorshift64*).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    /// A number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        if n == 0 { 0 } else { usize::try_from(self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33).unwrap_or(0) % n }
    }
}

/// CJK words (U+62A5 U+544A "report", U+8BA1 U+5212 "plan", U+56E2 U+961F "team").
const REPORT: &str = "\u{62a5}\u{544a}";
const PLAN: &str = "\u{8ba1}\u{5212}";
const TEAM: &str = "\u{56e2}\u{961f}";
const EMOJIS: [&str; 5] = ["\u{1F600}", "\u{1F44D}", "\u{1F389}", "\u{2705}", "\u{1F4CA}"];

fn emoji(index: usize) -> &'static str {
    EMOJIS.get(index % EMOJIS.len()).copied().unwrap_or_default()
}

/// Realistic Markdown of about `target_bytes` (UTF-8): headings, prose with CJK and emoji, lists,
/// GFM tables, and fenced code blocks (the benchmark document of Kition's TypeScript note model).
pub fn markdown_of_size(target_bytes: usize) -> String {
    let title = "# Benchmark Document".to_owned();
    let mut bytes = title.len();
    let mut blocks = vec![title];
    let mut index = 0usize;
    while bytes < target_bytes {
        let block = match index % 5 {
            0 => format!("{} Section {index} {}", "#".repeat(index % 3 + 1), emoji(index)),
            1 => format!(
                "Paragraph {index} covers the {PLAN} and the {REPORT} prepared by the {TEAM} {}. It restates ordinary English sentences long enough to look like real document prose, mixing in short phrases so neither the diff nor the encoder sees one repeated line across the whole file.",
                emoji(index + 1)
            ),
            2 => (0..5).map(|i| format!("- Item {index}.{i}: a task for the {TEAM} {}", emoji(index + i))).collect::<Vec<_>>().join("\n"),
            3 => {
                let rows: Vec<String> = (0..4).map(|i| format!("| Row {index}.{i} | {REPORT} | {TEAM} {i} |")).collect();
                format!("| Name | Status | Owner |\n| --- | --- | --- |\n{}", rows.join("\n"))
            }
            _ => format!("```ts\nfunction step{index}(value: number): number {{\n  return value * 2 + 1\n}}\n```"),
        };
        bytes += block.len() + 2;
        blocks.push(block);
        index += 1;
    }
    format!("{}\n", blocks.join("\n\n"))
}

/// `text` with about `fraction` of its characters replaced by `Z` at random, non-overlapping
/// places (each edit replaces one or two characters).
pub fn scatter_edits(text: &str, fraction: f64, seed: u64) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut rng = Rng::new(seed);
    let target = ((chars.len() as f64 * fraction) as usize).max(1);
    let count = ((target as f64 / 1.5) as usize).max(1);
    let mut positions = std::collections::BTreeSet::new();
    while positions.len() < count && positions.len() < chars.len() {
        positions.insert(rng.below(chars.len()));
    }
    let mut edits: Vec<TextEdit> = Vec::new();
    for from in positions {
        if edits.last().is_some_and(|last| from < last.to) {
            continue;
        }
        let to = (from + 1 + rng.below(2)).min(chars.len());
        edits.push(TextEdit::new(from, to, "Z"));
    }
    apply_edits(&chars, &edits)
}

/// The external-merge case of the TypeScript benchmark: `base`, `current` (0.5 % local edits) and
/// `incoming` (1 % external edits) of about `bytes` of Markdown. Edits of the two sides may touch,
/// so the merged text is not given; see [`merge_case`] for one with a known result.
pub fn bench_merge_case(bytes: usize) -> (String, String, String) {
    let base = markdown_of_size(bytes);
    let current = scatter_edits(&base, 0.005, 1001);
    let incoming = scatter_edits(&base, 0.01, 2002);
    (base, current, incoming)
}

const LOCAL_INSERTS: [&str; 3] = ["@", "~", "\u{1F431}"];
const EXTERNAL_INSERTS: [&str; 3] = ["%", "=", "\u{1F98A}"];

/// Scattered changes of `text` at random places at least `gap` characters apart (one per
/// `1 / density` characters on average), each replacing up to two characters with up to two from
/// its side's alphabet (characters the generated Markdown never contains); about `local_share` of
/// them are local. Answers the local, the external, and both sides' edits.
pub fn disjoint_changes(rng: &mut Rng, text: &[char], density: f64, local_share: f64, gap: usize) -> (Vec<TextEdit>, Vec<TextEdit>, Vec<TextEdit>) {
    let (mut local, mut external) = (Vec::new(), Vec::new());
    let spacing = (gap + 1).max((1.0 / density).round() as usize);
    let mut at = rng.below(spacing);
    while at < text.len() {
        let to = (at + rng.below(3)).min(text.len());
        let is_local = (rng.below(1000) as f64) < local_share * 1000.0;
        let alphabet = if is_local { &LOCAL_INSERTS } else { &EXTERNAL_INSERTS };
        let mut insert: String = (0..rng.below(3)).map(|_| alphabet.get(rng.below(alphabet.len())).copied().unwrap_or_default()).collect();
        if at == to && insert.is_empty() {
            insert = alphabet.first().copied().unwrap_or("@").to_owned();
        }
        (if is_local { &mut local } else { &mut external }).push(TextEdit::new(at, to, insert));
        at += gap + rng.below(2 * (spacing - gap) + 1);
    }
    let mut both: Vec<TextEdit> = local.iter().chain(&external).cloned().collect();
    both.sort_by_key(|edit| edit.from);
    (local, external, both)
}

/// A merge with a known result on about `bytes` of Markdown: one change every ~67 characters, a
/// third of them local (about 1 % external and 0.5 % local, as the benchmark), never touching.
/// Answers `base`, `current`, `incoming`, and the merged text.
pub fn merge_case(bytes: usize) -> (String, String, String, String) {
    let base = markdown_of_size(bytes);
    let chars: Vec<char> = base.chars().collect();
    let (local, external, both) = disjoint_changes(&mut Rng::new(31), &chars, 1.0 / 67.0, 1.0 / 3.0, 24);
    (base.clone(), apply_edits(&chars, &local), apply_edits(&chars, &external), apply_edits(&chars, &both))
}
