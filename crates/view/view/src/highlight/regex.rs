//! A small backtracking matcher for the JavaScript regular expressions CodeMirror's simple modes
//! are written in (what `stream.match(re)` does: a match where the stream stands): literals,
//! classes (`[a-z]`, `\d`, `\w`, `\s`, negated), `.`, groups (capturing, `(?:…)`,
//! lookahead), alternation, `* + ? {n,m}` (greedy and lazy), `^`, `$`, `\b`, `\B`, and the
//! `i` flag. It reads bytes: a character past ASCII is never a word character, as in
//! JavaScript.

use super::scan;

#[derive(Clone, Debug)]
enum Node {
    Byte(u8),
    Any,
    /// A class: its ranges, and whether it is negated.
    Class(Vec<(u8, u8)>, bool),
    Group(Box<Alternation>, Option<usize>),
    Look(Box<Alternation>, bool),
    Start,
    End,
    Boundary(bool),
    /// An item, its least and most counts, whether greedy, and (for a one-byte item) the bytes
    /// what follows it in its sequence can start with, when known.
    Repeat(Box<Node>, usize, usize, bool, Option<Box<[bool; 256]>>),
}

type Sequence = Vec<Node>;
/// Each group's span (none: it did not take part), the whole match's first.
pub type Captures = Vec<Option<(usize, usize)>>;
type Alternation = Vec<Sequence>;

/// A compiled pattern.
#[derive(Clone, Debug)]
pub struct Regex {
    root: Alternation,
    groups: usize,
    ignore_case: bool,
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    groups: usize,
}

const DIGIT: &[(u8, u8)] = &[(b'0', b'9')];
const WORD: &[(u8, u8)] = &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')];
const SPACE: &[(u8, u8)] = &[(b'\t', b'\r'), (b' ', b' ')];

