//! Ported from Kition's `documentText.spec.ts` and `documentMerge.spec.ts`, adapted to the kit's
//! design: mixed and lone-CR files keep their `\r` (the TypeScript rewrote them), positions are
//! Unicode scalar values (the TypeScript counted UTF-16 units and checked surrogate pairs).

use super::merge::{TextEdit, apply_edits, coalesce_edits, diff_text, merge_edits};
use super::*;
use crate::bench::{Rng, disjoint_changes};

/// CJK text (U+5B63 U+5EA6), as a named constant: source files stay English.
const QUARTER: &str = "\u{5b63}\u{5ea6}";
const SEASON: &str = "\u{5b63}";
const DEGREE: &str = "\u{5ea6}";
const BOM_CHAR: &str = "\u{feff}";

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn style(eol: Eol, bom: bool) -> LineStyle {
    LineStyle { eol, bom }
}

// ---- decode and encode -------------------------------------------------------------------

#[test]
fn keeps_lf_text_as_it_is() {
    assert_eq!(decode(b"# Plan\n\nBody\n").unwrap(), ("# Plan\n\nBody\n".to_owned(), style(Eol::Lf, false)));
}

#[test]
fn normalises_crlf_and_records_it() {
    assert_eq!(decode(b"a\r\nb\r\n").unwrap(), ("a\nb\n".to_owned(), style(Eol::Crlf, false)));
}

#[test]
fn strips_one_byte_order_mark_and_records_it() {
    assert_eq!(decode(format!("{BOM_CHAR}a").as_bytes()).unwrap(), ("a".to_owned(), style(Eol::Lf, true)));
    assert_eq!(decode(format!("{BOM_CHAR}{BOM_CHAR}a").as_bytes()).unwrap().0, format!("{BOM_CHAR}a"));
}

#[test]
fn treats_an_empty_file_and_one_without_line_endings_as_lf() {
    assert_eq!(decode(b"").unwrap(), (String::new(), style(Eol::Lf, false)));
    assert_eq!(decode(BOM).unwrap(), (String::new(), style(Eol::Lf, true)));
    let text = format!("{QUARTER}\u{1F600}");
    assert_eq!(decode(text.as_bytes()).unwrap(), (text, style(Eol::Lf, false)));
}

#[test]
fn keeps_mixed_endings_and_lone_cr_in_the_text() {
    assert_eq!(decode(b"a\r\nb\r\nc\nd").unwrap(), ("a\r\nb\r\nc\nd".to_owned(), style(Eol::Mixed, false)));
    assert_eq!(decode(b"a\rb\r").unwrap(), ("a\rb\r".to_owned(), style(Eol::Mixed, false)));
    assert_eq!(decode(b"a\r\n\rb").unwrap(), ("a\r\n\rb".to_owned(), style(Eol::Mixed, false)));
    assert_eq!(Endings::count("a\r\n\rb\n"), Endings { lf: 1, crlf: 1, cr: 1 });
}

#[test]
fn refuses_invalid_utf8_with_a_lossy_text_to_show() {
    let error = decode(b"ok\n\xFF\xFEbad\n").unwrap_err();
    assert_eq!(error.at, 3);
    assert_eq!(error.lossy, "ok\n\u{FFFD}\u{FFFD}bad\n");
    assert_eq!(decode(b"\xEF\xBB\xBFab\xC3").unwrap_err().at, 5);
}

#[test]
fn encode_restores_crlf_and_the_byte_order_mark() {
    assert_eq!(encode("a\nb\n", style(Eol::Crlf, true)), format!("{BOM_CHAR}a\r\nb\r\n").into_bytes());
    assert_eq!(encode("x\ny", style(Eol::Mixed, false)), b"x\ny");
    assert_eq!(encode("a", style(Eol::Lf, false)), b"a");
    // Text pasted with CRLF into a CRLF note is not doubled.
    assert_eq!(encode("a\r\nb\n", style(Eol::Crlf, false)), b"a\r\nb\r\n");
}

