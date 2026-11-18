//! Color models for math mode. Channel arithmetic is [`Dim`](crate::Dim) only.
//!
//! SVG `fill` / `stroke`, PNG (`tiny-skia`), and egui (`Color32`) all convert
//! through [`Color::to_rgba8`]. This module resolves a color to 8-bit sRGB so
//! every backend shares one contract.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use crate::dim::Dim;
use crate::error::Error;

// Changed by the Kition authors: the 68 color names of the dvips driver
// (`\color{TealBlue}`), each with its CMYK definition in percent, are a table of
// our own in place of upstream's data/dvipsnames.tsv, which this copy does not
// contain; see NOTICE.
const DVIPS_NAMES: [(&str, [i64; 4]); 68] = [
    ("GreenYellow", [15, 0, 69, 0]),
    ("Yellow", [0, 0, 100, 0]),
    ("Goldenrod", [0, 10, 84, 0]),
    ("Dandelion", [0, 29, 84, 0]),
    ("Apricot", [0, 32, 52, 0]),
    ("Peach", [0, 50, 70, 0]),
    ("Melon", [0, 46, 50, 0]),
    ("YellowOrange", [0, 42, 100, 0]),
    ("Orange", [0, 61, 87, 0]),
    ("BurntOrange", [0, 51, 100, 0]),
    ("Bittersweet", [0, 75, 100, 24]),
    ("RedOrange", [0, 77, 87, 0]),
    ("Mahogany", [0, 85, 87, 35]),
    ("Maroon", [0, 87, 68, 32]),
    ("BrickRed", [0, 89, 94, 28]),
    ("Red", [0, 100, 100, 0]),
    ("OrangeRed", [0, 100, 50, 0]),
    ("RubineRed", [0, 100, 13, 0]),
    ("WildStrawberry", [0, 96, 39, 0]),
    ("Salmon", [0, 53, 38, 0]),
    ("CarnationPink", [0, 63, 0, 0]),
    ("Magenta", [0, 100, 0, 0]),
    ("VioletRed", [0, 81, 0, 0]),
    ("Rhodamine", [0, 82, 0, 0]),
    ("Mulberry", [34, 90, 0, 2]),
    ("RedViolet", [7, 90, 0, 34]),
    ("Fuchsia", [47, 91, 0, 8]),
    ("Lavender", [0, 48, 0, 0]),
    ("Thistle", [12, 59, 0, 0]),
    ("Orchid", [32, 64, 0, 0]),
    ("DarkOrchid", [40, 80, 20, 0]),
    ("Purple", [45, 86, 0, 0]),
    ("Plum", [50, 100, 0, 0]),
    ("Violet", [79, 88, 0, 0]),
    ("RoyalPurple", [75, 90, 0, 0]),
    ("BlueViolet", [86, 91, 0, 4]),
    ("Periwinkle", [57, 55, 0, 0]),
    ("CadetBlue", [62, 57, 23, 0]),
    ("CornflowerBlue", [65, 13, 0, 0]),
    ("MidnightBlue", [98, 13, 0, 43]),
    ("NavyBlue", [94, 54, 0, 0]),
    ("RoyalBlue", [100, 50, 0, 0]),
    ("Blue", [100, 100, 0, 0]),
    ("Cerulean", [94, 11, 0, 0]),
    ("Cyan", [100, 0, 0, 0]),
    ("ProcessBlue", [96, 0, 0, 0]),
    ("SkyBlue", [62, 0, 12, 0]),
    ("Turquoise", [85, 0, 20, 0]),
    ("TealBlue", [86, 0, 34, 2]),
    ("Aquamarine", [82, 0, 30, 0]),
    ("BlueGreen", [85, 0, 33, 0]),
    ("Emerald", [100, 0, 50, 0]),
    ("JungleGreen", [99, 0, 52, 0]),
    ("SeaGreen", [69, 0, 50, 0]),
    ("Green", [100, 0, 100, 0]),
    ("ForestGreen", [91, 0, 88, 12]),
    ("PineGreen", [92, 0, 59, 25]),
    ("LimeGreen", [50, 0, 100, 0]),
    ("YellowGreen", [44, 0, 74, 0]),
    ("SpringGreen", [26, 0, 76, 0]),
    ("OliveGreen", [64, 0, 95, 40]),
    ("RawSienna", [0, 72, 100, 45]),
    ("Sepia", [0, 83, 100, 70]),
    ("Brown", [0, 81, 100, 60]),
    ("Tan", [14, 42, 56, 0]),
    ("Gray", [0, 0, 0, 50]),
    ("Black", [0, 0, 0, 100]),
    ("White", [0, 0, 0, 0]),
];

