//! Dates for comments and templates: now (UTC, or local for the editor's Insert date), ISO 8601
//! parsing, and `{{date}}` placeholders.

/// A civil date and time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The civil date of a day count from 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl DateTime {
    /// The time `millis` after 1970-01-01T00:00Z, in UTC.
    pub fn from_millis(millis: i64) -> Self {
        let seconds = millis.div_euclid(1000);
        let (year, month, day) = civil(seconds.div_euclid(86_400));
        let of_day = seconds.rem_euclid(86_400);
        let part = |value: i64| u32::try_from(value).unwrap_or(0);
        Self { year, month, day, hour: part(of_day / 3600), minute: part(of_day % 3600 / 60), second: part(of_day % 60) }
    }

    /// `2026-10-23`, `2026-10-23T14:05`, `2026-10-23T14:05:09` (and a zone suffix, ignored).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let digits = |range: std::ops::Range<usize>| {
            text.get(range).filter(|part| part.bytes().all(|b| b.is_ascii_digit())).and_then(|part| part.parse::<u32>().ok())
        };
        if text.get(4..5) != Some("-") || text.get(7..8) != Some("-") {
            return None;
        }
        let (year, month, day) = (i64::from(digits(0..4)?), digits(5..7)?, digits(8..10)?);
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        let mut found = Self { year, month, day, hour: 0, minute: 0, second: 0 };
        if text.len() > 10 {
            if !matches!(text.get(10..11), Some("T" | " ")) || text.get(13..14) != Some(":") {
                return None;
            }
            found.hour = digits(11..13).filter(|hour| *hour < 24)?;
            found.minute = digits(14..16).filter(|minute| *minute < 60)?;
            if text.get(16..17) == Some(":") {
                found.second = digits(17..19).filter(|second| *second < 61)?;
            }
        }
        Some(found)
    }

    /// `2026-10-23T14:05:09Z`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }

    /// The date in a `{{date:FORMAT}}` pattern: `YYYY`, `YY`, `MM`, `M`, `DD`, `D`, `HH`, `mm`,
    /// `ss`; other characters stay.
    pub fn format(&self, pattern: &str) -> String {
        let mut out = String::new();
        let mut rest = pattern;
        while let Some(c) = rest.chars().next() {
            let tokens: [(&str, String); 9] = [
                ("YYYY", format!("{:04}", self.year)),
                ("YY", format!("{:02}", self.year.rem_euclid(100))),
                ("MM", format!("{:02}", self.month)),
                ("M", self.month.to_string()),
                ("DD", format!("{:02}", self.day)),
                ("D", self.day.to_string()),
                ("HH", format!("{:02}", self.hour)),
                ("mm", format!("{:02}", self.minute)),
                ("ss", format!("{:02}", self.second)),
            ];
            if let Some((token, value)) = tokens.iter().find(|(token, _)| rest.starts_with(token)) {
                out.push_str(value);
                rest = rest.get(token.len()..).unwrap_or_default();
            } else {
                out.push(c);
                rest = rest.get(c.len_utf8()..).unwrap_or_default();
            }
        }
        out
    }
}

/// Now, in UTC.
pub fn now() -> DateTime {
    DateTime::from_millis(now_millis())
}

/// The most minutes a time zone is ahead of or behind UTC that a caller can give.
pub const MAX_OFFSET: i64 = 18 * 60;

/// Now, in local time: `offset` minutes ahead of UTC (behind when negative), or the time zone of
/// the page the engine runs in when not given. Outside a page (the headless kit) there is no local
/// time zone to read, and the time is UTC.
pub fn local_now(offset: Option<i64>) -> DateTime {
    let millis = now_millis();
    let offset = offset.unwrap_or_else(local_offset);
    DateTime::from_millis(millis.saturating_add(offset.saturating_mul(60_000)))
}

/// The page's offset from UTC in minutes (JavaScript's `getTimezoneOffset` is UTC minus local).
#[cfg(target_arch = "wasm32")]
fn local_offset() -> i64 {
    -(js_sys::Date::new_0().get_timezone_offset() as i64)
}

#[cfg(not(target_arch = "wasm32"))]
fn local_offset() -> i64 {
    0
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn now_millis() -> i64 {
    js_sys::Date::now() as i64
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(0))
}

/// `{{title}}`, `{{date}}`, `{{time}}` and `{{date:FORMAT}}` filled in, as Kition's note templates
/// do (`documentTemplates.ts`).
pub fn expand_template(markdown: &str, title: &str, when: &DateTime) -> String {
    let mut out = String::new();
    let mut rest = markdown;
    while let Some(open) = rest.find("{{") {
        out.push_str(rest.get(..open).unwrap_or_default());
        let after = rest.get(open + 2..).unwrap_or_default();
        let Some(close) = after.find("}}") else {
            out.push_str(rest.get(open..).unwrap_or_default());
            return out;
        };
        let name = after.get(..close).unwrap_or_default().trim();
        let value = match name {
            "title" => Some(title.to_owned()),
            "date" => Some(when.format("YYYY-MM-DD")),
            "time" => Some(when.format("HH:mm")),
            _ => name.strip_prefix("date:").map(|pattern| when.format(pattern.trim())),
        };
        match value {
            Some(value) => out.push_str(&value),
            None => out.push_str(rest.get(open..open + 2 + close + 2).unwrap_or_default()),
        }
        rest = after.get(close + 2..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_convert_parse_and_format() {
        assert_eq!(DateTime::from_millis(0).iso(), "1970-01-01T00:00:00Z");
        assert_eq!(DateTime::from_millis(1_792_764_309_000).iso(), "2026-10-23T14:05:09Z");
        let when = DateTime::parse("2026-10-23T14:05").unwrap();
        assert_eq!(when.format("DD.MM.YYYY HH:mm"), "23.10.2026 14:05");
        assert!(DateTime::parse("2026-13-01").is_none());
        assert!(DateTime::parse("yesterday").is_none());
        assert_eq!(
            expand_template("# {{title}}\n{{date}} {{time}} {{date:YYYY/M/D}} {{other}} {{", "Plan", &when),
            "# Plan\n2026-10-23 14:05 2026/10/23 {{other}} {{"
        );
    }
}
