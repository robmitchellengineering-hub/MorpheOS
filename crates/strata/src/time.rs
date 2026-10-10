//! Timestamps: nanoseconds since the Unix epoch, UTC.
//!
//! Small and dependency-free on purpose: Strata only needs to stamp versions,
//! print them, and parse the handful of forms a person types when asking for
//! "the folder as it was at …".

use std::time::{SystemTime, UNIX_EPOCH};

pub const SECOND: i64 = 1_000_000_000;

pub fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since 1970-01-01. Inverse of [`civil_from_days`].
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-10-10T13:05:09.123Z`
pub fn format(ns: i64) -> String {
    let secs = ns.div_euclid(SECOND);
    let millis = ns.rem_euclid(SECOND) / 1_000_000;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// Parse a point in time, relative to `now`:
///
/// - `-90s`, `-15m`, `-2h`, `-3d`: that long ago
/// - `@1760101509`: Unix seconds
/// - `2026-10-10T13:05:09Z` (optionally with `.fff`): UTC
pub fn parse(text: &str, now: i64) -> Option<i64> {
    let t = text.trim();
    if let Some(rel) = t.strip_prefix('-') {
        let (num, unit) = rel.split_at(rel.find(|c: char| !c.is_ascii_digit())?);
        let n: i64 = num.parse().ok()?;
        let mult = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86_400,
            _ => return None,
        };
        return Some(now - n * mult * SECOND);
    }
    if let Some(unix) = t.strip_prefix('@') {
        return unix.parse::<i64>().ok().map(|s| s * SECOND);
    }
    let t = t.strip_suffix('Z')?;
    let (date, clock) = t.split_once('T')?;
    let mut dp = date.split('-');
    let (y, mo, d) = (
        dp.next()?.parse::<i64>().ok()?,
        dp.next()?.parse::<u32>().ok()?,
        dp.next()?.parse::<u32>().ok()?,
    );
    if dp.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    let mut cp = hms.split(':');
    let (h, mi, s) = (
        cp.next()?.parse::<i64>().ok()?,
        cp.next()?.parse::<i64>().ok()?,
        cp.next()?.parse::<i64>().ok()?,
    );
    if cp.next().is_some() || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let nanos = if frac.is_empty() {
        0
    } else {
        if frac.len() > 9 || !frac.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        format!("{frac:0<9}").parse::<i64>().ok()?
    };
    Some((days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s) * SECOND + nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_parse_roundtrip() {
        let ns = 1_760_101_509_123_000_000;
        let text = format(ns);
        assert_eq!(text, "2025-10-10T13:05:09.123Z");
        assert_eq!(parse(&text, 0), Some(ns));
        assert_eq!(
            parse("2025-10-10T13:05:09Z", 0),
            Some(1_760_101_509 * SECOND)
        );
    }

    #[test]
    fn epoch_and_leap_day() {
        assert_eq!(format(0), "1970-01-01T00:00:00.000Z");
        let leap = parse("2024-02-29T12:00:00Z", 0).unwrap();
        assert_eq!(format(leap), "2024-02-29T12:00:00.000Z");
    }

    #[test]
    fn relative_and_unix() {
        let now = 1_000_000 * SECOND;
        assert_eq!(parse("-90s", now), Some(now - 90 * SECOND));
        assert_eq!(parse("-2h", now), Some(now - 7200 * SECOND));
        assert_eq!(parse("@42", now), Some(42 * SECOND));
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "yesterday",
            "-5x",
            "2025-13-01T00:00:00Z",
            "2025-10-10 13:05:09",
            "@x",
        ] {
            assert_eq!(parse(bad, 0), None, "{bad}");
        }
    }
}
