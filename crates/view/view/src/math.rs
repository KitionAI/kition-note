//! Math formulas (`$…$`, `$$…$$`) laid out as TeX does, with latex-rust (pure Rust), in STIX Two
//! Math (SIL Open Font License, the face latex-rust ships and is tested with), and flattened into
//! glyphs and rules the view draws: Obsidian draws its math as formulas away from the caret, and
//! so does the note.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use kition_note_fonts::FaceId;
use latex_rust::layout::{BoxContent, MathBox};
use latex_rust::{Dim, MathFont, MathStyle};

/// A formula in pixels: its width, its height above and depth below the baseline, its glyphs
/// (math face glyph id, x, baseline y with y down, pixels per font unit), and its rules
/// (x0, y0, x1, y1 from the baseline, y down).
#[derive(Debug, Default)]
pub struct Formula {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub face: Option<FaceId>,
    pub glyphs: Vec<(u32, f32, f32, f32)>,
    pub rules: Vec<[f32; 4]>,
}

fn px(dim: &Dim) -> f32 {
    f32::from_bits(dim.to_ieee32_bits())
}

fn font() -> Option<&'static MathFont> {
    static FONT: OnceLock<Option<MathFont>> = OnceLock::new();
    FONT.get_or_init(|| MathFont::stix_two_math().ok()).as_ref()
}

type Cache = HashMap<(String, bool, u32), Option<Arc<Formula>>>;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Formulas memoised past this many are dropped.
const CACHE_MAX: usize = 4096;

/// `source` (without its dollars) as a formula at `size` pixels to the em, in display style or
/// text style; `None` when it does not parse or lay out (the note then shows its source).
pub fn formula(source: &str, display: bool, size: f32) -> Option<Arc<Formula>> {
    let key = (source.to_owned(), display, size.to_bits());
    if let Some(found) = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key) {
        return found.clone();
    }
    let made = lay_out(source, display, size).map(Arc::new);
    let mut cached = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if cached.len() >= CACHE_MAX {
        cached.clear();
    }
    cached.insert(key, made.clone());
    made
}

fn lay_out(source: &str, display: bool, size: f32) -> Option<Formula> {
    let font = font()?;
    let ast = latex_rust::parse(source.trim()).ok()?;
    let boxed = latex_rust::layout(&ast, font, if display { MathStyle::Display } else { MathStyle::Text }).ok()?;
    let face = kition_note_fonts::math_face(latex_rust::STIX_TWO_MATH_OTF);
    let per_unit = size / f32::from(font.units_per_em().max(1));
    let mut formula =
        Formula { width: px(&boxed.width) * size, ascent: px(&boxed.height) * size, descent: px(&boxed.depth) * size, face, ..Formula::default() };
    walk(&boxed, 0.0, 0.0, size, per_unit, &mut formula);
    Some(formula)
}

/// Places `bx` with its left edge at `x` on a parent baseline `baseline` (pixels, y down), as
/// latex-rust's own SVG renderer reads the box model.
fn walk(bx: &MathBox, x: f32, parent_baseline: f32, em: f32, per_unit: f32, out: &mut Formula) {
    let baseline = parent_baseline - px(&bx.shift) * em;
    match &bx.content {
        BoxContent::Empty | BoxContent::Kern(_) => {}
        BoxContent::Glyph { glyph_id, scale, .. } => out.glyphs.push((u32::from(*glyph_id), x, baseline, per_unit * px(scale))),
        BoxContent::Rule => {
            let (width, height, depth) = (px(&bx.width) * em, px(&bx.height) * em, px(&bx.depth) * em);
            if width > 0.0 && height + depth > 0.0 {
                out.rules.push([x, baseline - height, x + width, baseline + depth]);
            }
        }
        BoxContent::HList(children) => {
            let mut at = x;
            for child in children {
                walk(child, at, baseline, em, per_unit, out);
                at += px(&child.width) * em;
            }
        }
        BoxContent::VList(children) => {
            let Some(first) = children.first() else { return };
            walk(first, x, baseline, em, per_unit, out);
            let mut below = px(&first.depth) * em;
            for child in children.iter().skip(1) {
                let child_baseline = baseline + below + px(&child.height) * em;
                walk(child, x, child_baseline, em, per_unit, out);
                below += (px(&child.height) + px(&child.depth)) * em;
            }
        }
        BoxContent::Overlap(children) => {
            for child in children {
                walk(child, x, baseline, em, per_unit, out);
            }
        }
        BoxContent::Color(_, inner) | BoxContent::BackColor(_, inner) => walk(inner, x, baseline, em, per_unit, out),
        BoxContent::Line { x1, y1, x2, y2, thickness } => {
            // Drawn as the line's bounding box when straight (cancel strokes are rare).
            let (ax, ay, bx2, by) = (x + px(x1) * em, baseline - px(y1) * em, x + px(x2) * em, baseline - px(y2) * em);
            let half = px(thickness) * em / 2.0;
            out.rules.push([ax.min(bx2) - half, ay.min(by) - half, ax.max(bx2) + half, ay.max(by) + half]);
        }
        BoxContent::Frame { thickness, inner, .. } => {
            let (width, height, depth, t) = (px(&bx.width) * em, px(&bx.height) * em, px(&bx.depth) * em, px(thickness) * em);
            let (top, bottom) = (baseline - height, baseline + depth);
            out.rules.push([x, top, x + width, top + t]);
            out.rules.push([x, bottom - t, x + width, bottom]);
            out.rules.push([x, top, x + t, bottom]);
            out.rules.push([x + width - t, top, x + width, bottom]);
            walk(inner, x, baseline, em, per_unit, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fraction_lays_out_with_a_bar_and_glyphs() {
        let fraction = formula(r"\frac{1}{3}", true, 16.0).unwrap();
        assert!(fraction.width > 4.0 && fraction.ascent > 8.0 && fraction.descent > 4.0, "{fraction:?}");
        assert_eq!(fraction.rules.len(), 1, "one fraction bar");
        assert_eq!(fraction.glyphs.len(), 2);
        assert!(formula(r"\frac{1}{", false, 16.0).is_none(), "a formula that does not parse shows its source");
        let inline = formula("E = mc^2", false, 16.0).unwrap();
        assert!(inline.glyphs.len() >= 5);
        let raised = inline.glyphs.last().unwrap();
        assert!(raised.2 < inline.glyphs[0].2, "the exponent is raised above the baseline");
    }

    #[test]
    fn a_dvips_color_name_lays_out_with_the_value_latex_gives_it() {
        let colored = formula(r"\color{TealBlue}{\alpha + \beta}", false, 16.0).unwrap();
        assert_eq!(colored.glyphs.len(), 3);
        // TealBlue is CMYK (0.86, 0, 0.34, 0.02) in LaTeX; the other 67 dvips names resolve too.
        assert_eq!(latex_rust::named_color("TealBlue").unwrap().css_hex(), "#23faa5");
        assert_eq!(latex_rust::named_color("Sepia").unwrap().css_hex(), "#4d0d00");
        assert_eq!(latex_rust::ColorTable::new().len(), 8 + 68, "the 8 base colors and the 68 dvips names");
    }
}
