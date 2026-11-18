//! `cargo xtask perf`: the note's timings on about 1 MB of Markdown, in a release build
//! (`docs/features/performance.md`). The external merge is the TypeScript note model's benchmark case
//! (Kition's `documentKdm.bench.spec.ts`: 1 % external and 0.5 % local scattered edits), timed
//! whole: diff, merge, and the replica transaction.

use std::process::Command;
use std::time::Instant;

use kition_kit::KitDocument;
use kition_kit::kdm::{ActorId, Origin};
use kition_kit::session::{KitSession, MemoryBlobs};
use kition_note_kdm::file::bench::{bench_merge_case, markdown_of_size, merge_case};
use kition_note_kdm::file::merge::merge_edits;
use kition_note_kdm::{NoteModel, merge_external};
use kition_note_kit::NoteDocument;

use crate::cargo;

/// Runs the measurements in a release build of xtask.
pub fn perf() -> Result<(), String> {
    let mut command = cargo();
    command.args(["run", "--release", "--quiet", "--package", "xtask", "--", "perf-measure"]);
    crate::run(command, "cargo run --release --package xtask -- perf-measure")
}

/// The median of `runs` timings of `action` in milliseconds; `setup` prepares each run untimed.
fn median<T>(runs: usize, mut setup: impl FnMut() -> Result<T, String>, mut action: impl FnMut(T) -> Result<(), String>) -> Result<f64, String> {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let context = setup()?;
        let started = Instant::now();
        action(context)?;
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    Ok(samples.get(samples.len() / 2).copied().unwrap_or(f64::NAN))
}

fn session(text: &str) -> Result<KitSession<NoteModel>, String> {
    Ok(KitSession::<NoteModel>::import("bench.md", text.as_bytes(), Box::new(MemoryBlobs::default()), ActorId::random())?)
}

/// The merge of `base → incoming` into a fresh replica holding `current`, timed whole.
fn merge_whole(base: &str, current: &str, incoming: &str) -> Result<f64, String> {
    median(
        7,
        || session(current),
        |mut session| {
            session.execute(&Origin::system("import"), |model, tx| merge_external(tx, base, incoming, model.body()))?;
            Ok(())
        },
    )
}

pub fn measure() -> Result<(), String> {
    let markdown = markdown_of_size(1_000_000);
    let bytes = markdown.as_bytes();
    println!("Note timings on {} bytes ({} characters) of generated Markdown, release build\n", bytes.len(), markdown.chars().count());
    println!("| Measurement | Median (ms) |");
    println!("|---|---|");
    let open = median(7, || Ok(()), |()| NoteDocument::open("bench.md", bytes).map(|_| ()))?;
    println!("| Open the file (decode, new replica in 16 KB pieces, page layout) | {open:.1} |");
    let doc = NoteDocument::open("bench.md", bytes)?;
    let save = median(7, || Ok(()), |()| doc.save(None).map(|_| ()))?;
    println!("| Save the file (export) | {save:.1} |");
    let pages = median(
        5,
        || Ok(()),
        |()| {
            let _ = kition_note_kit::page::Pages::layout(&markdown);
            Ok(())
        },
    )?;
    println!("| Page layout (read and lay out; `height` after a change) | {pages:.1} |");
    let raster = median(
        5,
        || Ok(kition_note_kit::page::Pages::layout(&markdown)),
        |laid| {
            let _ = laid.render(0.0, 0.0, 0.25, 153, 198);
            Ok(())
        },
    )?;
    println!("| First render after a change (pixel index of every page, then one thumbnail) | {raster:.1} |");
    let pdf = median(5, || Ok(()), |()| doc.export_pdf("letter").map(|_| ()))?;
    let html = median(
        5,
        || Ok(()),
        |()| {
            let _ = doc.export_html(true);
            Ok(())
        },
    )?;
    println!("| Export PDF, letter ({} pages) / HTML | {pdf:.1} / {html:.1} |", kition_note_kit::page::Pages::layout(&markdown).pages());
    let mut typing = NoteDocument::open("bench.md", bytes)?;
    let mut at = markdown.chars().count() / 2;
    let keystroke = median(
        21,
        || {
            at += 1;
            Ok(at)
        },
        |at| typing.execute("text.insert", serde_json::json!({ "at": at, "text": "x" })).map(|_| ()),
    )?;
    println!("| One keystroke (`text.insert`, transaction and view refresh) | {keystroke:.1} |");

    let (base, current, incoming) = bench_merge_case(1_000_000);
    let whole = merge_whole(&base, &current, &incoming)?;
    let (a, b, c): (Vec<char>, Vec<char>, Vec<char>) = (base.chars().collect(), incoming.chars().collect(), current.chars().collect());
    let mut edits = 0;
    let diff = median(
        7,
        || Ok(()),
        |()| {
            edits = merge_edits(&a, &b, &c).len();
            Ok(())
        },
    )?;
    println!("| External merge, TypeScript benchmark case (1 % external, 0.5 % local), whole | {whole:.1} |");
    println!("| The same, diff and merge alone ({edits} edits) | {diff:.1} |");
    let (base, current, incoming, _) = merge_case(1_000_000);
    let disjoint = merge_whole(&base, &current, &incoming)?;
    println!("| External merge, disjoint scattered edits (one per ~67 characters), whole | {disjoint:.1} |");
    for count in [500usize, 2000, 8000] {
        let step = markdown.chars().count() / (count + 1);
        let ops = median(
            5,
            || session(&markdown),
            |mut session| {
                session.execute(&Origin::system("import"), |_, tx| {
                    for k in (1..=count).rev() {
                        tx.text_insert(&kition_note_kdm::root_id(), kition_note_kdm::BODY, k * step, "Z")?;
                    }
                    Ok(())
                })?;
                Ok(())
            },
        )?;
        println!("| {count} single-character inserts in one system transaction | {ops:.1} |");
    }
    let (count, chunk) = (8000usize, 500usize);
    let step = markdown.chars().count() / (count + 1);
    let chunked = median(
        5,
        || session(&markdown),
        |mut session| {
            for first in (1..=count).step_by(chunk).collect::<Vec<_>>().into_iter().rev() {
                session.execute(&Origin::system("import"), |_, tx| {
                    for k in (first..(first + chunk).min(count + 1)).rev() {
                        tx.text_insert(&kition_note_kdm::root_id(), kition_note_kdm::BODY, k * step, "Z")?;
                    }
                    Ok(())
                })?;
            }
            Ok(())
        },
    )?;
    println!("| {count} single-character inserts in {} system transactions of {chunk} | {chunked:.1} |", count / chunk);
    println!("\nBudget for the external merge on 1 MB: 100 ms.");
    Ok(())
}

/// The machine, for the report.
pub fn machine() -> String {
    let output = |program: &str, args: &[&str]| {
        Command::new(program).args(args).output().ok().map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned()).unwrap_or_default()
    };
    format!("{} {}", output("uname", &["-sm"]), output("rustc", &["--version"]))
}
