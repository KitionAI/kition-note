//! Other files, as the host lends them (`kition_kit::host`): whether a linked file exists and the
//! text of a linked note. Headless, `kition-note` lends the workspace; in the page, Kition's kit
//! worker does. Without a host every answer is "unknown", so nothing is reported as missing.

use kition_note_check::FileLookup;

/// The host's lent files.
pub struct HostFiles;

/// The largest linked note read to check an anchor in it.
const MAX_TEXT: u64 = 4 * 1024 * 1024;

impl FileLookup for HostFiles {
    fn exists(&self, path: &str) -> Option<bool> {
        match kition_kit::host::size(path) {
            Ok(_) => Some(true),
            // Natively a missing file reads "<path>: entity not found"; Kition's host says
            // "<path> is not in the workspace". Anything else (no host, a path outside the
            // workspace) is not an answer.
            Err(message) if message.ends_with("not found") || message.ends_with("is not in the workspace") => Some(false),
            Err(_) => None,
        }
    }

    fn read_text(&self, path: &str) -> Option<String> {
        let size = kition_kit::host::size(path).ok()?;
        if size > MAX_TEXT {
            return None;
        }
        let bytes = kition_kit::host::read(path).ok()?;
        kition_note_kdm::file::decode(&bytes).ok().map(|(text, _)| text)
    }
}

/// Every file or folder parameter of the kit's commands (KHP A28), declared as typed files in the
/// commands' schemas (`kition_kit::files`). The note's commands work on its text and lend linked
/// files through the host: none names a file.
pub const FILES: &[kition_kit::files::FileParam] = &[];

#[cfg(test)]
mod file_param_tests {
    use kition_kit::KitDocument;

    use super::*;

    /// Parameters whose names read like files but are not.
    const NOT_FILES: &[(&str, &str)] = &[];

    #[test]
    fn every_file_parameter_is_declared() {
        let doc = crate::NoteDocument::create("Untitled.md", 64, 64).unwrap_or_else(|error| panic!("{error}"));
        let problems = kition_kit::files::check(&doc.commands(), FILES, NOT_FILES);
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
