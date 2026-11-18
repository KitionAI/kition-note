use kition_note_markdown::index;

use super::*;

#[test]
fn minimal_shrinks_to_the_difference() {
    let text = "abc def ghi";
    assert_eq!(minimal(text, 0..11, "abc xyz ghi"), vec![Edit::new(4..7, "xyz")]);
    assert!(minimal(text, 0..3, "abc").is_empty());
    assert_eq!(minimal("ééé", 0..6, "éaé"), vec![Edit::new(2..4, "a")]);
}

#[test]
fn insert_block_adds_blank_lines_only_where_needed() {
    assert_eq!(apply("a\nb\n", &[insert_block("a\nb\n", 2, "x")]), "a\n\nx\n\nb\n");
    assert_eq!(apply("a\n\nb\n", &[insert_block("a\n\nb\n", 3, "x")]), "a\n\nx\n\nb\n");
    assert_eq!(apply("a", &[insert_block("a", 1, "x")]), "a\n\nx\n");
    assert_eq!(apply("", &[insert_block("", 0, "x")]), "x\n");
}

#[test]
fn errors_say_their_reason_first() {
    assert_eq!(EditError::coded(Reason::Refused, "note.property-complex", "no").to_string(), "refused: note.property-complex: no");
    assert_eq!(invalid("bad").to_string(), "invalid-params: bad");
}

#[test]
fn overlapping_edits_are_refused() {
    assert!(normalize(vec![Edit::new(0..3, "x"), Edit::new(2..4, "y")]).is_err());
    assert_eq!(normalize(vec![Edit::new(5..6, "x"), Edit::new(0..1, "y")]).unwrap()[0].range, 0..1);
}

#[test]
fn content_end_drops_trailing_blank_lines() {
    let text = "# A\n\nx\n\n\n# B\n";
    assert_eq!(content_end(text, 0..9), 7);
    let ix = index(text);
    assert_eq!(&text[ix.headings[0].section.clone()], "# A\n\nx\n\n\n");
}
