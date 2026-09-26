//! `{variables}` at run time: filling them into a step's text, the values a
//! file trigger gives, and how an If step compares two values.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate};

use super::runs::{RunValue, ValueKind};
use crate::api::pickers::shorten_home;
use crate::workflow::validate::variables_in;
use crate::workflow::Op;

pub type Values = BTreeMap<String, RunValue>;

pub fn value(kind: ValueKind, value: impl Into<String>) -> RunValue {
    RunValue {
        kind,
        value: value.into(),
    }
}

/// `text` with every `{name}` replaced by its value. Braces that aren't a
/// variable stay as they are. Fails with the name of a variable that has no
/// value, which validation should already have ruled out.
pub fn fill(text: &str, values: &Values) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    for name in variables_in(text) {
        let token = format!("{{{name}}}");
        let at = rest.find(&token).expect("variables_in found it");
        let value = values.get(name).ok_or_else(|| name.to_owned())?;
        out.push_str(&rest[..at]);
        out.push_str(&value.value);
        rest = &rest[at + token.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// What a file trigger gives: `{file}` (the name without its extension),
/// `{extension}`, `{folder}` (with the home folder as `~`), `{dateAdded}` and
/// `{year}`.
pub fn file_values(path: &Path, home: &Path, added: DateTime<FixedOffset>) -> Values {
    let text = |s: Option<&std::ffi::OsStr>| {
        s.map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let folder = path
        .parent()
        .map(|p| shorten_home(p, home))
        .unwrap_or_default();
    Values::from([
        (
            "file".into(),
            value(ValueKind::Text, text(path.file_stem())),
        ),
        (
            "extension".into(),
            value(ValueKind::Text, text(path.extension())),
        ),
        ("folder".into(), value(ValueKind::Text, folder)),
        (
            "dateAdded".into(),
            value(ValueKind::Date, added.format("%Y-%m-%d").to_string()),
        ),
        (
            "year".into(),
            value(ValueKind::Number, added.year().to_string()),
        ),
    ])
}

/// Whether `left op right` holds. Both sides are compared as numbers when
/// both read as numbers, as dates when both read as `YYYY-MM-DD`, and
/// otherwise as text, ignoring case and outer spaces.
pub fn holds(left: &str, op: Op, right: &str) -> bool {
    let (left, right) = (left.trim(), right.trim());
    let order = match (number(left), number(right)) {
        (Some(l), Some(r)) => l.partial_cmp(&r),
        _ => match (date(left), date(right)) {
            (Some(l), Some(r)) => Some(l.cmp(&r)),
            _ => Some(left.to_lowercase().cmp(&right.to_lowercase())),
        },
    };
    let text = || (left.to_lowercase(), right.to_lowercase());
    match op {
        Op::Equal => order == Some(std::cmp::Ordering::Equal),
        Op::NotEqual => order != Some(std::cmp::Ordering::Equal),
        Op::Greater => order == Some(std::cmp::Ordering::Greater),
        Op::Less => order == Some(std::cmp::Ordering::Less),
        Op::GreaterOrEqual => order.is_some_and(|o| o.is_ge()),
        Op::LessOrEqual => order.is_some_and(|o| o.is_le()),
        Op::Contains => {
            let (l, r) = text();
            l.contains(&r)
        }
        Op::StartsWith => {
            let (l, r) = text();
            l.starts_with(&r)
        }
    }
}

/// A plain decimal number, such as `-12`, `4.50` or `1000`.
fn number(s: &str) -> Option<f64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let plain = !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && digits.bytes().filter(|&b| b == b'.').count() <= 1
        && digits != ".";
    plain.then(|| s.parse().ok()).flatten()
}

fn date(s: &str) -> Option<NaiveDate> {
    (s.len() == 10)
        .then(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use Op::*;

    fn values(pairs: &[(&str, &str)]) -> Values {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), value(ValueKind::Text, *v)))
            .collect()
    }

    #[test]
    fn variables_are_filled_in_and_other_braces_are_left_alone() {
        let v = values(&[("vendor", "Blue Bottle"), ("amount", "4.50")]);
        assert_eq!(
            fill("{vendor} {amount} {vendor} {not a var} {}", &v).unwrap(),
            "Blue Bottle 4.50 Blue Bottle {not a var} {}"
        );
        assert_eq!(fill("no variables", &v).unwrap(), "no variables");
    }

    #[test]
    fn a_value_that_looks_like_a_variable_isnt_filled_again() {
        let v = values(&[("a", "{b}"), ("b", "x")]);
        assert_eq!(fill("{a}-{b}", &v).unwrap(), "{b}-x");
    }

    #[test]
    fn a_variable_with_no_value_is_named() {
        assert_eq!(fill("Due {due}", &Values::new()), Err("due".to_string()));
    }

    #[test]
    fn a_file_gives_its_name_extension_folder_and_dates() {
        let added = DateTime::parse_from_rfc3339("2026-01-02T23:30:00-05:00").unwrap();
        let home = PathBuf::from("/Users/ada");
        let v = file_values(Path::new("/Users/ada/Downloads/my.scan.PDF"), &home, added);
        let get = |k: &str| (v[k].kind, v[k].value.as_str());
        assert_eq!(get("file"), (ValueKind::Text, "my.scan"));
        assert_eq!(get("extension"), (ValueKind::Text, "PDF"));
        assert_eq!(get("folder"), (ValueKind::Text, "~/Downloads"));
        // The Mac's own date, not UTC's.
        assert_eq!(get("dateAdded"), (ValueKind::Date, "2026-01-02"));
        assert_eq!(get("year"), (ValueKind::Number, "2026"));

        let v = file_values(Path::new("/Volumes/Scans/README"), &home, added);
        assert_eq!(v["extension"].value, "");
        assert_eq!(v["folder"].value, "/Volumes/Scans");
    }

    #[test]
    fn numbers_compare_as_numbers() {
        assert!(holds("1200", Greater, "1000"));
        assert!(holds("999.99", Less, "1000"));
        assert!(holds("1000", Equal, "1000.00"));
        assert!(holds(" 10 ", GreaterOrEqual, "9"));
        assert!(holds("-5", Less, "0"));
        // As text, "9" would come after "10".
        assert!(holds("9", Less, "10"));
    }

    #[test]
    fn dates_compare_as_dates() {
        assert!(holds("2026-09-14", Greater, "2026-01-31"));
        assert!(holds("2026-09-14", Equal, "2026-09-14"));
        assert!(holds("2025-12-31", LessOrEqual, "2026-01-01"));
    }

    #[test]
    fn anything_else_compares_as_text_ignoring_case_and_outer_spaces() {
        assert!(holds("  Receipt ", Equal, "receipt"));
        assert!(holds("Yes", NotEqual, "no"));
        assert!(holds("Screenshot 2026-09-14", StartsWith, "screenshot"));
        assert!(holds("Invoice from ACME", Contains, "acme"));
        assert!(!holds("Invoice", Contains, "receipt"));
        // A number beside text compares as text.
        assert!(holds("12 apples", NotEqual, "12"));
        assert!(!holds("1e3", Equal, "1000"));
    }

    #[test]
    fn contains_and_starts_with_read_numbers_as_text() {
        assert!(holds("2026-044", StartsWith, "2026"));
        assert!(holds("1000", Contains, "00"));
    }
}