impl Parser<'_> {
    fn peek(&self) -> u8 {
        self.bytes.get(self.at).copied().unwrap_or(0)
    }
    fn next(&mut self) -> u8 {
        let here = self.peek();
        self.at += 1;
        here
    }
    fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }

    fn alternation(&mut self) -> Alternation {
        let mut out = vec![self.sequence()];
        while !self.done() && self.peek() == b'|' {
            self.at += 1;
            out.push(self.sequence());
        }
        out
    }

    fn sequence(&mut self) -> Sequence {
        let mut out = Vec::new();
        while !self.done() && self.peek() != b'|' && self.peek() != b')' {
            let atom = self.atom();
            let quantified = self.quantifier(atom);
            out.push(quantified);
        }
        // A one-byte repetition learns what may follow it, so a match need not try every end.
        for index in 0..out.len() {
            let follow = out.get(index + 1..).and_then(first_bytes);
            if let Some(Node::Repeat(inner, _, _, _, slot)) = out.get_mut(index)
                && matches!(**inner, Node::Byte(_) | Node::Any | Node::Class(..))
            {
                *slot = follow.map(Box::new);
            }
        }
        out
    }

    fn quantifier(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            b'*' => (0, usize::MAX),
            b'+' => (1, usize::MAX),
            b'?' => (0, 1),
            b'{' => {
                let save = self.at;
                self.at += 1;
                let number = |parser: &mut Self| {
                    let start = parser.at;
                    while parser.peek().is_ascii_digit() {
                        parser.at += 1;
                    }
                    std::str::from_utf8(parser.bytes.get(start..parser.at).unwrap_or_default()).ok().and_then(|text| text.parse::<usize>().ok())
                };
                let min = number(self);
                let max = if self.peek() == b',' {
                    self.at += 1;
                    number(self).unwrap_or(usize::MAX)
                } else {
                    min.unwrap_or(0)
                };
                match (min, self.peek()) {
                    (Some(min), b'}') => (min, max),
                    _ => {
                        self.at = save;
                        return atom;
                    }
                }
            }
            _ => return atom,
        };
        self.at += 1;
        let greedy = if self.peek() == b'?' {
            self.at += 1;
            false
        } else {
            true
        };
        Node::Repeat(Box::new(atom), min, max, greedy, None)
    }

    /// The two hex digits after `\x`.
    fn hex(&mut self) -> u8 {
        let digits = self
            .bytes
            .get(self.at..self.at + 2)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        match digits {
            Some(value) => {
                self.at += 2;
                value
            }
            None => b'x',
        }
    }

    fn escape_class(byte: u8) -> Option<(Vec<(u8, u8)>, bool)> {
        Some(match byte {
            b'd' => (DIGIT.to_vec(), false),
            b'D' => (DIGIT.to_vec(), true),
            b'w' => (WORD.to_vec(), false),
            b'W' => (WORD.to_vec(), true),
            b's' => (SPACE.to_vec(), false),
            b'S' => (SPACE.to_vec(), true),
            _ => return None,
        })
    }

    fn escaped_byte(byte: u8) -> u8 {
        match byte {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'f' => 0x0c,
            b'v' => 0x0b,
            b'0' => 0,
            other => other,
        }
    }

    fn atom(&mut self) -> Node {
        match self.next() {
            b'.' => Node::Any,
            b'^' => Node::Start,
            b'$' => Node::End,
            b'(' => {
                let (look, negative, capture) = if self.peek() == b'?' {
                    self.at += 1;
                    match self.next() {
                        b'=' => (true, false, None),
                        b'!' => (true, true, None),
                        _ => (false, false, None),
                    }
                } else {
                    self.groups += 1;
                    (false, false, Some(self.groups))
                };
                let inner = self.alternation();
                if self.peek() == b')' {
                    self.at += 1;
                }
                if look { Node::Look(Box::new(inner), negative) } else { Node::Group(Box::new(inner), capture) }
            }
            b'[' => {
                let negated = self.peek() == b'^';
                if negated {
                    self.at += 1;
                }
                let mut ranges = Vec::new();
                while !self.done() && self.peek() != b']' {
                    let mut low = self.next();
                    if low == b'\\' {
                        let escape = self.next();
                        if let Some((class, negative)) = Self::escape_class(escape) {
                            if negative {
                                // A negated class inside a class: everything else (as ranges).
                                let mut start = 0u8;
                                for (from, to) in class {
                                    if from > start {
                                        ranges.push((start, from - 1));
                                    }
                                    start = to.saturating_add(1);
                                }
                                ranges.push((start, 255));
                            } else {
                                ranges.extend(class);
                            }
                            continue;
                        }
                        low = if escape == b'x' { self.hex() } else { Self::escaped_byte(escape) };
                    }
                    if self.peek() == b'-' && self.bytes.get(self.at + 1).is_some_and(|next| *next != b']') {
                        self.at += 1;
                        let mut high = self.next();
                        if high == b'\\' {
                            let escape = self.next();
                            if Self::escape_class(escape).is_some() {
                                // `\w-\.`: the dash is a character.
                                ranges.push((low, low));
                                ranges.push((b'-', b'-'));
                                if let Some((class, _)) = Self::escape_class(escape) {
                                    ranges.extend(class);
                                }
                                continue;
                            }
                            high = if escape == b'x' { self.hex() } else { Self::escaped_byte(escape) };
                        }
                        ranges.push((low, high));
                    } else {
                        ranges.push((low, low));
                    }
                }
                self.at += 1;
                Node::Class(ranges, negated)
            }
            b'\\' => {
                let escape = self.next();
                if let Some((class, negative)) = Self::escape_class(escape) {
                    return Node::Class(class, negative);
                }
                match escape {
                    b'b' => Node::Boundary(true),
                    b'B' => Node::Boundary(false),
                    b'x' => Node::Byte(self.hex()),
                    other => Node::Byte(Self::escaped_byte(other)),
                }
            }
            other => Node::Byte(other),
        }
    }
}

/// The bytes a match of `nodes` can start with (both cases of letters), or none when it may
/// match nothing or look around first.
fn first_bytes(nodes: &[Node]) -> Option<[bool; 256]> {
    let mut set = [false; 256];
    let mut mark = |byte: u8| {
        if let Some(slot) = set.get_mut(usize::from(byte.to_ascii_lowercase())) {
            *slot = true;
        }
        if let Some(slot) = set.get_mut(usize::from(byte.to_ascii_uppercase())) {
            *slot = true;
        }
    };
    match nodes.first()? {
        Node::Byte(byte) => mark(*byte),
        Node::Any => (0..=255u8).filter(|byte| *byte != b'\n').for_each(&mut mark),
        Node::Class(ranges, negated) => {
            (0..=255u8).filter(|byte| ranges.iter().any(|(low, high)| (low..=high).contains(&byte)) != *negated).for_each(&mut mark);
            if *negated {
                // A negated class with a letter of one case left out may take the other.
                (0..=255u8).for_each(&mut mark);
            }
        }
        Node::Group(alternatives, _) => {
            for sequence in alternatives.iter() {
                let first = first_bytes(sequence)?;
                for (byte, found) in first.iter().enumerate() {
                    if *found && let Ok(byte) = u8::try_from(byte) {
                        mark(byte);
                    }
                }
            }
        }
        Node::Repeat(inner, min, ..) if *min > 0 => return first_bytes(std::slice::from_ref(inner)),
        _ => return None,
    }
    Some(set)
}