#[test]
fn inserted_text_takes_the_notes_line_endings() {
    assert_eq!(normalize_inserted("a\r\nb\rc", Eol::Lf), "a\nb\nc");
    assert_eq!(normalize_inserted("a\r\nb", Eol::Crlf), "a\nb");
    assert_eq!(normalize_inserted("a\r\nb", Eol::Mixed), "a\r\nb");
    assert!(matches!(normalize_inserted("plain", Eol::Lf), std::borrow::Cow::Borrowed(_)));
}

/// A random file of lines in one ending style, with CJK, emoji, empty lines, and an optional BOM.
fn random_file(rng: &mut Rng, eol: &str) -> String {
    let pieces = ["a", "Plan", " ", "#", "|", QUARTER, "\u{1F600}", "\u{1F469}\u{200D}\u{1F4BB}", "\t", BOM_CHAR, "\u{e9}", "", "\0"];
    let count = rng.below(8);
    let lines: Vec<String> = (0..count).map(|_| (0..rng.below(5)).map(|_| pieces[rng.below(pieces.len())]).collect()).collect();
    let body = lines.join(eol) + if rng.below(2) == 0 { eol } else { "" };
    (if rng.below(3) == 0 { BOM_CHAR } else { "" }).to_owned() + &body
}

#[test]
fn every_valid_file_round_trips_byte_for_byte() {
    let mut rng = Rng::new(7);
    for _ in 0..4000 {
        let raw = match rng.below(4) {
            0 => random_file(&mut rng, "\n"),
            1 => random_file(&mut rng, "\r\n"),
            2 => random_file(&mut rng, "\r"),
            _ => random_file(&mut rng, "\n") + "\r\n" + &random_file(&mut rng, "\r\n") + "\r",
        };
        let (text, style) = decode(raw.as_bytes()).unwrap();
        assert_eq!(encode(&text, style), raw.as_bytes(), "{raw:?}");
        if style.eol != Eol::Mixed {
            assert!(!text.contains('\r'), "{raw:?}");
        }
    }
}

#[test]
fn pieces_end_after_line_breaks_and_rejoin_to_the_text() {
    let lines: Vec<String> = (0..3000).map(|index| format!("Line {index}: {QUARTER} \u{1F600} words")).collect();
    let text = format!("{}\n{}\n{}\n", lines.join("\n"), "\u{1F600}\u{1F44D}".repeat(6000), lines.join("\n"));
    let cut = pieces(&text, 16 * 1024);
    assert!(cut.len() > 5);
    assert_eq!(cut.concat(), text);
    for piece in &cut {
        assert!(piece.chars().count() <= 32 * 1024);
    }
    assert!(cut.iter().filter(|piece| !piece.ends_with('\n')).count() <= 6);
    assert_eq!(pieces("", 4), Vec::<&str>::new());
    assert_eq!(pieces("ab\ncd", 1), vec!["a", "b", "\n", "c", "d"]);
    assert_eq!(pieces("ab\ncd", 2), vec!["ab\n", "cd"]);
}

/// `pieces` as it was written first (each cut found character by character).
fn pieces_by_characters(text: &str, piece: usize) -> Vec<&str> {
    let piece = piece.max(1);
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let cut = rest.char_indices().nth(piece).map_or(rest.len(), |(at, _)| at);
        let end = if cut >= rest.len() {
            rest.len()
        } else {
            let back = rest.get(..cut).and_then(|head| head.char_indices().last()).map_or(0, |(at, _)| at);
            let window = rest.get(back..).unwrap_or_default();
            let limit = window.char_indices().nth(piece).map_or(window.len(), |(at, _)| at);
            match window.get(..limit).and_then(|w| w.find('\n')) {
                Some(newline) => back + newline + 1,
                None => cut,
            }
        };
        let (head, tail) = rest.split_at(end);
        out.push(head);
        rest = tail;
    }
    out
}

