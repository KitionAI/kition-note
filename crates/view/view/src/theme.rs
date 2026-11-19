//! Sizes, spacing, and colors of the note's column, after Obsidian's light theme in live preview
//! (a white surface, a readable column of at most 700 px, its type scale, faint Markdown syntax),
//! with Kition's purple `#5645d4` as the accent; and its dark palette, after Obsidian's default
//! dark theme, with Kition's dark purple `#8b7cf0` (`kit.json`'s dark brand). The two palettes
//! share every length: switching them moves nothing.

use crate::style::{Block, LineStyle};

/// Inline code's size against the text around it.
pub const INLINE_CODE: f32 = 0.875;
/// A tag's size against the text around it.
pub const TAG: f32 = 0.875;

/// Obsidian's callout types, by id from 1: their names, color, and icon (an index into the
/// view's icons). A name no type has is a note.
pub const CALLOUT_TYPES: [(&[&str], [u8; 3], u8); 13] = [
    (&["note"], [8, 109, 221], 1),
    (&["abstract", "summary", "tldr"], [0, 191, 188], 5),
    (&["info"], [8, 109, 221], 6),
    (&["todo"], [8, 109, 221], 7),
    (&["tip", "hint", "important"], [0, 191, 188], 2),
    (&["success", "check", "done"], [8, 185, 78], 8),
    (&["question", "help", "faq"], [236, 117, 0], 9),
    (&["warning", "caution", "attention"], [236, 117, 0], 3),
    (&["failure", "fail", "missing"], [233, 49, 71], 10),
    (&["danger", "error"], [233, 49, 71], 4),
    (&["bug"], [233, 49, 71], 11),
    (&["example"], [120, 82, 238], 12),
    (&["quote", "cite"], [158, 158, 158], 13),
];

/// The callout type id (from 1) of a callout written `[!kind]`.
pub fn callout_type(kind: &str) -> u8 {
    let kind = kind.to_ascii_lowercase();
    CALLOUT_TYPES.iter().position(|(names, ..)| names.contains(&kind.as_str())).map_or(1, |at| u8::try_from(at + 1).unwrap_or(1))
}

/// The callout types' colors in the dark palette, in `CALLOUT_TYPES`' order: Obsidian's dark
/// theme's blue, cyan, green, orange, red, purple, and grey.
const DARK_CALLOUT_COLORS: [[u8; 3]; 13] = [
    [2, 122, 255],
    [83, 223, 221],
    [2, 122, 255],
    [2, 122, 255],
    [83, 223, 221],
    [68, 207, 110],
    [233, 151, 63],
    [233, 151, 63],
    [251, 70, 76],
    [251, 70, 76],
    [251, 70, 76],
    [168, 130, 255],
    [158, 158, 158],
];

/// The highlight colours' backgrounds in the dark palette, in `style::HIGHLIGHT_COLORS`' order:
/// Obsidian's dark theme's red, orange, yellow, green, blue, and purple at 30% over the paper.
const DARK_HIGHLIGHTS: [[u8; 4]; 6] =
    [[96, 42, 44, 255], [91, 66, 40, 255], [88, 88, 55, 255], [41, 83, 54, 255], [22, 58, 98, 255], [71, 60, 98, 255]];

/// A callout type's color in `theme`'s palette.
pub fn callout_color(theme: &Theme, kind: u8) -> [u8; 4] {
    let at = usize::from(kind.saturating_sub(1));
    let light = CALLOUT_TYPES.get(at).map_or([8, 109, 221], |(_, color, _)| *color);
    let [r, g, b] = match theme.scheme {
        Scheme::Light => light,
        Scheme::Dark => DARK_CALLOUT_COLORS.get(at).copied().unwrap_or([2, 122, 255]),
    };
    [r, g, b, 255]
}

