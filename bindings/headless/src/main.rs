//! `kition-note`: the Note kit for Kition's agent. The same document the in-page editor runs
//! (`kition-note-kit`), served headless over the Kit Protocol by `kition-headless`, as the kit's
//! `kit.json` declares it, with the object model (`model.describe`, `model.batch`) and the replica
//! methods (`model.open`, `model.import`, `model.export`, `model.save`) over the document's change
//! exchange (engine contract 2).

/// Commands whose file parameters the engine reads through lent files: none.
const SANDBOXED: &[&str] = &[];

fn main() -> std::process::ExitCode {
    kition_note::add_shipped_fonts();
    kition_headless::run_manifest_model::<kition_note_kit::NoteDocument>(include_str!("../../../kit.json"), SANDBOXED, kition_note_kit::EDITOR_STATE)
}