#[test]
fn pieces_and_endings_are_as_counted_character_by_character() {
    let mut rng = Rng::new(11);
    for _ in 0..3000 {
        let eol = ["\n", "\r\n", "\r"][rng.below(3)];
        let text = (0..rng.below(6)).map(|_| random_file(&mut rng, eol)).collect::<Vec<_>>().join(if rng.below(2) == 0 { "\n" } else { "" });
        for piece in [1, 2, 3, 5, 8, 64] {
            assert_eq!(pieces(&text, piece), pieces_by_characters(&text, piece), "{text:?} by {piece}");
        }
        let mut counted = Endings::default();
        let mut bytes = text.bytes().peekable();
        while let Some(byte) = bytes.next() {
            match byte {
                b'\n' => counted.lf += 1,
                b'\r' if bytes.peek() == Some(&b'\n') => {
                    bytes.next();
                    counted.crlf += 1;
                }
                b'\r' => counted.cr += 1,
                _ => {}
            }
        }
        assert_eq!(Endings::count(&text), counted, "{text:?}");
    }
}

// ---- diff_text ---------------------------------------------------------------------------

fn diff(a: &str, b: &str) -> Vec<TextEdit> {
    diff_text(&chars(a), &chars(b))
}

fn edit(from: usize, to: usize, insert: &str) -> TextEdit {
    TextEdit::new(from, to, insert)
}

fn apply(text: &str, edits: &[TextEdit]) -> String {
    apply_edits(&chars(text), edits)
}

fn merged(base: &str, incoming: &str, current: &str) -> String {
    apply(current, &merge_edits(&chars(base), &chars(incoming), &chars(current)))
}

/// Edits are sorted, separated, non-empty, and inside the text.
fn assert_well_formed(edits: &[TextEdit], length: usize) {
    let mut last: Option<usize> = None;
    for edit in edits {
        assert!(last.is_none_or(|last| edit.from > last), "{edits:?}");
        assert!(edit.to >= edit.from);
        assert!(edit.from != edit.to || !edit.insert.is_empty());
        last = Some(edit.to);
    }
    assert!(last.unwrap_or(0) <= length);
}

#[test]
fn diff_finds_nothing_between_equal_texts_and_handles_empty_sides() {
    assert_eq!(diff("", ""), vec![]);
    assert_eq!(diff("a\nb", "a\nb"), vec![]);
    assert_eq!(diff("", "abc"), vec![edit(0, 0, "abc")]);
    assert_eq!(diff("abc", ""), vec![edit(0, 3, "")]);
}

#[test]
fn diff_refines_a_changed_line_to_the_changed_characters() {
    assert_eq!(diff("one\nthe quick brown fox\nthree\n", "one\nthe quick red fox\nthree\n"), vec![edit(14, 19, "red")]);
    assert_eq!(diff("the brown fox", "the red fox"), vec![edit(4, 9, "red")]);
}

#[test]
fn diff_keeps_distant_changes_separate_and_finds_whole_lines() {
    let lines: Vec<String> = (0..50).map(|index| format!("line {index}")).collect();
    let mut changed = lines.clone();
    changed[3] = "line three".into();
    changed[40] = "line 40!".into();
    let edits = diff(&lines.join("\n"), &changed.join("\n"));
    assert_eq!(edits.len(), 2);
    assert_eq!(apply(&lines.join("\n"), &edits), changed.join("\n"));
    assert_eq!(diff("a\nb\nc\n", "a\nx\nb\nc\n"), vec![edit(2, 2, "x\n")]);
    assert_eq!(diff("a\nb\nc\n", "a\nc\n"), vec![edit(2, 4, "")]);
}

#[test]
fn diff_counts_characters_not_utf16_units() {
    assert_eq!(diff("a\u{1F600}b", "a\u{1F601}b"), vec![edit(1, 2, "\u{1F601}")]);
    assert_eq!(diff("\u{1F600}", "\u{1F600}\u{1F600}"), vec![edit(1, 1, "\u{1F600}")]);
    assert_eq!(diff(&format!("{QUARTER}\u{1F600}\n"), &format!("{QUARTER}\u{1F44D}\n")), vec![edit(2, 3, "\u{1F44D}")]);
}

