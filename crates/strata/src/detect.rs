//! Ransomware-like activity: many ordinary files turning into ciphertext at once.
//!
//! Encryption shows up as entropy: documents sit around 4–6 bits per byte, and
//! ciphertext close to 8. One file going from low to high entropy is just
//! someone zipping something. **Many in a short window** is the signature of
//! ransomware, in either of its two usual shapes:
//!
//! - **in place:** `report.docx` is rewritten as ciphertext;
//! - **replace:** `report.docx.locked` (ciphertext) appears and `report.docx`
//!   is deleted, in either order, in the same folder. The names must be related
//!   (one starts with the other, or they share a stem): a deleted document and an
//!   unrelated new photo in the same folder are ordinary work. Ransomware that
//!   gives files random names is caught by v0 only through in-place rewrites.
//!
//! The detector only reads versions Strata has already recorded, so the files
//! from before the burst are safe in the store by the time it speaks. Its alert
//! carries the moment just before the burst began: the rewind point.
//!
//! v0 can only observe and advise. Freezing the process doing it needs fanotify
//! (which reports the writer's PID) and lands with the heal engine.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::index::{Kind, Version};
use crate::time::SECOND;

/// Below this, a file looks like ordinary content.
pub const LOW_ENTROPY: f64 = 6.0;
/// Above this, a file looks like ciphertext (or already-compressed data).
pub const HIGH_ENTROPY: f64 = 7.5;
/// Entropy estimates on tiny files are meaningless: 64 bytes can never exceed 6 bits.
pub const MIN_SIZE: u64 = 1024;
/// Hits further apart than this belong to different bursts.
pub const MAX_GAP: i64 = 10 * SECOND;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub first_ts: i64,
    pub files: usize,
    #[serde(serialize_with = "lossy_set")]
    pub folders: BTreeSet<PathBuf>,
    /// Rewind to this moment to get the files back.
    pub rewind_to: i64,
}

fn lossy_set<S: serde::Serializer>(v: &BTreeSet<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(v.iter().map(|p| p.to_string_lossy().into_owned()))
}

pub struct Detector {
    window: i64,
    threshold: usize,
    /// Suspicious events in the window: (time, folder).
    hits: VecDeque<(i64, PathBuf)>,
    /// Low-entropy files deleted recently, not yet matched: (time, path).
    deleted_plain: VecDeque<(i64, PathBuf)>,
    /// High-entropy files created recently, not yet matched: (time, path).
    new_cipher: VecDeque<(i64, PathBuf)>,
    alerted: bool,
}

impl Default for Detector {
    fn default() -> Self {
        Detector::new(60 * SECOND, 8)
    }
}

fn is_plain(v: &Version) -> bool {
    v.kind == Kind::File && v.size >= MIN_SIZE && v.entropy < LOW_ENTROPY
}

fn is_cipher(v: &Version) -> bool {
    v.kind == Kind::File && v.size >= MIN_SIZE && v.entropy > HIGH_ENTROPY
}

