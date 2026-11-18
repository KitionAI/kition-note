//! Every language the highlighter knows, run over random and adversarial lines (deterministic
//! seeds): no panic, every line ends, and every line's spans are in order, apart, inside the
//! line, and on character boundaries.

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

/// Pieces the lines are built from: what opens and closes things in many languages, words,
/// numbers, spaces, and characters past ASCII.
const PIECES: &[&str] = &[
    "\"", "'", "`", "\\", "/*", "*/", "//", "#", "--", "(*", "*)", "{-", "-}", "<!--", "-->", "{", "}", "(", ")", "[", "]", "<", ">", "$", "@", "%",
    "&", "|", "^", "~", "!", "?", ":", ";", ",", ".", "=", "+", "-", "*", "/", "#{", "${", "\"\"\"", "'''", "```", "r#\"", "<<EOF", "EOF", "=begin",
    "=end", "\t", " ", "  ", "if", "end", "def", "let", "x", "foo_bar", "Name", "0x1F", "1.5e-3", "42", "_", "é", "中文", "😀", "\u{a0}", "\u{200b}",
    "ß", "\r", "\\n", "%{", "{{", "}}", "{%", "%}", "{*", "*}", "|>", "=>", "::", "...",
];

fn random_line(random: &mut Random) -> String {
    let mut line = String::new();
    let pieces = random.below(24);
    for _ in 0..pieces {
        line.push_str(PIECES[random.below(PIECES.len())]);
    }
    line
}

/// Lines no random draw is likely to make: very long, deeply nested, only openers.
fn adversarial_lines() -> Vec<String> {
    // Full lengths in release builds (where the speed checks run); a tenth in debug ones.
    let n = |count: usize| if cfg!(debug_assertions) { count / 10 + 1 } else { count };
    let mut lines = vec![
        "(".repeat(n(2000)),
        ")".repeat(n(2000)),
        "{".repeat(n(1000)) + &"}".repeat(n(1000)),
        "[".repeat(n(1500)),
        "\"".to_string() + &"a".repeat(n(20_000)),
        "/*".to_string() + &"x".repeat(n(20_000)),
        "x ".repeat(n(10_000)),
        "中".repeat(n(5_000)),
        "😀".repeat(n(3_000)),
        "\\".repeat(n(4_001)),
        "#{".repeat(n(800)),
        "<a ".repeat(n(800)),
        "1".repeat(n(10_000)),
        ".".repeat(n(10_000)),
        "'".repeat(n(3_001)),
        "-".repeat(n(10_000)),
        " \t".repeat(n(5_000)),
    ];
    lines.push(String::new());
    lines
}

/// The fence names the highlighter takes (read from `grammar`'s arms, so every language is in).
fn names() -> Vec<String> {
    let source = include_str!("mod.rs");
    let start = source.find("pub fn grammar").unwrap_or(0);
    let end = source.get(start..).and_then(|rest| rest.find("_ => return None")).map_or(source.len(), |found| start + found);
    let mut names = Vec::new();
    for line in source.get(start..end).unwrap_or("").lines() {
        let Some((left, _)) = line.split_once("=>") else { continue };
        for part in left.split('|') {
            let name = part.trim().trim_matches('"');
            if !name.is_empty() && part.trim().starts_with('"') {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// What is wrong with the line's spans, if anything.
fn check(language: Language, text: &str, state: &mut State) -> Option<String> {
    let spans = line(language, text, state);
    let mut end = 0;
    for (range, _) in &spans {
        let problem = if range.start < end {
            "spans out of order or overlapping"
        } else if range.start >= range.end {
            "an empty span"
        } else if range.end > text.len() {
            "a span past the line's end"
        } else if !(text.is_char_boundary(range.start) && text.is_char_boundary(range.end)) {
            "a span splits a character"
        } else {
            end = range.end;
            continue;
        };
        let shown: String = text.chars().take(80).collect();
        return Some(format!("{problem} in {shown:?} ({range:?})"));
    }
    None
}

#[test]
fn every_language_takes_any_line() {
    let names = names();
    assert!(names.len() > 100, "the grammar's names are read ({})", names.len());
    let adversarial = adversarial_lines();
    let mut problems = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let Some(language) = grammar(name) else { panic!("{name} is a fence name but has no language") };
        let started = std::time::Instant::now();
        let mut random = Random(0x9e37_79b9_7f4a_7c15 ^ (index as u64 + 1));
        // A block of random lines, its state carried, then each adversarial line in it and alone.
        let mut state = State::default();
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut found = None;
            for _ in 0..300 {
                let text = random_line(&mut random);
                found = found.or_else(|| check(language, &text, &mut state));
            }
            for text in &adversarial {
                let started = std::time::Instant::now();
                found = found.or_else(|| check(language, text, &mut state));
                found = found.or_else(|| check(language, text, &mut State::default()));
                // (A 10 kB line of spaces costs NSIS's patterns about a second, as it costs CodeMirror's own.)
                // Timed again when slow: other tests running beside it slow it down for a while.
                let slow = |started: std::time::Instant| !cfg!(debug_assertions) && started.elapsed().as_millis() > 3000;
                if slow(started) && {
                    let again = std::time::Instant::now();
                    let _ = check(language, text, &mut State::default());
                    slow(again)
                } {
                    let shown: String = text.chars().take(12).collect();
                    found = found.or_else(|| Some(format!("slow ({:?}) on {shown:?}… ({} bytes)", started.elapsed(), text.len())));
                }
            }
            found
        }));
        let found = run.unwrap_or_else(|panic| {
            let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()));
            Some(format!("panicked: {}", message.unwrap_or_default()))
        });
        if let Some(problem) = found {
            problems.push(format!("{name}: {problem}"));
        }
        let elapsed = started.elapsed();
        if !cfg!(debug_assertions) && elapsed.as_secs() >= 10 {
            problems.push(format!("{name}: took {elapsed:?}"));
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