/// `text` with a few random replacements.
fn mutate(rng: &mut Rng, text: &str, alphabet: &[&str]) -> String {
    let mut points: Vec<String> = text.chars().map(String::from).collect();
    for _ in 0..rng.below(4) {
        let at = rng.below(points.len() + 1);
        let removed = rng.below(4).min(points.len() - at);
        let inserted: Vec<String> = (0..rng.below(3)).map(|_| alphabet[rng.below(alphabet.len())].to_owned()).collect();
        points.splice(at..at + removed, inserted);
    }
    points.concat()
}

#[test]
fn diff_reproduces_the_target_for_random_texts() {
    let mut rng = Rng::new(11);
    let alphabet = ["a", "b", "\n", " ", SEASON, "\u{1F600}", "\u{1F44D}", "\n\n", "\r"];
    for _ in 0..3000 {
        let random_text = |rng: &mut Rng| (0..rng.below(40)).map(|_| alphabet[rng.below(alphabet.len())]).collect::<String>();
        let a = random_text(&mut rng);
        let b = if rng.below(4) == 0 { random_text(&mut rng) } else { mutate(&mut rng, &a, &alphabet) };
        let edits = diff(&a, &b);
        assert_well_formed(&edits, a.chars().count());
        assert_eq!(apply(&a, &edits), b, "{a:?} -> {b:?}");
    }
}

#[test]
fn diff_gives_up_on_detail_not_correctness_for_large_unrelated_texts() {
    let mut rng = Rng::new(12);
    let mut big = |letters: &[char]| (0..20_000).map(|_| letters[rng.below(letters.len())]).collect::<String>();
    let (a, b) = (big(&['a', 'b', '\n']), big(&['a', 'b', '\n']));
    assert_eq!(apply(&a, &diff(&a, &b)), b);
}

// ---- merge_edits -------------------------------------------------------------------------

#[test]
fn merge_simple_outcomes() {
    // The file did not change, or already holds the local text: nothing to do.
    assert!(merge_edits(&chars("a\nb\n"), &chars("a\nb\n"), &chars("a\nB\nlocal\n")).is_empty());
    assert!(merge_edits(&chars("a\n"), &chars("a\nb\n"), &chars("a\nb\n")).is_empty());
    assert_eq!(merged("a\nb\n", "a\nchanged\nb\nc\n", "a\nb\n"), "a\nchanged\nb\nc\n");
    let base = "# Plan\n\nFirst point.\nSecond point.\nThird point.\n";
    let current = "# Plan\n\nFirst point, edited here.\nSecond point.\nThird point.\n";
    let incoming = "# Plan\n\nFirst point.\nSecond point.\nThird point, edited outside.\n";
    assert_eq!(merged(base, incoming, current), "# Plan\n\nFirst point, edited here.\nSecond point.\nThird point, edited outside.\n");
    assert_eq!(merged("the quick brown fox", "the quick brown cat", "the slow brown fox"), "the slow brown cat");
    assert_eq!(merged("a\nb\nc\n", "a\nB\nc\nd\n", "a\nB\nc\n"), "a\nB\nc\nd\n");
    assert_eq!(merged("", "from file", ""), "from file");
    assert_eq!(merged("", "file\n", "local\n"), "local\nfile\n");
    assert_eq!(merged("gone", "", ""), "");
    assert_eq!(merged("text", "", "text, edited"), ", edited");
}

