//! Keeps a note's comment threads (rule M23) whole after merges. Ported from Kition's
//! `packages/document-editor/src/kdm/documentComments.ts`.
//!
//! Each command keeps the threads whole on its own replica, but concurrent ones may not: two
//! people deleting the two comments of a thread leave it with none, and a reply to a thread
//! someone else deleted is a comment in no thread. Deleting the last comment of a thread deletes
//! it, so the repair does the same, and deletes comments no thread holds. Every replica that
//! merges the same changes repairs them the same way.

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
