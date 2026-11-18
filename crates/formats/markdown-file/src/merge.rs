//! Text differences and the three-way merge of an external file change into the note's text.
//!
//! Ported from Kition's `packages/document-editor/src/kdm/documentMerge.ts`. Positions are Unicode
//! scalar values (the KDM's unit, rule M18), so no edit can split a character; the TypeScript
//! counted UTF-16 code units and stepped around surrogate pairs.
//!
//! [`diff_text`] is a patience line diff (lines unique to both sides anchor it), refined to
//! characters inside changed lines with a bounded Myers diff; its cost grows with the number of
//! changes, not the size of the texts. [`merge_edits`] brings an external change `base → incoming`
//! into `current` (which holds local edits made since `base`), keeping every insertion of both
//! sides.

use std::collections::HashMap;

/// Replaces `from..to` of a text with `insert` (scalar indices). Lists of edits are sorted and
/// separated: each starts after the previous one ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

impl TextEdit {
    pub fn new(from: usize, to: usize, insert: impl Into<String>) -> Self {
        Self { from, to, insert: insert.into() }
    }
}

/// A differing region of two token sequences: `a[a_from, a_to)` became `b[b_from, b_to)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hunk {
    a_from: usize,
    a_to: usize,
    b_from: usize,
    b_to: usize,
}

/// Myers gives up on detail (not on correctness) past about this many token comparisons.
const DIFF_WORK: usize = 20_000_000;
/// And past this many edits, which bounds its memory (about the square of it).
const DIFF_MAX_EDITS: usize = 2_000;
/// Character detail is kept for changes of up to this many characters, or an eighth of the
/// changed range if more.
const REFINE_MIN_EDITS: usize = 64;
/// Character refinement stops, leaving line-level edits, after about this many diff steps plus
/// `REFINE_WORK_PER_UNIT` per character of the changed middles of both texts.
const REFINE_WORK: usize = 2_000_000;
const REFINE_WORK_PER_UNIT: usize = 2;
/// A changed line longer than this (in characters, on either side) is replaced as a whole.
const REFINE_MAX_LENGTH: usize = 64 * 1024;
/// A range of lines without anchors no longer than this (in characters, on either side) is left
/// to the character diff whole.
const CHARACTER_GAP_LENGTH: usize = 4 * 1024;
/// The line diff stops looking for anchors after scanning this many times the lines of both texts.
const ANCHOR_WORK_FACTOR: usize = 32;

/// `values[index]`, or `fallback` past the end: indices below come from the algorithm's own
/// bounds, so the fallback is never used, but nothing here may panic on input.
fn at<T: Copy>(values: &[T], index: usize, fallback: T) -> T {
    values.get(index).copied().unwrap_or(fallback)
}

fn slice<T>(values: &[T], from: usize, to: usize) -> &[T] {
    values.get(from..to.max(from)).unwrap_or_default()
}

fn string(chars: &[char]) -> String {
    chars.iter().collect()
}

/// The length of the common prefix of `a` and `b`, up to `limit`.
pub fn common_prefix(a: &[char], b: &[char], limit: usize) -> usize {
    a.iter().zip(b).take(limit).take_while(|(x, y)| x == y).count()
}

/// The length of the common suffix of `a` and `b`, up to `limit`.
pub fn common_suffix(a: &[char], b: &[char], limit: usize) -> usize {
    a.iter().rev().zip(b.iter().rev()).take(limit).take_while(|(x, y)| x == y).count()
}

/// Counts the steps of the diffs it is given to, so that their total can be bounded.
#[derive(Default)]
struct Meter {
    work: usize,
}

/// The shortest edit script from `a` to `b` (Myers), as hunks; `None` when it needs more than
/// `max_edits` insertions and deletions. Adds its steps to `meter`.
fn myers<T: PartialEq>(a: &[T], b: &[T], max_edits: usize, meter: &mut Meter) -> Option<Vec<Hunk>> {
    let (n, m) = (a.len(), b.len());
    let max = n + m;
    if max == 0 {
        return Some(Vec::new());
    }
    let offset = max + 1;
    let mut v = vec![0usize; 2 * max + 3];
    // `trace[d]`: the furthest x per diagonal before step d, for diagonals -d-1..=d+1.
    let mut trace: Vec<Vec<usize>> = Vec::new();
    let limit = max.min(max_edits);
    let mut work = 0usize;
    for d in 0..=limit {
        trace.push(slice(&v, offset - d - 1, offset + d + 2).to_vec());
        work += 2 * d + 3;
        let mut k = -(d as isize);
        while k <= d as isize {
            let index = (offset as isize + k) as usize;
            let (left, right) = (at(&v, index.wrapping_sub(1), 0), at(&v, index + 1, 0));
            let mut x = if k == -(d as isize) || (k != d as isize && left < right) { right } else { left + 1 };
            let mut y = (x as isize - k) as usize;
            let snake_from = x;
            while x < n && y < m && a.get(x) == b.get(y) {
                x += 1;
                y += 1;
            }
            work += x - snake_from;
            if let Some(slot) = v.get_mut(index) {
                *slot = x;
            }
            if x >= n && y >= m {
                meter.work += work;
                return Some(backtrack(&trace, d, n, m));
            }
            k += 2;
        }
    }
    meter.work += work;
    None
}

