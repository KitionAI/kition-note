//! What a note keeps when its host reads its file again (kition-sdk's reread contract,
//! `docs/platform/kits/call-context.md`, "Session Continuity On Reread").
//!
//! A clean note whose file another program changed is read again: the host opens the file as a
//! fresh note, hands it the note it replaces ([`NoteDocument::carry_from`]), checks that the open
//! note's editor state resolves on it ([`NoteDocument::put_back_state`]) and then asks the open
//! note to take the file's difference into its own session ([`NoteDocument::reconcile`]). The
//! note always can: the file is the text, so the difference is a text diff, written into the
//! replica as one import (outside both undo histories, the threads re-anchored), and the session
//! keeps its undo histories, its selection, its folds and its comment threads with their ids. A
//! note whose file is not valid UTF-8 (read-only, before or after) is not reconciled; the host
//! then uses the fresh note, which [`NoteDocument::carry_from`] gave the threads and the
//! projection's ids.
//!
//! The editor state is the selection and the folds as byte offsets; it names no text, so a change
//! of the document alone does not read as a change of the editor state. The note remembers the
//! text it answered each recent state on: a state taken on an earlier text (before an agent's
//! write, or before the file was read again) is moved along the change since, as another person's
//! change moves the editor ([`crate::editor::Editor::put_back`]). A state the note did not answer
//! recently is taken as offsets of the text now.

use std::sync::Arc;

use kition_kit::kdm::ObjectId;
use kition_kit::kdm::comment::{COMMENT_THREAD_TYPE, COMMENT_TYPE};
use kition_kit::session::Damage;
use kition_note_kdm::{set_style, write_body};
use serde_json::{Value, json};

use crate::{NoteDocument, run};

/// How many recent editor states a note remembers the text of.
const STATED: usize = 8;

/// The editor states a note answered recently, each with the text it was answered on, newest last.
#[derive(Default)]
pub struct Stated(Vec<(Value, Arc<str>)>);

impl Stated {
    fn remember(&mut self, state: &Value, text: &str) {
        if let Some(at) = self.0.iter().position(|(known, on)| known == state && **on == *text) {
            let entry = self.0.remove(at);
            self.0.push(entry);
            return;
        }
        // The text, shared with the entries that hold it already.
        let text = self.0.iter().rev().find(|(_, on)| **on == *text).map_or_else(|| Arc::from(text), |(_, on)| on.clone());
        if self.0.len() == STATED {
            self.0.remove(0);
        }
        self.0.push((state.clone(), text));
    }

    /// The text the newest answer of `state` was given on.
    fn text(&self, state: &Value) -> Option<Arc<str>> {
        self.0.iter().rev().find(|(known, _)| known == state).map(|(_, text)| text.clone())
    }
}

fn offset(value: &Value, name: &str) -> Result<usize, String> {
    value.as_u64().and_then(|at| usize::try_from(at).ok()).ok_or_else(|| format!("the editor state's `{name}` is not a byte offset"))
}

impl NoteDocument {
    /// `editor_state`: `{ anchor, focus, folded }`, byte offsets of the text now. Remembers the
    /// text, so the state can be put back after it changed.
    pub(crate) fn state_now(&self) -> Value {
        let state = json!({ "anchor": self.editor.anchor, "focus": self.editor.focus, "folded": self.editor.folded });
        self.stated.borrow_mut().remember(&state, self.text());
        state
    }

    /// `restore_editor_state`: the selection and folds of `saved`, moved along the change from the
    /// text it was taken on to the text now. `null` (no state) changes nothing. Never an edit, and
    /// `revision()` does not move: what the view shows is taken as already drawn.
    pub(crate) fn put_back_state(&mut self, saved: &Value) -> Result<(), String> {
        if saved.is_null() {
            return Ok(());
        }
        let (anchor, focus) = (offset(&saved["anchor"], "anchor")?, offset(&saved["focus"], "focus")?);
        let folded = saved["folded"]
            .as_array()
            .ok_or("the editor state's `folded` is not a list")?
            .iter()
            .map(|at| offset(at, "folded"))
            .collect::<Result<Vec<_>, _>>()?;
        let from = self.stated.borrow().text(saved);
        self.editor.put_back(from.as_deref(), anchor, focus, &folded)?;
        if self.painted.get().is_some() {
            self.painted.set(Some(self.painted_key()));
        }
        Ok(())
    }

