//! The note's exports and page layout as commands: `file.export {format}` (HTML or PDF, the command
//! `note_export` runs, `kit.json` `agent.export`) and `page.layout`, read-only, for the agent and
//! the editor; the page's editor takes the bytes through the `exportHtml` and `exportPdf` extras
//! instead of base64.
//!
//! The commands answer the file's content; they do not write a file. Writing an engine's output
//! to a file the host creates (KHP-0002 E16, `to: file(create, …)` and `takeOutput`) is not in
//! kition-sdk yet, and neither is the stepped export (`exportStep`, E8).

use kition_note_export::layout::{Layout, Unsupported};
use kition_note_export::{Options, PageSize};
use serde_json::{Value, json};

use crate::NoteDocument;

/// At most this many placed blocks in one `page.layout` answer (A25 paging by `page`).
const MAX_BLOCKS: usize = 500;

fn page_size(params: &Value) -> Result<PageSize, String> {
    match params.get("pageSize") {
        None | Some(Value::Null) => Ok(PageSize::LETTER),
        Some(Value::String(name)) => {
            PageSize::named(name).ok_or_else(|| format!("invalid-params: pageSize must be \"letter\" or \"a4\", not {name:?}"))
        }
        Some(other) => Err(format!("invalid-params: pageSize must be a string, not {other}")),
    }
}

fn flag(params: &Value, key: &str, default: bool) -> Result<bool, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(other) => Err(format!("invalid-params: {key} must be a boolean, not {other}")),
    }
}

/// What the pages could not draw as it is, for the answer.
pub fn unsupported(unsupported: &Unsupported) -> Value {
    json!({
        "missingGlyphs": unsupported.missing_glyphs,
        "missingSample": unsupported.missing_sample,
        "images": unsupported.images,
        "embeds": unsupported.embeds,
        "math": unsupported.math,
        "mermaid": unsupported.mermaid,
        "html": unsupported.html,
    })
}

fn rounded(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn layout_answer(layout: &Layout, page: Option<usize>) -> Value {
    let blocks: Vec<Value> = layout
        .blocks
        .iter()
        .filter(|placed| page.is_none_or(|page| placed.page == page))
        .take(MAX_BLOCKS)
        .map(|placed| json!({ "page": placed.page + 1, "line": placed.line, "kind": placed.kind, "x": rounded(placed.x), "y": rounded(placed.y), "width": rounded(placed.w), "height": rounded(placed.h) }))
        .collect();
    let overflow: Vec<Value> = layout
        .overflow
        .iter()
        .map(|overflow| json!({ "page": overflow.page + 1, "line": overflow.line, "kind": overflow.kind, "width": rounded(overflow.width) }))
        .collect();
    json!({
        "pages": layout.pages.len().max(1),
        "pageSize": { "width": layout.size.width, "height": layout.size.height },
        "blocks": blocks,
        "overflow": overflow,
        "unsupported": unsupported(&layout.unsupported),
    })
}

/// Standard base64 (RFC 4648, with padding).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let symbol = |index: u32| ALPHABET.get((index & 63) as usize).map_or('A', |byte| char::from(*byte));
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let (a, b, c) = (u32::from(chunk.first().copied().unwrap_or(0)), chunk.get(1).copied().map(u32::from), chunk.get(2).copied().map(u32::from));
        let triple = (a << 16) | (b.unwrap_or(0) << 8) | c.unwrap_or(0);
        out.push(symbol(triple >> 18));
        out.push(symbol(triple >> 12));
        out.push(if b.is_some() { symbol(triple >> 6) } else { '=' });
        out.push(if c.is_some() { symbol(triple) } else { '=' });
    }
    out
}

/// The note's file name with another extension.
fn named(name: &str, extension: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    let stem = base.rsplit_once('.').map_or(base, |(stem, _)| stem);
    format!("{stem}.{extension}")
}

/// The formats `file.export` writes (`kit.json` `exports`, KHP A41).
pub const EXPORTS: &[&str] = &["html", "pdf"];

/// A file `file.export` wrote: its bytes, and for a PDF its page count and what its pages could not
/// draw.
pub struct Written {
    pub bytes: Vec<u8>,
    pub pdf: Option<(usize, Value)>,
}

/// The note `text` written as `format` (one of [`EXPORTS`]) with `params` (`standalone` for HTML,
/// `pageSize` for a PDF).
pub fn write(text: &str, name: &str, format: &str, params: &Value) -> Result<Written, String> {
    match format {
        "html" => Ok(Written { bytes: kition_note_export::export_html(text, name, flag(params, "standalone", true)?).into_bytes(), pdf: None }),
        "pdf" => {
            let (pdf, layout) = kition_note_export::export_pdf(text, name, Options { size: page_size(params)?, ..Options::default() });
            Ok(Written { bytes: pdf, pdf: Some((layout.pages.len().max(1), unsupported(&layout.unsupported))) })
        }
        other => Err(format!("invalid-params: format must be one of {}, not {other:?}", EXPORTS.join(", "))),
    }
}

