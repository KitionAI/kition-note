//! The faces pages are set in: PDF's standard Helvetica (regular, bold, oblique, bold oblique) and
//! Courier, which every PDF reader has, so the engine ships no font. Their widths come from Adobe's
//! core font metrics (AFM), in thousandths of the font size; a face draws the Windows-1252
//! characters (`WinAnsiEncoding`). Any other character (Chinese, Japanese, Korean, emoji, most
//! symbols) has no glyph in these faces: it is drawn as a box and counted as missing.

/// A face of the page fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    Mono,
    MonoBold,
}

impl Face {
    pub fn of(bold: bool, italic: bool, mono: bool) -> Self {
        match (mono, bold, italic) {
            (true, true, _) => Face::MonoBold,
            (true, false, _) => Face::Mono,
            (false, true, true) => Face::BoldItalic,
            (false, true, false) => Face::Bold,
            (false, false, true) => Face::Italic,
            (false, false, false) => Face::Regular,
        }
    }

    /// The PDF base font.
    pub fn base_font(self) -> &'static str {
        match self {
            Face::Regular => "Helvetica",
            Face::Bold => "Helvetica-Bold",
            Face::Italic => "Helvetica-Oblique",
            Face::BoldItalic => "Helvetica-BoldOblique",
            Face::Mono => "Courier",
            Face::MonoBold => "Courier-Bold",
        }
    }

    pub const ALL: [Face; 6] = [Face::Regular, Face::Bold, Face::Italic, Face::BoldItalic, Face::Mono, Face::MonoBold];

    fn bold(self) -> bool {
        matches!(self, Face::Bold | Face::BoldItalic)
    }

    fn mono(self) -> bool {
        matches!(self, Face::Mono | Face::MonoBold)
    }
}

/// Helvetica, characters 32 to 126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0 to ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ to O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P to _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` to o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p to ~
];

/// Helvetica-Bold, characters 32 to 126.
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, // 0 to ?
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, // @ to O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, // P to _
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, // ` to o
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, // p to ~
];

/// The Windows-1252 byte of a character, when the page fonts draw it.
pub fn win_ansi(c: char) -> Option<u8> {
    let code = u32::from(c);
    match code {
        0x20..=0x7e | 0xa0..=0xff => u8::try_from(code).ok(),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8a,
            '‹' => 0x8b,
            'Œ' => 0x8c,
            'Ž' => 0x8e,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9a,
            '›' => 0x9b,
            'œ' => 0x9c,
            'ž' => 0x9e,
            'Ÿ' => 0x9f,
            _ => return None,
        }),
    }
}

/// The width of a Windows-1252 byte above 126, in thousandths, from its letter where it has one.
fn upper_width(byte: u8, bold: bool) -> u16 {
    let base = |letter: u8| -> u16 { ascii_width(letter, bold) };
    match byte {
        0x80 => 556,
        0x82 | 0x91 | 0x92 => {
            if bold {
                278
            } else {
                222
            }
        }
        0x83 | 0x86 | 0x87 | 0x96 => 556,
        0x84 | 0x93 | 0x94 => {
            if bold {
                500
            } else {
                333
            }
        }
        0x85 | 0x89 | 0x8c | 0x97 | 0x99 | 0xc6 => 1000,
        0x88 | 0x98 | 0x8b | 0x9b => 333,
        0x8a => base(b'S'),
        0x8e => base(b'Z'),
        0x95 => 350,
        0x9a => base(b's'),
        0x9c => 944,
        0x9e => base(b'z'),
        0x9f => base(b'Y'),
        0xa0 => 278,
        0xa1 | 0xa8 | 0xaf | 0xb2 | 0xb3 | 0xb4 | 0xb8 | 0xb9 | 0xad => 333,
        0xa2..=0xa5 | 0xa7 | 0xab | 0xbb | 0xb5 => 556,
        0xa6 => 260,
        0xa9 | 0xae => 737,
        0xaa => 370,
        0xac | 0xb1 | 0xd7 | 0xf7 => 584,
        0xb0 => 400,
        0xb6 => 537,
        0xb7 => 278,
        0xba => 365,
        0xbc..=0xbe => 834,
        0xbf => 611,
        0xc0..=0xc5 => base(b'A'),
        0xc7 => base(b'C'),
        0xc8..=0xcb => base(b'E'),
        0xcc..=0xcf => base(b'I'),
        0xd0 => 722,
        0xd1 => base(b'N'),
        0xd2..=0xd6 | 0xd8 => base(b'O'),
        0xd9..=0xdc => base(b'U'),
        0xdd => base(b'Y'),
        0xde => 667,
        0xdf => 611,
        0xe0..=0xe5 => base(b'a'),
        0xe6 => 889,
        0xe7 => base(b'c'),
        0xe8..=0xeb => base(b'e'),
        0xec..=0xef => base(b'i'),
        0xf0 | 0xf2..=0xf6 => base(b'o'),
        0xf1 => base(b'n'),
        0xf8 => 611,
        0xf9..=0xfc => base(b'u'),
        0xfd | 0xff => base(b'y'),
        0xfe => base(b'p'),
        _ => 556,
    }
}

fn ascii_width(byte: u8, bold: bool) -> u16 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    byte.checked_sub(32).and_then(|index| table.get(usize::from(index))).copied().unwrap_or(556)
}

/// The width of one character drawn in `face` at size 1; `None` when the face has no glyph for it.
pub fn char_width(face: Face, c: char) -> Option<f64> {
    let byte = win_ansi(c)?;
    if face.mono() {
        return Some(0.6);
    }
    let units = if byte <= 0x7e { ascii_width(byte, face.bold()) } else { upper_width(byte, face.bold()) };
    Some(f64::from(units) / 1000.0)
}

/// The width a missing character's box takes, at size 1: wide characters (CJK, emoji) a full em.
pub fn missing_width(c: char) -> f64 {
    if is_wide(c) { 1.0 } else { 0.6 }
}

/// East Asian wide and fullwidth characters, and emoji, roughly by block.
pub fn is_wide(c: char) -> bool {
    matches!(u32::from(c),
        0x1100..=0x115f | 0x2e80..=0x303e | 0x3041..=0x33ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xa000..=0xa4cf
        | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe4f | 0xff00..=0xff60 | 0xffe0..=0xffe6
        | 0x1f300..=0x1f64f | 0x1f900..=0x1f9ff | 0x20000..=0x2fffd | 0x30000..=0x3fffd)
}

/// The width of `text` in `face` at `size`, missing characters as their boxes.
pub fn text_width(face: Face, size: f64, text: &str) -> f64 {
    text.chars().map(|c| char_width(face, c).unwrap_or_else(|| missing_width(c))).sum::<f64>() * size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_core_metrics() {
        assert_eq!(char_width(Face::Regular, 'a'), Some(0.556));
        assert_eq!(char_width(Face::Bold, 'a'), Some(0.556));
        assert_eq!(char_width(Face::Bold, 'b'), Some(0.611));
        assert_eq!(char_width(Face::Regular, 'é'), Some(0.556));
        assert_eq!(char_width(Face::Mono, 'W'), Some(0.6));
        assert_eq!(char_width(Face::Regular, '—'), Some(1.0));
        assert_eq!(char_width(Face::Regular, '中'), None);
        assert!((text_width(Face::Regular, 10.0, "中a") - 15.56).abs() < 1e-9);
    }
}