#[test]
fn merge_overlapping_edits() {
    assert_eq!(merged("a\n", "a\nfile\n", "a\nlocal\n"), "a\nlocal\nfile\n");
    assert_eq!(merged("the brown fox", "the red fox", "the grey fox"), "the greyred fox");
    assert_eq!(merged("the brown fox", "the red fox", "the green fox"), "the greered fox");
    assert_eq!(merged("one two three four", "one four", "one two 3 four"), "one 3four");
    assert_eq!(merged("keep: abcdef :keep", "keep:  :keep", "keep: abcXYZdef :keep"), "keep: XYZ :keep");
    assert_eq!(merged("old line one\nold line two\n", "brand new\n", "old line one\nmine\nold line two\n"), "brand new\nmine\n");
    assert_eq!(merged("\u{1F600}\u{1F600}", "\u{1F600}\u{1F389}", "\u{1F44D}\u{1F600}"), "\u{1F44D}\u{1F389}");
    let mixed = merged(&format!("{SEASON}\u{1F600}{DEGREE}"), &format!("{SEASON}\u{1F389}{DEGREE}"), &format!("{SEASON}\u{1F600}\u{1F600}{DEGREE}"));
    assert!(mixed.starts_with(SEASON) && mixed.ends_with(DEGREE));
}

const LOCAL: [&str; 5] = ["A", "B", "C", "\u{1F44D}", "\n"];
const EXTERNAL: [&str; 5] = ["1", "2", "3", "\u{1F389}", "\n"];

struct PointEdit {
    at: usize,
    removed: usize,
    inserted: Vec<String>,
}

fn random_insert(rng: &mut Rng, alphabet: &[&str]) -> Vec<String> {
    (0..rng.below(4)).map(|_| alphabet[rng.below(alphabet.len())].to_owned()).collect()
}

fn apply_point_edits(points: &[String], edits: &[&PointEdit]) -> String {
    let mut out = points.to_vec();
    let mut sorted: Vec<&&PointEdit> = edits.iter().collect();
    sorted.sort_by_key(|edit| std::cmp::Reverse(edit.at));
    for edit in sorted {
        out.splice(edit.at..edit.at + edit.removed, edit.inserted.iter().cloned());
    }
    out.concat()
}

/// Code points used once each, so a diff of an edited text is unambiguous.
fn unique_base(rng: &mut Rng) -> Vec<String> {
    let mut points: Vec<String> = Vec::new();
    let mut next = 0x4e00u32;
    for index in 0..rng.below(30) {
        let roll = rng.below(10);
        if roll == 0 && index > 0 && points[index - 1] != "\n" {
            points.push("\n".into());
        } else if roll == 1 {
            points.push(char::from_u32(0x1f600 + index as u32).unwrap().to_string());
        } else {
            points.push(char::from_u32(next).unwrap().to_string());
            next += 1;
        }
    }
    points
}

#[test]
fn merge_keeps_disjoint_local_and_external_edits() {
    let mut rng = Rng::new(21);
    let mut checked = 0;
    for _ in 0..1500 {
        let points = unique_base(&mut rng);
        let n = points.len();
        if n < 3 {
            continue;
        }
        let first_at = rng.below(n - 2);
        let first_end = first_at + rng.below((n - 2 - first_at).min(3) + 1);
        let second_at = first_end + 1 + rng.below(n - first_end - 1);
        let second_end = second_at + rng.below(n - second_at + 1);
        let mut first = PointEdit { at: first_at, removed: first_end - first_at, inserted: vec![] };
        let mut second = PointEdit { at: second_at, removed: second_end - second_at, inserted: vec![] };
        let (local, external) = if rng.below(2) == 0 { (&mut first, &mut second) } else { (&mut second, &mut first) };
        local.inserted = random_insert(&mut rng, &LOCAL);
        external.inserted = random_insert(&mut rng, &EXTERNAL);
        let (local, external) = (&*local, &*external);
        // An insertion that starts or ends with a line break next to a base line break has two
        // equally short diffs (it slides across that break), so where it lands next to the other
        // side's edit is not defined by the edits alone: such runs are skipped. Only line breaks
        // repeat in the base; every other character is unique.
        let ambiguous = |edit: &PointEdit| {
            let before = edit.at.checked_sub(1).and_then(|at| points.get(at)).is_some_and(|point| point == "\n");
            let after = points.get(edit.at + edit.removed).is_some_and(|point| point == "\n");
            let breaks = edit.inserted.first().is_some_and(|point| point == "\n") || edit.inserted.last().is_some_and(|point| point == "\n");
            (before || after) && breaks
        };
        if ambiguous(local) || ambiguous(external) {
            continue;
        }
        let base = points.concat();
        let current = apply_point_edits(&points, &[local]);
        let incoming = apply_point_edits(&points, &[external]);
        let expected = apply_point_edits(&points, &[local, external]);
        assert_eq!(merged(&base, &incoming, &current), expected, "{base:?} {current:?} {incoming:?}");
        checked += 1;
    }
    assert!(checked > 1000, "only {checked} runs were unambiguous");
}

