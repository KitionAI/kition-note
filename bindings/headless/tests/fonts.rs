//! `kition-note` draws with the fonts the kit ships (KHP A39), wherever it runs: its previews lay
//! the note out with the faces its editor gives the engine (Inter, JetBrains Mono, Droid Sans
//! Fallback and Noto Color Emoji, `kition_note_fonts::CHECKOUT_FILES`), with no environment
//! variable naming a font folder. Before, the executable registered no face, so previews drew the
//! note's text as bars.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn issue_headless_shipped_fonts_the_executable_has_its_fonts_without_an_environment_variable() {
    assert_eq!(kition_note::shipped_files(), kition_note_fonts::CHECKOUT_FILES, "the faces the editor gives the engine");
    assert_eq!(kition_note::add_shipped_fonts(), kition_note_fonts::CHECKOUT_FILES.len(), "every face is built in and registers");
    assert!(kition_note_fonts::has_fonts());
}
