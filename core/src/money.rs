//! Parsing of amounts and dates as they appear in Indian bank statements.

use crate::model::{Direction, Paise};
use chrono::{Datelike, Duration, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amount {
    /// Absolute value in paise.
    pub paise: Paise,
    /// The text carried a minus sign or parentheses.
    pub negative: bool,
    /// An explicit "Dr"/"Cr" marker next to the number.
    pub marker: Option<Direction>,
}

impl Amount {
    /// Signed value: negative when marked negative or "Dr".
    pub fn signed(&self) -> Paise {
        if self.negative || self.marker == Some(Direction::Debit) {
            -self.paise
        } else {
            self.paise
        }
    }
}

/// Parse "1,23,456.78", "₹ 1,299.50 Dr", "(250.00)", "-12.5", "1234" etc.
/// Returns None for blank cells and placeholders like "-" or "--".
pub fn parse_amount(raw: &str) -> Option<Amount> {
    let mut s: String = raw
        .replace('\u{a0}', " ")
        .replace('₹', "")
        .replace(',', "")
        .trim()
        .to_string();
    if s.is_empty() || s.chars().all(|c| c == '-' || c == ' ' || c == '.') {
        return None;
    }
    for prefix in ["INR", "Rs.", "Rs", "RS.", "RS", "rs."] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
            break;
        }
    }
    let mut marker = None;
    let upper = s.to_ascii_uppercase();
    for (suffix, dir) in [
        ("DR.", Direction::Debit),
        ("CR.", Direction::Credit),
        ("DR", Direction::Debit),
        ("CR", Direction::Credit),
    ] {
        if upper.ends_with(suffix) {
            s = s[..s.len() - suffix.len()].trim().to_string();
            marker = Some(dir);
            break;
        }
    }
    if marker.is_none() {
        for (prefix, dir) in [("DR", Direction::Debit), ("CR", Direction::Credit)] {
            if upper.starts_with(prefix) {
                s = s[prefix.len()..].trim_start_matches(['.', ' ']).to_string();
                marker = Some(dir);
                break;
            }
        }
    }
    let mut negative = false;
    if s.starts_with('(') && s.ends_with(')') {
        negative = true;
        s = s[1..s.len() - 1].trim().to_string();
    }
    if let Some(rest) = s.strip_prefix('-') {
        negative = true;
        s = rest.trim().to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim().to_string();
    }
    if let Some(rest) = s.strip_suffix('-') {
        negative = true;
        s = rest.trim().to_string();
    }
    s = s.replace(' ', "");
    let paise = parse_decimal_paise(&s)?;
    Some(Amount { paise, negative, marker })
}

fn parse_decimal_paise(s: &str) -> Option<Paise> {
    if s.is_empty() {
        return None;
    }
    let simple = s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.matches('.').count() <= 1;
    if simple {
        let (int_part, frac_part) = match s.split_once('.') {
            Some((i, f)) => (i, f),
            None => (s, ""),
        };
        if frac_part.len() <= 2 {
            let int_val: i64 = if int_part.is_empty() { 0 } else { int_part.parse().ok()? };
            let frac_val: i64 = match frac_part.len() {
                0 => 0,
                1 => frac_part.parse::<i64>().ok()? * 10,
                _ => frac_part.parse().ok()?,
            };
            return int_val.checked_mul(100)?.checked_add(frac_val);
        }
    }
    // Excel floats like 1234.5000000001 or scientific notation.
    let f: f64 = s.parse().ok()?;
    if !f.is_finite() {
        return None;
    }
    Some((f.abs() * 100.0).round() as i64)
}

const DATE_FORMATS: &[&str] = &[
    // Two-digit-year formats first: %y needs exactly two digits, so "2026" fails here.
    "%d/%m/%y",
    "%d-%m-%y",
    "%d.%m.%y",
    "%d-%b-%y",
    "%d %b %y",
    "%d/%b/%y",
    "%d/%m/%Y",
    "%d-%m-%Y",
    "%d.%m.%Y",
    "%Y-%m-%d",
    "%Y/%m/%d",
    "%d %b %Y",
    "%d-%b-%Y",
    "%d/%b/%Y",
    "%d %B %Y",
    "%d-%B-%Y",
    "%b %d, %Y",
    "%B %d, %Y",
    "%d%b%Y",
];

fn plausible(d: NaiveDate) -> bool {
    (1990..=2100).contains(&d.year())
}

fn try_formats(s: &str) -> Option<NaiveDate> {
    for f in DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            if plausible(d) {
                return Some(d);
            }
        }
    }
    None
}