#[test]
fn merge_answers_nothing_for_an_unchanged_file_and_the_file_without_local_edits() {
    let mut rng = Rng::new(22);
    let alphabet = ["a", "b", "\n", SEASON, "\u{1F600}", "\u{1F44D}"];
    for _ in 0..1000 {
        let base: String = (0..rng.below(30)).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        let edited = mutate(&mut rng, &base, &alphabet);
        assert!(merge_edits(&chars(&base), &chars(&base), &chars(&edited)).is_empty());
        assert_eq!(merged(&base, &edited, &base), edited);
    }
}

#[test]
fn merge_never_fails_on_random_edit_pairs_and_keeps_every_insertion() {
    let mut rng = Rng::new(23);
    let base_alphabet = ["a", "b", " ", "\n", SEASON, "\u{1F600}"];
    let only = |text: &str, alphabet: &[&str]| text.chars().filter(|c| *c != '\n' && alphabet.contains(&c.to_string().as_str())).collect::<String>();
    for _ in 0..1500 {
        let points: Vec<String> = (0..rng.below(25)).map(|_| base_alphabet[rng.below(base_alphabet.len())].to_owned()).collect();
        let random_edits = |rng: &mut Rng, alphabet: &[&str]| {
            let mut edits = Vec::new();
            let mut at = 0;
            let mut count = rng.below(4);
            while count > 0 && at <= points.len() {
                at += rng.below(points.len() - at + 1);
                let removed = rng.below((points.len() - at).min(4) + 1);
                edits.push(PointEdit { at, removed, inserted: random_insert(rng, alphabet) });
                at += removed + 1;
                count -= 1;
            }
            edits
        };
        let local = random_edits(&mut rng, &LOCAL);
        let external = random_edits(&mut rng, &EXTERNAL);
        let base = points.concat();
        let current = apply_point_edits(&points, &local.iter().collect::<Vec<_>>());
        let incoming = apply_point_edits(&points, &external.iter().collect::<Vec<_>>());
        let edits = merge_edits(&chars(&base), &chars(&incoming), &chars(&current));
        assert_well_formed(&edits, current.chars().count());
        let result = apply(&current, &edits);
        assert_eq!(only(&result, &LOCAL), only(&current, &LOCAL), "{base:?} {current:?} {incoming:?}");
        assert_eq!(only(&result, &EXTERNAL), only(&incoming, &EXTERNAL), "{base:?} {current:?} {incoming:?}");
    }
}

/// Markdown of about `size` characters: headings, prose with CJK and emoji, lists, tables, code.
fn generated_markdown(size: usize) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let (mut index, mut length) = (0usize, 0usize);
    while length < size {
        let block = match index % 5 {
            0 => format!("{} Section {index} \u{1F600}", "#".repeat(index % 3 + 1)),
            1 => format!(
                "Paragraph {index} covers the {QUARTER} plan \u{1F389} in ordinary sentences, long enough to look like real prose, and a few more words."
            ),
            2 => (0..4).map(|item| format!("- Item {index}.{item}: a task \u{1F44D}")).collect::<Vec<_>>().join("\n"),
            3 => {
                let rows: Vec<String> = (0..3).map(|row| format!("| Row {index}.{row} | {SEASON} |")).collect();
                format!("| Name | Status |\n| --- | --- |\n{}", rows.join("\n"))
            }
            _ => format!("```ts\nfunction step{index}() {{\n  return 1\n}}\n```"),
        };
        length += block.chars().count() + 2;
        blocks.push(block);
        index += 1;
    }
    format!("{}\n", blocks.join("\n\n"))
}

