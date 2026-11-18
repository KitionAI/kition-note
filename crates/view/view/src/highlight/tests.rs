use super::*;

/// The code corpora and how Obsidian colored them (`web/tests/obsidian-code-tokens.mjs` records
/// it): each code line's color per character, a letter each.
const CORPORA: &[(&str, &str)] = &[
    ("code-corpus.md", include_str!("../../fixtures/code-corpus.md")),
    ("code-corpus-2.md", include_str!("../../fixtures/code-corpus-2.md")),
    ("code-corpus-3.md", include_str!("../../fixtures/code-corpus-3.md")),
    ("code-corpus-4.md", include_str!("../../fixtures/code-corpus-4.md")),
    ("code-corpus-5.md", include_str!("../../fixtures/code-corpus-5.md")),
    ("code-corpus-6.md", include_str!("../../fixtures/code-corpus-6.md")),
    ("code-corpus-7.md", include_str!("../../fixtures/code-corpus-7.md")),
    ("code-corpus-8.md", include_str!("../../fixtures/code-corpus-8.md")),
    ("code-corpus-9.md", include_str!("../../fixtures/code-corpus-9.md")),
    ("code-corpus-10.md", include_str!("../../fixtures/code-corpus-10.md")),
    ("code-corpus-11.md", include_str!("../../fixtures/code-corpus-11.md")),
    ("code-corpus-12.md", include_str!("../../fixtures/code-corpus-12.md")),
    ("code-corpus-13.md", include_str!("../../fixtures/code-corpus-13.md")),
    ("code-corpus-14.md", include_str!("../../fixtures/code-corpus-14.md")),
    ("code-corpus-15.md", include_str!("../../fixtures/code-corpus-15.md")),
    ("code-corpus-16.md", include_str!("../../fixtures/code-corpus-16.md")),
    ("code-corpus-17.md", include_str!("../../fixtures/code-corpus-17.md")),
    ("code-corpus-18.md", include_str!("../../fixtures/code-corpus-18.md")),
    ("code-corpus-19.md", include_str!("../../fixtures/code-corpus-19.md")),
    ("code-corpus-20.md", include_str!("../../fixtures/code-corpus-20.md")),
    ("code-corpus-21.md", include_str!("../../fixtures/code-corpus-21.md")),
    ("code-corpus-22.md", include_str!("../../fixtures/code-corpus-22.md")),
    ("code-corpus-23.md", include_str!("../../fixtures/code-corpus-23.md")),
    ("code-corpus-24.md", include_str!("../../fixtures/code-corpus-24.md")),
    ("code-corpus-25.md", include_str!("../../fixtures/code-corpus-25.md")),
    ("code-corpus-26.md", include_str!("../../fixtures/code-corpus-26.md")),
    ("code-corpus-27.md", include_str!("../../fixtures/code-corpus-27.md")),
    ("code-corpus-28.md", include_str!("../../fixtures/code-corpus-28.md")),
    ("code-corpus-29.md", include_str!("../../fixtures/code-corpus-29.md")),
    ("code-corpus-30.md", include_str!("../../fixtures/code-corpus-30.md")),
    ("code-corpus-31.md", include_str!("../../fixtures/code-corpus-31.md")),
    ("code-corpus-32.md", include_str!("../../fixtures/code-corpus-32.md")),
    ("code-corpus-33.md", include_str!("../../fixtures/code-corpus-33.md")),
    ("code-corpus-34.md", include_str!("../../fixtures/code-corpus-34.md")),
    ("code-corpus-35.md", include_str!("../../fixtures/code-corpus-35.md")),
    ("code-corpus-36.md", include_str!("../../fixtures/code-corpus-36.md")),
    ("code-corpus-37.md", include_str!("../../fixtures/code-corpus-37.md")),
    ("code-corpus-38.md", include_str!("../../fixtures/code-corpus-38.md")),
    ("code-corpus-39.md", include_str!("../../fixtures/code-corpus-39.md")),
    ("code-corpus-40.md", include_str!("../../fixtures/code-corpus-40.md")),
    ("code-corpus-41.md", include_str!("../../fixtures/code-corpus-41.md")),
    ("code-corpus-42.md", include_str!("../../fixtures/code-corpus-42.md")),
    ("code-corpus-43.md", include_str!("../../fixtures/code-corpus-43.md")),
    ("code-corpus-44.md", include_str!("../../fixtures/code-corpus-44.md")),
    ("code-corpus-45.md", include_str!("../../fixtures/code-corpus-45.md")),
    ("code-corpus-46.md", include_str!("../../fixtures/code-corpus-46.md")),
    ("code-corpus-47.md", include_str!("../../fixtures/code-corpus-47.md")),
    ("code-corpus-48.md", include_str!("../../fixtures/code-corpus-48.md")),
];
const COLORS: &str = include_str!("../../fixtures/code-tokens.txt");