/// 8-bit sRGB color. Values are integers; conversion from unit intervals uses `Dim`.
///
/// SVG `fill` / `stroke`, PNG (`tiny-skia`), and egui (`Color32`) all convert
/// through [`Color::to_rgba8`].
///
/// # Examples
///
/// ```
/// use latex_rust::Color;
///
/// let c = Color::rgb(255, 0, 0);
/// assert_eq!(c.css_hex(), "#ff0000");
/// assert_eq!(c.to_rgba8(), [255, 0, 0, 255]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    /// Red 0–255.
    pub r: u8,
    /// Green 0–255.
    pub g: u8,
    /// Blue 0–255.
    pub b: u8,
}

impl Color {
    /// Opaque sRGB triple.
    ///
    /// # Examples
    ///
    /// ```
    /// use latex_rust::Color;
    /// assert_eq!(Color::rgb(0, 0, 0).css_hex(), "#000000");
    /// ```
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// CSS hex `#rrggbb`.
    #[must_use]
    pub fn css_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Opaque sRGB bytes for PNG (`tiny-skia`) and egui (`Color32`) backends.
    #[must_use]
    pub const fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, 255]
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.css_hex())
    }
}

const BASE: &[(&str, Color)] = &[
    ("black", Color::rgb(0, 0, 0)),
    ("white", Color::rgb(255, 255, 255)),
    ("red", Color::rgb(255, 0, 0)),
    ("green", Color::rgb(0, 255, 0)),
    ("blue", Color::rgb(0, 0, 255)),
    ("cyan", Color::rgb(0, 255, 255)),
    ("magenta", Color::rgb(255, 0, 255)),
    ("yellow", Color::rgb(255, 255, 0)),
];

/// Named-color registry, including `\definecolor` overrides.
///
/// # Examples
///
/// ```
/// use latex_rust::ColorTable;
///
/// let mut t = ColorTable::new();
/// assert!(!t.is_empty());
/// t.define("ok", "named", "red").unwrap();
/// assert_eq!(t.get("ok").unwrap().css_hex(), "#ff0000");
/// ```
#[derive(Clone, Debug)]
pub struct ColorTable {
    named: BTreeMap<String, Color>,
}

impl Default for ColorTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorTable {
    /// Standard LaTeX names plus the 68 dvipsnames (CMYK → sRGB via `Dim`).
    #[must_use]
    pub fn new() -> Self {
        builtin().clone()
    }

    /// Number of registered names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.named.len()
    }

    /// True when no names are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    /// Look up a named color. Unknown names are [`Error::Unsupported`].
    pub fn get(&self, name: &str) -> Result<Color, Error> {
        self.named
            .get(name)
            .copied()
            .ok_or_else(|| Error::Unsupported {
                what: format!("named color {name}"),
            })
    }

    /// `\definecolor{name}{model}{spec}`.
    pub fn define(&mut self, name: &str, model: &str, spec: &str) -> Result<Color, Error> {
        let c = parse_color_spec(model, spec, Some(self))?;
        self.named.insert(name.to_string(), c);
        Ok(c)
    }
}

fn dvipsnames() -> &'static [(String, Color)] {
    static T: OnceLock<Vec<(String, Color)>> = OnceLock::new();
    T.get_or_init(load_dvips).as_slice()
}

fn load_dvips() -> Vec<(String, Color)> {
    DVIPS_NAMES
        .iter()
        .filter_map(|(name, [c, m, y, k])| {
            let percent = |v: &i64| Dim::ratio(*v, 100);
            let color = cmyk_to_rgb(&percent(c), &percent(m), &percent(y), &percent(k)).ok()?;
            Some(((*name).to_string(), color))
        })
        .collect()
}

/// Parse `{model}{spec}` as in `\definecolor` / `\color[model]{spec}`.
///
/// `named` uses `table` when provided, otherwise a fresh standard table.
///
/// # Arguments
///
/// * `model` — `named`, `rgb`, `RGB`, `HTML`, `cmyk`, or `gray`.
/// * `spec` — model-specific components (comma-separated, or a name / hex).
/// * `table` — optional named-color registry for the `named` model.
///
/// # Returns
///
/// An opaque sRGB [`Color`].
///
/// # Errors
///
/// * [`Error::Unsupported`] — unknown model or unknown named color.
/// * [`Error::Malformed`] — wrong component count or out-of-range value.
///
/// # Examples
///
/// ```
/// use latex_rust::parse_color_spec;
///
/// let c = parse_color_spec("RGB", "255,0,0", None).unwrap();
/// assert_eq!(c.css_hex(), "#ff0000");
/// ```
pub fn parse_color_spec(
    model: &str,
    spec: &str,
    table: Option<&ColorTable>,
) -> Result<Color, Error> {
    let model = model.trim();
    let spec = spec.trim();
    match model {
        "named" => match table {
            Some(t) => t.get(spec),
            None => builtin().get(spec),
        },
        "rgb" => {
            let v = unit_components(spec, 3)?;
            Ok(Color::rgb(
                unit_to_u8(&v[0])?,
                unit_to_u8(&v[1])?,
                unit_to_u8(&v[2])?,
            ))
        }
        "RGB" => {
            let v = unit_components(spec, 3)?;
            Ok(Color::rgb(
                byte_channel(&v[0])?,
                byte_channel(&v[1])?,
                byte_channel(&v[2])?,
            ))
        }
        "HTML" => parse_html(spec),
        "cmyk" => {
            let v = unit_components(spec, 4)?;
            cmyk_to_rgb(&v[0], &v[1], &v[2], &v[3])
        }
        "gray" => {
            let v = unit_components(spec, 1)?;
            let g = unit_to_u8(&v[0])?;
            Ok(Color::rgb(g, g, g))
        }
        other => Err(Error::Unsupported {
            what: format!("color model {other}"),
        }),
    }
}