fn folder(p: &Path) -> PathBuf {
    p.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn stem(p: &Path) -> String {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.split('.').next().unwrap_or_default().to_string()
}

/// Could `cipher` be the encrypted replacement of `plain`? Same folder, and
/// `report.docx` → `report.docx.locked` or `report.docx` → `report.enc`.
fn replaces(cipher: &Path, plain: &Path) -> bool {
    if folder(cipher) != folder(plain) {
        return false;
    }
    let (c, p) = (cipher.file_name(), plain.file_name());
    let starts = match (c, p) {
        (Some(c), Some(p)) => c.to_string_lossy().starts_with(&*p.to_string_lossy()),
        _ => false,
    };
    starts || (!stem(plain).is_empty() && stem(cipher) == stem(plain))
}

/// Remove and report the first entry related to `path`, if any.
fn take(q: &mut VecDeque<(i64, PathBuf)>, related: impl Fn(&Path) -> bool) -> bool {
    match q.iter().position(|(_, p)| related(p)) {
        Some(i) => {
            q.remove(i);
            true
        }
        None => false,
    }
}

impl Detector {
    pub fn new(window: i64, threshold: usize) -> Self {
        Detector {
            window,
            threshold,
            hits: VecDeque::new(),
            deleted_plain: VecDeque::new(),
            new_cipher: VecDeque::new(),
            alerted: false,
        }
    }

    /// Feed one recorded change: the version before (if any) and the new one.
    pub fn observe(&mut self, prev: Option<&Version>, new: &Version) -> Option<Alert> {
        let now = new.ts;
        for q in [
            &mut self.hits,
            &mut self.deleted_plain,
            &mut self.new_cipher,
        ] {
            while q.front().is_some_and(|(t, _)| now - t > self.window) {
                q.pop_front();
            }
        }
        if self.hits.is_empty() {
            self.alerted = false; // quiet for a whole window: the next burst alerts again
        }

        let dir = folder(&new.path);
        let was_plain = prev.is_some_and(is_plain);
        let suspicious = match new.kind {
            // In place: an ordinary file rewritten as ciphertext.
            Kind::File if is_cipher(new) && was_plain => true,
            // Replace, ciphertext first: does a plain file in this folder vanish too?
            Kind::File if is_cipher(new) => {
                if take(&mut self.deleted_plain, |plain| replaces(&new.path, plain)) {
                    true
                } else {
                    self.new_cipher.push_back((now, new.path.clone()));
                    false
                }
            }
            // Replace, deletion first (or second).
            Kind::Deleted if was_plain => {
                if take(&mut self.new_cipher, |cipher| replaces(cipher, &new.path)) {
                    true
                } else {
                    self.deleted_plain.push_back((now, new.path.clone()));
                    false
                }
            }
            _ => false,
        };
        if !suspicious {
            return None;
        }
        self.hits.push_back((now, dir));
        if self.alerted {
            return None;
        }
        // Only the dense run of hits ending now counts. An attack is fast; a
        // stray coincidence (someone deleted a document and saved a photo in the
        // same folder) a minute earlier must neither trigger an alert nor drag
        // the rewind point back over legitimate work.
        let mut start = self.hits.len() - 1;
        while start > 0 && self.hits[start].0 - self.hits[start - 1].0 <= MAX_GAP {
            start -= 1;
        }
        let burst = self.hits.len() - start;
        if burst < self.threshold {
            return None;
        }
        self.alerted = true;
        let first_ts = self.hits[start].0;
        Some(Alert {
            first_ts,
            files: burst,
            folders: self
                .hits
                .iter()
                .skip(start)
                .map(|(_, d)| d.clone())
                .collect(),
            rewind_to: first_ts - SECOND,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(path: &str, ts: i64, kind: Kind, entropy: f64) -> Version {
        Version {
            id: 0,
            path: PathBuf::from(path),
            ts,
            kind,
            size: if kind == Kind::File { 10_000 } else { 0 },
            mode: 0o644,
            content: String::new(),
            entropy,
        }
    }

    #[test]
    fn in_place_encryption_burst_alerts_once() {
        let mut d = Detector::new(60 * SECOND, 5);
        let mut alerts = Vec::new();
        for i in 0..12 {
            let p = format!("/docs/f{i}.txt");
            let before = v(&p, 0, Kind::File, 4.5);
            let after = v(&p, (100 + i) * SECOND, Kind::File, 7.99);
            alerts.extend(d.observe(Some(&before), &after));
        }
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].first_ts, 100 * SECOND);
        assert_eq!(alerts[0].rewind_to, 99 * SECOND);
        assert_eq!(alerts[0].files, 5);
    }

    #[test]
    fn replace_pattern_in_either_order() {
        for cipher_first in [true, false] {
            let mut d = Detector::new(60 * SECOND, 4);
            let mut alerts = Vec::new();
            for i in 0..6 {
                let t = (10 + i) * SECOND;
                let plain = v(&format!("/d/f{i}.doc"), 0, Kind::File, 4.0);
                let gone = v(&format!("/d/f{i}.doc"), t, Kind::Deleted, 0.0);
                let locked = v(&format!("/d/f{i}.doc.locked"), t, Kind::File, 7.98);
                if cipher_first {
                    alerts.extend(d.observe(None, &locked));
                    alerts.extend(d.observe(Some(&plain), &gone));
                } else {
                    alerts.extend(d.observe(Some(&plain), &gone));
                    alerts.extend(d.observe(None, &locked));
                }
            }
            assert_eq!(alerts.len(), 1, "cipher_first={cipher_first}");
        }
    }

    #[test]
    fn a_stray_earlier_hit_does_not_move_the_rewind_point() {
        let mut d = Detector::new(60 * SECOND, 4);
        // Ordinary work at t=10s: a document deleted, a photo saved, same folder.
        d.observe(
            Some(&v("/d/old.doc", 0, Kind::File, 4.0)),
            &v("/d/old.doc", 10 * SECOND, Kind::Deleted, 0.0),
        );
        assert!(
            d.observe(None, &v("/d/photo.jpg", 10 * SECOND, Kind::File, 7.9))
                .is_none()
        );
        // The attack starts at t=40s.
        let mut alerts = Vec::new();
        for i in 0..6 {
            let p = format!("/d/f{i}.txt");
            alerts.extend(d.observe(
                Some(&v(&p, 0, Kind::File, 4.0)),
                &v(&p, (40 + i) * SECOND, Kind::File, 7.9),
            ));
        }
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].first_ts, 40 * SECOND);
        assert_eq!(alerts[0].rewind_to, 39 * SECOND);
    }

    #[test]
    fn related_names_only() {
        assert!(replaces(
            Path::new("/d/a.docx.locked"),
            Path::new("/d/a.docx")
        ));
        assert!(replaces(Path::new("/d/a.enc"), Path::new("/d/a.docx")));
        assert!(!replaces(Path::new("/d/photo.jpg"), Path::new("/d/a.docx")));
        assert!(!replaces(
            Path::new("/e/a.docx.locked"),
            Path::new("/d/a.docx")
        ));
    }

    #[test]
    fn ordinary_work_does_not_alert() {
        let mut d = Detector::new(60 * SECOND, 3);
        let mut n = 0;
        for i in 0..50 {
            let p = format!("/d/f{i}");
            // Text edits, a few new zip files, and deleted zips: none look like an attack.
            n += d
                .observe(
                    Some(&v(&p, 0, Kind::File, 4.0)),
                    &v(&p, i * SECOND, Kind::File, 4.2),
                )
                .is_some() as usize;
            n += d
                .observe(
                    None,
                    &v(&format!("/d/z{i}.zip"), i * SECOND, Kind::File, 7.9),
                )
                .is_some() as usize;
            n += d
                .observe(
                    Some(&v(&format!("/d/old{i}.zip"), 0, Kind::File, 7.9)),
                    &v(&format!("/d/old{i}.zip"), i * SECOND, Kind::Deleted, 0.0),
                )
                .is_some() as usize;
        }
        assert_eq!(n, 0);
    }

    #[test]
    fn slow_changes_spread_beyond_the_window_do_not_alert() {
        let mut d = Detector::new(60 * SECOND, 3);
        let mut n = 0;
        for i in 0..10 {
            let p = format!("/d/f{i}");
            n += d
                .observe(
                    Some(&v(&p, 0, Kind::File, 4.0)),
                    &v(&p, i * 120 * SECOND, Kind::File, 7.9),
                )
                .is_some() as usize;
        }
        assert_eq!(n, 0);
    }
}