/// A callout type's tint behind its box: its color at a tenth over the paper.
pub fn callout_tint(theme: &Theme, kind: u8) -> [u8; 4] {
    let [r, g, b, _] = callout_color(theme, kind);
    let mix = |paper: u8, color: u8| (f32::from(paper) * 0.9 + f32::from(color) * 0.1).round() as u8;
    [mix(theme.paper[0], r), mix(theme.paper[1], g), mix(theme.paper[2], b), 255]
}

/// A nested callout's tint: its color at a tenth over `under` (the outer callout's tint), blended
/// as Obsidian blends a callout's background: darkened on the light paper (`mix-blend-mode:
/// darken`), lightened on the dark one (`lighten`), so the nested box shows on either.
pub fn callout_tint_over(theme: &Theme, under: [u8; 4], kind: u8) -> [u8; 4] {
    let [ur, ug, ub, _] = under;
    let [r, g, b, _] = callout_color(theme, kind);
    let dark = theme.scheme == Scheme::Dark;
    let mix = |back: u8, front: u8| {
        let (back, front) = (f32::from(back), f32::from(front));
        let blended = if dark { back.max(front) } else { back.min(front) };
        (back + 0.1 * (blended - back)).round() as u8
    };
    [mix(ur, r), mix(ug, g), mix(ub, b), 255]
}

/// A callout type's icon (an index into the view's icons, from 1).
pub fn callout_icon(kind: u8) -> u8 {
    CALLOUT_TYPES.get(usize::from(kind.saturating_sub(1))).map_or(1, |(.., icon)| *icon)
}

/// Which palette a theme paints with: the host's color scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scheme {
    #[default]
    Light,
    Dark,
}

impl Scheme {
    /// The scheme a host names (`light` or `dark`, as the Kit UI protocol's `colorScheme`).
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// The column and its look. Lengths are document pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// The palette the colors below are from.
    pub scheme: Scheme,
    /// The document's width.
    pub width: f32,
    /// The text column's left and right margins.
    pub margin: f32,
    pub top: f32,
    pub bottom: f32,
    /// The least height of the document, so a short note still reads as a page.
    pub min_height: f32,
    /// The tallest a picture draws.
    pub image_height: f32,
    pub body_size: f32,
    pub code_size: f32,
    /// Line height over font size for prose.
    pub leading: f32,
    pub paper: [u8; 4],
    pub text: [u8; 4],
    pub heading: [u8; 4],
    /// Markdown syntax, list markers, and comments in code.
    pub muted: [u8; 4],
    /// Front matter that is not YAML (Obsidian's `is-invalid` red).
    pub invalid: [u8; 4],
    /// Secondary text: property keys, done tasks.
    pub secondary: [u8; 4],
    pub link: [u8; 4],
    pub math: [u8; 4],
    pub code_background: [u8; 4],
    pub code_block: [u8; 4],
    pub quote_bar: [u8; 4],
    pub rule: [u8; 4],
    pub highlight: [u8; 4],
    pub tag_background: [u8; 4],
    pub table_border: [u8; 4],
    pub table_header: [u8; 4],
    /// The accent: quote bars, checked boxes.
    pub accent: [u8; 4],
    /// The guides under a nested list's levels.
    pub indent_guide: [u8; 4],
    /// Code highlighting, by token (Obsidian's light theme): keywords, strings, numbers,
    /// comments and meta, variables, operators and tags, properties (builtins, types,
    /// attributes), brackets and punctuation, links.
    pub code_keyword: [u8; 4],
    pub code_literal: [u8; 4],
    pub code_number: [u8; 4],
    pub code_comment: [u8; 4],
    pub code_variable: [u8; 4],
    pub code_operator: [u8; 4],
    pub code_property: [u8; 4],
    pub code_punctuation: [u8; 4],
    pub code_link: [u8; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            scheme: Scheme::Light,
            width: 816.0,
            margin: (816.0 - Self::COLUMN) / 2.0,
            top: 32.0,
            bottom: 120.0,
            min_height: 1056.0,
            image_height: 480.0,
            body_size: 16.0,
            code_size: 14.0,
            leading: 1.5,
            paper: [255, 255, 255, 255],
            text: [34, 34, 34, 255],
            heading: [34, 34, 34, 255],
            muted: [171, 171, 171, 255],
            invalid: [233, 49, 71, 255],
            secondary: [92, 92, 92, 255],
            link: [86, 69, 212, 255],
            math: [110, 64, 170, 255],
            code_background: [250, 250, 250, 255],
            code_block: [250, 250, 250, 255],
            quote_bar: [86, 69, 212, 255],
            rule: [228, 228, 228, 255],
            highlight: [246, 230, 179, 255],
            tag_background: [238, 236, 251, 255],
            table_border: [228, 228, 228, 255],
            table_header: [255, 255, 255, 255],
            accent: [86, 69, 212, 255],
            indent_guide: [224, 224, 224, 255],
            code_keyword: [213, 57, 132, 255],
            code_literal: [8, 185, 78, 255],
            code_number: [120, 82, 238, 255],
            code_comment: [171, 171, 171, 255],
            code_variable: [0, 191, 188, 255],
            code_operator: [233, 49, 71, 255],
            code_property: [224, 172, 0, 255],
            code_punctuation: [92, 92, 92, 255],
            code_link: [138, 92, 245, 255],
        }
    }
}

