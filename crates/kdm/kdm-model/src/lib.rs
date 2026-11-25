//! The Note kit's document on the Kit Data Model: schema `kit.note` v1 (`model/schema.json`).
//!
//! A note is one `Note` object whose `body` text is the whole Markdown file, with two registers
//! saying how the file writes it (`eol`, `bom`), and comment threads beside it (rule M23). The
//! file is the text: export writes the stored characters back, nothing is regenerated from a
//! parse, so a round trip is byte-identical by construction (`docs/design.md` section 2).
//!
//! Ported from Kition's `packages/document-editor/src/kdm` (`documentSchema.ts`,
//! `documentText.ts`, `markdownFile.ts`, `documentComments.ts`) and the TypeScript engine
//! `packages/document-kit/src/engine/documentModel.ts`.

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use kition_kit::kdm::{ActorId, DocId, ObjectId, Origin, Replica, Schema, Tx};
use kition_kit::session::{BlobSink, BlobSource, Damage, KitError, KitModel, Result};
use kition_note_markdown_file::merge::{TextEdit, diff_text, merge_edits};
use kition_note_markdown_file::{Eol, LineStyle, decode, encode, pieces};
use serde_json::{Map, Value, json};

mod repair;

pub use kition_note_markdown_file as file;
pub use repair::repair_comment_threads;

/// The kit id the schema names.
pub const KIT: &str = "kit.note";
/// The root type and its fields.
pub const NOTE: &str = "Note";
pub const BODY: &str = "body";
pub const EOL: &str = "eol";
pub const BOM: &str = "bom";

/// The body is written to a new replica in pieces of about this many characters, last first: an
/// edit inside one text element costs time in proportion to the element's length.
pub const BODY_PIECE: usize = 16 * 1024;

const SCHEMA: &str = include_str!("../model/schema.json");

/// The `Note` object's id: one per document, written by the replica that creates it, so every
/// reader finds it without an index.
pub fn root_id() -> ObjectId {
    ObjectId::from_string("note")
}

/// The kit's schema.
pub fn schema() -> Arc<Schema> {
    static PARSED: OnceLock<Arc<Schema>> = OnceLock::new();
    PARSED
        .get_or_init(|| {
            #[allow(clippy::expect_used)]
            Arc::new(Schema::parse(SCHEMA).expect("model/schema.json is a fixed, tested file"))
        })
        .clone()
}

/// The materialised view: the note's text and the style its file is written in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteModel {
    body: String,
    style: LineStyle,
}

impl NoteModel {
    /// The Markdown text as the replica holds it.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// How the file writes the text.
    pub fn style(&self) -> LineStyle {
        self.style
    }

    fn read(replica: &Replica) -> Result<Self> {
        Ok(Self { body: replica.text(&root_id(), BODY)?, style: style_of(replica) })
    }
}

/// The line style the `Note` object's registers record (absent: `lf`, no byte order mark).
pub fn style_of(replica: &Replica) -> LineStyle {
    let eol = replica.get(&root_id(), EOL).ok().and_then(|value| value.as_str().and_then(Eol::parse)).unwrap_or_default();
    let bom = replica.get(&root_id(), BOM).ok().and_then(|value| value.as_bool()).unwrap_or(false);
    LineStyle { eol, bom }
}

/// The registers of `style`: only what differs from the defaults is written.
fn style_fields(style: LineStyle) -> Map<String, Value> {
    let mut fields = Map::new();
    if style.eol != Eol::Lf {
        fields.insert(EOL.into(), json!(style.eol.as_str()));
    }
    if style.bom {
        fields.insert(BOM.into(), json!(true));
    }
    fields
}

/// Writes `style` into the registers when it differs from what they hold.
pub fn set_style(tx: &mut Tx<'_>, current: LineStyle, style: LineStyle) -> Result<()> {
    if current.eol != style.eol {
        tx.set(&root_id(), EOL, json!(style.eol.as_str()))?;
    }
    if current.bom != style.bom {
        tx.set(&root_id(), BOM, json!(style.bom))?;
    }
    Ok(())
}

/// A new note replica holding `text` in `style`, written in the creating commit (not an undo
/// step). The body goes in pieces of about [`BODY_PIECE`] characters, last first, so they stay
/// separate text elements in the replica and its snapshots.
pub fn create_replica(text: &str, style: LineStyle, actor: ActorId, origin: &Origin) -> Result<Replica> {
    create_replica_with(text, style, actor, origin, |_| Ok(()))
}