/// Runs one of the export commands on `text`.
pub fn run(text: &str, name: &str, command: &str, params: &Value) -> Result<Value, String> {
    match command {
        "file.export" => {
            let format = params
                .get("format")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("invalid-params: format must be one of {}", EXPORTS.join(", ")))?;
            let written = write(text, name, format, params)?;
            let mime = if format == "pdf" { "application/pdf" } else { "text/html" };
            let mut answer = serde_json::Map::new();
            answer.insert("format".into(), json!(format));
            answer.insert("data".into(), json!(base64(&written.bytes)));
            answer.insert("bytes".into(), json!(written.bytes.len()));
            answer.insert("name".into(), json!(named(name, format)));
            answer.insert("mime".into(), json!(mime));
            if let Some((pages, unsupported)) = written.pdf {
                answer.insert("pages".into(), json!(pages));
                answer.insert("unsupported".into(), unsupported);
            }
            Ok(Value::Object(answer))
        }
        "page.layout" => {
            let page = match params.get("page") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let number = value
                        .as_u64()
                        .filter(|number| *number >= 1)
                        .ok_or_else(|| format!("invalid-params: page must be a page number from 1, not {value}"))?;
                    Some(usize::try_from(number - 1).map_err(|_| "out-of-range: page".to_owned())?)
                }
            };
            let layout = kition_note_export::pages(text, Options { size: page_size(params)?, ..Options::default() });
            if let Some(page) = page
                && page >= layout.pages.len().max(1)
            {
                return Err(format!("out-of-range: the note has {} pages", layout.pages.len().max(1)));
            }
            Ok(layout_answer(&layout, page))
        }
        _ => Err(format!("unknown command {command}")),
    }
}

impl NoteDocument {
    /// The `exportHtml` extra.
    pub fn export_html(&self, standalone: bool) -> String {
        kition_note_export::export_html(self.text(), &self.name, standalone)
    }

    /// The `exportPdf` extra: `letter` or `a4`.
    pub fn export_pdf(&self, page_size: &str) -> Result<Vec<u8>, String> {
        let size =
            PageSize::named(page_size).ok_or_else(|| format!("invalid-params: the page size must be \"letter\" or \"a4\", not {page_size:?}"))?;
        Ok(kition_note_export::export_pdf(self.text(), &self.name, Options { size, ..Options::default() }).0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_like_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_commands_answer_the_exports_and_the_layout() {
        use kition_kit::KitDocument;
        let mut doc = NoteDocument::open("Plan.md", b"# Plan\n\nGoals and \xe4\xb8\xad.\n\n```mermaid\ngraph TD\n```\n").expect("opens");
        let decoded = |answer: &Value| {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap_or_default()).unwrap_or_default()
        };
        let html = doc.execute("file.export", json!({ "format": "html", "standalone": false })).expect("html");
        assert_eq!(html["name"], "Plan.html");
        assert_eq!(html["mime"], "text/html");
        let markup = String::from_utf8(decoded(&html)).unwrap_or_default();
        assert!(markup.starts_with("<main>") && markup.contains("<h1 id=\"plan\">Plan</h1>"), "{markup}");
        let pdf = doc.execute("file.export", json!({ "format": "pdf", "pageSize": "a4" })).expect("pdf");
        assert_eq!(pdf["name"], "Plan.pdf");
        assert!(pdf["data"].as_str().is_some_and(|data| data.starts_with("JVBERi0xLjQ")), "base64 of %PDF-1.4");
        assert_eq!(pdf["bytes"].as_u64(), Some(decoded(&pdf).len() as u64));
        assert_eq!(pdf["unsupported"]["missingGlyphs"], 1);
        assert_eq!(pdf["unsupported"]["mermaid"], 1);
        let layout = doc.execute("page.layout", json!({ "page": 1 })).expect("layout");
        assert_eq!(layout["pages"], 1);
        let kinds: Vec<String> = layout["blocks"]
            .as_array()
            .map(|blocks| blocks.iter().map(|block| format!("{}@{}", block["kind"].as_str().unwrap_or(""), block["line"])).collect())
            .unwrap_or_default();
        assert_eq!(kinds, ["heading@1", "paragraph@3", "mermaid@5"]);
        assert!(doc.execute("page.layout", json!({ "page": 2 })).is_err_and(|error| error.starts_with("out-of-range")));
        assert!(doc.execute("file.export", json!({ "format": "pdf", "pageSize": "a3" })).is_err_and(|error| error.starts_with("invalid-params")));
        assert!(doc.execute("file.export", json!({ "format": "docx" })).is_err_and(|error| error.starts_with("invalid-params")));
        assert!(doc.execute("file.export", json!({})).is_err_and(|error| error.starts_with("invalid-params")));
        assert_eq!(doc.export_pdf("letter").map(|bytes| bytes.starts_with(b"%PDF")), Ok(true));
        assert!(!doc.dirty(), "exports change nothing");
    }

    #[test]
    fn names_take_the_export_extension() {
        assert_eq!(named("Plan.md", "pdf"), "Plan.pdf");
        assert_eq!(named("notes/Plan.v2.markdown", "html"), "Plan.v2.html");
        assert_eq!(named("README", "html"), "README.html");
    }
}
