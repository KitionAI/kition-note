use crate::doc::{Block, Inline, read};
use crate::layout::{Mark, Options, PageSize, layout};
use crate::raster::Raster;
use crate::{export_html, export_pdf, pages};

const SAMPLE: &str = "---\ntitle: Plan\n---\n# Plan\n\nGoals with ==a highlight== and #work. ^goals\n\n%%hidden note%%\n\n> [!warning] Watch out\n> Dates move.\n\n- [x] Draft\n- [ ] Ship[^1]\n\n| Item | Owner |\n| :--- | ----: |\n| Ship | Ann |\n\n[^1]: Before the review.\n";

#[test]
fn reads_the_obsidian_syntax_into_blocks() {
    let doc = read(SAMPLE);
    assert_eq!(doc.title.as_deref(), Some("Plan"));
    let kinds: Vec<&str> = doc
        .blocks
        .iter()
        .map(|located| match &located.block {
            Block::Heading { .. } => "heading",
            Block::Paragraph { .. } => "paragraph",
            Block::Quote { .. } => "quote",
            Block::List { .. } => "list",
            Block::Table { .. } => "table",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["heading", "paragraph", "quote", "list", "table"], "the comment and the frontmatter draw nothing");
    assert_eq!(doc.blocks.first().map(|located| located.line), Some(4));
    let Some(Block::Paragraph { runs, id }) = doc.blocks.get(1).map(|located| &located.block) else { panic!("paragraph") };
    assert_eq!(id.as_deref(), Some("goals"));
    assert!(runs.iter().any(|inline| matches!(inline, Inline::Text(text, style) if text == "a highlight" && style.mark)));
    assert!(runs.iter().any(|inline| matches!(inline, Inline::Text(text, style) if text == "#work" && style.tag)));
    let Some(Block::Quote { callout: Some(callout), blocks }) = doc.blocks.get(2).map(|located| &located.block) else { panic!("callout") };
    assert_eq!((callout.kind.as_str(), callout.title.as_str()), ("warning", "Watch out"));
    assert_eq!(blocks.len(), 1);
    let Some(Block::List { items, .. }) = doc.blocks.get(3).map(|located| &located.block) else { panic!("list") };
    assert_eq!(items.iter().map(|item| item.task).collect::<Vec<_>>(), [Some(true), Some(false)]);
    assert_eq!(doc.footnotes.len(), 1);
}

#[test]
fn every_mark_stays_on_its_page_and_long_notes_add_pages() {
    let long = format!("# Long\n\n{}", "A sentence that keeps going on and on. ".repeat(900));
    for size in [PageSize::LETTER, PageSize::A4] {
        let laid = layout(&read(&long), Options { size, margin: 72.0 });
        assert!(laid.pages.len() > 3, "{} pages", laid.pages.len());
        for page in &laid.pages {
            for mark in &page.marks {
                if let Mark::Text { x, y, .. } = mark {
                    assert!(*x >= 72.0 - 0.01 && *x <= size.width - 72.0, "x {x}");
                    assert!(*y >= 72.0 && *y <= size.height - 72.0 + 0.01, "y {y}");
                }
            }
        }
    }
}

#[test]
fn wide_characters_wrap_and_are_counted_as_missing_glyphs() {
    let laid = pages(&format!("{}\n", "中文".repeat(200)), Options::default());
    assert_eq!(laid.unsupported.missing_glyphs, 400);
    assert_eq!(laid.unsupported.missing_sample, "中文");
    let rows = laid
        .pages
        .iter()
        .flat_map(|page| &page.marks)
        .filter_map(|mark| if let Mark::Missing { y, .. } = mark { Some((*y * 10.0).round() as i64) } else { None })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(rows.len() > 5, "the text wraps into lines: {rows:?}");
}

#[test]
fn code_wider_than_the_column_wraps_and_is_reported() {
    let laid = pages(&format!("```\n{}\n```\n", "x".repeat(200)), Options::default());
    assert_eq!(laid.overflow.len(), 1);
    assert_eq!(laid.overflow.first().map(|overflow| overflow.kind), Some("code"));
    let laid = pages("```mermaid\ngraph TD; A-->B\n```\n\n$$\nx^2\n$$\n\n![[Board.kboard]] ![chart](chart.png)\n", Options::default());
    assert_eq!((laid.unsupported.mermaid, laid.unsupported.math, laid.unsupported.embeds, laid.unsupported.images), (1, 1, 1, 1));
}

#[test]
fn the_pdf_is_well_formed_and_deterministic() {
    let (pdf, laid) = export_pdf(SAMPLE, "Plan.md", Options::default());
    assert!(pdf.starts_with(b"%PDF-1.4\n"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    let find = |needle: &[u8]| pdf.windows(needle.len()).rposition(|window| window == needle);
    let start = find(b"startxref\n").map(|at| at + "startxref\n".len()).unwrap_or(0);
    let tail = String::from_utf8_lossy(pdf.get(start..).unwrap_or_default()).into_owned();
    let xref: usize = tail.lines().next().and_then(|line| line.parse().ok()).unwrap_or(0);
    assert!(pdf.get(xref..).is_some_and(|rest| rest.starts_with(b"xref\n")), "startxref points at the table");
    // Every object offset in the table points at its object.
    let table = String::from_utf8_lossy(pdf.get(xref..).unwrap_or_default()).into_owned();
    let entries: Vec<&str> = table.lines().skip(3).take_while(|line| line.ends_with(" n ")).collect();
    assert!(entries.len() > 9);
    for (index, entry) in entries.iter().enumerate() {
        let offset: usize = entry.get(..10).and_then(|digits| digits.parse().ok()).unwrap_or(0);
        assert!(pdf.get(offset..).is_some_and(|rest| rest.starts_with(format!("{} 0 obj\n", index + 1).as_bytes())), "object {}", index + 1);
    }
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(text.matches("/Type /Page ").count(), laid.pages.len());
    assert!(text.contains("/BaseFont /Helvetica-Bold"));
    assert!(text.contains("(Plan) Tj"));
    assert!(text.contains("(Goals with) Tj"), "words of one style on a line are one text run");
    assert_eq!(export_pdf(SAMPLE, "Plan.md", Options::default()).0, pdf, "the same note gives the same bytes");
    let (_, linked) = export_pdf("See [the site](https://kition.ai).\n", "Links.md", Options::default());
    let (bytes, _) = export_pdf("See [the site](https://kition.ai).\n", "Links.md", Options::default());
    assert!(String::from_utf8_lossy(&bytes).contains("/URI (https://kition.ai)"));
    assert_eq!(linked.pages.len(), 1);
}

#[test]
fn the_html_matches_its_expected_file() {
    let html = export_html(include_str!("../../../../conformance/export.md"), "export.md", true);
    let expected = include_str!("../../../../conformance/export.html");
    assert_eq!(html, expected, "conformance/export.html");
}

#[test]
fn html_refuses_script_links_and_escapes_text() {
    let html = export_html("[x](javascript:alert(1)) <b>kept</b> & \"q\"\n", "x.md", false);
    assert!(html.contains("href=\"#\""));
    assert!(html.contains("&amp; &quot;q&quot;"));
    assert!(html.contains("<b>kept</b>"), "raw inline HTML is the note's own");
}

#[test]
fn pixels_show_text_rules_and_paper() {
    let laid = pages("# Title\n\n---\n\nBody text.\n", Options::default());
    let raster = Raster::new(&laid);
    assert_eq!((raster.width(), raster.height()), (612, 792));
    // The heading's first letters are a bar; the margin is paper; outside the page is transparent.
    let first = laid
        .pages
        .first()
        .and_then(|page| page.marks.iter().find_map(|mark| if let Mark::Text { x, y, size, .. } = mark { Some((*x, *y, *size)) } else { None }));
    let (x, y, size) = first.unwrap_or_default();
    assert_ne!(raster.pixel((x + 2.0) as i64, (y - size * 0.3) as i64), [255, 255, 255, 255]);
    assert_eq!(raster.pixel(10, 10), [255, 255, 255, 255]);
    assert_eq!(raster.pixel(-1, 0), [0, 0, 0, 0]);
    // A whole-number reduction is the box mean of the pixels at scale 1.
    let full = raster.render(0.0, 0.0, 1.0, 612, 792);
    let half = raster.render(0.0, 0.0, 0.5, 306, 396);
    let at = |rgba: &[u8], width: usize, x: usize, y: usize| -> [u64; 4] {
        std::array::from_fn(|channel| u64::from(rgba.get((y * width + x) * 4 + channel).copied().unwrap_or(0)))
    };
    for (hx, hy) in [(40usize, 40usize), (60, 45), (100, 200)] {
        let mut sum = [0u64; 4];
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let px = at(&full, 612, hx * 2 + dx, hy * 2 + dy);
            for channel in 0..4 {
                sum[channel] += px[channel];
            }
        }
        let mean: [u64; 4] = std::array::from_fn(|channel| (sum[channel] + 2) / 4);
        assert_eq!(at(&half, 306, hx, hy), mean, "({hx}, {hy})");
    }
}

#[test]
fn coloured_highlights_export_as_obsidians_reading_view_writes_them() {
    // Obsidian 1.14.5's reading view: `<mark data-highlight="red">red</mark>`, the emoji gone,
    // a plain highlight a plain mark.
    let html = export_html("A ==🔴red== and ==🟢green== and ==plain== text.\n", "x.md", false);
    assert!(html.contains("<mark data-highlight=\"red\">red</mark>"), "{html}");
    assert!(html.contains("<mark data-highlight=\"green\">green</mark>"), "{html}");
    assert!(html.contains("<mark>plain</mark>"), "{html}");
    assert!(!html.contains('🔴'));
    let doc = read("==🔵blue==\n");
    let Some(Block::Paragraph { runs, .. }) = doc.blocks.first().map(|located| &located.block) else { panic!("paragraph") };
    assert!(runs.iter().any(|inline| matches!(inline, Inline::Text(text, style) if text == "blue" && style.mark_color == Some("blue"))));
}
