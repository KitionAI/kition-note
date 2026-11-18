use std::collections::BTreeMap;

use super::*;

struct Files(BTreeMap<&'static str, &'static str>);

impl FileLookup for Files {
    fn exists(&self, path: &str) -> Option<bool> {
        Some(self.0.contains_key(path.trim_start_matches('/')))
    }
    fn read_text(&self, path: &str) -> Option<String> {
        self.0.get(path.trim_start_matches('/')).map(|text| (*text).to_owned())
    }
}

fn codes_of(text: &str, files: &dyn FileLookup) -> Vec<(String, Option<String>)> {
    let ix = index(text);
    check(&Input { text, index: &ix, invalid_utf8: None, mixed_line_endings: false, files })
        .into_iter()
        .map(|found| (found.code, found.fix.map(|fix| format!("{} {}", fix.command, fix.params.unwrap_or_default()))))
        .collect()
}

fn codes(text: &str) -> Vec<String> {
    codes_of(text, &NoFiles).into_iter().map(|(code, _)| code).collect()
}

#[test]
fn a_clean_note_has_no_diagnostics() {
    assert!(
        codes("---\ntitle: Plan\ntags: [a]\n---\n# Plan\n\n## Risks ^r\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nSee [[#Risks]] and [[#^r]].\n")
            .is_empty()
    );
}

#[test]
fn links_are_unknown_without_a_host() {
    assert!(codes("[[Elsewhere]] ![[pic.png]] [x](a.md)\n").is_empty());
}

#[test]
fn links_resolve_through_the_lookup() {
    let files = Files(BTreeMap::from([("Plan.md", "# Plan\n\n## Risks\n"), ("pic.png", "")]));
    let found = codes_of(
        "[[Plan]] [[Plan#Risks]] [[Plan#risks]] [[Plan#Riskz]] [[Plan#R-isks!]] ![[pic.png]] ![[gone.png]] [x](gone.md) [y](Plan.md)\n",
        &files,
    );
    let names: Vec<&str> = found.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(names, vec!["note.broken-anchor", "note.broken-anchor", "note.broken-embed", "note.broken-link"]);
    let hinted: Vec<&Option<String>> = found.iter().filter(|(code, _)| code == "note.broken-anchor").map(|(_, fix)| fix).collect();
    assert_eq!(hinted[0], &None);
    assert_eq!(hinted[1].as_deref(), Some("link.update {\"destination\":\"Plan#Risks\",\"index\":4,\"line\":1}"));
}

#[test]
fn every_code_has_its_trigger() {
    let cases: &[(&str, &str)] = &[
        ("note.unclosed-fence", "# Setup\n\n```bash\nnpm install\n"),
        ("note.unclosed-fence", "$$\nx\n"),
        ("note.frontmatter-invalid", "---\ntitle: [unclosed\n---\n"),
        ("note.frontmatter-duplicate-key", "---\ntags: [a]\ntags: [b]\n---\n"),
        ("note.broken-link", "[[]] and [x]()\n"),
        ("note.broken-anchor", "# Plan\n\n[[#Risks]]\n"),
        ("note.duplicate-block-id", "A ^a1\n\nB ^a1\n"),
        ("note.table-column-mismatch", "| a | b |\n| - | - |\n| 1 | 2 | 3 |\n"),
        ("note.heading-level-skip", "# T\n\n### D\n"),
        ("note.property-complex", "---\na:\n  b: c\n---\n"),
    ];
    for (code, text) in cases {
        let found = codes(text);
        assert!(found.iter().all(|found| found == code), "{code}: {found:?}");
        assert!(!found.is_empty(), "{code}: nothing found in {text:?}");
    }
}

#[test]
fn fixes_name_commands_with_params() {
    let found = codes_of("# T\n\n### D\n\nA ^a\n\nB ^a\n\n---\n", &NoFiles);
    let fixes: Vec<String> = found.into_iter().filter_map(|(_, fix)| fix).collect();
    assert!(fixes.contains(&"block.setId {\"blockId\":\"a\",\"id\":\"a-2\",\"occurrence\":2}".to_owned()), "{fixes:?}");
    assert!(fixes.contains(&"heading.setLevel {\"level\":2,\"line\":3}".to_owned()), "{fixes:?}");
    let found = codes_of("```\ncode", &NoFiles);
    assert_eq!(found[0].1.as_deref(), Some("text.insert {\"at\":8,\"text\":\"\\n```\\n\"}"));
}

#[test]
fn targets_say_where() {
    let text = "# Plan\n\n### Deep\n";
    let ix = index(text);
    let found = check(&Input { text, index: &ix, invalid_utf8: None, mixed_line_endings: false, files: &NoFiles });
    assert_eq!(found[0].target.path.as_deref(), Some("body:3:1"));
    assert_eq!(found[0].target.object_id.as_deref(), Some("note"));
    assert!(found[0].message.contains("line 3"), "{}", found[0].message);
    kition_kit::diagnostics::check_diagnostics(&found).unwrap();
}

#[test]
fn each_code_is_capped() {
    let text: String = (0..150).map(|n| format!("{n}. [[]]\n")).collect();
    let found = codes(&text);
    assert_eq!(found.iter().filter(|code| *code == "note.broken-link").count(), PER_CODE);
    assert_eq!(found.last().map(String::as_str), Some("note.truncated"));
}

#[test]
fn the_catalogue_lists_every_code() {
    let catalogue: serde_json::Value = serde_json::from_str(include_str!("../../../../agent/checks.json")).unwrap();
    let listed: Vec<(&str, &str)> =
        catalogue.as_array().unwrap().iter().map(|entry| (entry["code"].as_str().unwrap(), entry["severity"].as_str().unwrap())).collect();
    let codes: Vec<(&str, String)> = CODES.iter().map(|(code, severity, _)| (*code, format!("{severity:?}").to_lowercase())).collect();
    assert_eq!(listed.len(), codes.len());
    for ((a, sa), (b, sb)) in listed.iter().zip(&codes) {
        assert_eq!((*a, *sa), (*b, sb.as_str()));
    }
}