fn backtrack(trace: &[Vec<usize>], steps: usize, n: usize, m: usize) -> Vec<Hunk> {
    let mut reversed: Vec<Hunk> = Vec::new();
    let (mut x, mut y) = (n as isize, m as isize);
    for d in (1..=steps).rev() {
        let before = trace.get(d).map(Vec::as_slice).unwrap_or_default();
        let d = d as isize;
        let value = |k: isize| at(before, (k + d + 1) as usize, 0) as isize;
        let k = x - y;
        let down = k == -d || (k != d && value(k - 1) < value(k + 1));
        let previous_k = if down { k + 1 } else { k - 1 };
        let previous_x = value(previous_k);
        let previous_y = previous_x - previous_k;
        let start_x = if down { previous_x } else { previous_x + 1 };
        reversed.push(Hunk { a_from: previous_x as usize, a_to: start_x as usize, b_from: previous_y as usize, b_to: (start_x - k) as usize });
        x = previous_x;
        y = previous_y;
    }
    let mut hunks: Vec<Hunk> = Vec::new();
    for step in reversed.into_iter().rev() {
        match hunks.last_mut() {
            Some(last) if last.a_to == step.a_from && last.b_to == step.b_from => {
                last.a_to = step.a_to;
                last.b_to = step.b_to;
            }
            _ => hunks.push(step),
        }
    }
    hunks
}

/// Hunks of `a` to `b`, or one hunk of everything when the exact script costs too much.
fn diff_tokens<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Hunk> {
    let size = a.len() + b.len();
    let max_edits = DIFF_MAX_EDITS.min(64.max(DIFF_WORK / size.max(1)));
    myers(a, b, max_edits, &mut Meter::default()).unwrap_or_else(|| vec![Hunk { a_from: 0, a_to: a.len(), b_from: 0, b_to: b.len() }])
}

/// Indexes of a longest strictly increasing subsequence of `values`.
fn longest_increasing(values: &[usize]) -> Vec<usize> {
    // `tails[length - 1]`: the index of the smallest value ending an increasing run of that length.
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![usize::MAX; values.len()];
    for (index, &value) in values.iter().enumerate() {
        let low = tails.partition_point(|&tail| at(values, tail, 0) < value);
        if let Some(slot) = previous.get_mut(index) {
            *slot = if low > 0 { at(&tails, low - 1, usize::MAX) } else { usize::MAX };
        }
        if low == tails.len() {
            tails.push(index);
        } else if let Some(slot) = tails.get_mut(low) {
            *slot = index;
        }
    }
    let mut out = vec![0usize; tails.len()];
    let mut index = tails.last().copied().unwrap_or(usize::MAX);
    for slot in out.iter_mut().rev() {
        *slot = index;
        index = at(&previous, index, usize::MAX);
    }
    out
}