/// Where a match is at: the text, the captures.
struct Matcher<'a> {
    text: &'a [u8],
    start: usize,
    ignore_case: bool,
    captures: Captures,
    steps: usize,
}

fn word_at(text: &[u8], at: usize) -> bool {
    text.get(at).is_some_and(|byte| scan::js_word(*byte))
}

impl Matcher<'_> {
    fn class_has(&self, ranges: &[(u8, u8)], negated: bool, byte: u8) -> bool {
        let inside = |byte: u8| ranges.iter().any(|(low, high)| (*low..=*high).contains(&byte));
        let found = inside(byte) || (self.ignore_case && (inside(byte.to_ascii_lowercase()) || inside(byte.to_ascii_uppercase())));
        found != negated
    }

    /// Matches `nodes` from `at`, then calls `rest` with where it ends; true when `rest` does.
    fn sequence(&mut self, nodes: &[Node], at: usize, rest: &mut dyn FnMut(&mut Self, usize) -> bool) -> bool {
        self.steps += 1;
        if self.steps > 200_000 {
            return false;
        }
        let Some((first, others)) = nodes.split_first() else { return rest(self, at) };
        match first {
            Node::Byte(byte) => {
                let here = self.text.get(at).copied();
                let same = here.is_some_and(|here| here == *byte || (self.ignore_case && here.eq_ignore_ascii_case(byte)));
                same && self.sequence(others, at + 1, rest)
            }
            Node::Any => self.text.get(at).is_some_and(|byte| *byte != b'\n') && self.sequence(others, at + 1, rest),
            Node::Class(ranges, negated) => {
                let ok = self.text.get(at).is_some_and(|byte| self.class_has(ranges, *negated, *byte));
                ok && self.sequence(others, at + 1, rest)
            }
            Node::Start => at == self.start && self.sequence(others, at, rest),
            Node::End => at == self.text.len() && self.sequence(others, at, rest),
            Node::Boundary(wanted) => {
                let before = at > self.start && word_at(self.text, at - 1);
                let boundary = before != word_at(self.text, at);
                boundary == *wanted && self.sequence(others, at, rest)
            }
            Node::Look(inner, negative) => {
                let saved = self.captures.clone();
                let found = self.alternation(inner, at, &mut |_, _| true);
                if found == *negative {
                    self.captures = saved;
                    return false;
                }
                if *negative {
                    self.captures = saved;
                }
                self.sequence(others, at, rest)
            }
            Node::Group(inner, capture) => {
                let capture = *capture;
                self.alternation(inner, at, &mut |matcher: &mut Self, end| {
                    let saved = capture.and_then(|index| matcher.captures.get(index).copied().flatten());
                    if let Some(slot) = capture.and_then(|index| matcher.captures.get_mut(index)) {
                        *slot = Some((at, end));
                    }
                    if matcher.sequence(others, end, rest) {
                        return true;
                    }
                    if let Some(slot) = capture.and_then(|index| matcher.captures.get_mut(index)) {
                        *slot = saved;
                    }
                    false
                })
            }
            Node::Repeat(inner, min, max, greedy, follow) => match self.single(inner) {
                // A repeated single byte: count how far it goes, then try the rest from each
                // count (no recursion per byte, so long lines cannot exhaust the stack).
                Some(()) => {
                    let mut reach = at;
                    while reach - at < *max && self.text.get(reach).is_some_and(|byte| self.one_byte(inner, *byte)) {
                        reach += 1;
                    }
                    if reach - at < *min {
                        return false;
                    }
                    let lowest = at + min;
                    let may_follow = |end: usize| {
                        follow.as_ref().is_none_or(|set| self.text.get(end).is_some_and(|byte| set.get(usize::from(*byte)).copied().unwrap_or(false)))
                    };
                    if *greedy {
                        (lowest..=reach).rev().any(|end| may_follow(end) && self.sequence(others, end, rest))
                    } else {
                        (lowest..=reach).any(|end| may_follow(end) && self.sequence(others, end, rest))
                    }
                }
                None if self.fixed(inner) => {
                    // A repeated group of fixed-width choices: each round takes its first choice
                    // that fits (all the patterns here have at most one), then the rest is tried
                    // from each round's end, without recursion per round.
                    let mut ends = vec![at];
                    let mut reach = at;
                    while ends.len() - 1 < *max {
                        match self.fixed_match(inner, reach) {
                            Some(end) if end > reach => {
                                reach = end;
                                ends.push(end);
                            }
                            _ => break,
                        }
                    }
                    let usable = ends.get(*min..).unwrap_or_default();
                    if *greedy {
                        usable.iter().rev().any(|end| self.sequence(others, *end, rest))
                    } else {
                        usable.iter().any(|end| self.sequence(others, *end, rest))
                    }
                }
                None => self.repeat(inner, *min, *max, *greedy, 0, at, usize::MAX, others, rest),
            },
        }
    }

    /// Whether a node matches exactly one byte (a byte, `.`, a class).
    fn single(&self, node: &Node) -> Option<()> {
        matches!(node, Node::Byte(_) | Node::Any | Node::Class(..)).then_some(())
    }

    /// Whether a node is a group whose every choice is a fixed run of one-byte nodes.
    fn fixed(&self, node: &Node) -> bool {
        match node {
            Node::Group(alternatives, _) => {
                alternatives.iter().all(|sequence| sequence.iter().all(|item| self.single(item).is_some() || self.fixed(item)))
            }
            _ => false,
        }
    }

    /// Where a fixed group's first fitting choice from `at` ends.
    fn fixed_match(&self, node: &Node, at: usize) -> Option<usize> {
        let Node::Group(alternatives, _) = node else {
            return self.text.get(at).filter(|byte| self.one_byte(node, **byte)).map(|_| at + 1);
        };
        alternatives.iter().find_map(|sequence| {
            sequence.iter().try_fold(at, |position, item| {
                if self.fixed(item) {
                    self.fixed_match(item, position)
                } else {
                    self.text.get(position).filter(|byte| self.one_byte(item, **byte)).map(|_| position + 1)
                }
            })
        })
    }

    /// Whether a one-byte node takes `byte`.
    fn one_byte(&self, node: &Node, byte: u8) -> bool {
        match node {
            Node::Byte(wanted) => byte == *wanted || (self.ignore_case && byte.eq_ignore_ascii_case(wanted)),
            Node::Any => byte != b'\n',
            Node::Class(ranges, negated) => self.class_has(ranges, *negated, byte),
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &mut self,
        inner: &Node,
        min: usize,
        max: usize,
        greedy: bool,
        count: usize,
        at: usize,
        last: usize,
        others: &[Node],
        rest: &mut dyn FnMut(&mut Self, usize) -> bool,
    ) -> bool {
        // An empty iteration past the minimum ends the loop (as JavaScript's does).
        if count > min && at == last {
            return false;
        }
        if count < min {
            return self.one_more(inner, min, max, greedy, count, at, others, rest);
        }
        // Greedy: one more first, then the rest; lazy: the rest first.
        if greedy && self.one_more(inner, min, max, greedy, count, at, others, rest) {
            return true;
        }
        if self.sequence(others, at, rest) {
            return true;
        }
        !greedy && self.one_more(inner, min, max, greedy, count, at, others, rest)
    }

    /// One more of a repetition's item, then the repetition's rest.
    #[allow(clippy::too_many_arguments)]
    fn one_more(
        &mut self,
        inner: &Node,
        min: usize,
        max: usize,
        greedy: bool,
        count: usize,
        at: usize,
        others: &[Node],
        rest: &mut dyn FnMut(&mut Self, usize) -> bool,
    ) -> bool {
        count < max
            && self.sequence(std::slice::from_ref(inner), at, &mut |matcher: &mut Self, end| {
                matcher.repeat(inner, min, max, greedy, count + 1, end, at, others, rest)
            })
    }

    fn alternation(&mut self, alternatives: &Alternation, at: usize, rest: &mut dyn FnMut(&mut Self, usize) -> bool) -> bool {
        alternatives.iter().any(|sequence| self.sequence(sequence, at, rest))
    }
}

impl Regex {
    /// Compiles `pattern`, case-insensitive when `ignore_case`.
    pub fn new(pattern: &str, ignore_case: bool) -> Regex {
        let mut parser = Parser { bytes: pattern.as_bytes(), at: 0, groups: 0 };
        let root = parser.alternation();
        Regex { root, groups: parser.groups, ignore_case }
    }

    /// The match starting at `at` in `text` (where `^` and `\b` see `at` as the text's start, as
    /// the stream's `match` does): its end and its groups.
    pub fn match_at(&self, text: &[u8], at: usize) -> Option<(usize, Captures)> {
        let mut matcher = Matcher { text, start: at, ignore_case: self.ignore_case, captures: vec![None; self.groups + 1], steps: 0 };
        let mut end = None;
        let found = matcher.alternation(&self.root, at, &mut |_, finish| {
            end = Some(finish);
            true
        });
        found.then(|| (end.unwrap_or(at), matcher.captures))
    }
}