/// Parse a statement date. Indian statements are day-first.
pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    let s = raw.replace('\u{a0}', " ");
    let s = s.trim();
    if s.is_empty() || s.len() > 40 {
        return None;
    }
    // Excel serial date numbers (e.g. 46054 = 2026-02-01).
    if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        if let Ok(n) = s.parse::<f64>() {
            if (25_000.0..80_000.0).contains(&n) {
                let base = NaiveDate::from_ymd_opt(1899, 12, 30)?;
                return base.checked_add_signed(Duration::days(n.trunc() as i64));
            }
        }
    }
    // ISO date-time like 2026-02-01T00:00:00
    if s.len() >= 10 && s.as_bytes().get(4) == Some(&b'-') {
        if let Ok(d) = NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d") {
            if plausible(d) {
                return Some(d);
            }
        }
    }
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(d) = try_formats(&collapsed) {
        return Some(d);
    }
    let tokens: Vec<&str> = collapsed.split(' ').collect();
    if tokens.len() > 1 {
        // "01/02/2026 10:22:33" or "01 Feb 2026 10:22"
        if let Some(d) = try_formats(tokens[0]) {
            return Some(d);
        }
        if tokens.len() >= 3 {
            if let Some(d) = try_formats(&tokens[..3].join(" ")) {
                return Some(d);
            }
        }
    }
    None
}

/// Format paise as "1,23,456.78" (Indian grouping), with a leading '-' when negative.
pub fn format_inr(p: Paise) -> String {
    let neg = p < 0;
    let a = p.unsigned_abs();
    let rupees = a / 100;
    let paise = a % 100;
    let digits = rupees.to_string();
    let mut out = String::new();
    if digits.len() > 3 {
        let (head, tail) = digits.split_at(digits.len() - 3);
        let head_chars: Vec<char> = head.chars().collect();
        let mut groups: Vec<String> = Vec::new();
        let mut i = head_chars.len();
        while i > 0 {
            let start = i.saturating_sub(2);
            groups.push(head_chars[start..i].iter().collect());
            i = start;
        }
        groups.reverse();
        out.push_str(&groups.join(","));
        out.push(',');
        out.push_str(tail);
    } else {
        out.push_str(&digits);
    }
    format!("{}{}.{:02}", if neg { "-" } else { "" }, out, paise)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("84,590.00").unwrap().paise, 8_459_000);
        assert_eq!(parse_amount("1,69,590.00").unwrap().paise, 16_959_000);
        assert_eq!(parse_amount("410").unwrap().paise, 41_000);
        assert_eq!(parse_amount("12.5").unwrap().paise, 1_250);
        assert_eq!(parse_amount("1234.5000000001").unwrap().paise, 123_450);
        assert!(parse_amount("").is_none());
        assert!(parse_amount(" ").is_none());
        assert!(parse_amount("-").is_none());
        assert!(parse_amount("--").is_none());
        let a = parse_amount("₹1,23,456.00").unwrap();
        assert_eq!(a.paise, 12_345_600);
        let a = parse_amount("1,299.50 Dr").unwrap();
        assert_eq!(a.marker, Some(Direction::Debit));
        assert_eq!(a.signed(), -129_950);
        let a = parse_amount("2,000.00 Cr").unwrap();
        assert_eq!(a.marker, Some(Direction::Credit));
        assert_eq!(a.signed(), 200_000);
        let a = parse_amount("(250.00)").unwrap();
        assert!(a.negative);
        assert_eq!(a.signed(), -25_000);
        assert_eq!(parse_amount("-12.00").unwrap().signed(), -1_200);
        assert_eq!(parse_amount("Rs. 99").unwrap().paise, 9_900);
        assert!(parse_amount("abc").is_none());
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("02/10/26"), Some(d(2026, 10, 2)));
        assert_eq!(parse_date("02/10/2026"), Some(d(2026, 10, 2)));
        assert_eq!(parse_date("1 Jan 2024"), Some(d(2024, 1, 1)));
        assert_eq!(parse_date("01-Feb-2026"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("01-FEB-26"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("15-03-2026"), Some(d(2026, 3, 15)));
        assert_eq!(parse_date("2026-02-01"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("2026-02-01T00:00:00"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("01/02/2026 10:22:33"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("46054"), Some(d(2026, 2, 1)));
        assert_eq!(parse_date("Narration"), None);
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("********"), None);
    }

    #[test]
    fn inr_format() {
        assert_eq!(format_inr(12_345_678_900), "12,34,56,789.00");
        assert_eq!(format_inr(100_050), "1,000.50");
        assert_eq!(format_inr(99_900), "999.00");
        assert_eq!(format_inr(-5), "-0.05");
    }
}
