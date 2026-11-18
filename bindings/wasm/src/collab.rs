//! The members a host uses to keep a note's replica beside its file (engine contract 2): open a
//! file with the replica the host stored for it, the replica's id, and the blob exchange (a note
//! has no blobs). The object model is served over the change exchange (`ReplicaPort`).

use std::sync::Arc;

use kition_kit::kdm::{ActorId, Origin, Schema};
use kition_kit::session::{KitModel, KitSession, KitSessionDocument, MemoryBlobs};
use kition_kit::{KitCommand, KitDocument};
use kition_note_kdm::file::decode;
use kition_note_kdm::{NoteModel, merge_external, set_style, write_body};
use serde_json::{Value, json};

use crate::{NoteDocument, check_name};

/// An empty blob bundle: a note references no blobs.
pub fn encode_blobs() -> Vec<u8> {
    Vec::new()
}

/// A note takes no blobs: an empty bundle is fine, anything else is refused.
fn check_no_blobs(blobs: &[u8]) -> Result<(), String> {
    if blobs.is_empty() { Ok(()) } else { Err("a note references no blobs; the bundle must be empty".into()) }
}

impl NoteDocument {
    /// Opens the file `bytes` with the replica the host stored for it (`snapshot`): the note keeps
    /// its KDM identity (document id, comments, history) across sessions. What the file holds that
    /// the replica does not is recorded as one `system` import change (a line diff refined to
    /// characters, never a whole replace), and so is the file's line style: the file is the
    /// document. Empty `bytes` open the replica alone. A replica that does not open (damaged, of
    /// another kit, a newer schema) is ignored and the file opens as [`KitDocument::open`] opens
    /// it; so does a file that is not valid UTF-8 (read-only).
    pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        check_name(name)?;
        check_no_blobs(blobs)?;
        let mut session = match KitSession::<NoteModel>::open_snapshot(snapshot, Box::new(MemoryBlobs::default()), ActorId::random()) {
            Ok(session) => session,
            Err(error) if bytes.is_empty() => return Err(format!("the stored replica does not open: {error}")),
            Err(_) => return Self::open(name, bytes),
        };
        if !bytes.is_empty() {
            let Ok((incoming, style)) = decode(bytes) else { return Self::open(name, bytes) };
            let (current, current_style) = (session.model().body().to_owned(), session.model().style());
            if current != incoming || current_style != style {
                session.execute(&Origin::system("import"), |_, tx| {
                    set_style(tx, current_style, style)?;
                    write_body(tx, &current, &incoming)
                })?;
            }
        }
        let mut document = Self::from_session(session);
        document.name = name.to_owned();
        Ok(document)
    }

    /// The file changed on disk to `bytes`: merges the change since the file was last read or
    /// written into the note as one `system` change, keeping the edits made here meanwhile (every
    /// insertion of both sides survives; `docs/design.md` 2.5). Never overwrites unsaved edits.
    /// Answers `{ "changed": bool }`; a file that is not valid UTF-8 is refused. A note that ends up
    /// holding exactly the file (it had no unsaved edits, or the file took them) is saved: there is
    /// nothing left to write (Kit UI protocol `document/fileChanged`, a reload of a clean note).
    pub fn merge_file(&mut self, bytes: &[u8]) -> Result<Value, String> {
        if self.is_read_only() {
            return Err("read-only: the note's file is not valid UTF-8".into());
        }
        let (incoming, style) = decode(bytes).map_err(|error| format!("note.invalid-utf8: {error}"))?;
        let base = std::mem::take(&mut self.base);
        let current_style = self.session.model().style();
        let result = self.session.execute(&Origin::system("import"), |model, tx| {
            set_style(tx, current_style, style)?;
            merge_external(tx, &base, &incoming, model.body())
        });
        match result {
            Ok((changed, damage)) => {
                self.base = incoming;
                self.changed(&damage);
                // A change made in the file: the projection's ids follow it as such (`projection`).
                self.tracked.borrow_mut().follow(self.session.model().body(), true);
                if self.session.model().body() == self.base && self.session.model().style() == style {
                    self.saved = self.revision;
                }
                Ok(json!({ "changed": changed || current_style != style }))
            }
            Err(error) => {
                self.base = base;
                Err(error.to_string())
            }
        }
    }

    /// The replica's document id (stable across saves; never the file's path).
    pub fn doc_id(&self) -> String {
        self.session.replica().doc_id().to_string()
    }

    /// Every blob the replica references: none.
    pub fn blobs(&self) -> Vec<u8> {
        encode_blobs()
    }

    /// Takes a peer's blobs: a note has none, so only an empty bundle is accepted.
    pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), String> {
        check_no_blobs(bytes)
    }
}