impl Theme {
    /// The readable column's widest (Obsidian's readable line length).
    pub const COLUMN: f32 = 700.0;
    /// The least space beside the column.
    pub const SIDE: f32 = 32.0;

    /// The dark palette, after Obsidian's default dark theme (a `#1e1e1e` surface, `#dadada`
    /// text, `#666666` faint syntax, `#363636` borders, `#242424` code), with Kition's dark purple
    /// `#8b7cf0` as the accent. Every length is the light theme's.
    pub fn dark() -> Self {
        let accent = [139, 124, 240, 255];
        Self {
            scheme: Scheme::Dark,
            paper: [30, 30, 30, 255],
            text: [218, 218, 218, 255],
            heading: [218, 218, 218, 255],
            muted: [102, 102, 102, 255],
            invalid: [251, 70, 76, 255],
            secondary: [179, 179, 179, 255],
            link: accent,
            math: [190, 160, 240, 255],
            code_background: [36, 36, 36, 255],
            code_block: [36, 36, 36, 255],
            quote_bar: accent,
            rule: [54, 54, 54, 255],
            // Obsidian's `rgba(255, 208, 0, 0.4)` over the paper.
            highlight: [120, 101, 18, 255],
            // The accent at a tenth over the paper.
            tag_background: [41, 39, 51, 255],
            table_border: [54, 54, 54, 255],
            table_header: [30, 30, 30, 255],
            accent,
            // Obsidian's `rgba(255, 255, 255, 0.12)` over the paper.
            indent_guide: [57, 57, 57, 255],
            code_keyword: [250, 153, 205, 255],
            code_literal: [68, 207, 110, 255],
            code_number: [168, 130, 255, 255],
            code_comment: [102, 102, 102, 255],
            code_variable: [83, 223, 221, 255],
            code_operator: [251, 82, 88, 255],
            code_property: [224, 222, 113, 255],
            code_punctuation: [179, 179, 179, 255],
            code_link: [166, 138, 249, 255],
            ..Self::default()
        }
    }

    /// The palette of `scheme`, with this theme's lengths (its width, margins, sizes): only colors
    /// change.
    pub fn with_scheme(&self, scheme: Scheme) -> Self {
        let palette = match scheme {
            Scheme::Light => Self::default(),
            Scheme::Dark => Self::dark(),
        };
        Self {
            width: self.width,
            margin: self.margin,
            top: self.top,
            bottom: self.bottom,
            min_height: self.min_height,
            image_height: self.image_height,
            body_size: self.body_size,
            code_size: self.code_size,
            leading: self.leading,
            ..palette
        }
    }