/// Hunks of the line ids `a` to `b` (ids below `id_count`), at a cost that grows with the number
/// of changes: lines that occur once in both (within the range being diffed) and keep their order
/// anchor the diff (patience diff); the ranges between anchors are diffed the same way. A range
/// without such lines is one hunk when `gap_length` (its length in text) is at most
/// `CHARACTER_GAP_LENGTH`, and diffed by a bounded Myers otherwise.
fn diff_lines(a: &[u32], b: &[u32], id_count: usize, gap_length: impl Fn(usize, usize, usize, usize) -> usize) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = Vec::new();
    let mut emit = |a_from: usize, a_to: usize, b_from: usize, b_to: usize| match out.last_mut() {
        Some(last) if last.a_to == a_from && last.b_to == b_from => {
            last.a_to = a_to;
            last.b_to = b_to;
        }
        _ => out.push(Hunk { a_from, a_to, b_from, b_to }),
    };
    let mut count_a = vec![0u32; id_count];
    let mut count_b = vec![0u32; id_count];
    let mut index_in_a = vec![0usize; id_count];
    let mut work = 0usize;
    let work_limit = ANCHOR_WORK_FACTOR * (a.len() + b.len());
    // Ranges still to diff, the next one last.
    let mut pending: Vec<[usize; 4]> = vec![[0, a.len(), 0, b.len()]];
    let token = |tokens: &[u32], index: usize| at(tokens, index, u32::MAX);
    while let Some([mut a_from, mut a_to, mut b_from, mut b_to]) = pending.pop() {
        while a_from < a_to && b_from < b_to && token(a, a_from) == token(b, b_from) {
            a_from += 1;
            b_from += 1;
        }
        while a_from < a_to && b_from < b_to && token(a, a_to - 1) == token(b, b_to - 1) {
            a_to -= 1;
            b_to -= 1;
        }
        if a_from == a_to || b_from == b_to {
            if a_from < a_to || b_from < b_to {
                emit(a_from, a_to, b_from, b_to);
            }
            continue;
        }
        // Lines unique to the range on both sides, in the order of `b`.
        let mut anchor_a: Vec<usize> = Vec::new();
        let mut anchor_b: Vec<usize> = Vec::new();
        work += (a_to - a_from) + (b_to - b_from);
        if work <= work_limit {
            for i in a_from..a_to {
                let id = token(a, i) as usize;
                if let (Some(count), Some(slot)) = (count_a.get_mut(id), index_in_a.get_mut(id)) {
                    *count += 1;
                    *slot = i;
                }
            }
            for j in b_from..b_to {
                if let Some(count) = count_b.get_mut(token(b, j) as usize) {
                    *count += 1;
                }
            }
            for j in b_from..b_to {
                let id = token(b, j) as usize;
                if at(&count_a, id, 0) == 1 && at(&count_b, id, 0) == 1 {
                    anchor_a.push(at(&index_in_a, id, 0));
                    anchor_b.push(j);
                }
            }
            for i in a_from..a_to {
                if let Some(count) = count_a.get_mut(token(a, i) as usize) {
                    *count = 0;
                }
            }
            for j in b_from..b_to {
                if let Some(count) = count_b.get_mut(token(b, j) as usize) {
                    *count = 0;
                }
            }
        }
        if anchor_a.is_empty() {
            if gap_length(a_from, a_to, b_from, b_to) <= CHARACTER_GAP_LENGTH {
                emit(a_from, a_to, b_from, b_to);
                continue;
            }
            for hunk in diff_tokens(slice(a, a_from, a_to), slice(b, b_from, b_to)) {
                emit(a_from + hunk.a_from, a_from + hunk.a_to, b_from + hunk.b_from, b_from + hunk.b_to);
            }
            continue;
        }
        // The anchors that keep their order split the range; the gaps between them come next, in order.
        let kept = longest_increasing(&anchor_a);
        let anchor = |k: usize| (at(&anchor_a, at(&kept, k, 0), 0), at(&anchor_b, at(&kept, k, 0), 0));
        let (last_a, last_b) = anchor(kept.len() - 1);
        pending.push([last_a + 1, a_to, last_b + 1, b_to]);
        for k in (1..kept.len()).rev() {
            let (before_a, before_b) = anchor(k - 1);
            let (here_a, here_b) = anchor(k);
            pending.push([before_a + 1, here_a, before_b + 1, here_b]);
        }
        let (first_a, first_b) = anchor(0);
        pending.push([a_from, first_a, b_from, first_b]);
    }
    out
}

fn hunk_size(hunk: &Hunk) -> usize {
    (hunk.a_to - hunk.a_from).max(hunk.b_to - hunk.b_from)
}

/// Joins hunks separated by an equality no longer than the changes on both sides of it, so that
/// replacing a word is one edit rather than a scatter of single characters it happens to share.
fn join_noise(hunks: Vec<Hunk>) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = Vec::new();
    for hunk in hunks {
        let mut current = hunk;
        while let Some(last) = out.last() {
            let equal = current.a_from - last.a_to;
            if equal > hunk_size(last) || equal > hunk_size(&current) {
                break;
            }
            current = Hunk { a_from: last.a_from, a_to: current.a_to, b_from: last.b_from, b_to: current.b_to };
            out.pop();
        }
        out.push(current);
    }
    out
}

