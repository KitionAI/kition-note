//! Text positions in the unit a caller counts in (design 2.6, rule M18): Unicode scalar values
//! (agents, Rust callers; the KDM's unit) or UTF-16 code units (the CodeMirror editor).

use serde_json::Value;

/// How a request counts positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Scalar,
    Utf16,
}

impl Unit {
    /// The `unit` parameter: `"scalar"` (default) or `"utf16"`.
    pub fn of(params: &Value) -> Result<Self, String> {
        match params.get("unit") {
            None | Some(Value::Null) => Ok(Self::Scalar),
            Some(Value::String(unit)) if unit == "scalar" => Ok(Self::Scalar),
            Some(Value::String(unit)) if unit == "utf16" => Ok(Self::Utf16),
            Some(other) => Err(format!("invalid-params: unit must be \"scalar\" or \"utf16\", not {other}")),
        }
    }

    /// `position` (in this unit) of `text` as a scalar index; past the end or inside a character
    /// is `out-of-range`.
    pub fn to_scalar(self, text: &str, position: usize) -> Result<usize, String> {
        match self {
            Self::Scalar => {
                let len = text.chars().count();
                if position > len {
                    return Err(format!("out-of-range: {position} is past the end of the text ({len} characters)"));
                }
                Ok(position)
            }
            Self::Utf16 => {
                let mut units = 0usize;
                for (index, c) in text.chars().enumerate() {
                    if units == position {
                        return Ok(index);
                    }
                    units += c.len_utf16();
                    if units > position {
                        return Err(format!("out-of-range: {position} is inside a character"));
                    }
                }
                if units == position {
                    Ok(text.chars().count())
                } else {
                    Err(format!("out-of-range: {position} is past the end of the text ({units} UTF-16 units)"))
                }
            }
        }
    }

    /// The scalar index `index` of `text` counted in this unit.
    pub fn from_scalar(self, text: &str, index: usize) -> usize {
        match self {
            Self::Scalar => index,
            Self::Utf16 => text.chars().take(index).map(char::len_utf16).sum(),
        }
    }

    /// The length of `text` in this unit.
    pub fn len(self, text: &str) -> usize {
        match self {
            Self::Scalar => text.chars().count(),
            Self::Utf16 => text.encode_utf16().count(),
        }
    }
}

/// The byte range of the scalar range `from..to` of `text`.
pub fn byte_range(text: &str, from: usize, to: usize) -> std::ops::Range<usize> {
    let boundary = |index: usize| text.char_indices().map(|(at, _)| at).chain(std::iter::once(text.len())).nth(index).unwrap_or(text.len());
    let start = boundary(from);
    start..boundary(to.max(from)).max(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_positions_convert_and_never_split_a_pair() {
        let text = "a\u{1F600}b";
        assert_eq!(Unit::Utf16.to_scalar(text, 0).unwrap(), 0);
        assert_eq!(Unit::Utf16.to_scalar(text, 1).unwrap(), 1);
        assert!(Unit::Utf16.to_scalar(text, 2).unwrap_err().starts_with("out-of-range"));
        assert_eq!(Unit::Utf16.to_scalar(text, 3).unwrap(), 2);
        assert_eq!(Unit::Utf16.to_scalar(text, 4).unwrap(), 3);
        assert!(Unit::Utf16.to_scalar(text, 5).is_err());
        assert_eq!(Unit::Utf16.from_scalar(text, 2), 3);
        assert_eq!(Unit::Utf16.len(text), 4);
        assert_eq!(Unit::Scalar.len(text), 3);
        assert!(Unit::Scalar.to_scalar(text, 4).is_err());
    }

    #[test]
    fn byte_ranges_follow_characters() {
        let text = "a\u{00e9}b";
        assert_eq!(byte_range(text, 1, 2), 1..3);
        assert_eq!(byte_range(text, 0, 3), 0..4);
        assert_eq!(byte_range(text, 3, 3), 4..4);
        assert_eq!(&text[byte_range(text, 2, 3)], "b");
    }

    #[test]
    fn the_unit_parameter_is_checked() {
        assert_eq!(Unit::of(&serde_json::json!({})).unwrap(), Unit::Scalar);
        assert_eq!(Unit::of(&serde_json::json!({ "unit": "utf16" })).unwrap(), Unit::Utf16);
        assert!(Unit::of(&serde_json::json!({ "unit": 3 })).is_err());
    }
}