    /// The background a coloured highlight draws (one of `style::HIGHLIGHT_COLORS`): itself on
    /// the light paper, the same hue over the dark one.
    pub fn highlight_background(&self, color: [u8; 4]) -> [u8; 4] {
        let [r, g, b, _] = color;
        let light = [r, g, b, 255];
        match self.scheme {
            Scheme::Light => light,
            Scheme::Dark => crate::style::HIGHLIGHT_COLORS
                .iter()
                .position(|(_, known)| *known == color)
                .and_then(|at| DARK_HIGHLIGHTS.get(at))
                .copied()
                .unwrap_or(light),
        }
    }

    /// The text column's width.
    pub fn content_width(&self) -> f32 {
        self.width - 2.0 * self.margin
    }

    /// The text size of a block's lines.
    pub fn size_of(&self, block: Block) -> f32 {
        match block {
            Block::Heading(1) => 25.9,
            Block::Heading(2) => 23.4,
            Block::Heading(3) => 21.1,
            Block::Heading(4) => 19.0,
            Block::Heading(5) => 17.2,
            Block::Heading(_) => 16.0,
            Block::Code { .. } | Block::Frontmatter | Block::Property => self.code_size,
            _ => self.body_size,
        }
    }

    /// A row's height in a block.
    pub fn line_height_of(&self, block: Block) -> f32 {
        match block {
            Block::Heading(level) => {
                let leading = match level {
                    1 | 2 => 1.2,
                    3 => 1.3,
                    4 => 1.4,
                    _ => self.leading,
                };
                (self.size_of(block) * leading).round()
            }
            Block::Code { .. } => (self.code_size * self.leading).round(),
            _ => (self.body_size * self.leading).round(),
        }
    }

    /// Space above a block's line: a heading's (live preview's paragraph spacing).
    pub fn space_above(&self, block: Block) -> f32 {
        match block {
            Block::Heading(_) => self.body_size,
            _ => 0.0,
        }
    }

    /// Where a line's text starts in the column: code blocks are inset inside their box.
    pub fn indent_of(&self, line: LineStyle) -> f32 {
        // A quote in a list item is set in by the item's continuation offset too (its text 38.3 px
        // in at 16 px text in Obsidian: 1 em, then the item's indent and `> ` as wide as they
        // are; its quote's 14 px are part of it).
        let in_item =
            if line.quote > 0 && line.list > 0 && line.callout == 0 && line.block == Block::Paragraph { self.body_size * 1.52 } else { 0.0 };
        match line.block {
            Block::Code { .. } => 16.0 + Self::nest(line),
            _ => Self::nest(line) + in_item,
        }
    }

    /// A callout's text inset (24 px, Obsidian's callout padding, twice in a nested callout) or
    /// a quote's (14 px a level).
    fn nest(line: LineStyle) -> f32 {
        if line.callout > 0 {
            24.0 * Self::callout_depth(line) + 26.0 * f32::from(Self::quotes_in_callout(line))
        } else {
            14.0 * f32::from(line.quote)
        }
    }

    /// The plain quotes a callout's line is in inside the callout (`> > text`): Obsidian renders
    /// a callout's content, so each is a rendered quote (a 2 px bar, then 24 px of padding).
    pub fn quotes_in_callout(line: LineStyle) -> u8 {
        if line.callout > 0 && line.outer == 0 { line.quote.saturating_sub(1) } else { 0 }
    }

    /// How many callouts a line is in: 0, 1, or 2 (a nested callout's).
    fn callout_depth(line: LineStyle) -> f32 {
        if line.callout == 0 {
            0.0
        } else if line.outer > 0 {
            2.0
        } else {
            1.0
        }
    }

    /// The room a line leaves on the right: a callout's 12 px padding (each callout's, nested).
    pub fn inset_right(&self, line: LineStyle) -> f32 {
        12.0 * Self::callout_depth(line)
    }

    /// A tab's width at `size`: Obsidian's list indent (2.25 em), so a nested item sits under its
    /// parent's text.
    pub fn tab_width(&self, size: f32) -> f32 {
        size * 2.25
    }
}
