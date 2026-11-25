//! Keeps a note's comment threads (rule M23) whole after a merged draft, an import or an undo.
//! Ported from Kition's `packages/document-editor/src/kdm/documentComments.ts`.
//!
//! Each command keeps the threads whole, but what it is merged with or taken back over may not: an
//! agent's draft deleting one comment of a thread while the person deletes the other leaves the
//! thread with none (the list merge takes both removals, kition-sdk data model 6.6 rule D6), and
//! the person's Undo of the step that opened a thread deletes it while a reply added since stays,
//! a comment in no thread. Deleting the last comment of a thread deletes it, so the repair does
//! the same, and deletes comments no thread holds. (A draft replying to a thread the document
//! deleted meanwhile is refused by the merge, rule D8.)

use std::collections::BTreeSet;

use kition_kit::kdm::comment::{COMMENT_THREAD_TYPE, COMMENT_TYPE};
use kition_kit::kdm::{KdmError, ObjectId, Tx};

const COMMENTS: &str = "comments";

/// Repairs the comment threads of `tx`'s document: list entries naming no comment, or one the
/// list holds already, are removed; threads left without a comment are deleted, and so are
/// comments no thread holds. Answers whether it changed anything.
pub fn repair_comment_threads(tx: &mut Tx<'_>) -> Result<bool, KdmError> {
    let mut changed = false;
    let mut held: BTreeSet<ObjectId> = BTreeSet::new();
    for thread in tx.objects_of_type(COMMENT_THREAD_TYPE) {
        let items = tx.list_items(&thread, COMMENTS)?;
        for index in (0..items.len()).rev() {
            let Some(item) = items.get(index) else { continue };
            let repeated = items.iter().position(|other| other == item).is_some_and(|first| first < index);
            if !repeated && tx.exists(item) && tx.object_type(item).is_ok_and(|ty| ty == COMMENT_TYPE) {
                continue;
            }
            tx.list_remove(&thread, COMMENTS, index)?;
            changed = true;
        }
        let kept = tx.list_items(&thread, COMMENTS)?;
        if kept.is_empty() {
            tx.delete(&thread)?;
            changed = true;
            continue;
        }
        held.extend(kept);
    }
    for comment in tx.objects_of_type(COMMENT_TYPE) {
        if held.contains(&comment) {
            continue;
        }
        tx.delete(&comment)?;
        changed = true;
    }
    Ok(changed)
}
