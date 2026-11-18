//! The pages as a PDF 1.4 file: the layout's marks in each page's content stream, the text in the
//! standard Helvetica and Courier faces (Windows-1252, not embedded: every reader has them), links
//! as URI annotations. Deterministic: the same note and options give the same bytes (no dates, no
//! ids). Streams are not compressed.

use crate::layout::{Color, Layout, Mark};
use crate::metrics::{Face, win_ansi};

/// A number for a content stream: at most two decimals, no trailing zeros.
fn num(value: f64) -> String {
    if !value.is_finite() {
        return "0".into();
    }
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() { "0".into() } else { text.to_owned() }
}

fn color(color: Color) -> String {
    format!("{} {} {}", num(f64::from(color.0) / 255.0), num(f64::from(color.1) / 255.0), num(f64::from(color.2) / 255.0))
}

/// A PDF literal string of Windows-1252 bytes; characters it cannot hold become `?` (the layout
/// draws those as boxes and never puts them in text marks).
fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('(');
    for c in text.chars() {
        let byte = win_ansi(c).unwrap_or(b'?');
        match byte {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(char::from(byte));
            }
            0x20..=0x7e => out.push(char::from(byte)),
            _ => out.push_str(&format!("\\{byte:03o}")),
        }
    }
    out.push(')');
    out
}

/// A PDF text string for any Unicode text (the document title): UTF-16BE with a byte order mark.
fn unicode_string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        out.push_str(&format!("{unit:04X}"));
    }
    out.push('>');
    out
}

fn font_name(face: Face) -> &'static str {
    match face {
        Face::Regular => "F1",
        Face::Bold => "F2",
        Face::Italic => "F3",
        Face::BoldItalic => "F4",
        Face::Mono => "F5",
        Face::MonoBold => "F6",
    }
}

/// One page's content stream and its links (in PDF coordinates).
fn content(marks: &[Mark], height: f64) -> (String, Vec<([f64; 4], String)>) {
    let mut out = String::new();
    let mut links = Vec::new();
    let flip = |y: f64| height - y;
    for mark in marks {
        match mark {
            Mark::Rect { x, y, w, h, fill } => {
                out.push_str(&format!("{} rg {} {} {} {} re f\n", color(*fill), num(*x), num(flip(y + h)), num(*w), num(*h)))
            }
            Mark::Line { x1, y1, x2, y2, width, color: stroke } => {
                out.push_str(&format!(
                    "{} w {} RG {} {} m {} {} l S\n",
                    num(*width),
                    color(*stroke),
                    num(*x1),
                    num(flip(*y1)),
                    num(*x2),
                    num(flip(*y2))
                ));
            }
            Mark::Text { x, y, size, face, color: fill, text } => {
                out.push_str(&format!(
                    "BT /{} {} Tf {} rg {} {} Td {} Tj ET\n",
                    font_name(*face),
                    num(*size),
                    color(*fill),
                    num(*x),
                    num(flip(*y)),
                    literal(text)
                ));
            }
            Mark::Missing { x, y, w, h, color: stroke } => {
                out.push_str(&format!("0.6 w {} RG {} {} {} {} re S\n", color(*stroke), num(*x), num(flip(y + h)), num(*w), num(*h)));
            }
            Mark::Link { x, y, w, h, uri } => links.push(([*x, flip(y + h), x + w, flip(*y)], uri.clone())),
        }
    }
    (out, links)
}

/// A URI for a link annotation: printable ASCII only, escaped as a literal string.
fn uri_literal(uri: &str) -> String {
    let mut out = String::from("(");
    for byte in uri.bytes() {
        match byte {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(char::from(byte));
            }
            0x21..=0x7e => out.push(char::from(byte)),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out.push(')');
    out
}

/// Writes `layout` as a PDF; `title` goes into the document information.
pub fn write(layout: &Layout, title: &str) -> Vec<u8> {
    let (width, height) = (layout.size.width, layout.size.height);
    // Objects 1 catalog, 2 pages, 3 information, 4..=9 fonts; then per page its page, content and links.
    let mut objects: Vec<String> = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objects.push(String::new());
    objects.push(format!("<< /Title {} /Producer (Kition Note) >>", unicode_string(title)));
    for face in Face::ALL {
        objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>", face.base_font()));
    }
    let fonts = Face::ALL.iter().enumerate().map(|(index, face)| format!("/{} {} 0 R", font_name(*face), index + 4)).collect::<Vec<_>>().join(" ");
    let mut kids = Vec::new();
    let pages: Vec<&[Mark]> = if layout.pages.is_empty() { vec![&[]] } else { layout.pages.iter().map(|page| page.marks.as_slice()).collect() };
    for marks in pages {
        let (stream, links) = content(marks, height);
        let page_id = objects.len() + 1;
        let content_id = page_id + 1;
        let link_ids: Vec<usize> = (0..links.len()).map(|index| content_id + 1 + index).collect();
        let annots = if link_ids.is_empty() {
            String::new()
        } else {
            format!(" /Annots [{}]", link_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" "))
        };
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {fonts} >> >> /Contents {content_id} 0 R{annots} >>",
            num(width),
            num(height)
        ));
        objects.push(format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()));
        for (rect, uri) in links {
            objects.push(format!(
                "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI {} >> >>",
                num(rect[0]),
                num(rect[1]),
                num(rect[2]),
                num(rect[3]),
                uri_literal(&uri)
            ));
        }
        kids.push(page_id);
    }
    if let Some(pages) = objects.get_mut(1) {
        *pages =
            format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" "), kids.len());
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}