fn letter(kind: Option<Kind>) -> char {
    match kind {
        None => '.',
        Some(Kind::Keyword) => 'k',
        Some(Kind::String) => 's',
        Some(Kind::Number) => 'n',
        Some(Kind::Comment) => 'c',
        Some(Kind::Variable) => 'v',
        Some(Kind::Operator) => 'o',
        Some(Kind::Property) => 'p',
        Some(Kind::Punctuation) => 'b',
        Some(Kind::Link) => 'l',
    }
}

/// Obsidian's colors for each corpus's lines, by line number.
fn recorded() -> Vec<(String, std::collections::BTreeMap<usize, String>)> {
    let mut out: Vec<(String, std::collections::BTreeMap<usize, String>)> = Vec::new();
    for line in COLORS.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            out.push((name.to_string(), std::collections::BTreeMap::new()));
        } else if let Some((number, colors)) = line.split_once('\t')
            && let Some((_, lines)) = out.last_mut()
            && let Ok(number) = number.parse::<usize>()
        {
            lines.insert(number, colors.to_string());
        }
    }
    out
}

/// Our colors for a line, a letter per character.
fn ours(language: Language, text: &str, state: &mut State) -> String {
    let spans = line(language, text, state);
    text.char_indices().map(|(at, _)| letter(spans.iter().find(|(range, _)| range.contains(&at)).map(|(_, kind)| *kind))).collect()
}

#[test]
fn code_is_colored_as_obsidian_colors_it() {
    let recorded = recorded();
    let mut wrong = Vec::new();
    let mut compared = 0;
    for (name, corpus) in CORPORA {
        let Some((_, colors)) = recorded.iter().find(|(recorded, _)| recorded == name) else { continue };
        let mut block: Option<(Option<Language>, String, State)> = None;
        for (index, text) in corpus.lines().enumerate() {
            let number = index + 1;
            let trimmed = text.trim_start();
            if trimmed.starts_with("```") {
                block = match block {
                    Some(_) => None,
                    None => {
                        let info = trimmed.trim_start_matches('`').trim().to_string();
                        Some((grammar(&info), info, State::default()))
                    }
                };
                continue;
            }
            let Some((language, info, state)) = block.as_mut() else { continue };
            let Some(expected) = colors.get(&number) else { continue };
            let got = match language {
                Some(language) => ours(*language, text, state),
                None => ".".repeat(text.chars().count()),
            };
            compared += 1;
            // Spaces take no color one can see.
            let differs = text.chars().zip(expected.chars().zip(got.chars())).any(|(c, (want, have))| !c.is_whitespace() && want != have);
            if differs {
                wrong.push(format!("{name}:{number} [{info}]\n  text {text}\n  want {expected}\n  have {got}"));
            }
        }
    }
    assert!(compared > 0, "the fixture has lines to compare");
    assert!(wrong.is_empty(), "{} of {compared} lines colored unlike Obsidian:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn a_block_comment_and_a_string_run_on_to_the_next_line() {
    let mut state = State::default();
    assert_eq!(ours(Language::Rust, "fn a() { /* open", &mut state), "kk.......ccccccc");
    assert_eq!(ours(Language::Rust, "still */ fn", &mut state), "cccccccc.kk");
    let mut state = State::default();
    ours(Language::Python, "x = \"\"\"start", &mut state);
    assert_eq!(ours(Language::Python, "end\"\"\" + 1", &mut state), "ssssss.o.n");
    assert!(grammar("unknown").is_none());
}

#[test]
fn fenced_code_carries_its_tokens_in_its_style_bytes() {
    use crate::style::{Styles, flag};
    let text = "```rust\nlet s = \"a\"; // c\n```\n\n```\nlet s\n```\n\n```py\nlet s\n```\n";
    let styles = Styles::of(text);
    let kind = |needle: &str, nth: usize| {
        let at = text.match_indices(needle).nth(nth).map(|(at, _)| at).unwrap_or(0);
        Kind::of_code((styles.bytes.get(at).copied().unwrap_or(0) & flag::TOKEN) >> flag::COLOR_SHIFT)
    };
    assert_eq!(kind("let", 0), Some(Kind::Keyword));
    assert_eq!(kind("\"a\"", 0), Some(Kind::String));
    assert_eq!(kind("// c", 0), Some(Kind::Comment));
    assert_eq!(kind("```", 1), None, "the closing fence is not code");
    assert_eq!(kind("let", 1), None, "a block without a language is not highlighted");
    assert_eq!(kind("let", 2), None, "Obsidian does not highlight `py`");
}
