//! Retention: which old versions may go when space is needed.
//!
//! The same three-tier rule as the Manifest's `storage.retention` (doc 04):
//! every version younger than `all_versions`, then one per hour up to
//! `hourly`, then one per day up to `daily`, then nothing. Whatever the age,
//! **the newest version of every path is always kept**, so pruning can thin
//! out history but never make a file unrecoverable.

use crate::time::SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub all_versions_ns: i64,
    pub hourly_ns: i64,
    pub daily_ns: i64,
}

impl Retention {
    pub fn from_manifest(all_versions_hours: u32, hourly_days: u32, daily_days: u32) -> Self {
        Retention {
            all_versions_ns: all_versions_hours as i64 * 3600 * SECOND,
            hourly_ns: hourly_days as i64 * 86_400 * SECOND,
            daily_ns: daily_days as i64 * 86_400 * SECOND,
        }
    }
}

/// Given one path's versions as (id, ts) sorted oldest first, return the ids
/// that may be removed at time `now`.
pub fn prunable(versions: &[(i64, i64)], now: i64, r: &Retention) -> Vec<i64> {
    let Some(newest) = versions.last() else {
        return Vec::new();
    };
    let mut kept_buckets = std::collections::HashSet::new();
    let mut out = Vec::new();
    // Walk newest first, so the version kept in each bucket is that bucket's newest.
    for &(id, ts) in versions.iter().rev() {
        if id == newest.0 {
            continue;
        }
        let age = now - ts;
        let bucket = if age < r.all_versions_ns {
            continue;
        } else if age < r.hourly_ns {
            ("h", ts.div_euclid(3600 * SECOND))
        } else if age < r.daily_ns {
            ("d", ts.div_euclid(86_400 * SECOND))
        } else {
            out.push(id);
            continue;
        };
        if !kept_buckets.insert(bucket) {
            out.push(id);
        }
    }
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600 * SECOND;
    const D: i64 = 24 * H;

    fn policy() -> Retention {
        Retention::from_manifest(24, 30, 365)
    }

    #[test]
    fn newest_is_kept_however_old() {
        let now = 1000 * D;
        assert!(prunable(&[(1, 0)], now, &policy()).is_empty());
        assert_eq!(prunable(&[(1, 0), (2, 1)], now, &policy()), [1]);
    }

    #[test]
    fn recent_versions_are_all_kept() {
        let now = 100 * D;
        let v: Vec<_> = (0..50).map(|i| (i, now - H + i * SECOND)).collect();
        assert!(prunable(&v, now, &policy()).is_empty());
    }

    #[test]
    fn one_per_hour_then_one_per_day() {
        let now = 100 * D;
        // Three versions in the same hour, two days ago: keep the newest of them.
        let base = now - 2 * D;
        let base = base - base.rem_euclid(H) + 60 * SECOND;
        let v = vec![
            (1, base),
            (2, base + 60 * SECOND),
            (3, base + 120 * SECOND),
            (9, now),
        ];
        assert_eq!(prunable(&v, now, &policy()), [1, 2]);
        // Three versions on the same day, sixty days ago: keep the newest of them.
        let day = now - 60 * D;
        let day = day - day.rem_euclid(D) + H;
        let v = vec![(1, day), (2, day + H), (3, day + 2 * H), (9, now)];
        assert_eq!(prunable(&v, now, &policy()), [1, 2]);
    }

    #[test]
    fn beyond_the_last_tier_goes() {
        let now = 1000 * D;
        let v = vec![(1, now - 400 * D), (2, now - 399 * D), (3, now)];
        assert_eq!(prunable(&v, now, &policy()), [1, 2]);
    }
}
