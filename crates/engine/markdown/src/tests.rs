use super::*;

fn s(text: &str, range: &Range<usize>) -> String {
    text[range.clone()].to_owned()
}

#[test]
fn the_obsidian_layer_is_found_outside_code() {
    let t = "---\ntags: [a]\n---\n# Plan #not-heading-tag\n\nSee [[Note#Risks|risks]] and ![[pic.png|100]] #proj/sub #2026 #项目\n\n`#code [[x]]` ==hi== %%hidden\nstill%% done ^blk-1\n\n> [!tip]- Fold here\n> body\n\n```\n#nope [[nope]]\n```\n";
    let ix = index(t);
    assert_eq!(ix.tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(), vec!["not-heading-tag", "proj/sub", "项目"]);
    let wiki: Vec<_> = ix
        .links
        .iter()
        .filter(|link| link.kind.is_wiki())
        .map(|link| (s(t, &link.range), link.file.clone(), link.anchor.clone(), link.text.clone()))
        .collect();
    assert_eq!(
        wiki,
        vec![
            ("[[Note#Risks|risks]]".into(), "Note".into(), Some(Anchor::Heading("Risks".into())), Some("risks".into())),
            ("![[pic.png|100]]".into(), "pic.png".into(), None, Some("100".into())),
        ]
    );
    assert_eq!(ix.highlights.iter().map(|r| s(t, r)).collect::<Vec<_>>(), vec!["==hi=="]);
    assert_eq!(ix.comments.iter().map(|r| s(t, r)).collect::<Vec<_>>(), vec!["%%hidden\nstill%%"]);
    assert_eq!(ix.block_ids.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), vec!["blk-1"]);
    assert_eq!(ix.callouts.len(), 1);
    assert_eq!((ix.callouts[0].kind.as_str(), ix.callouts[0].fold, ix.callouts[0].title.as_str()), ("tip", Some('-'), "Fold here"));
    assert_eq!(ix.headings[0].text, "Plan #not-heading-tag");
    let fm = ix.frontmatter.as_ref().unwrap();
    assert_eq!(s(t, &fm.range), "---\ntags: [a]\n---\n");
}

#[test]
fn headings_have_paths_and_sections() {
    let t = "# Plan\n\nIntro\n\n## Risks\n\nMany\n\n### Detail\n\nx\n\n## Owners\n\nAnn\n\nTitle\n=====\n\n> # Quoted\n";
    let ix = index(t);
    let paths: Vec<Vec<String>> = ix.headings.iter().map(|h| h.path.clone()).collect();
    assert_eq!(
        paths,
        vec![
            vec!["Plan".to_owned()],
            vec!["Plan".to_owned(), "Risks".to_owned()],
            vec!["Plan".to_owned(), "Risks".to_owned(), "Detail".to_owned()],
            vec!["Plan".to_owned(), "Owners".to_owned()],
            vec!["Title".to_owned()],
        ]
    );
    assert_eq!(s(t, &ix.headings[1].section), "## Risks\n\nMany\n\n### Detail\n\nx\n\n");
    assert_eq!(s(t, &ix.headings[1].marker), "##");
    assert!(ix.headings[4].setext);
    assert_eq!(s(t, &ix.headings[4].marker), "=====");
    assert_eq!(s(t, &ix.headings[0].section), &t[..t.find("Title").unwrap()]);
}