/// Character-level edits from `x` to `y`, relative to `x`, offset by `offset`; `None` when they
/// would change more than about an eighth of the characters.
fn refine(x: &[char], y: &[char], offset: usize, meter: &mut Meter) -> Option<Vec<TextEdit>> {
    let prefix = common_prefix(x, y, x.len().min(y.len()));
    let suffix = common_suffix(x, y, x.len().min(y.len()) - prefix);
    let xs = slice(x, prefix, x.len() - suffix);
    let ys = slice(y, prefix, y.len() - suffix);
    let start = offset + prefix;
    if xs.is_empty() || ys.is_empty() {
        return Some(vec![TextEdit::new(start, start + xs.len(), string(ys))]);
    }
    let size = xs.len() + ys.len();
    let hunks = myers(xs, ys, DIFF_MAX_EDITS.min(REFINE_MIN_EDITS.max(size / 8)), meter)?;
    Some(
        join_noise(hunks)
            .into_iter()
            .map(|hunk| TextEdit::new(start + hunk.a_from, start + hunk.a_to, string(slice(ys, hunk.b_from, hunk.b_to))))
            .collect(),
    )
}

/// Whether all that `edits` (relative to `x`, offset by `offset`) keep of `x` is white space.
fn keeps_only_white_space(x: &[char], offset: usize, edits: &[TextEdit]) -> bool {
    let mut from = offset;
    for edit in edits {
        if !slice(x, from - offset, edit.from - offset).iter().all(|c| c.is_whitespace()) {
            return false;
        }
        from = edit.to;
    }
    slice(x, from - offset, x.len()).iter().all(|c| c.is_whitespace())
}

/// Line ids of a text and the offset of each line (plus the end).
struct Lines {
    tokens: Vec<u32>,
    starts: Vec<usize>,
}

/// Line ids shared by the texts diffed against each other.
type LineIds<'t> = HashMap<&'t [char], u32>;

/// The lines of `text` in `[from, to)` (each with its `\n`, the last possibly without).
fn line_tokens<'t>(text: &'t [char], ids: &mut LineIds<'t>, from: usize, to: usize) -> Lines {
    let mut tokens = Vec::new();
    let mut starts = Vec::new();
    let mut start = from;
    while start < to {
        let end = slice(text, start, to).iter().position(|&c| c == '\n').map_or(to, |newline| start + newline + 1);
        let line = slice(text, start, end);
        let next = u32::try_from(ids.len()).unwrap_or(u32::MAX);
        tokens.push(*ids.entry(line).or_insert(next));
        starts.push(start);
        start = end;
    }
    starts.push(to);
    Lines { tokens, starts }
}

/// The index of the line of `lines` that starts at `offset` (a line boundary by construction).
fn line_at(lines: &Lines, offset: usize) -> usize {
    lines.starts.partition_point(|&start| start < offset)
}

/// The lines of a text diffed against several others, cut once (on first use).
struct LineCache<'t> {
    ids: LineIds<'t>,
    lines: Option<Lines>,
}

impl LineCache<'_> {
    fn new() -> Self {
        Self { ids: HashMap::new(), lines: None }
    }
}

/// The edits from `a` to `b`: a line diff, refined to characters inside changed lines. Edits are
/// sorted, separated, and relative to `a`. Rather than a slow diff, a changed line longer than
/// `REFINE_MAX_LENGTH` is replaced as a whole, and heavily rewritten texts get line-level edits
/// once character refinement has used its budget.
pub fn diff_text(a: &[char], b: &[char]) -> Vec<TextEdit> {
    diff_text_with(a, b, &mut LineCache::new())
}

/// [`diff_text`] of two strings.
pub fn diff_strings(a: &str, b: &str) -> Vec<TextEdit> {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    diff_text(&a, &b)
}

/// Adds an edit, joined to the previous one when they touch, so that edits stay separated.
fn push(out: &mut Vec<TextEdit>, edit: TextEdit) {
    match out.last_mut() {
        Some(last) if last.to == edit.from => {
            last.to = edit.to;
            last.insert.push_str(&edit.insert);
        }
        _ => out.push(edit),
    }
}