fn builtin() -> &'static ColorTable {
    static T: OnceLock<ColorTable> = OnceLock::new();
    T.get_or_init(|| {
        let mut named = BTreeMap::new();
        for (n, c) in BASE {
            named.insert((*n).to_string(), *c);
        }
        for (n, c) in dvipsnames() {
            named.insert(n.clone(), *c);
        }
        ColorTable { named }
    })
}

/// Named color from the standard table (`red`, `RoyalBlue`, …).
///
/// # Arguments
///
/// * `name` — LaTeX or dvipsnames identifier (case-sensitive as stored).
///
/// # Returns
///
/// The registered sRGB color.
///
/// # Errors
///
/// [`Error::Unsupported`] when the name is not in the table.
///
/// # Examples
///
/// ```
/// use latex_rust::named_color;
///
/// assert_eq!(named_color("red").unwrap().css_hex(), "#ff0000");
/// assert!(named_color("not-a-color").is_err());
/// ```
pub fn named_color(name: &str) -> Result<Color, Error> {
    builtin().get(name)
}

fn parse_html(spec: &str) -> Result<Color, Error> {
    let s = spec.trim().trim_start_matches('#');
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(malformed(format!("HTML color {spec}")));
    }
    let n = u32::from_str_radix(s, 16).map_err(|_| malformed(format!("HTML color {spec}")))?;
    Ok(Color::rgb(
        ((n >> 16) & 0xff) as u8,
        ((n >> 8) & 0xff) as u8,
        (n & 0xff) as u8,
    ))
}

fn unit_components(spec: &str, n: usize) -> Result<Vec<Dim>, Error> {
    let parts: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != n {
        return Err(malformed(format!(
            "color spec `{spec}` (need {n} components)"
        )));
    }
    Ok(parts.iter().map(|p| Dim::parse(p)).collect())
}

fn cmyk_to_rgb(c: &Dim, m: &Dim, y: &Dim, k: &Dim) -> Result<Color, Error> {
    let one = Dim::one();
    let r = &(&one - c) * &(&one - k);
    let g = &(&one - m) * &(&one - k);
    let b = &(&one - y) * &(&one - k);
    Ok(Color::rgb(
        unit_to_u8(&r)?,
        unit_to_u8(&g)?,
        unit_to_u8(&b)?,
    ))
}

fn unit_to_u8(d: &Dim) -> Result<u8, Error> {
    if d.is_nan() {
        return Err(malformed("color component NaN"));
    }
    let zero = Dim::zero();
    let one = Dim::one();
    let clamped = if matches!(d.cmp(&zero), Some(core::cmp::Ordering::Less)) {
        zero
    } else if matches!(d.cmp(&one), Some(core::cmp::Ordering::Greater)) {
        one
    } else {
        d.clone()
    };
    let scaled = clamped * Dim::from_i64(255);
    let rounded = &scaled + &Dim::ratio(1, 2);
    Ok(floor_u8(&rounded))
}

fn byte_channel(d: &Dim) -> Result<u8, Error> {
    if d.is_nan() {
        return Err(malformed("RGB component NaN"));
    }
    let zero = Dim::zero();
    let max = Dim::from_i64(255);
    if matches!(d.cmp(&zero), Some(core::cmp::Ordering::Less))
        || matches!(d.cmp(&max), Some(core::cmp::Ordering::Greater))
    {
        return Err(malformed("RGB component out of 0..255"));
    }
    let rounded = d + &Dim::ratio(1, 2);
    Ok(floor_u8(&rounded))
}

fn malformed(what: impl Into<String>) -> Error {
    Error::Malformed { what: what.into() }
}

fn floor_u8(d: &Dim) -> u8 {
    let mut ans = 0u8;
    let mut bit = 128u8;
    while bit > 0 {
        let cand = ans.saturating_add(bit);
        if Dim::from_i64(i64::from(cand))
            .cmp(d)
            .is_some_and(|o| o != core::cmp::Ordering::Greater)
        {
            ans = cand;
        }
        bit /= 2;
    }
    ans
}
