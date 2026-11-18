//! Code blocks highlighted when they are laid out, not when the note is parsed: a large note's
//! code took most of its opening. A block's tokens depend on its text and language only, so
//! highlighting it later (in the styles before and after the live preview's hiding, which never
//! touch a code block's token bits) gives the bytes parsing it whole would. Lines of a block not
//! highlighted yet are only ever guessed (`layout::lay_out`); `View::realize` and the layouts of
//! single lines highlight a block before they lay out its lines.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use crate::style::{CodeBlocks, Styles, Tokens};

/// Highlighted blocks kept: a block's tokens are made again after the whole update only when its
/// text changed.
const MEMO_MAX: usize = 4096;

/// The note's code blocks not highlighted yet, and the tokens of blocks highlighted before.
#[derive(Default)]
pub(crate) struct Lights {
    /// Byte ranges (in the note's order) and languages.
    unlit: CodeBlocks,
    /// A block's tokens (from its first line's start) by its lines' text and its language.
    memo: HashMap<u64, Arc<Tokens>>,
}

impl Lights {
    /// After a whole parse: its blocks, none highlighted.
    pub(crate) fn reset(&mut self, code: CodeBlocks) {
        self.unlit = code;
    }

    /// The blocks not highlighted yet.
    pub(crate) fn unlit(&self) -> &[(Range<usize>, Arc<str>)] {
        &self.unlit
    }

    /// After an edit replaced the bytes `old` (now `delta` bytes longer) with a part whose blocks
    /// are `code` (offsets from `at`): its old blocks go, the later ones move.
    pub(crate) fn edit(&mut self, old: &Range<usize>, delta: isize, at: usize, code: &[(Range<usize>, Arc<str>)]) {
        let mut next: CodeBlocks = Vec::with_capacity(self.unlit.len() + code.len());
        let mut added = false;
        for (block, language) in std::mem::take(&mut self.unlit) {
            if block.start >= old.end {
                if !added {
                    next.extend(code.iter().map(|(inner, language)| (inner.start + at..inner.end + at, language.clone())));
                    added = true;
                }
                next.push((block.start.saturating_add_signed(delta)..block.end.saturating_add_signed(delta), language));
            } else if block.start < old.start {
                next.push((block, language));
            }
        }
        if !added {
            next.extend(code.iter().map(|(inner, language)| (inner.start + at..inner.end + at, language.clone())));
        }
        self.unlit = next;
    }

    /// Highlights the blocks not highlighted yet that touch `bytes`, in `styles` and `base`.
    /// Answers whether there were any.
    pub(crate) fn light(&mut self, text: &str, bytes: Range<usize>, styles: &mut Styles, mut base: Option<&mut Styles>) -> bool {
        let from = self.unlit.partition_point(|(block, _)| block.end < bytes.start);
        let to = self.unlit.partition_point(|(block, _)| block.start <= bytes.end).max(from);
        if from == to {
            return false;
        }
        let blocks: CodeBlocks = self.unlit.drain(from..to).collect();
        if self.memo.len() > MEMO_MAX {
            self.memo.clear();
        }
        for (block, language) in blocks {
            let line_start = styles.starts.get(styles.line_of(block.start)).copied().unwrap_or(0).min(block.start);
            let mut hasher = rustc_hash::FxHasher::default();
            text.get(line_start..block.end).hash(&mut hasher);
            language.hash(&mut hasher);
            let key = hasher.finish();
            let tokens = match self.memo.get(&key) {
                Some(found) => found.clone(),
                None => {
                    let made: Tokens = styles
                        .tokens(text, &block, &language)
                        .into_iter()
                        .map(|(span, value)| (span.start - line_start..span.end - line_start, value))
                        .collect();
                    let made = Arc::new(made);
                    self.memo.insert(key, made.clone());
                    made
                }
            };
            styles.light(&tokens, line_start);
            if let Some(base) = base.as_deref_mut() {
                base.light(&tokens, line_start);
            }
        }
        true
    }
}