#[test]
fn table_cells_have_segments_and_contents() {
    let t = "Intro\n\n| Item   | Owner |\n| :----- | ----: |\n| Ship   | Ann   |\n|  | x \\| y |\n\n> | a | b |\n> |---|---|\n> | 1 | 2 |\n";
    let ix = index(t);
    assert_eq!(ix.tables.len(), 2);
    let table = &ix.tables[0];
    assert_eq!(table.align, vec![Align::Left, Align::Right]);
    assert_eq!(table.rows.len(), 3);
    assert_eq!(s(t, &table.rows[1].cells[0].segment), " Ship   ");
    assert_eq!(s(t, &table.rows[1].cells[0].content), "Ship");
    assert_eq!(s(t, &table.rows[2].cells[0].content), "");
    assert_eq!(s(t, &table.rows[2].cells[1].content), "x \\| y");
    assert_eq!(s(t, &table.delimiter.cells[1].content), "----:");
    assert!(table.padded(t));
    let quoted = &ix.tables[1];
    assert_eq!(s(t, &quoted.rows[1].cells[0].content), "1");
    assert_eq!(s(t, &quoted.delimiter.cells[0].content), "---");
    assert!(quoted.rows[0].leading_pipe && quoted.rows[0].trailing_pipe);
    assert!(!quoted.padded(t));
}

#[test]
fn rows_without_outer_pipes() {
    let t = "a | b\n--|--\n1 | 2\n";
    let ix = index(t);
    let row = &ix.tables[0].rows[1];
    assert!(!row.leading_pipe && !row.trailing_pipe);
    assert_eq!(row.cells.iter().map(|c| s(t, &c.content)).collect::<Vec<_>>(), vec!["1", "2"]);
}

#[test]
fn tasks_know_their_marker() {
    let t = "- [ ] one\n- [x] two ^t2\n  - [ ] nested\n";
    let ix = index(t);
    assert_eq!(ix.tasks.len(), 3);
    assert_eq!(s(t, &ix.tasks[0].marker), "[ ]");
    assert!(ix.tasks[1].done);
    assert_eq!(s(t, &ix.block_ids[0].block), "- [x] two ^t2");
}

#[test]
fn markdown_links_know_their_destination_and_text() {
    let t = "See [the plan](docs/my%20plan.md#Risks \"title\"), ![img](<a b.png>), <https://x.org> and https://example.com/a.\n\n[ref]: https://r.org\n\nA [ref] link.\n";
    let ix = index(t);
    let first = &ix.links[0];
    assert_eq!(first.kind, LinkKind::Markdown);
    assert_eq!(s(t, first.destination_range.as_ref().unwrap()), "docs/my%20plan.md#Risks");
    assert_eq!(first.file, "docs/my plan.md");
    assert_eq!(first.anchor, Some(Anchor::Heading("Risks".into())));
    assert_eq!(first.text.as_deref(), Some("the plan"));
    let image = &ix.links[1];
    assert_eq!(image.kind, LinkKind::Image);
    assert_eq!(s(t, image.destination_range.as_ref().unwrap()), "<a b.png>");
    assert_eq!(image.file, "a b.png");
    assert_eq!(ix.links[2].kind, LinkKind::Autolink);
    assert!(ix.links[2].is_external());
    assert_eq!(ix.links[3].kind, LinkKind::BareUrl);
    assert_eq!(ix.links[3].destination, "https://example.com/a");
    assert_eq!(ix.links[4].kind, LinkKind::Reference);
    assert_eq!(ix.links[4].destination, "https://r.org");
}

#[test]
fn properties_are_read_line_by_line_with_their_style_and_type() {
    let t = "---\ntitle: Plan\ndate: 2026-10-22\nquoted: \"x\"\ntags:\n  - a\n  - b\nflow: [a, b]\nempty:\nauthor:\n  name: Ann\nsummary: |\n  Two\n# comment\nn: 3\n---\nBody\n";
    let ix = index(t);
    let fm = ix.frontmatter.as_ref().unwrap();
    let styles: Vec<(&str, &str)> = fm.entries.iter().map(|e| (e.key.as_str(), e.style.name())).collect();
    assert_eq!(
        styles,
        vec![
            ("title", "plain"),
            ("date", "plain"),
            ("quoted", "double-quoted"),
            ("tags", "block-list"),
            ("flow", "flow-list"),
            ("empty", "empty"),
            ("author", "complex"),
            ("summary", "complex"),
            ("n", "plain"),
        ]
    );
    assert_eq!(fm.comments.len(), 1);
    assert!(fm.invalid.is_none());
    let typed: Vec<&str> = fm.entries.iter().map(|e| e.typed.as_ref().map_or("?", Typed::kind)).collect();
    assert_eq!(typed, vec!["string", "date", "string", "list", "list", "null", "other", "string", "number"]);
    assert_eq!(s(t, &fm.entries[3].lines), "tags:\n  - a\n  - b\n");
    assert_eq!(fm.entries[3].items.iter().map(|r| s(t, r)).collect::<Vec<_>>(), vec!["  - a", "  - b"]);
}

