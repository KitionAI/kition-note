//! `text.find` and `text.findReplace`.

use std::ops::Range;

use regex::{Regex, RegexBuilder};

use crate::{Edit, Result, invalid, not_found};

/// The longest pattern accepted, and the most matches answered or replaced.
const MAX_PATTERN: usize = 4096;
pub const MAX_MATCHES: usize = 10_000;

/// Which matches `text.findReplace` replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Occurrence {
    /// The n-th match, from 1.
    Nth(usize),
    All,
}

fn pattern(find: &str, regex: bool, case_sensitive: bool) -> Result<Regex> {
    if find.is_empty() {
        return Err(invalid("find is empty"));
    }
    if find.len() > MAX_PATTERN {
        return Err(invalid(format!("find is longer than {MAX_PATTERN} bytes")));
    }
    let source = if regex { find.to_owned() } else { regex::escape(find) };
    RegexBuilder::new(&source)
        .case_insensitive(!case_sensitive)
        .multi_line(true)
        .size_limit(1 << 20)
        .build()
        .map_err(|error| invalid(format!("find is not a valid regular expression: {error}")))
}

/// Every non-empty match of `find` in `text` that starts at or after byte `from` (where the note's
/// body starts), up to `limit`.
pub fn find_all(text: &str, from: usize, find: &str, regex: bool, case_sensitive: bool, limit: usize) -> Result<Vec<Range<usize>>> {
    let pattern = pattern(find, regex, case_sensitive)?;
    Ok(pattern
        .find_iter(text)
        .filter(|found| !found.range().is_empty() && found.start() >= from)
        .take(limit.min(MAX_MATCHES))
        .map(|found| found.range())
        .collect())
}

/// The edits replacing `occurrence` of `find` with `replace` (`$1` names a group when `regex`),
/// and how many were replaced; `not-found` when nothing matches. Only matches starting at or after
/// byte `from` count: the note's body starts after its front matter, whose properties change
/// through their own commands. The whole text is still searched, so `^` and `\b` read the text
/// before `from`.
pub fn replace(
    text: &str,
    from: usize,
    find: &str,
    replace: &str,
    occurrence: Occurrence,
    regex: bool,
    case_sensitive: bool,
) -> Result<(Vec<Edit>, usize)> {
    let pattern = pattern(find, regex, case_sensitive)?;
    let mut edits = Vec::new();
    let counted = |captures: &regex::Captures<'_>| captures.get(0).is_some_and(|found| !found.range().is_empty() && found.start() >= from);
    for (index, captures) in pattern.captures_iter(text).filter(counted).enumerate() {
        let wanted = match occurrence {
            Occurrence::All => true,
            Occurrence::Nth(n) => index + 1 == n,
        };
        if !wanted {
            continue;
        }
        let Some(found) = captures.get(0) else { continue };
        let mut insert = String::new();
        if regex {
            captures.expand(replace, &mut insert);
        } else {
            insert.push_str(replace);
        }
        edits.push(Edit::new(found.range(), insert));
        if matches!(occurrence, Occurrence::Nth(_)) || edits.len() >= MAX_MATCHES {
            break;
        }
    }
    if edits.is_empty() {
        return Err(not_found(match occurrence {
            Occurrence::Nth(n) if n > 1 => format!("{find:?} does not occur {n} times"),
            _ if from > 0 => format!("{find:?} does not occur in the note's body (the front matter's properties change with property.set)"),
            _ => format!("{find:?} does not occur in the note"),
        }));
    }
    let count = edits.len();
    Ok((edits, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply;

    #[test]
    fn finds_and_replaces() {
        let text = "Ann and ann and ANN.";
        assert_eq!(find_all(text, 0, "ann", false, true, 10).unwrap(), vec![8..11]);
        assert_eq!(find_all(text, 0, "ann", false, false, 10).unwrap().len(), 3);
        let (edits, count) = replace(text, 0, "ann", "Bo", Occurrence::All, false, false).unwrap();
        assert_eq!((apply(text, &edits).as_str(), count), ("Bo and Bo and Bo.", 3));
        let (edits, _) = replace(text, 0, "ann", "Bo", Occurrence::Nth(2), false, false).unwrap();
        assert_eq!(apply(text, &edits), "Ann and Bo and ANN.");
        let (edits, _) = replace("2026-10-23", 0, r"(\d+)-(\d+)-(\d+)", "$3.$2.$1", Occurrence::All, true, true).unwrap();
        assert_eq!(apply("2026-10-23", &edits), "23.10.2026");
        assert!(replace(text, 0, "zz", "x", Occurrence::All, false, true).is_err());
        assert!(find_all(text, 0, "(", true, true, 10).is_err());
        assert!(find_all(text, 0, "", false, true, 10).is_err());
        assert_eq!(find_all("a.b", 0, ".", false, true, 10).unwrap(), vec![1..2]);
        assert_eq!(find_all(text, 4, "ann", false, false, 10).unwrap(), vec![8..11, 16..19]);
        // Matches before `from` are not counted.
        let (edits, count) = replace(text, 4, "ann", "Bo", Occurrence::Nth(1), false, false).unwrap();
        assert_eq!((apply(text, &edits).as_str(), count), ("Ann and Bo and ANN.", 1));
    }
}