/// [`create_replica`] whose creating commit also runs `more` (comment threads a note read again
/// keeps, `kition-note-kit`'s continuity).
pub fn create_replica_with(
    text: &str,
    style: LineStyle,
    actor: ActorId,
    origin: &Origin,
    more: impl FnOnce(&mut Tx<'_>) -> kition_kit::kdm::Result<()>,
) -> Result<Replica> {
    let (replica, ()) = Replica::create_with(schema(), DocId::new(), actor, origin, |tx| {
        tx.create_with_id(NOTE, &root_id(), style_fields(style))?;
        for piece in pieces(text, BODY_PIECE).into_iter().rev() {
            tx.text_insert(&root_id(), BODY, 0, piece)?;
        }
        more(tx)
    })?;
    Ok(replica)
}

/// Applies `edits` (sorted, separated, relative to the field's text when the call starts; scalar
/// indices) to the note's body.
pub fn apply_text_edits(tx: &mut Tx<'_>, edits: &[TextEdit]) -> Result<()> {
    // Last edit first, so every index still counts in the old text.
    for edit in edits.iter().rev() {
        if edit.to > edit.from {
            tx.text_delete(&root_id(), BODY, edit.from, edit.to - edit.from)?;
        }
        if !edit.insert.is_empty() {
            tx.text_insert(&root_id(), BODY, edit.from, &edit.insert)?;
        }
    }
    Ok(())
}

/// Writes the difference from `current` (the body now) to `target` as small edits: lines that did
/// not change are not touched.
pub fn write_body(tx: &mut Tx<'_>, current: &str, target: &str) -> Result<bool> {
    let (a, b): (Vec<char>, Vec<char>) = (current.chars().collect(), target.chars().collect());
    let edits = diff_text(&a, &b);
    apply_text_edits(tx, &edits)?;
    Ok(!edits.is_empty())
}

/// Merges the file change `base → incoming` into the body, whose text is `current` (holding local
/// edits made since `base`): every insertion of both sides is kept. Answers whether the body
/// changed.
pub fn merge_external(tx: &mut Tx<'_>, base: &str, incoming: &str, current: &str) -> Result<bool> {
    if base == incoming {
        return Ok(false);
    }
    let (base, incoming, current): (Vec<char>, Vec<char>, Vec<char>) =
        (base.chars().collect(), incoming.chars().collect(), current.chars().collect());
    if tx.text_len(&root_id(), BODY)? != current.len() {
        return Err(KitError::new("the merge's current text is not the note's text"));
    }
    let edits = merge_edits(&base, &incoming, &current);
    apply_text_edits(tx, &edits)?;
    Ok(!edits.is_empty())
}

impl KitModel for NoteModel {
    fn schema() -> Arc<Schema> {
        schema()
    }

    fn load(replica: &Replica, _blobs: &dyn BlobSource) -> Result<Self> {
        Self::read(replica)
    }

    fn apply(&mut self, replica: &Replica, _changed: &BTreeSet<ObjectId>, _blobs: &dyn BlobSource) -> Result<Damage> {
        *self = Self::read(replica)?;
        Ok(Damage::Full)
    }

    /// After a merged draft, an import or an undo (the SDK's repair): a draft and the person
    /// deleting the comments of one thread between them leave it with none, and the person's Undo
    /// of a thread leaves the replies added since in no thread (`repair_comment_threads`).
    fn validate(tx: &mut Tx<'_>, _changed: &BTreeSet<ObjectId>) -> Result<bool> {
        Ok(repair_comment_threads(tx)?)
    }

    /// A new note from a Markdown file. A file that is not valid UTF-8 has no replica form: the
    /// document opens it read-only on its own (`NoteDocument::open`), and this refuses it.
    fn import(_name: &str, bytes: &[u8], _blobs: &mut dyn BlobSink, actor: ActorId) -> Result<Replica> {
        let (text, style) = decode(bytes).map_err(|error| KitError::new(format!("note.invalid-utf8: {error}")))?;
        create_replica(&text, style, actor, &Origin::system("import"))
    }

    fn export(&self, _replica: &Replica, _name: &str, _blobs: &dyn BlobSource) -> Result<Vec<u8>> {
        Ok(encode(&self.body, self.style))
    }
}

/// The `Note` object's registers as JSON, for `inspect`.
pub fn registers(replica: &Replica) -> Value {
    let style = style_of(replica);
    json!({ EOL: style.eol.as_str(), BOM: style.bom })
}

#[cfg(test)]
mod tests;
