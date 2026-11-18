//! The note as an HTML file: the reading view's blocks with the Obsidian syntax rendered
//! (wikilinks, embeds, callouts, highlights, tags, block ids, footnotes), `%%comments%%` and the
//! frontmatter left out, a stylesheet inlined. Math and Mermaid stay their source (no script runs
//! in the file); raw HTML in the note is kept, as Markdown renders it.

use crate::doc::{Align, Block, Doc, Inline, Item, Located, Runs, Style};

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A link target for an attribute: `javascript:` and other script schemes are dropped.
fn href(destination: &str) -> String {
    let lower = destination.trim().to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("vbscript:") || lower.starts_with("data:text/html") {
        return "#".into();
    }
    escape(destination)
}

/// A wikilink's target as a relative link: the note's file (`.md` added when it has no extension) and its anchor.
fn wiki_href(target: &str) -> String {
    let (file, anchor) = match target.split_once('#') {
        Some((file, anchor)) => (file, Some(anchor)),
        None => (target, None),
    };
    let mut out = String::new();
    if !file.is_empty() {
        out.push_str(&percent(file));
        if !file.rsplit('/').next().unwrap_or(file).contains('.') {
            out.push_str(".md");
        }
    }
    if let Some(anchor) = anchor {
        out.push('#');
        out.push_str(&percent(&slug(anchor.trim_start_matches('^'))));
    }
    escape(&out)
}