/// [`diff_text`], with the lines of `a` from (and cut into) `cache`.
fn diff_text_with<'t>(a: &'t [char], b: &'t [char], cache: &mut LineCache<'t>) -> Vec<TextEdit> {
    if a == b {
        return Vec::new();
    }
    // The common start and end, cut back to whole lines.
    let raw_prefix = common_prefix(a, b, a.len().min(b.len()));
    let start = slice(a, 0, raw_prefix).iter().rposition(|&c| c == '\n').map_or(0, |newline| newline + 1);
    let mut suffix = common_suffix(a, b, a.len().min(b.len()) - raw_prefix);
    let suffix_start = a.len() - suffix;
    if suffix > 0 && suffix_start > start && at(a, suffix_start - 1, '\n') != '\n' {
        suffix = slice(a, suffix_start, a.len()).iter().position(|&c| c == '\n').map_or(0, |newline| a.len() - (suffix_start + newline + 1));
    }
    let (a_end, b_end) = (a.len() - suffix, b.len() - suffix);
    let mut out: Vec<TextEdit> = Vec::new();
    if start == a_end || start == b_end {
        out.push(TextEdit::new(start, a_end, string(slice(b, start, b_end))));
        return out;
    }
    let al = cache.lines.get_or_insert_with(|| line_tokens(a, &mut cache.ids, 0, a.len()));
    let first_line = line_at(al, start);
    let a_tokens = slice(&al.tokens, first_line, line_at(al, a_end)).to_vec();
    let a_starts = al.starts.clone();
    let bl = line_tokens(b, &mut cache.ids, start, b_end);
    let a_start = |line: usize| at(&a_starts, first_line + line, a.len());
    let b_start = |line: usize| at(&bl.starts, line, b_end);
    let gap_length = |a_from: usize, a_to: usize, b_from: usize, b_to: usize| (a_start(a_to) - a_start(a_from)).max(b_start(b_to) - b_start(b_from));
    let mut meter = Meter::default();
    let refine_budget = REFINE_WORK + REFINE_WORK_PER_UNIT * ((a_end - start) + (b_end - start));
    let hunks = diff_lines(&a_tokens, &bl.tokens, cache.ids.len(), gap_length);
    let context = Refining { a, b, a_tokens: &a_tokens, b_tokens: &bl.tokens, a_start: &a_start, b_start: &b_start, refine_budget };
    for hunk in hunks {
        context.refine_hunk(hunk, true, &mut meter, &mut out);
    }
    out
}

/// What refining the hunks of one line diff reads.
struct Refining<'a, FA: Fn(usize) -> usize, FB: Fn(usize) -> usize> {
    a: &'a [char],
    b: &'a [char],
    a_tokens: &'a [u32],
    b_tokens: &'a [u32],
    a_start: &'a FA,
    b_start: &'a FB,
    refine_budget: usize,
}

impl<FA: Fn(usize) -> usize, FB: Fn(usize) -> usize> Refining<'_, FA, FB> {
    /// Refines a hunk of lines to characters. A hunk of several lines whose character edits keep
    /// only white space is replaced as a whole. A hunk of several lines longer than
    /// `CHARACTER_GAP_LENGTH`, or too changed for character detail, is diffed by lines first when
    /// `by_lines`; what that leaves changed is refined line by line, pairing lines in order. Other
    /// hunks too large or too changed for character detail are replaced as a whole.
    fn refine_hunk(&self, hunk: Hunk, by_lines: bool, meter: &mut Meter, out: &mut Vec<TextEdit>) {
        let (a_from, a_to) = ((self.a_start)(hunk.a_from), (self.a_start)(hunk.a_to));
        let x = slice(self.a, a_from, a_to);
        let y = slice(self.b, (self.b_start)(hunk.b_from), (self.b_start)(hunk.b_to));
        let whole = TextEdit::new(a_from, a_to, string(y));
        let several_lines = hunk.a_to - hunk.a_from > 1 || hunk.b_to - hunk.b_from > 1;
        let limit = if several_lines { CHARACTER_GAP_LENGTH } else { REFINE_MAX_LENGTH };
        let edits = if x.len() <= limit && y.len() <= limit && meter.work <= self.refine_budget { refine(x, y, a_from, meter) } else { None };
        if let Some(edits) = edits {
            if several_lines && keeps_only_white_space(x, a_from, &edits) {
                push(out, whole);
            } else {
                for edit in edits {
                    push(out, edit);
                }
            }
            return;
        }
        if !several_lines {
            push(out, whole);
            return;
        }
        let parts = if by_lines {
            diff_tokens(slice(self.a_tokens, hunk.a_from, hunk.a_to), slice(self.b_tokens, hunk.b_from, hunk.b_to))
        } else {
            Vec::new()
        };
        if parts.len() > 1 {
            for part in parts {
                let sub = Hunk {
                    a_from: hunk.a_from + part.a_from,
                    a_to: hunk.a_from + part.a_to,
                    b_from: hunk.b_from + part.b_from,
                    b_to: hunk.b_from + part.b_to,
                };
                self.refine_hunk(sub, false, meter, out);
            }
            return;
        }
        // No line in common (or too many changes to tell): pair the lines in order; the lines left
        // over on one side are inserted or deleted.
        let pairs = (hunk.a_to - hunk.a_from).min(hunk.b_to - hunk.b_from);
        for k in 0..pairs {
            let sub = Hunk { a_from: hunk.a_from + k, a_to: hunk.a_from + k + 1, b_from: hunk.b_from + k, b_to: hunk.b_from + k + 1 };
            self.refine_hunk(sub, false, meter, out);
        }
        let rest_from = (self.a_start)(hunk.a_from + pairs);
        let rest = string(slice(self.b, (self.b_start)(hunk.b_from + pairs), (self.b_start)(hunk.b_to)));
        if rest_from < a_to || !rest.is_empty() {
            push(out, TextEdit::new(rest_from, a_to, rest));
        }
    }
}

