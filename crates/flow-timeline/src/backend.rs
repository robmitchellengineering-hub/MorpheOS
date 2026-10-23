//! Where the moments come from and where a rewind goes: Strata on a machine, or a fake with
//! a fixed list and known counts, for the compositor tests.
//!
//! Every Strata call sits behind the [`Backend`] trait, so a non-root shell (E11) swaps in a
//! broker without touching the store's permissions or the model (doc 20, S6 trap 4).

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::timeline::{Rewind, RewindPoint};

pub trait Backend {
    /// The moments a person can go back to, oldest first, one per folder an alert names.
    fn alerts(&mut self) -> Result<Vec<RewindPoint>, String>;
    /// Rewind one folder to a moment. `dry_run` plans it and reports the counts without
    /// changing anything; `false` does it.
    fn rewind(&mut self, point: &RewindPoint, dry_run: bool) -> Result<Rewind, String>;
}

/// Strata itself: `alerts` reads the recorded ransomware bursts, and `rewind` runs Strata's
/// rewind. The dry run is a real Strata call (`dry_run=true`), so the counts the person
/// confirms are the plan the confirm then runs (doc 20, S6).
pub struct StrataBackend {
    strata: strata::Strata,
}

impl StrataBackend {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, String> {
        Ok(Self {
            strata: strata::Strata::open(root).map_err(|e| e.to_string())?,
        })
    }
}

impl Backend for StrataBackend {
    fn alerts(&mut self) -> Result<Vec<RewindPoint>, String> {
        let alerts = self.strata.alerts().map_err(|e| e.to_string())?;
        let mut points = Vec::new();
        for alert in alerts {
            for folder in alert.folders {
                points.push(RewindPoint {
                    folder: folder.to_string_lossy().into_owned(),
                    files: alert.files,
                    at: alert.rewind_to,
                });
            }
        }
        Ok(points)
    }

    fn rewind(&mut self, point: &RewindPoint, dry_run: bool) -> Result<Rewind, String> {
        let report = self
            .strata
            .rewind(Path::new(&point.folder), point.at, dry_run)
            .map_err(|e| e.to_string())?;
        report_to_rewind(&report)
    }
}

/// The counts a rewind reports, read from Strata's report.
///
/// WHY READ THE JSON. `RewindReport.steps` is `Vec<Step>`, and `Step` is not re-exported from
/// `strata` (its module is private), so the two step kinds cannot be named from here. The
/// report's JSON `action` tag — `restore` and `move-away` — is the serialized contract the
/// `strata` CLI already prints, so it is read rather than the type. `restore` is "comes back";
/// `move-away` is "goes to quarantine".
fn report_to_rewind(report: &strata::RewindReport) -> Result<Rewind, String> {
    let value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    let mut come_back = 0;
    let mut quarantine = 0;
    if let Some(steps) = value.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            match step.get("action").and_then(|a| a.as_str()) {
                Some("restore") => come_back += 1,
                Some("move-away") => quarantine += 1,
                _ => {}
            }
        }
    }
    let failed = value
        .get("failed")
        .and_then(|f| f.as_array())
        .map_or(0, |f| f.len());
    Ok(Rewind {
        come_back,
        quarantine,
        failed,
    })
}

/// A fixed list and known counts. `spec` is `folder:files,folder:files,...`; the real rewind
/// waits `delay`, so the window has time to paint its progress before it lands (H17).
pub struct Fake {
    points: Vec<RewindPoint>,
    counts: Rewind,
    delay: Duration,
}

impl Fake {
    pub fn parse(
        spec: &str,
        come_back: usize,
        quarantine: usize,
        delay_ms: u64,
    ) -> Result<Self, String> {
        let points = spec
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|item| {
                let mut f = item.split(':');
                let (Some(folder), Some(files), None) = (f.next(), f.next(), f.next()) else {
                    return Err(format!("bad rewind point {item:?}: folder:files"));
                };
                let files = files
                    .parse()
                    .map_err(|_| format!("bad file count in {item:?}"))?;
                Ok(RewindPoint {
                    folder: folder.into(),
                    files,
                    // The fake never rewinds for real, so the moment is a fixed sentinel.
                    at: 1_760_000_000_000_000_000,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            points,
            counts: Rewind {
                come_back,
                quarantine,
                failed: 0,
            },
            delay: Duration::from_millis(delay_ms),
        })
    }
}

impl Backend for Fake {
    fn alerts(&mut self) -> Result<Vec<RewindPoint>, String> {
        Ok(self.points.clone())
    }

    fn rewind(&mut self, _point: &RewindPoint, dry_run: bool) -> Result<Rewind, String> {
        if !dry_run {
            // Long enough for the window to show its progress, and for a test to catch two
            // frames while it runs, so a frozen client cannot read as a finished one (H17).
            std::thread::sleep(self.delay);
        }
        Ok(self.counts.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_returns_its_fixed_counts_for_dry_run_and_for_real() {
        let mut f = Fake::parse(
            "/home/morph/Documents:24,/home/morph/Documents/clients:8",
            24,
            25,
            0,
        )
        .unwrap();
        assert_eq!(f.alerts().unwrap().len(), 2);
        let p = f.alerts().unwrap()[0].clone();
        assert_eq!(p.folder, "/home/morph/Documents");
        assert_eq!(p.files, 24);
        let dry = f.rewind(&p, true).unwrap();
        assert_eq!((dry.come_back, dry.quarantine), (24, 25));
        let real = f.rewind(&p, false).unwrap();
        assert_eq!((real.come_back, real.quarantine), (24, 25));
        assert!(Fake::parse("bad", 1, 1, 0).is_err());
        assert!(Fake::parse("folder", 1, 1, 0).is_err());
    }

    #[test]
    fn the_strata_backend_maps_alerts_and_counts_a_dry_run() {
        let tmp = std::env::temp_dir().join(format!("flow-timeline-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let docs = tmp.join("Documents");
        std::fs::create_dir_all(&docs).unwrap();
        let strata = strata::Strata::open(tmp.join("store")).unwrap();

        // The state before the burst: two plain files, snapshotted.
        std::fs::write(docs.join("a.txt"), "hello").unwrap();
        std::fs::write(docs.join("b.txt"), "world").unwrap();
        strata.scan(&docs).unwrap();

        // The rewind point, just after that snapshot.
        let at = strata::time::now_ns();

        // The burst: a.txt rewritten in place, and a new file that did not exist at `at`.
        std::fs::write(docs.join("a.txt"), "ciphertext!").unwrap();
        std::fs::write(docs.join("c.txt"), "new").unwrap();
        strata.snapshot(&docs.join("c.txt")).unwrap();

        strata
            .record_alert(&strata::detect::Alert {
                first_ts: at,
                files: 2,
                folders: [docs.clone()].into_iter().collect(),
                rewind_to: at,
            })
            .unwrap();

        let mut backend = StrataBackend { strata };
        let points = backend.alerts().unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].folder, docs.to_string_lossy());
        assert_eq!(points[0].files, 2);

        let dry = backend.rewind(&points[0], true).unwrap();
        assert_eq!(dry.come_back, 1, "a.txt comes back");
        assert_eq!(dry.quarantine, 1, "c.txt goes to quarantine");
        assert_eq!(dry.failed, 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
