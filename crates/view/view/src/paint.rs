//! The view's pixels, drawn with vello_cpu: the page, each visible line's boxes (code blocks,
//! quote bars, rules, inline backgrounds), then its glyphs as outlines.

use kurbo::{Affine, BezPath, Rect, RoundedRect, RoundedRectRadii, Shape};
use vello_cpu::peniko::Color;
use vello_cpu::{PixmapMut, RenderContext, Resources};

use crate::View;
use crate::layout::{Placed, RowRect};
use crate::style::Block;

fn color(rgba: [u8; 4]) -> Color {
    Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// Italic is drawn slanted: no bundled family has an italic instance.
const SLANT: f64 = -0.2;

pub fn frame(view: &View, x: f64, y: f64, scale: f64, width: u16, height: u16) -> Vec<u8> {
    let (width, height) = (width.max(1), height.max(1));
    let mut ctx = RenderContext::new(width, height);
    let to_screen = Affine::scale(scale) * Affine::translate((-x, -y));
    let theme = view.theme();
    let doc = Rect::new(0.0, 0.0, f64::from(theme.width), f64::from(view.height()));
    ctx.set_transform(to_screen);
    ctx.set_paint(color(theme.paper));
    ctx.fill_rect(&doc);
    let visible = Rect::new(x, y, x + f64::from(width) / scale, y + f64::from(height) / scale);
    if let Some(title) = view.title_line() {
        draw_line(&mut ctx, view, &title, to_screen);
    }
    let lines = view.lines();
    // A line's bottom padding can draw (a math block's preview), so it counts as the line's.
    let first = lines.partition_point(|line| f64::from(line.y + line.boxed.height + line.pad_bottom) < visible.y0 - 4.0);
    let last = first + lines.get(first..).map_or(0, |rest| rest.partition_point(|line| f64::from(line.y) <= visible.y1 + 4.0));
    let shown = lines.get(first..last).unwrap_or(&[]);
    // Every block's boxes first, then every line's text: a box that reaches up over the line
    // before it (a callout's empty line) never covers that line's text.
    for (offset, line) in shown.iter().enumerate() {
        let index = first + offset;
        let prev = index.checked_sub(1).and_then(|before| lines.get(before));
        draw_blocks(&mut ctx, view, line, prev, lines.get(index + 1), to_screen);
    }
    // The text open comment threads are anchored to, tinted under its glyphs.
    for line in shown {
        draw_marks(&mut ctx, view, line, to_screen);
    }
    for line in shown {
        draw_line(&mut ctx, view, line, to_screen);
    }
    if !view.properties_overlaid()
        && let Some(block) = view.properties_block()
        && f64::from(block.top) < visible.y1
        && f64::from(block.bottom) > visible.y0
    {
        draw_properties(&mut ctx, view, &block, to_screen);
    }
    ctx.flush();
    let mut pixels = vec![0u8; usize::from(width) * usize::from(height) * 4];
    let mut resources = Resources::new();
    if let Some(target) = PixmapMut::new(width, height, &mut pixels) {
        ctx.render(target, &mut resources);
    }
    straight(&mut pixels);
    pixels
}

/// A box behind lines that join into one block: rounded at the block's first and last line. Its
/// top and bottom fall on whole device pixels, so a line's box meets the next one's without the
/// faint seam two half-covered tinted edges leave.
fn block_box(ctx: &mut RenderContext, area: Rect, first: bool, last: bool, radius: f64, to_screen: Affine) {
    let [_, _, _, scale, _, shift] = to_screen.as_coeffs();
    let snap = |y: f64| if scale > 0.0 { ((y * scale + shift).round() - shift) / scale } else { y };
    let area = Rect::new(area.x0, snap(area.y0), area.x1, snap(area.y1));
    let (top, bottom) = (if first { radius } else { 0.0 }, if last { radius } else { 0.0 });
    ctx.fill_path(&RoundedRect::from_rect(area, RoundedRectRadii::new(top, top, bottom, bottom)).to_path(0.1));
}

/// A block's own boxes behind its text.
fn draw_blocks(ctx: &mut RenderContext, view: &View, line: &Placed, prev: Option<&Placed>, next: Option<&Placed>, to_screen: Affine) {
    let theme = view.theme();
    let left = f64::from(theme.margin);
    let right = f64::from(theme.width - theme.margin);
    let top = f64::from(line.y);
    let bottom = f64::from(line.y + line.boxed.height);
    // The gap down to the next line, when it continues this block.
    let gap = |joins: bool| if joins { next.map_or(0.0, |next| f64::from(next.y) - bottom) } else { 0.0 };
    ctx.set_transform(to_screen);
    match line.style.block {
        // A callout's code block draws inside the callout's box (below).
        Block::Code { .. } if line.style.callout > 0 => {}
        Block::Code { .. } => {
            // One rounded box for the whole block, as wide as the column.
            let same = |other: &Placed| matches!(other.style.block, Block::Code { .. });
            let joins = next.is_some_and(same);
            let first = !prev.is_some_and(same);
            ctx.set_paint(color(theme.code_block));
            block_box(ctx, Rect::new(left, top, right, bottom + gap(joins)), first, !joins, 4.0, to_screen);
        }
        Block::Rule => {
            let middle = ((top + bottom) / 2.0).round();
            ctx.set_paint(color(theme.rule));
            ctx.fill_rect(&Rect::new(left, middle - 1.0, right, middle + 1.0));
        }
        _ => {}
    }
    // A code line's indent guides: a hairline at the start of each whole indent (a tab or four
    // spaces) of its leading space, as Obsidian draws them; a shorter indent has none.
    if matches!(line.style.block, Block::Code { fence: false } | Block::Indented) && line.boxed.height > 0.0 {
        let starts = crate::layout::indent_unit_starts(view.text().get(line.range.clone()).unwrap_or(""));
        let starts: Vec<usize> = if line.style.block == Block::Indented { starts.into_iter().take(1).collect() } else { starts };
        if !starts.is_empty() {
            ctx.set_paint(color(theme.indent_guide));
            // An indented code block's guide is 14 px into its indent, as Obsidian draws it.
            let inset = if line.style.block == Block::Indented { 14.0 } else { 0.0 };
            for start in starts {
                let x = (f64::from(view.caret(line.range.start + start).x) + inset).round();
                ctx.fill_rect(&Rect::new(x, top, x + 1.0, bottom));
            }
        }
    }
    // A quote's indented text: a hairline 15 px into each whole indent after its marks, as
    // Obsidian draws its quote indents.
    if line.style.quote > 0 && line.style.callout == 0 && line.style.list == 0 && line.style.block == Block::Paragraph {
        let starts = crate::layout::leading_unit_starts(view.text().get(line.range.clone()).unwrap_or(""), true);
        ctx.set_paint(color(theme.indent_guide));
        for start in starts {
            let x = (f64::from(view.caret(line.range.start + start).x) + 15.0).round();
            ctx.fill_rect(&Rect::new(x - 0.5, top, x + 0.5, bottom));
        }
    }
    // A footnote definition's indented continuation: a hairline 0.85 em into each indent, at the
    // definition's size, as Obsidian draws it.
    if line.style.block == Block::Paragraph
        && line.style.list == 0
        && line.style.quote == 0
        && view.styles().bytes.get(line.range.start).is_some_and(|value| value & crate::style::flag::FOOTDEF != 0)
    {
        let size = f64::from(theme.size_of(line.style.block) * crate::theme::INLINE_CODE);
        let starts = crate::layout::indent_unit_starts(view.text().get(line.range.clone()).unwrap_or(""));
        ctx.set_paint(color(theme.indent_guide));
        for start in starts {
            let x = (f64::from(view.caret(line.range.start + start).x) + size * 0.85 + 1.0).round();
            ctx.fill_rect(&Rect::new(x - 0.5, top, x + 0.5, bottom));
        }
    }
    // A nested list's indent guides: a thin line under each level's bullet, as Obsidian draws
    // them, joined to the next line when it is nested as deep.
    let tabs = |placed: &Placed| {
        if placed.style.list == 0 {
            return 0;
        }
        view.text().get(placed.range.clone()).map_or(0, |content| crate::layout::indent_unit_starts(content).len())
    };
    let depth = tabs(line);
    if depth > 0 {
        let size = theme.size_of(line.style.block);
        let tab = f64::from(theme.tab_width(size));
        let next_depth = next.map_or(0, tabs);
        ctx.set_paint(color(theme.indent_guide));
        for level in 0..depth {
            let x = (left + tab * level as f64 + f64::from(size) * 0.97).round();
            let joins = next_depth > level;
            ctx.fill_rect(&Rect::new(x - 0.5, top, x + 0.5, bottom + gap(joins)));
        }
    }
    // A heading's fold chevron in the margin: pointing right when folded, down when open.
    let hovered = view.hover().is_some_and(|hover| view.lines().get(hover).is_some_and(|placed| std::ptr::eq(placed, line)));
    if let Some(folded) = line.fold.filter(|folded| *folded || hovered) {
        // Obsidian's right-triangle icon: a 10 px chevron stroked 1 px, pointing down when
        // open and right when folded, in the faint grey or (folded) the accent.
        let (half, rise) = (3.75, 1.875);
        let cx = f64::from(view.theme().margin + view.fold_x(line));
        let row = f64::from(line.boxed.rows.first().map_or(line.boxed.height, |row| row.height));
        let cy = top + row / 2.0 + if line.style.list > 0 { 1.0 } else { 0.0 };
        let mut path = BezPath::new();
        if folded {
            path.move_to((cx - rise, cy - half));
            path.line_to((cx + rise, cy));
            path.line_to((cx - rise, cy + half));
        } else {
            path.move_to((cx - half, cy - rise));
            path.line_to((cx, cy + rise));
            path.line_to((cx + half, cy - rise));
        }
        ctx.set_paint(color(if folded { theme.link } else { theme.muted }));
        ctx.set_stroke(kurbo::Stroke::new(1.0).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
        ctx.stroke_path(&path);
    }
    // A callout: a rounded box in its kind's tint across the column, with its padding.
    let (box_top, box_bottom) = (top - f64::from(line.pad_top), bottom + f64::from(line.pad_bottom));
    let box_gap = |joins: bool| if joins { next.map_or(0.0, |next| f64::from(next.y - next.pad_top) - box_bottom) } else { 0.0 };
    if line.boxed.height <= 0.0 {
        return;
    }
    if line.style.callout > 0 {
        // The outermost callout's box, then a nested callout's inside it: 24 px in (the outer
        // box's padding) and 12 px from its right, as Obsidian nests them.
        let style = line.style;
        let outermost = |other: &Placed| if other.style.outer > 0 { other.style.outer } else { other.style.callout };
        let kind = outermost(line);
        let joins = next.is_some_and(|next| outermost(next) == kind && next.boxed.height > 0.0);
        let first = prev.is_none_or(|prev| outermost(prev) != kind);
        let area = Rect::new(left, box_top, right, box_bottom + box_gap(joins));
        let tint = crate::theme::callout_tint(theme, kind);
        ctx.set_paint(color(tint));
        block_box(ctx, area, first, !joins, 4.0, to_screen);
        if style.outer > 0 {
            let same = |other: &Placed| other.style.callout == style.callout && other.style.outer == style.outer;
            let inner_joins = next.is_some_and(|next| same(next) && next.boxed.height > 0.0);
            let inner_first = !prev.is_some_and(same);
            // The outer box's end below the nested one's last line is the outer box's alone.
            let outer_end = if joins { 0.0 } else { 28.0 };
            let bottom = box_bottom - outer_end + if inner_joins { box_gap(true) } else { 0.0 };
            ctx.set_paint(color(crate::theme::callout_tint_over(theme, tint, style.callout)));
            block_box(ctx, Rect::new(left + 24.0, box_top, right - 12.0, bottom), inner_first, !inner_joins, 4.0, to_screen);
        }
        // A code block in a callout: its box inside the callout's, 24 px in and 12 px from its
        // right (the callout's padding), as Obsidian renders it.
        if matches!(style.block, Block::Code { .. }) {
            let code = |other: &Placed| matches!(other.style.block, Block::Code { .. }) && other.style.callout == style.callout;
            let code_joins = next.is_some_and(code);
            let code_first = !prev.is_some_and(code);
            let below = if code_joins { next.map_or(0.0, |next| f64::from(next.y) - bottom) } else { 0.0 };
            ctx.set_paint(color(theme.code_block));
            block_box(ctx, Rect::new(left + 24.0, top, right - 12.0, bottom + below), code_first, !code_joins, 4.0, to_screen);
        }
        // A plain quote in a callout: its 2 px bar at the callout's text, each level 26 px in.
        let quotes = crate::theme::Theme::quotes_in_callout(style);
        for level in 0..quotes {
            let x = left + 24.0 + 26.0 * f64::from(level);
            let joins = next.is_some_and(|next| next.style.callout == style.callout && crate::theme::Theme::quotes_in_callout(next.style) > level);
            let below = if joins { next.map_or(0.0, |next| f64::from(next.y) - bottom) } else { 0.0 };
            ctx.set_paint(color(theme.quote_bar));
            ctx.fill_rect(&Rect::new(x, top, x + 2.0, bottom + below));
        }
        // A nested list in a callout (rendered, as Obsidian renders a callout's content): a guide
        // 13.6 px left of each enclosing item's text (its nested list's line), joined to the next
        // line when that is nested as deep.
        if line.style.callout > 0 && line.style.list > 1 && line.style.block == Block::Paragraph {
            let text_start = |placed: &Placed| {
                let content = view.text().get(placed.range.clone()).unwrap_or("");
                let mut at = crate::layout::quote_marks_end(content);
                let rest = content.get(at..).unwrap_or("");
                at += rest.len() - rest.trim_start_matches([' ', '\t']).len();
                let body = content.get(at..).unwrap_or("");
                let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                if ["- ", "* ", "+ "].iter().any(|marker| body.starts_with(marker)) {
                    at += 2;
                } else if digits > 0 && body.get(digits..).is_some_and(|after| after.starts_with(". ") || after.starts_with(") ")) {
                    at += digits + 2;
                }
                if content.get(at..).is_some_and(|rest| rest.len() >= 4 && rest.starts_with('[') && rest.get(2..4) == Some("] ")) {
                    at += 4;
                }
                placed.range.start + at
            };
            let size = f64::from(theme.size_of(line.style.block));
            let tab = f64::from(theme.tab_width(theme.size_of(line.style.block)));
            let x0 = f64::from(view.caret(text_start(line)).x);
            let next_depth = next
                .filter(|next| next.style.callout == line.style.callout && next.style.block == Block::Paragraph)
                .map_or(0, |next| next.style.list);
            ctx.set_paint(color(theme.indent_guide));
            for level in 1..line.style.list {
                let x = (x0 - tab * f64::from(level) - size * 0.85).round();
                let joins = next_depth > line.style.list - level;
                ctx.fill_rect(&Rect::new(x - 0.5, top, x + 0.5, bottom + gap(joins)));
            }
        }
        return;
    }
    // A quote: a thin bar in the accent for each level, 14 px apart from the column's edge.
    for level in 0..line.style.quote {
        let x = left + 14.0 * f64::from(level);
        let joins = next.is_some_and(|next| next.style.quote > level);
        ctx.set_paint(color(theme.quote_bar));
        ctx.fill_rect(&Rect::new(x, box_top, x + 2.0, box_bottom + box_gap(joins)));
    }
}

fn rect_of(line: &Placed, top: f32, left: f32, rect: &RowRect) -> Rect {
    Rect::new(f64::from(left + rect.x0), f64::from(line.y + top + rect.y0), f64::from(left + rect.x1), f64::from(line.y + top + rect.y1))
}

/// Callout icons (Lucide, ISC licence), in a 24-unit box, stroked 2 units wide with round ends,
/// as Obsidian draws them: pencil, flame, triangle-alert, zap, clipboard-list, info,
/// circle-check, check, circle-help, x, bug, list, quote (`theme::CALLOUT_TYPES` names them).
const CALLOUT_ICONS: [&str; 13] = [
    "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z M15 5l4 4",
    "M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z",
    "M21.73 18l-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3z M12 9v4 M12 17h.01",
    "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z",
    "M9 2h6a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2 M12 11h4 M12 16h4 M8 11h.01 M8 16h.01",
    "M22 12a10 10 0 1 1-20 0a10 10 0 1 1 20 0z M12 16v-4 M12 8h.01",
    "M22 12a10 10 0 1 1-20 0a10 10 0 1 1 20 0z M9 12l2 2l4-4",
    "M20 6L9 17l-5-5",
    "M22 12a10 10 0 1 1-20 0a10 10 0 1 1 20 0z M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3 M12 17h.01",
    "M18 6L6 18 M6 6l12 12",
    "M8 2l1.88 1.88 M14.12 3.88L16 2 M9 7.13v-1a3.003 3.003 0 1 1 6 0v1 M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6 M12 20v-9 M6.53 9C4.6 8.8 3 7.1 3 5 M6 13H2 M3 21c0-2.1 1.7-3.9 3.8-4 M20.97 5c0 2.1-1.6 3.8-3.5 4 M22 13h-4 M17.2 17c2.1.1 3.8 1.9 3.8 4",
    "M3 12h.01 M3 18h.01 M3 6h.01 M8 12h13 M8 18h13 M8 6h13",
    "M16 3a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2a1 1 0 0 1 1 1v1a2 2 0 0 1-2 2a1 1 0 0 0-1 1v2a1 1 0 0 0 1 1a6 6 0 0 0 6-6V5a2 2 0 0 0-2-2z M5 3a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2a1 1 0 0 1 1 1v1a2 2 0 0 1-2 2a1 1 0 0 0-1 1v2a1 1 0 0 0 1 1a6 6 0 0 0 6-6V5a2 2 0 0 0-2-2z",
];

/// The external link icon (Lucide `external-link`), after links to the web.
const EXTERNAL_LINK_ICON: &str = "M15 3h6v6 M10 14L21 3 M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6";

fn draw_icon(ctx: &mut RenderContext, area: Rect, kind: u8, to_screen: Affine) {
    let data = match kind {
        crate::layout::EXTERNAL_ICON => Some(EXTERNAL_LINK_ICON),
        crate::layout::CHEVRON_RIGHT => Some("M9 18l6-6-6-6"),
        crate::layout::CHEVRON_DOWN => Some("M6 9l6 6 6-6"),
        _ => CALLOUT_ICONS.get(usize::from(kind.saturating_sub(1))).copied(),
    };
    let Some(path) = data.and_then(|data| BezPath::from_svg(data).ok()) else { return };
    let scale = area.width() / 24.0;
    ctx.set_transform(to_screen * Affine::translate((area.x0, area.y0)) * Affine::scale(scale));
    ctx.set_stroke(kurbo::Stroke::new(2.0).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
    ctx.stroke_path(&path);
}

fn draw_rect(ctx: &mut RenderContext, line: &Placed, top: f32, left: f32, rect: &RowRect, to_screen: Affine) {
    let area = rect_of(line, top, left, rect);
    ctx.set_transform(to_screen);
    ctx.set_paint(color(rect.color));
    if rect.icon > 0 {
        draw_icon(ctx, area, rect.icon, to_screen);
    } else if rect.check {
        // A check mark inside the box.
        let (w, h) = (area.width(), area.height());
        let mut path = BezPath::new();
        path.move_to((area.x0 + w * 0.22, area.y0 + h * 0.52));
        path.line_to((area.x0 + w * 0.42, area.y0 + h * 0.72));
        path.line_to((area.x0 + w * 0.78, area.y0 + h * 0.3));
        ctx.set_stroke(kurbo::Stroke::new((w * 0.14).max(1.0)).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
        ctx.stroke_path(&path);
    } else if rect.dot {
        let (cx, cy) = ((area.x0 + area.x1) / 2.0, (area.y0 + area.y1) / 2.0);
        ctx.fill_path(&kurbo::Circle::new((cx, cy), area.width() / 2.0).to_path(0.1));
    } else if rect.outline {
        ctx.set_stroke(kurbo::Stroke::new(1.0));
        let inner = area.inset(-0.5);
        if rect.radius > 0.0 {
            ctx.stroke_path(&RoundedRect::from_rect(inner, f64::from(rect.radius)).to_path(0.1));
        } else {
            ctx.stroke_rect(&inner);
        }
    } else if rect.radius > 0.0 {
        ctx.fill_path(&RoundedRect::from_rect(area, f64::from(rect.radius)).to_path(0.1));
    } else {
        // A plain fill (a rule, a band) on whole device pixels, as a browser draws borders: a
        // 1 px rule at a fractional height stays one crisp line.
        let [scale_x, _, _, scale_y, shift_x, shift_y] = to_screen.as_coeffs();
        let snap = |value: f64, scale: f64, shift: f64| if scale > 0.0 { ((value * scale + shift).round() - shift) / scale } else { value };
        let snapped = Rect::new(
            snap(area.x0, scale_x, shift_x),
            snap(area.y0, scale_y, shift_y),
            snap(area.x1, scale_x, shift_x),
            snap(area.y1, scale_y, shift_y),
        );
        // A rule never snaps away to nothing.
        let snapped =
            if snapped.height() <= 0.0 { Rect::new(snapped.x0, snapped.y0, snapped.x1, snapped.y0 + 1.0 / scale_y.max(1.0)) } else { snapped };
        let snapped =
            if snapped.width() <= 0.0 { Rect::new(snapped.x0, snapped.y0, snapped.x0 + 1.0 / scale_x.max(1.0), snapped.y1) } else { snapped };
        ctx.fill_rect(&snapped);
    }
}

fn draw_line(ctx: &mut RenderContext, view: &View, line: &Placed, to_screen: Affine) {
    let left = view.theme().margin;
    let boxed = &line.boxed;
    // Where a folded list item's marker ends (its number is drawn in the accent).
    let marker_end = if line.fold == Some(true) && line.style.list > 0 {
        let content = view.text().get(line.range.clone()).unwrap_or("");
        let body = content.trim_start_matches([' ', '\t']);
        let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let after = content.len() - body.len() + digits + 1;
        if digits > 0 {
            boxed.rows.first().and_then(|row| row.stops.iter().find(|stop| stop.byte >= after)).map_or(0.0, |stop| stop.x)
        } else {
            f32::MIN
        }
    } else {
        f32::MIN
    };
    for (index, row) in boxed.rows.iter().enumerate() {
        let marker_end = if index == 0 { marker_end } else { f32::MIN };
        for rect in &row.behind {
            draw_rect(ctx, line, row.top, left, rect, to_screen);
        }
        let top = f64::from(line.y + row.top);
        let mut current: Option<u16> = None;
        for glyph in &row.glyphs {
            let Some(style) = boxed.styles.get(usize::from(glyph.style)) else { continue };
            // An emoji in color: its layers, in font units with y up.
            if let Some(steps) = kition_note_fonts::color_steps(glyph.face, glyph.id) {
                let scale = f64::from(glyph.scale);
                let base =
                    to_screen * Affine::translate((f64::from(left + glyph.x), top + f64::from(glyph.y))) * Affine::scale_non_uniform(scale, -scale);
                crate::color::paint(ctx, base, &steps, style.color);
                current = None;
                continue;
            }
            if glyph.x < marker_end {
                // A folded numbered item's number, in the accent as Obsidian draws it.
                ctx.set_paint(color(view.theme().link));
                current = None;
            } else if current != Some(glyph.style) {
                ctx.set_paint(color(style.color));
                current = Some(glyph.style);
            }
            let path: std::sync::Arc<BezPath> = kition_note_fonts::outline(glyph.face, style.weight, glyph.id);
            if path.elements().is_empty() {
                continue;
            }
            let slant = if style.italic { Affine::skew(SLANT, 0.0) } else { Affine::IDENTITY };
            ctx.set_transform(
                to_screen * Affine::translate((f64::from(left + glyph.x), top + f64::from(glyph.y))) * slant * Affine::scale(f64::from(glyph.scale)),
            );
            ctx.fill_path(&path);
        }
        if let Some((name, rect)) = &row.image
            && let Some(picture) = view.picture(name)
        {
            let area = rect_of(line, row.top, left, rect);
            ctx.set_transform(to_screen);
            ctx.set_paint(vello_cpu::Image {
                image: vello_cpu::ImageSource::Pixmap(picture.pixmap.clone()),
                sampler: vello_cpu::peniko::ImageSampler::default().with_quality(vello_cpu::peniko::ImageQuality::Medium),
            });
            ctx.set_paint_transform(
                Affine::translate((area.x0, area.y0))
                    * Affine::scale_non_uniform(area.width() / f64::from(picture.width.max(1)), area.height() / f64::from(picture.height.max(1))),
            );
            ctx.fill_rect(&area);
            ctx.reset_paint_transform();
        }
        for rect in &row.over {
            if rect.dot && line.fold == Some(true) {
                // A folded item's bullet: in the accent, in a 4 px ring of it at a tenth, as Obsidian's.
                let theme = view.theme();
                let mut ring = theme.link;
                ring[3] = 26;
                draw_rect(
                    ctx,
                    line,
                    row.top,
                    left,
                    &RowRect { x0: rect.x0 - 4.0, y0: rect.y0 - 4.0, x1: rect.x1 + 4.0, y1: rect.y1 + 4.0, color: ring, ..*rect },
                    to_screen,
                );
                draw_rect(ctx, line, row.top, left, &RowRect { color: theme.link, ..*rect }, to_screen);
                continue;
            }
            draw_rect(ctx, line, row.top, left, rect, to_screen);
        }
    }
    if line.fold == Some(true) {
        draw_fold_placeholder(ctx, view, line, to_screen);
    }
}

/// A folded line's "…" after its text: 9 px after it, in the muted grey, at the line's size, as
/// Obsidian's `.cm-foldPlaceholder`.
fn draw_fold_placeholder(ctx: &mut RenderContext, view: &View, line: &Placed, to_screen: Affine) {
    let Some(row) = line.boxed.rows.last() else { return };
    let theme = view.theme();
    let size = theme.size_of(line.style.block);
    let end = row.stops.last().map_or(0.0, |stop| stop.x);
    let baseline = row.glyphs.first().map_or(row.height * 0.72, |glyph| glyph.y);
    let weight = row.glyphs.first().and_then(|glyph| line.boxed.styles.get(usize::from(glyph.style))).map_or(400, |style| style.weight);
    let top = f64::from(line.y + row.top);
    let mut x = theme.margin + end + 9.0;
    ctx.set_paint(color(theme.muted));
    for run in kition_note_fonts::shape("\u{2026}", kition_note_fonts::Family::Sans, weight) {
        let scale = size / run.upem;
        for glyph in &run.glyphs {
            let path = kition_note_fonts::outline(run.face, weight, glyph.id);
            ctx.set_transform(
                to_screen
                    * Affine::translate((f64::from(x + glyph.x_offset * scale), top + f64::from(baseline - glyph.y_offset * scale)))
                    * Affine::scale(f64::from(scale)),
            );
            ctx.fill_path(&path);
            x += glyph.advance * scale;
        }
    }
}

/// A comment thread's anchored text: tinted, with a line under it (the editor's
/// `.note-thread-range`).
const MARK_TINT: [u8; 4] = [250, 204, 21, 82];
const MARK_LINE: [u8; 4] = [234, 179, 8, 204];

fn draw_marks(ctx: &mut RenderContext, view: &View, line: &Placed, to_screen: Affine) {
    let left = f64::from(view.theme().margin);
    for mark in view.marks() {
        let (start, end) = (mark.start.max(line.range.start), mark.end.min(line.range.end));
        if start >= end {
            continue;
        }
        let (start, end) = (start - line.range.start, end - line.range.start);
        for row in &line.boxed.rows {
            let (from, to) = (start.max(row.start), end.min(row.end));
            if from >= to {
                continue;
            }
            let x_at = |byte: usize| row.stops.iter().find(|stop| stop.byte >= byte).or(row.stops.last()).map_or(0.0, |stop| f64::from(stop.x));
            let (x0, x1) = (left + x_at(from), left + x_at(to));
            if x1 <= x0 {
                continue;
            }
            let top = f64::from(line.y + row.top);
            let bottom = top + f64::from(row.height);
            ctx.set_transform(to_screen);
            ctx.set_paint(color(MARK_TINT));
            ctx.fill_rect(&Rect::new(x0, top, x1, bottom));
            ctx.set_paint(color(MARK_LINE));
            ctx.fill_rect(&Rect::new(x0, bottom - 2.0, x1, bottom));
        }
    }
}

/// The width of `text` at `size` px and `weight` in the prose face.
fn text_width(text: &str, size: f32, weight: u16) -> f32 {
    kition_note_fonts::shape(text, kition_note_fonts::Family::Sans, weight)
        .iter()
        .map(|run| run.glyphs.iter().map(|glyph| glyph.advance * size / run.upem).sum::<f32>())
        .sum()
}

/// Draws `text` from `x` on `baseline` at `size` px and `weight` in the prose face; answers its
/// width.
#[allow(clippy::too_many_arguments)]
fn draw_text(ctx: &mut RenderContext, text: &str, x: f32, baseline: f32, size: f32, weight: u16, rgba: [u8; 4], to_screen: Affine) -> f32 {
    let mut pen = x;
    ctx.set_paint(color(rgba));
    for run in kition_note_fonts::shape(text, kition_note_fonts::Family::Sans, weight) {
        let scale = size / run.upem;
        for glyph in &run.glyphs {
            let path = kition_note_fonts::outline(run.face, weight, glyph.id);
            ctx.set_transform(
                to_screen
                    * Affine::translate((f64::from(pen + glyph.x_offset * scale), f64::from(baseline - glyph.y_offset * scale)))
                    * Affine::scale(f64::from(scale)),
            );
            ctx.fill_path(&path);
            pen += glyph.advance * scale;
        }
    }
    pen - x
}

/// A Tessera icon `size` px wide with its top-left at (`x`, `y`), stroked in `rgba`.
fn draw_tessera(ctx: &mut RenderContext, name: &str, x: f64, y: f64, size: f64, rgba: [u8; 4], to_screen: Affine) {
    let Some(icon) = kition_tessera_icons::icon(name) else { return };
    let scale = size / f64::from(kition_tessera_icons::GRID);
    ctx.set_transform(to_screen * Affine::translate((x, y)) * Affine::scale(scale));
    ctx.set_paint(color(rgba));
    ctx.set_stroke(kurbo::Stroke::new(f64::from(kition_tessera_icons::STROKE)).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
    for data in icon.paths.iter().chain(icon.fills) {
        if let Ok(path) = BezPath::from_svg(data) {
            ctx.stroke_path(&path);
        }
    }
    for data in icon.fills {
        if let Ok(path) = BezPath::from_svg(data) {
            ctx.fill_path(&path);
        }
    }
}

/// The properties block as the editor draws it (`web/src/Properties.tsx` and its styles, after
/// Obsidian's): the heading, then a row per key with its type's icon, its name in a 124 px column
/// and its value; tags as pills, a checkbox as a box.
fn draw_properties(ctx: &mut RenderContext, view: &View, block: &crate::PropertiesBlock, to_screen: Affine) {
    let theme = view.theme();
    let (muted, text, accent, tag) = (theme.secondary, theme.text, theme.accent, theme.tag_background);
    let left = theme.margin;
    // The heading: 4 px padding 8 px down, 16 px at weight 500 on a 19 px line.
    draw_text(ctx, "Properties", left + 4.0, block.top + 12.0 + 15.0, 16.0, 500, text, to_screen);
    if block.folded {
        return;
    }
    let size = 14.0;
    for row in &block.rows {
        let height = if row.height > 0.0 { row.height } else { 29.0 };
        let middle = row.y + height / 2.0;
        let baseline = middle + size * 0.36;
        let icon = match row.kind {
            "tags" => "tag",
            "aliases" => "corner-up-right",
            "list" => "list",
            "number" => "hash",
            "checkbox" => "square-check",
            "date" => "calendar",
            "datetime" => "clock",
            _ => "text-align-start",
        };
        draw_tessera(ctx, icon, f64::from(left + 4.0), f64::from(middle - 8.0), 16.0, muted, to_screen);
        draw_text(ctx, &row.key, left + 28.0, baseline, size, 400, muted, to_screen);
        let mut x = left + 152.0;
        match row.kind {
            "tags" => {
                for value in &row.values {
                    let width = text_width(value, size, 400) + 2.0 * 0.65 * size;
                    ctx.set_transform(to_screen);
                    ctx.set_paint(color(tag));
                    let pill = Rect::new(f64::from(x), f64::from(middle - 11.0), f64::from(x + width), f64::from(middle + 11.0));
                    ctx.fill_path(&RoundedRect::from_rect(pill, 11.0).to_path(0.1));
                    draw_text(ctx, value, x + 0.65 * size, baseline, size, 400, accent, to_screen);
                    x += width + 4.0;
                }
            }
            "checkbox" => {
                let on = row.values.first().is_some_and(|value| value == "true");
                let area = Rect::new(f64::from(x), f64::from(middle - 7.0), f64::from(x + 14.0), f64::from(middle + 7.0));
                ctx.set_transform(to_screen);
                if on {
                    ctx.set_paint(color(accent));
                    ctx.fill_path(&RoundedRect::from_rect(area, 3.0).to_path(0.1));
                    let mut check = BezPath::new();
                    check.move_to((area.x0 + 3.5, area.y0 + 7.0));
                    check.line_to((area.x0 + 6.0, area.y0 + 9.5));
                    check.line_to((area.x0 + 10.5, area.y0 + 4.5));
                    ctx.set_paint(color(theme.paper));
                    ctx.set_stroke(kurbo::Stroke::new(1.6).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
                    ctx.stroke_path(&check);
                } else {
                    ctx.set_paint(color(theme.muted));
                    ctx.set_stroke(kurbo::Stroke::new(1.0));
                    ctx.stroke_path(&RoundedRect::from_rect(area.inset(-0.5), 3.0).to_path(0.1));
                }
            }
            "list" | "aliases" => {
                for value in &row.values {
                    x += draw_text(ctx, value, x, baseline, size, 400, text, to_screen) + 8.0;
                }
            }
            _ => {
                let value = row.values.first().map_or("", String::as_str);
                draw_text(ctx, value, x, baseline, size, 400, text, to_screen);
            }
        }
    }
}

/// Premultiplied to straight alpha, in place.
fn straight(pixels: &mut [u8]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        let alpha = u32::from(px[3]);
        if alpha != 0 && alpha != 255 {
            for channel in px.iter_mut().take(3) {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
}