/// `text` with `edits` (relative to `text`) applied.
pub fn apply_edits(text: &[char], edits: &[TextEdit]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for edit in edits {
        out.extend(slice(text, from, edit.from));
        out.push_str(&edit.insert);
        from = edit.to;
    }
    out.extend(slice(text, from, text.len()));
    out
}

/// The edits that bring the external change `base → incoming` into `current` (which holds local
/// edits made since `base`), relative to `current`.
///
/// Both changes are diffed from `base`; then base text survives when neither side deleted it, and
/// every insertion of both sides is kept, the local one first where both insert at one place. An
/// external edit of a range that was also edited locally inserts its text next to the local edit
/// and deletes only the base text that is still there. A change made identically on both sides is
/// applied once.
pub fn merge_edits(base: &[char], incoming: &[char], current: &[char]) -> Vec<TextEdit> {
    if incoming == base || incoming == current {
        return Vec::new();
    }
    let mut base_lines = LineCache::new();
    let external = diff_text_with(base, incoming, &mut base_lines);
    let local = if base == current { Vec::new() } else { diff_text_with(base, current, &mut base_lines) };
    let mut out: Vec<TextEdit> = Vec::new();
    let (mut li, mut ei) = (0usize, 0usize);
    // The position in `current` reached so far.
    let mut position = 0usize;
    // Ends (in `base`) of the local and external deletions under way.
    let (mut local_end, mut external_end) = (0usize, 0usize);
    let mut index = 0usize;
    loop {
        let local_here = local.get(li).filter(|edit| edit.from == index);
        let external_here = external.get(ei).filter(|edit| edit.from == index);
        if let Some(edit) = local_here {
            position += edit.insert.chars().count();
            local_end = edit.to;
            li += 1;
        }
        if let Some(edit) = external_here {
            let same = local_here.is_some_and(|local| local.to == edit.to && local.insert == edit.insert);
            if !same {
                if !edit.insert.is_empty() {
                    push(&mut out, TextEdit::new(position, position, edit.insert.clone()));
                }
                external_end = edit.to;
            }
            ei += 1;
        }
        if index >= base.len() {
            break;
        }
        let mut next = base.len();
        if let Some(edit) = local.get(li) {
            next = next.min(edit.from);
        }
        if let Some(edit) = external.get(ei) {
            next = next.min(edit.from);
        }
        if local_end > index {
            next = next.min(local_end);
        }
        if external_end > index {
            next = next.min(external_end);
        }
        if local_end <= index {
            let length = next - index;
            if external_end > index {
                push(&mut out, TextEdit::new(position, position + length, ""));
            }
            position += length;
        }
        index = next;
    }
    out
}

/// `edits` (sorted and separated, relative to `text`) with each run of edits separated by at most
/// `gap` unchanged characters joined into one edit, which replaces the unchanged text between them
/// by itself. Applying the result gives the same text. Opt-in: the joined unchanged text is
/// deleted and inserted again, so concurrent edits inside it do not survive a merge.
pub fn coalesce_edits(edits: &[TextEdit], text: &[char], gap: usize) -> Vec<TextEdit> {
    let mut out: Vec<TextEdit> = Vec::new();
    for edit in edits {
        match out.last_mut() {
            Some(last) if edit.from - last.to <= gap => {
                last.insert.extend(slice(text, last.to, edit.from));
                last.insert.push_str(&edit.insert);
                last.to = edit.to;
            }
            _ => out.push(edit.clone()),
        }
    }
    out
}