#[test]
fn invalid_and_duplicated_properties_are_reported() {
    let ix = index("---\ntitle: [unclosed\n---\n# Note\n");
    let invalid = ix.frontmatter.as_ref().unwrap().invalid.clone().unwrap();
    assert!(invalid.line >= 2, "{invalid:?}");
    let ix = index("---\ntags: [a]\ntags: [b]\n---\n");
    assert_eq!(ix.frontmatter.as_ref().unwrap().duplicates, vec!["tags".to_owned()]);
}

#[test]
fn unclosed_fences_and_math_are_found() {
    let ix = index("# Setup\n\n```bash\nnpm install\n\nMore text that became code.\n");
    assert!(ix.code[0].unclosed);
    let ix = index("```\nok\n```\n\n~~~~\nx\n~~~~\n");
    assert!(ix.code.iter().all(|code| !code.unclosed));
    assert!(index("$$\nx = 1\n").unclosed_math.is_some());
    assert!(index("$$\nx = 1\n$$\n").unclosed_math.is_none());
}

#[test]
fn block_ids_on_their_own_line_name_the_block_before() {
    let t = "| a |\n| - |\n| 1 |\n\n^tbl\n\nPara ^p1\n";
    let ix = index(t);
    let table_id = ix.block_ids.iter().find(|b| b.id == "tbl").unwrap();
    assert!(table_id.own_line);
    assert_eq!(s(t, &table_id.block), "| a |\n| - |\n| 1 |\n\n^tbl");
    assert_eq!(ix.tables[0].rows.len(), 2);
    let para = ix.block_ids.iter().find(|b| b.id == "p1").unwrap();
    assert_eq!(s(t, &para.block), "Para ^p1");
}

#[test]
fn words_count_letters_and_cjk() {
    assert_eq!(index("---\na: b c\n---\nOne two-three, 四五\n").words, 4);
}

#[test]
fn every_range_is_on_char_boundaries_for_hostile_input() {
    for t in [
        "\u{FEFF}# 标题\n| 中 | 文 |\n|--|--|\n| ü | 😀 |\n",
        "[[",
        "![[",
        "| a\n|-\n",
        "---\n",
        "%%",
        "==",
        "#",
        "^",
        "> [!",
        "`",
        "[x](",
        "h https://",
    ] {
        let ix = index(t);
        for heading in &ix.headings {
            assert!(t.get(heading.section.clone()).is_some());
        }
        for link in &ix.links {
            assert!(t.get(link.range.clone()).is_some());
        }
        for table in &ix.tables {
            for row in table.lines() {
                for cell in &row.cells {
                    assert!(t.get(cell.segment.clone()).is_some(), "{t:?}");
                }
            }
        }
    }
}

#[test]
fn a_www_address_in_text_is_a_link_as_obsidian_reads_it() {
    let t = "Go to www.example.com/a, not www.x or awww.example.com.\n";
    let ix = index(t);
    let urls: Vec<_> = ix.links.iter().filter(|link| link.kind == LinkKind::BareUrl).collect();
    assert_eq!(urls.len(), 1);
    assert_eq!(s(t, &urls[0].range), "www.example.com/a");
    assert_eq!(urls[0].destination, "http://www.example.com/a");
    assert!(urls[0].is_external());
}
