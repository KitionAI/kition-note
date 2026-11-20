//! `kition-note` exports as its manifest declares (KHP A35, A41; Kition platform K5, K21): one
//! command, the one `agent.export` names (what `note_export` runs), writes every `exports`
//! extension with `format` and answers the file as base64 in the field `agent.export.data` names;
//! the agent may run it, `kit.initialize` reports the formats, and `document.export` writes the same
//! files. The HTML is a whole document holding the note's text that loads nothing from the
//! network; the PDF has a header, a trailer and at least one page.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use base64::Engine as _;
use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit_server::protocol::Origin;
use kition_kit_server::{KitBackend, OpenRequest};
use kition_note_kit::NoteDocument;
use serde_json::{Map, Value, json};

const MANIFEST: &str = include_str!("../../../kit.json");

/// Elements HTML closes on its own.
const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"];

/// The HTML's elements nest: every element opened is closed, in order, and nothing is left open.
fn assert_html_parses(html: &str) {
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            rest = &comment[comment.find("-->").expect("an unclosed comment") + 3..];
            continue;
        }
        let end = rest.find('>').unwrap_or_else(|| panic!("an unclosed tag at {:?}", &rest[..rest.len().min(40)]));
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if tag.starts_with('!') {
            continue;
        }
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        assert!(!name.is_empty(), "a tag without a name: <{tag}>");
        if tag.starts_with('/') {
            assert_eq!(open.pop().as_deref(), Some(name.as_str()), "</{name}> closes another element");
        } else if !VOID.contains(&name.as_str()) && !tag.ends_with('/') {
            if name == "script" || name == "style" {
                // Raw text up to its end tag.
                let close = format!("</{name}");
                rest = &rest[rest.find(&close).unwrap_or_else(|| panic!("<{name}> is not closed"))..];
            }
            open.push(name);
        }
    }
    assert!(open.is_empty(), "left open: {open:?}");
}

fn assert_valid(format: &str, bytes: &[u8], text: &str) {
    match format {
        "html" => {
            let html = std::str::from_utf8(bytes).unwrap();
            assert!(html.trim_start().to_ascii_lowercase().starts_with("<!doctype html>"), "not an HTML document");
            assert_html_parses(html);
            assert!(html.contains(text), "the HTML holds the note's text");
            for remote in ["<link", "<script", "@import", "url(http", "src=\"http"] {
                assert!(!html.contains(remote), "the HTML loads {remote} from elsewhere");
            }
        }
        "pdf" => {
            assert!(bytes.starts_with(b"%PDF-"), "no PDF header");
            assert!(bytes.trim_ascii_end().ends_with(b"%%EOF"), "no PDF trailer");
            let pdf = String::from_utf8_lossy(bytes).replace("/Type/", "/Type /");
            let pages = pdf.matches("/Type /Page").count() - pdf.matches("/Type /Pages").count();
            assert!(pages >= 1, "the PDF has no page");
        }
        other => panic!("kit.json exports {other}, which this test does not read"),
    }
}

#[test]
fn issue_agent_export_every_declared_format_exports_through_one_command() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let export = &manifest["agent"]["export"];
    let command = export["command"].as_str().unwrap_or_else(|| panic!("kit.json has no agent.export.command (KHP A41, K21): {export}"));
    let data = export["data"].as_str().unwrap_or_else(|| panic!("kit.json has no agent.export.data (K21): {export}"));
    let formats: Vec<&str> = manifest["exports"].as_array().unwrap().iter().map(|entry| entry["extension"].as_str().unwrap()).collect();
    assert_eq!(formats, ["html", "pdf"]);

    let mut kit = Headless::<NoteDocument>::with_replica_port(KitInfo::from_manifest(MANIFEST, &[], kition_note_kit::EDITOR_STATE).unwrap());
    assert_eq!(kit.capabilities()["formats"]["export"], json!(formats), "kit.initialize reports the exports");
    let root = std::env::temp_dir().join(format!("kition-note-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/basic.md"), root.join("basic.md")).unwrap();
    let folder = HostFolder::new(&root).unwrap();
    kit.set_host(folder.host());
    let opened = kit.open(OpenRequest { file: &folder.file("basic.md").unwrap(), read_only: false, create: None }).unwrap();
    let listed = kit.commands(Some(&opened.id)).unwrap();
    assert!(listed.iter().any(|listed| listed.id == command), "the agent may run {command}");

    for format in &formats {
        let mut params: Map<String, Value> = export["params"].as_object().cloned().unwrap_or_default();
        params.insert("format".into(), json!(format));
        let executed =
            kit.execute(&opened.id, command, params, Origin::Agent).unwrap_or_else(|error| panic!("{command} {format}: {}", error.message));
        assert!(!executed.changed, "an export never edits the note");
        let answer = executed.result.unwrap();
        let encoded = answer[data].as_str().unwrap_or_else(|| panic!("{command} {format} answers no `{data}`: {answer}"));
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        assert_valid(format, &bytes, "Ship the beta to ten teams");

        let name = format!("basic.{format}");
        kit.export(&opened.id, &folder.export(&name).unwrap(), format, None)
            .unwrap_or_else(|error| panic!("document.export {format}: {}", error.message));
        assert_eq!(std::fs::read(root.join(&name)).unwrap(), bytes, "document.export writes what {command} answers");
    }
    assert!(!kit.document(&opened.id).unwrap().dirty);
    let _ = std::fs::remove_dir_all(root);
}