    /// `carry_replica_objects`: what only the session of `previous` (the note this one, read from
    /// the same file, replaces) held: its comment threads with their ids, each found again by the
    /// text it quotes or kept orphaned; the projection's ids, followed to this text as a change
    /// made in the file; and its text, which its editor state refers to. The threads are written
    /// as an import, so this note has no undo step and no unsaved change.
    pub(crate) fn carry_from(&mut self, previous: &Self) -> Result<(), String> {
        self.stated.borrow_mut().remember(&previous.state_now(), previous.text());
        if self.is_read_only() || previous.is_read_only() {
            return Ok(());
        }
        let (before, now) = (previous.text(), self.text().to_owned());
        let threads = kition_kit::kdm::comment::threads(previous.session.replica()).map_err(|error| error.to_string())?;
        if !threads.is_empty() {
            self.session
                .reread(|_, tx| {
                    for thread in &threads {
                        let mut fields = serde_json::Map::new();
                        fields.insert("anchor".into(), run::reanchor(before, &now, &thread.anchor));
                        fields.insert("resolved".into(), json!(thread.resolved));
                        tx.create_with_id(COMMENT_THREAD_TYPE, &thread.id, fields)?;
                        for (at, comment) in thread.comments.iter().enumerate() {
                            let mut fields = serde_json::Map::new();
                            fields.insert("author".into(), json!(comment.author));
                            if let Some(name) = &comment.name {
                                fields.insert("name".into(), json!(name));
                            }
                            fields.insert("created".into(), json!(comment.created));
                            tx.create_with_id(COMMENT_TYPE, &comment.id, fields)?;
                            if !comment.body.is_empty() {
                                tx.text_insert(&comment.id, "body", 0, &comment.body)?;
                            }
                            tx.list_insert(&thread.id, "comments", at, &comment.id)?;
                        }
                    }
                    Ok(())
                })
                .map_err(|error| error.to_string())?;
        }
        let mut tracked = previous.ids().clone();
        tracked.follow(&now, true);
        *self.tracked.borrow_mut() = tracked;
        // Threads orphaned before the file changed are not news.
        *self.told_orphans.borrow_mut() =
            run::orphaned_of(before, threads.iter().map(|thread| (thread.id.as_str().to_owned(), thread.anchor.clone())))
                .into_iter()
                .map(|(thread, _)| thread)
                .collect();
        Ok(())
    }

    /// `reconcile_reread`: the text of `fresh` (this note's file read again) taken into this
    /// session as one import, a line diff refined to characters with only the changed text
    /// written, the line style when it changed, and each thread's anchor where its quoted text
    /// went; outside both undo histories. The selection, folds and undo records move with the
    /// change as with another person's; the threads, the projection's ids and the replica's
    /// object ids stay. `false`, changing nothing, when either note is read-only (not valid
    /// UTF-8). An error leaves the session as it was.
    pub(crate) fn reconcile(&mut self, fresh: &Self) -> Result<bool, String> {
        if self.is_read_only() || fresh.is_read_only() {
            return Ok(false);
        }
        let (before, incoming) = (self.text().to_owned(), fresh.text());
        let (current_style, style) = (self.session.model().style(), fresh.session.model().style());
        let threads = kition_kit::kdm::comment::threads(self.session.replica()).map_err(|error| error.to_string())?;
        let anchors: Vec<(ObjectId, Value)> = threads
            .into_iter()
            .filter_map(|thread| {
                let anchor = run::reanchor(&before, incoming, &thread.anchor);
                (anchor != thread.anchor).then_some((thread.id, anchor))
            })
            .collect();
        let damage = self
            .session
            .reread(|_, tx| {
                set_style(tx, current_style, style)?;
                write_body(tx, &before, incoming)?;
                for (thread, anchor) in &anchors {
                    tx.set(thread, "anchor", anchor.clone())?;
                }
                Ok(())
            })
            .map_err(|error| error.to_string())?;
        if damage != Damage::None {
            self.changed_outside(&damage);
            // A change made in the file: the projection's ids follow it as such (`projection`).
            self.tracked.borrow_mut().follow(self.session.model().body(), true);
        }
        self.base = incoming.to_owned();
        Ok(true)
    }
}
