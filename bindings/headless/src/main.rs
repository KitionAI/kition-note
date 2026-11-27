//! `kition-note`: the Note kit for Kition's agent. The same document the in-page editor runs
//! (`kition-note-kit`), served headless over the Kit Protocol by `kition-headless`, as the kit's
//! `kit.json` declares it, with the object model (`model.describe`, `model.batch`) and the replica
//! methods (`model.open`, `model.import`, `model.export`, `model.save`) over the document's change
//! exchange (engine contract 2), and the identity a file carries (`file.identity`,
//! `file.fork`): a `.md` file is the note's text and embeds no replica, so it carries none.

use kition_headless::{FileIdentity, Headless, KitInfo};

/// Commands whose file parameters the engine reads through lent files: none.
const SANDBOXED: &[&str] = &[];

fn main() -> std::process::ExitCode {
    kition_note::add_shipped_fonts();
    let kit = match KitInfo::from_manifest(include_str!("../../../kit.json"), SANDBOXED, kition_note_kit::EDITOR_STATE) {
        Ok(kit) => kit,
        Err(error) => {
            eprintln!("kit: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let backend = Headless::<kition_note_kit::NoteDocument>::with_replica_port(kit.clone()).with_file_identity(FileIdentity::NotEmbedded);
    kition_headless::serve_backend(&kit, backend)
}
