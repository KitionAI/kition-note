//! What `kition-note` gives the engine before it serves: the fonts the kit ships.

include!(concat!(env!("OUT_DIR"), "/shipped_fonts.rs"));

/// The font files built into the executable (`build.rs`), relative to a kition-fonts checkout.
pub fn shipped_files() -> Vec<&'static str> {
    SHIPPED.iter().map(|(file, _)| *file).collect()
}

/// Gives the engine the fonts the kit ships (the faces the in-page editor gives it: Inter for
/// prose, JetBrains Mono for code, Droid Sans Fallback for Chinese and Japanese, Noto Color Emoji),
/// built into the executable, so notes lay out and draw with real glyphs with no font folder to
/// find; answers how many files the engine took.
pub fn add_shipped_fonts() -> usize {
    SHIPPED.iter().filter(|(_, bytes)| kition_note_fonts::add_font(bytes.to_vec()).is_ok()).count()
}
