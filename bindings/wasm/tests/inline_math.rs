//! Regression: `$$…$$` opened and closed on one line, with text after
//! it, is inline math, not an unclosed math block.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_note_kit::NoteDocument;

fn codes(text: &str) -> Vec<String> {
    NoteDocument::open("note.md", text.as_bytes()).unwrap().check().into_iter().map(|found| found.code).collect()
}

#[test]
fn inline_display_math_is_closed() {
    for text in ["# Energy\n\n$$E=mc^2$$ and text\n", "$$a+b$$, then $$c$$ too.\n\nMore.\n", "Text $$x$$\n", "$$E=mc^2$$\n"] {
        assert!(!codes(text).contains(&"note.unclosed-fence".to_owned()), "{text:?}: {:?}", codes(text));
    }
    // A block that really is open still reports.
    assert!(codes("$$\nx\n").contains(&"note.unclosed-fence".to_owned()));
    assert!(codes("# A\n\n$$ x = 1\nmore\n").contains(&"note.unclosed-fence".to_owned()));
}
