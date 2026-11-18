//! The note's pages for `render` and `render_area` (manifest `ui.layout: pages`): the reading
//! view laid out by `kition-note-export` on US Letter pages (612 × 792 document pixels, one-inch
//! margins), one under the other. Boxes and rules are exact; text is greeked as bars until the
//! engine has a glyph rasteriser. `render` and `render_area` read the same pixel function, so a
//! whole-number reduction is the exact box mean the contract asks for.

use std::cell::OnceCell;

use kition_note_export::raster::Raster;
use kition_note_export::{Layout, Options};

/// Page size in document pixels (points).
pub const PAGE_WIDTH: u32 = 612;
pub const PAGE_HEIGHT: u32 = 792;

/// A note's pages: laid out, and indexed for drawing the first time something is drawn.
#[derive(Clone, Debug)]
pub struct Pages {
    layout: Layout,
    raster: OnceCell<Raster>,
}

impl Pages {
    pub fn layout(text: &str) -> Self {
        Self { layout: kition_note_export::pages(text, Options::default()), raster: OnceCell::new() }
    }

    fn raster(&self) -> &Raster {
        self.raster.get_or_init(|| Raster::new(&self.layout))
    }

    pub fn pages(&self) -> u32 {
        u32::try_from(self.layout.pages.len()).unwrap_or(u32::MAX).max(1)
    }

    pub fn height(&self) -> u32 {
        self.pages().saturating_mul(PAGE_HEIGHT)
    }

    pub fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Vec<u8> {
        let (width, height) = (width.clamp(1, kition_kit::MAX_RENDER_SIDE), height.clamp(1, kition_kit::MAX_RENDER_SIDE));
        self.raster().render(x, y, scale, width, height)
    }

    pub fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Vec<u8> {
        let (width, height) = (width.clamp(1, kition_kit::MAX_RENDER_SIDE), height.clamp(1, kition_kit::MAX_RENDER_SIDE));
        self.raster().render_area(x0, y0, x1, y1, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAPER: [u8; 4] = [255, 255, 255, 255];

    #[test]
    fn a_note_draws_on_letter_pages_and_long_notes_add_pages() {
        let pages = Pages::layout("# Title\n\nbody");
        assert_eq!(pages.pages(), 1);
        assert_eq!(pages.height(), PAGE_HEIGHT);
        let rgba = pages.render(0.0, 0.0, 1.0, PAGE_WIDTH, PAGE_HEIGHT);
        assert_eq!(rgba.len(), (PAGE_WIDTH * PAGE_HEIGHT * 4) as usize);
        assert!(rgba.chunks(4).any(|px| px != PAPER), "the text draws");
        assert_eq!(rgba.get(..4), Some(&PAPER[..]), "the margin is paper");
        let long = "word ".repeat(20000);
        assert!(Pages::layout(&long).pages() > 1);
    }

    #[test]
    fn an_empty_note_is_one_blank_page_and_outside_is_transparent() {
        let pages = Pages::layout("");
        assert_eq!(pages.height(), PAGE_HEIGHT);
        assert!(pages.render(0.0, 0.0, 0.25, 153, 198).chunks(4).all(|px| px == PAPER));
        assert_eq!(pages.render(-4.0, 0.0, 1.0, 1, 1), vec![0, 0, 0, 0]);
    }
}