/// The object model (`KitObjectModel 1.0`) headless, over the document's change exchange.
impl kition_kit::session::model::ReplicaPort for NoteDocument {
    fn model_schema(&self) -> Arc<Schema> {
        NoteModel::schema()
    }

    fn model_export(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        kition_kit::session::__export_changes(self, since)
    }

    fn model_import(&mut self, bytes: &[u8]) -> Result<(), String> {
        KitSessionDocument::import_changes(self, bytes).map(|_| ())
    }

    fn model_command(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        self.execute_as(command, params, origin)
    }

    fn model_commands(&self) -> Vec<KitCommand> {
        self.commands()
    }

    /// A shared note with no file yet: the replica alone, failing rather than falling back to an
    /// empty note.
    fn model_open_replica(name: &str, snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        check_name(name)?;
        check_no_blobs(blobs)?;
        let session = KitSession::<NoteModel>::open_snapshot(snapshot, Box::new(MemoryBlobs::default()), ActorId::random())?;
        let mut document = Self::from_session(session);
        document.name = name.to_owned();
        Ok(document)
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use kition_kit::wasm_bindgen::prelude::*;

    use crate::__kition_kit_export::WebDocument;
    use crate::NoteDocument;

    #[wasm_bindgen(js_class = "NoteDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        #[wasm_bindgen(js_name = openWithReplica)]
        pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<WebDocument, JsError> {
            NoteDocument::open_with_replica(name, bytes, snapshot, blobs).map(WebDocument).map_err(|error| JsError::new(&error))
        }

        #[wasm_bindgen(getter, js_name = hasReplica)]
        pub fn has_replica(&self) -> bool {
            true
        }

        #[wasm_bindgen(getter, js_name = docId)]
        pub fn doc_id(&self) -> String {
            self.0.doc_id()
        }

        pub fn blobs(&self) -> Vec<u8> {
            self.0.blobs()
        }

        #[wasm_bindgen(js_name = blobsSince)]
        pub fn blobs_since(&self, _since: &[u8]) -> Vec<u8> {
            self.0.blobs()
        }

        /// The file changed on disk: merges it, keeping the edits made here. `{ "changed": bool }`.
        #[wasm_bindgen(js_name = mergeFile)]
        pub fn merge_file(&mut self, bytes: &[u8]) -> Result<String, JsError> {
            self.0.merge_file(bytes).map(|value| value.to_string()).map_err(|error| JsError::new(&error))
        }

        /// The note as a standalone HTML file (an extra; `file.export` answers the same as base64).
        #[wasm_bindgen(js_name = exportHtml)]
        pub fn export_html(&self, standalone: bool) -> String {
            self.0.export_html(standalone)
        }

        /// The note as PDF bytes, on `letter` or `a4` pages (an extra; `file.export` answers base64).
        #[wasm_bindgen(js_name = exportPdf)]
        pub fn export_pdf(&self, page_size: &str) -> Result<Vec<u8>, JsError> {
            self.0.export_pdf(page_size).map_err(|error| JsError::new(&error))
        }

        #[wasm_bindgen(js_name = addBlobs)]
        pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), JsError> {
            self.0.add_blobs(bytes).map_err(|error| JsError::new(&error))
        }
    }
}
