//! `kition-note-export`: the Note kit's reading view. A note's Markdown is read into blocks
//! (`doc`), laid out on pages (`layout`), and drawn three ways from the same layout: pixels for
//! previews (`raster`), a PDF (`pdf`), and, from the blocks, a standalone HTML file (`html`).
//!
//! What the pages cannot draw yet is counted in [`layout::Unsupported`] rather than guessed: the
//! page fonts are PDF's standard Helvetica and Courier (Windows-1252 only, so Chinese, Japanese,
//! Korean and emoji are boxes), images and embeds are labelled boxes (the engine reads no other
//! file), math and Mermaid are their source, and the pixels greek text as bars.

pub mod doc;
pub mod html;
pub mod layout;
pub mod metrics;
pub mod pdf;
pub mod raster;

pub use layout::{Layout, Options, PageSize};

/// The note's title: its first heading, else `name` without its extension.
pub fn title(doc: &doc::Doc, name: &str) -> String {
    if let Some(title) = doc.title.as_ref().filter(|title| !title.trim().is_empty()) {
        return title.trim().to_owned();
    }
    let base = name.rsplit('/').next().unwrap_or(name);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem).to_owned()
}

/// Lays out `text` on pages.
pub fn pages(text: &str, options: Options) -> Layout {
    layout::layout(&doc::read(text), options)
}

/// The note as a PDF, and the layout it drew.
pub fn export_pdf(text: &str, name: &str, options: Options) -> (Vec<u8>, Layout) {
    let doc = doc::read(text);
    let layout = layout::layout(&doc, options);
    (pdf::write(&layout, &title(&doc, name)), layout)
}

/// The note as HTML: a whole file with its stylesheet when `standalone`.
pub fn export_html(text: &str, name: &str, standalone: bool) -> String {
    let doc = doc::read(text);
    html::write(&doc, &title(&doc, name), standalone)
}

#[cfg(test)]
mod tests;