fn largest_edit(edits: &[TextEdit]) -> usize {
    edits.iter().map(|edit| (edit.to - edit.from).max(edit.insert.chars().count())).max().unwrap_or(0)
}

#[test]
fn merge_keeps_every_scattered_edit_of_a_large_document_in_small_edits() {
    let base = chars(&generated_markdown(1_000_000));
    for seed in [31, 32] {
        let mut rng = Rng::new(seed);
        let (local, external, both) = disjoint_changes(&mut rng, &base, 1.0 / 67.0, 1.0 / 3.0, 24);
        let current = chars(&apply_edits(&base, &local));
        let incoming = chars(&apply_edits(&base, &external));
        let expected = apply_edits(&base, &both);
        let edits = merge_edits(&base, &incoming, &current);
        assert_well_formed(&edits, current.len());
        assert!(apply_edits(&current, &edits) == expected, "seed {seed}");
        assert!(largest_edit(&edits) <= 64 * 1024);
        for text in [&incoming, &current] {
            let side = diff_text(&base, text);
            assert!(apply_edits(&base, &side) == text.iter().collect::<String>());
            assert!(largest_edit(&side) < 1000);
        }
    }
}

#[test]
fn diff_of_a_heavily_rewritten_document_stays_line_sized() {
    let base = generated_markdown(500_000);
    let rewrite = |line: &str| {
        if line.starts_with("Paragraph") || line.starts_with("- ") { line.split(' ').rev().collect::<Vec<_>>().join(" ") } else { line.to_owned() }
    };
    let incoming = base.split('\n').map(rewrite).collect::<Vec<_>>().join("\n");
    let edits = diff(&base, &incoming);
    assert_well_formed(&edits, base.chars().count());
    assert!(apply(&base, &edits) == incoming);
    assert!(largest_edit(&edits) < 1000);
}

#[test]
fn a_block_whose_every_line_changed_is_refined_line_by_line() {
    for count in [100, 2000] {
        let base: String = (0..count).map(|index| format!("Line {index}, some ordinary text for the document.\n")).collect();
        let incoming = base.replace("ordinary", "plain");
        let edits = diff(&base, &incoming);
        assert_eq!(edits.len(), count);
        assert_eq!(largest_edit(&edits), "ordinary".len());
        let current = base.replacen("Line 7, some", "Line 7, one", 1);
        assert_eq!(merged(&base, &incoming, &current), incoming.replacen("Line 7, some", "Line 7, one", 1));
    }
}

#[test]
fn a_huge_changed_line_is_replaced_whole() {
    let mut rng = Rng::new(35);
    let mut block = |letters: &[char]| (0..100_000).map(|_| letters[rng.below(letters.len())]).collect::<String>();
    let base = format!("start\n{}\nend\n", block(&['a', 'b']));
    let incoming = format!("start\n{}\nend\n", block(&['a', 'b']));
    let n = base.chars().count();
    assert_eq!(diff(&base, &incoming), vec![edit(6, n - 4, &incoming[6..incoming.len() - 4])]);
}

#[test]
fn coalescing_joins_edits_across_short_unchanged_runs() {
    let text = chars("ab\u{1F600}cd\u{1F600}efghij");
    let edits = vec![edit(1, 2, "B"), edit(3, 4, "C"), edit(5, 6, ""), edit(11, 12, "J")];
    let joined = coalesce_edits(&edits, &text, 1);
    assert_eq!(joined, vec![edit(1, 6, "B\u{1F600}Cd"), edit(11, 12, "J")]);
    assert_eq!(apply_edits(&text, &joined), apply_edits(&text, &edits));
    assert_eq!(coalesce_edits(&edits, &text, 0), edits);
}
