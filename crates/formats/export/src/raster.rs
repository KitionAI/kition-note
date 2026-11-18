//! The pages as pixels, for `KitDocument::render` (previews, thumbnails, the agent's look at a
//! page): the layout's boxes and rules drawn exactly, one pixel per point, and its text greeked as
//! bars over each run's x-height (the engine has no glyph rasteriser yet).
//! Pages sit one under the other, without gaps.

use crate::layout::{Color, Layout, Mark};

const PAPER: [u8; 4] = [255, 255, 255, 255];
/// Rows per bucket of the shape index.
const BAND: i64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    color: [u8; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PagePixels {
    shapes: Vec<Shape>,
    /// For each band of rows, the shapes that touch it, in drawing order.
    bands: Vec<Vec<usize>>,
}

/// A layout's pages as pixels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Raster {
    width: u32,
    page_height: u32,
    pages: Vec<PagePixels>,
}

fn rgba(color: Color) -> [u8; 4] {
    [color.0, color.1, color.2, 255]
}

/// Text drawn as a bar: its color toward the paper, so a page reads as text at a glance.
fn greek(color: Color) -> [u8; 4] {
    let mix = |value: u8| (u16::from(value) * 55 / 100 + 255 * 45 / 100) as u8;
    [mix(color.0), mix(color.1), mix(color.2), 255]
}

fn shape(x: f64, y: f64, w: f64, h: f64, color: [u8; 4]) -> Option<Shape> {
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (x0, y0) = (x.round() as i64, y.round() as i64);
    let x1 = ((x + w).round() as i64).max(x0 + 1);
    let y1 = ((y + h).round() as i64).max(y0 + 1);
    Some(Shape { x0, y0, x1, y1, color })
}

fn shapes_of(marks: &[Mark]) -> Vec<Shape> {
    let mut out = Vec::new();
    for mark in marks {
        match mark {
            Mark::Rect { x, y, w, h, fill } => out.extend(shape(*x, *y, *w, *h, rgba(*fill))),
            Mark::Line { x1, y1, x2, y2, width, color } => {
                let thick = width.max(1.0);
                let (left, top) = (x1.min(*x2), y1.min(*y2));
                let (w, h) = ((x2 - x1).abs(), (y2 - y1).abs());
                if h < 0.01 {
                    out.extend(shape(left, top - thick / 2.0, w.max(1.0), thick, rgba(*color)));
                } else if w < 0.01 {
                    out.extend(shape(left - thick / 2.0, top, thick, h.max(1.0), rgba(*color)));
                } else {
                    out.extend(shape(left, top, w, h, rgba(*color)));
                }
            }
            Mark::Text { x, y, size, face, color, text } => {
                let width = crate::metrics::text_width(*face, *size, text);
                // Inner spaces stay bars too: a run reads as a line of text.
                out.extend(shape(*x, y - size * 0.55, width, size * 0.5, greek(*color)));
            }
            Mark::Missing { x, y, w, h, color } => {
                let c = greek(*color);
                out.extend(shape(*x, *y, *w, 1.0, c));
                out.extend(shape(*x, y + h - 1.0, *w, 1.0, c));
                out.extend(shape(*x, *y, 1.0, *h, c));
                out.extend(shape(x + w - 1.0, *y, 1.0, *h, c));
            }
            Mark::Link { .. } => {}
        }
    }
    out
}

impl Raster {
    pub fn new(layout: &Layout) -> Self {
        let width = layout.size.width.round().max(1.0) as u32;
        let page_height = layout.size.height.round().max(1.0) as u32;
        let bands_per_page = usize::try_from(i64::from(page_height).div_euclid(BAND) + 1).unwrap_or(1);
        let pages = layout
            .pages
            .iter()
            .map(|page| {
                let shapes = shapes_of(&page.marks);
                let mut bands = vec![Vec::new(); bands_per_page];
                for (index, shape) in shapes.iter().enumerate() {
                    let first = shape.y0.max(0).div_euclid(BAND);
                    let last = (shape.y1 - 1).max(0).div_euclid(BAND);
                    for band in first..=last {
                        if let Some(list) = usize::try_from(band).ok().and_then(|band| bands.get_mut(band)) {
                            list.push(index);
                        }
                    }
                }
                PagePixels { shapes, bands }
            })
            .collect();
        Self { width, page_height, pages }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn pages(&self) -> u32 {
        u32::try_from(self.pages.len()).unwrap_or(u32::MAX).max(1)
    }

    pub fn height(&self) -> u32 {
        self.pages().saturating_mul(self.page_height)
    }

    /// One document pixel: transparent outside the pages.
    pub fn pixel(&self, x: i64, y: i64) -> [u8; 4] {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height()) {
            return [0; 4];
        }
        let page_height = i64::from(self.page_height);
        let (page, inside) = (y / page_height, y % page_height);
        let Some(page) = usize::try_from(page).ok().and_then(|page| self.pages.get(page)) else { return PAPER };
        let Some(band) = usize::try_from(inside / BAND).ok().and_then(|band| page.bands.get(band)) else { return PAPER };
        for index in band.iter().rev() {
            if let Some(shape) = page.shapes.get(*index)
                && x >= shape.x0
                && x < shape.x1
                && inside >= shape.y0
                && inside < shape.y1
            {
                return shape.color;
            }
        }
        PAPER
    }

    /// `KitDocument::render` (the caller bounds `width` and `height`): below scale 1 each output pixel is the mean of the pixels it covers;
    /// above 1 pixels repeat.
    pub fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Vec<u8> {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let (width, height) = (width.max(1), height.max(1));
        if scale >= 1.0 {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for row in 0..height {
                let sy = (y + f64::from(row) / scale).floor() as i64;
                for col in 0..width {
                    out.extend(self.pixel((x + f64::from(col) / scale).floor() as i64, sy));
                }
            }
            return out;
        }
        let (x0, y0) = (x.floor() as i32, y.floor() as i32);
        let span_x = ((f64::from(width) / scale).ceil() as i32).max(1);
        let span_y = ((f64::from(height) / scale).ceil() as i32).max(1);
        self.render_area(x0, y0, x0.saturating_add(span_x), y0.saturating_add(span_y), width, height)
    }

    /// `KitDocument::render_area`: output pixel `t` covers source `[t·span/n, (t+1)·span/n)`.
    pub fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Vec<u8> {
        let (width, height) = (width.max(1), height.max(1));
        let span_x = i64::from(x1.saturating_sub(x0)).max(1);
        let span_y = i64::from(y1.saturating_sub(y0)).max(1);
        let (n, m) = (i64::from(width), i64::from(height));
        let edges = |t: i64, span: i64, count: i64| {
            let a = t * span / count;
            (a, ((t + 1) * span / count).max(a + 1))
        };
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for row in 0..m {
            let (sy0, sy1) = edges(row, span_y, m);
            for col in 0..n {
                let (sx0, sx1) = edges(col, span_x, n);
                let mut sum = [0u64; 4];
                for sy in sy0..sy1 {
                    for sx in sx0..sx1 {
                        let px = self.pixel(i64::from(x0) + sx, i64::from(y0) + sy);
                        for (total, value) in sum.iter_mut().zip(px) {
                            *total += u64::from(value);
                        }
                    }
                }
                let count = ((sx1 - sx0) * (sy1 - sy0)).max(1) as u64;
                out.extend(sum.map(|total| ((total + count / 2) / count) as u8));
            }
        }
        out
    }
}