fn percent(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(char::from(byte)),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A heading's anchor: lowercase words joined by hyphens.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_owned()
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif", "bmp"];

fn open_style(style: &Style, out: &mut String) {
    if let Some(link) = &style.link {
        if style.wiki {
            out.push_str(&format!("<a class=\"internal-link\" href=\"{}\">", wiki_href(link)));
        } else {
            out.push_str(&format!("<a href=\"{}\">", href(link)));
        }
    }
    if style.sup {
        out.push_str("<sup>");
    }
    if style.mark {
        // A coloured highlight as Obsidian's reading view writes it.
        match style.mark_color {
            Some(color) => out.push_str(&format!("<mark data-highlight=\"{color}\">")),
            None => out.push_str("<mark>"),
        }
    }
    if style.strike {
        out.push_str("<del>");
    }
    if style.bold {
        out.push_str("<strong>");
    }
    if style.italic {
        out.push_str("<em>");
    }
    if style.tag {
        out.push_str("<span class=\"tag\">");
    }
}

fn close_style(style: &Style, out: &mut String) {
    if style.tag {
        out.push_str("</span>");
    }
    if style.italic {
        out.push_str("</em>");
    }
    if style.bold {
        out.push_str("</strong>");
    }
    if style.strike {
        out.push_str("</del>");
    }
    if style.mark {
        out.push_str("</mark>");
    }
    if style.sup {
        out.push_str("</sup>");
    }
    if style.link.is_some() {
        out.push_str("</a>");
    }
}

fn runs(runs: &Runs, out: &mut String) {
    for inline in runs {
        match inline {
            Inline::Text(text, style) => {
                if style.raw {
                    out.push_str(text);
                    continue;
                }
                open_style(style, out);
                if style.math {
                    out.push_str(&format!("<code class=\"math\">{}</code>", escape(text)));
                } else if style.code {
                    out.push_str(&format!("<code>{}</code>", escape(text)));
                } else {
                    out.push_str(&escape(text));
                }
                close_style(style, out);
            }
            Inline::Soft => out.push('\n'),
            Inline::Break => out.push_str("<br>\n"),
            Inline::Footnote(label, number) => out.push_str(&format!(
                "<sup class=\"footnote-ref\"><a href=\"#fn-{}\" id=\"fnref-{}\">{number}</a></sup>",
                escape(&slug(label)),
                escape(&slug(label))
            )),
            Inline::Image { alt, destination, embed } => {
                let extension = destination.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
                if !*embed || IMAGE_EXTENSIONS.contains(&extension.as_str()) {
                    out.push_str(&format!("<img src=\"{}\" alt=\"{}\">", href(&percent_if_wiki(destination, *embed)), escape(alt)));
                } else {
                    out.push_str(&format!(
                        "<span class=\"embed\"><a class=\"internal-link\" href=\"{}\">{}</a></span>",
                        wiki_href(destination),
                        escape(if alt.is_empty() { destination } else { alt })
                    ));
                }
            }
        }
    }
}

fn percent_if_wiki(destination: &str, embed: bool) -> String {
    if embed { percent(destination) } else { destination.to_owned() }
}

fn id_attr(id: &Option<String>) -> String {
    id.as_ref().map(|id| format!(" id=\"^{}\"", escape(id))).unwrap_or_default()
}

fn blocks(blocks: &[Located], out: &mut String) {
    for located in blocks {
        block(&located.block, out);
    }
}

fn block(block: &Block, out: &mut String) {
    match block {
        Block::Heading { level, runs: content, id } => {
            let level = (*level).clamp(1, 6);
            let anchor = id.clone().map(|id| format!("^{id}")).unwrap_or_else(|| slug(&crate::doc::plain_text(content)));
            out.push_str(&format!("<h{level} id=\"{}\">", escape(&anchor)));
            runs(content, out);
            out.push_str(&format!("</h{level}>\n"));
        }
        Block::Paragraph { runs: content, id } => {
            out.push_str(&format!("<p{}>", id_attr(id)));
            runs(content, out);
            out.push_str("</p>\n");
        }
        Block::Code { info, text } => {
            let class = if info.is_empty() { String::new() } else { format!(" class=\"language-{}\"", escape(info)) };
            out.push_str(&format!("<pre><code{class}>{}</code></pre>\n", escape(text)));
        }
        Block::Math(text) => out.push_str(&format!("<pre class=\"math\"><code>{}</code></pre>\n", escape(text))),
        Block::Html(text) => out.push_str(text),
        Block::Rule => out.push_str("<hr>\n"),
        Block::Quote { callout: Some(callout), blocks: inner } => {
            out.push_str(&format!(
                "<div class=\"callout\" data-callout=\"{}\">\n<div class=\"callout-title\">{}</div>\n<div class=\"callout-content\">\n",
                escape(&callout.kind),
                escape(if callout.title.is_empty() { &callout.kind } else { &callout.title })
            ));
            blocks(inner, out);
            out.push_str("</div>\n</div>\n");
        }
        Block::Quote { callout: None, blocks: inner } => {
            out.push_str("<blockquote>\n");
            blocks(inner, out);
            out.push_str("</blockquote>\n");
        }
        Block::List { start, items } => list(*start, items, out),
        Block::Table { aligns, rows, head } => {
            out.push_str("<table>\n");
            for (index, row) in rows.iter().enumerate() {
                if index == 0 && *head > 0 {
                    out.push_str("<thead>\n");
                }
                if index == *head {
                    out.push_str("<tbody>\n");
                }
                out.push_str("<tr>");
                let cell = if index < *head { "th" } else { "td" };
                for (column, content) in row.iter().enumerate() {
                    let align = match aligns.get(column) {
                        Some(Align::Left) => " style=\"text-align: left\"",
                        Some(Align::Center) => " style=\"text-align: center\"",
                        Some(Align::Right) => " style=\"text-align: right\"",
                        _ => "",
                    };
                    out.push_str(&format!("<{cell}{align}>"));
                    runs(content, out);
                    out.push_str(&format!("</{cell}>"));
                }
                out.push_str("</tr>\n");
                if index + 1 == *head {
                    out.push_str("</thead>\n");
                }
            }
            if rows.len() > *head {
                out.push_str("</tbody>\n");
            }
            out.push_str("</table>\n");
        }
    }
}

fn list(start: Option<u64>, items: &[Item], out: &mut String) {
    let tasks = items.iter().any(|item| item.task.is_some());
    let class = if tasks { " class=\"contains-task-list\"" } else { "" };
    match start {
        Some(1) => out.push_str(&format!("<ol{class}>\n")),
        Some(first) => out.push_str(&format!("<ol start=\"{first}\"{class}>\n")),
        None => out.push_str(&format!("<ul{class}>\n")),
    }
    for item in items {
        match item.task {
            Some(done) => {
                out.push_str(&format!("<li class=\"task-list-item\"><input type=\"checkbox\" disabled{}> ", if done { " checked" } else { "" }))
            }
            None => out.push_str("<li>"),
        }
        // A tight item's one paragraph is its text.
        match item.blocks.as_slice() {
            [Located { block: Block::Paragraph { runs: content, .. }, .. }] => runs(content, out),
            inner => {
                out.push('\n');
                blocks(inner, out);
            }
        }
        out.push_str("</li>\n");
    }
    out.push_str(if start.is_some() { "</ol>\n" } else { "</ul>\n" });
}

const STYLESHEET: &str = "\
body{margin:0;background:#fff;color:#1a1a1a;font:16px/1.6 -apple-system,BlinkMacSystemFont,\"Segoe UI\",Helvetica,Arial,\"PingFang SC\",\"Microsoft YaHei\",sans-serif}
main{max-width:720px;margin:0 auto;padding:48px 24px}
h1,h2,h3,h4,h5,h6{line-height:1.3;margin:1.4em 0 .5em}
h1{font-size:2em}h2{font-size:1.5em}h3{font-size:1.25em}
a{color:#5645d4}
code{font:.9em ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;background:#f6f5f4;border-radius:4px;padding:.1em .3em}
pre{background:#f6f5f4;border-radius:8px;padding:12px 14px;overflow:auto}
pre code{background:none;padding:0}
blockquote{margin:1em 0;padding:0 1em;border-left:3px solid #e5e3df;color:#5d5b54}
.callout{margin:1em 0;padding:10px 14px;border-left:3px solid #5645d4;border-radius:8px;background:#f6f5ff}
.callout-title{font-weight:600;color:#5645d4;text-transform:capitalize}
mark{background:#fff0b3}
mark[data-highlight=red]{background:#f8c1c8}mark[data-highlight=orange]{background:#f9d6b2}mark[data-highlight=yellow]{background:#f6e6b2}
mark[data-highlight=green]{background:#b5eaca}mark[data-highlight=blue]{background:#b5d3f5}mark[data-highlight=purple]{background:#d7cbfa}
.tag{color:#5645d4}
table{border-collapse:collapse;margin:1em 0}
th,td{border:1px solid #e5e3df;padding:6px 10px}
th{background:#fafaf9}
hr{border:0;border-top:1px solid #e5e3df;margin:2em 0}
img{max-width:100%}
.contains-task-list{list-style:none;padding-left:1.2em}
.footnotes{margin-top:3em;font-size:.9em;color:#5d5b54}
";

/// Writes the note's HTML: a whole file with its stylesheet when `standalone`, else the `<main>` element only.
pub fn write(doc: &Doc, title: &str, standalone: bool) -> String {
    let mut body = String::from("<main>\n");
    blocks(&doc.blocks, &mut body);
    if !doc.footnotes.is_empty() {
        body.push_str("<section class=\"footnotes\">\n<hr>\n<ol>\n");
        for (label, _, content) in &doc.footnotes {
            body.push_str(&format!("<li id=\"fn-{}\">\n", escape(&slug(label))));
            blocks(content, &mut body);
            body.push_str(&format!("<a href=\"#fnref-{}\">↩</a></li>\n", escape(&slug(label))));
        }
        body.push_str("</ol>\n</section>\n");
    }
    body.push_str("</main>\n");
    if !standalone {
        return body;
    }
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"Kition Note\">\n<title>{}</title>\n<style>\n{STYLESHEET}</style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        escape(title)
    )
}
