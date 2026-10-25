//! The space guard: keep the store's filesystem from filling completely.
//!
//! The Manifest promises a free-space floor (`storage.min_free_percent`, doc 20 S7) that
//! the system keeps by evicting old versions. This module is the half of that promise
//! Strata owns: read how full the filesystem holding the store is, and when it is below
//! the floor, thin out old versions by the retention policy. The policy and the floor are
//! both decided by the Manifest (materialised by `morphd` into files Strata reads), so
//! the guard obeys the generation in force rather than a hard-coded number — AGENTS.md H5.

use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::retention::Retention;
use crate::{Error, Result, Strata};

/// Free space on the filesystem holding `path`, as a whole percent (0–100).
///
/// `statvfs` is the only thing that knows, and it is POSIX — present on Linux (where
/// Strata runs) and on macOS (where the crate is developed), unlike inotify. `f_bavail`
/// (blocks available to an unprivileged user) is the number that matters: the floor is a
/// promise about the disk staying usable, not about the root reservation.
pub fn free_percent(path: &Path) -> Result<u8> {
    // statvfs takes a NUL-terminated path. A path is an OsStr, which need not be UTF-8
    // on Linux, so build the C string from raw bytes rather than going through a String.
    let mut cpath = path.as_os_str().as_bytes().to_vec();
    cpath.push(0);
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `cpath` is a valid, NUL-terminated byte string and `stat` is a writable
    // `statvfs`; the call only reads the path and fills the struct.
    let rc = unsafe { libc::statvfs(cpath.as_ptr().cast::<libc::c_char>(), &mut stat) };
    if rc != 0 {
        return Err(Error::Io {
            what: format!("reading free space on {}", path.display()),
            source: std::io::Error::last_os_error(),
        });
    }
    if stat.f_blocks == 0 {
        return Err(Error::Refused(format!(
            "{} reports a filesystem with no blocks",
            path.display()
        )));
    }
    // Truncate, never round up: a reported value below the floor must mean "really below".
    let percent = (stat.f_bavail as u128 * 100 / stat.f_blocks as u128) as u8;
    Ok(percent)
}

/// What one pass of [`guard`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardReport {
    /// Free space the guard measured, as a whole percent.
    pub free_percent: u8,
    /// Free space after pruning, as a whole percent — the "now" a warning reports.
    pub free_percent_after: u8,
    /// The floor from the Manifest (`storage.min_free_percent`).
    pub floor_percent: u8,
    pub versions_removed: usize,
    pub objects_removed: u64,
    pub bytes_freed: u64,
}

/// Thin out old versions when the store's filesystem is below `floor_percent` free.
///
/// Returns `None` when free space is at or above the floor — nothing to do. Below the
/// floor it runs the same prune a person runs by hand, so it never deletes more than
/// the retention policy allows: the newest version of every file is always kept. If
/// pruning cannot get back above the floor, the caller reports that instead of deleting
/// beyond the policy (doc 20, S7 note 5 — Rob's decision: only warn).
///
/// The guard exists to recover a **full** disk, so it frees space before it writes the
/// index: the object deletes that make room run first (they need no free space), and the
/// index write that records the prune runs second, once there is room. If that record
/// write is still refused, the deletes have already recovered the space, so the guard
/// reports the recovery instead of failing the whole boot.
pub fn guard(
    strata: &mut Strata,
    floor_percent: u8,
    policy: &Retention,
    now: i64,
) -> Result<Option<GuardReport>> {
    let free = free_percent(strata.root())?;
    if free >= floor_percent {
        return Ok(None);
    }
    let doomed = strata.prune_plan(policy, now)?;
    let gc = strata.collect(&doomed, now)?;
    // Recording the prune is the only index write; if it fails the space is already
    // back, so the guard must not turn that into a failed boot. The doomed rows stay
    // until a later pass, but their objects are gone and the disk is usable again.
    let versions_removed = match strata.record_prune(&doomed) {
        Ok(n) => n,
        Err(e) => {
            eprintln!(
                "space-guard: freed {} objects ({} bytes) but could not record the prune in the version index: {e}",
                gc.objects_removed, gc.bytes_freed
            );
            0
        }
    };
    let after = free_percent(strata.root())?;
    Ok(Some(GuardReport {
        free_percent: free,
        free_percent_after: after,
        floor_percent,
        versions_removed,
        objects_removed: gc.objects_removed,
        bytes_freed: gc.bytes_freed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("strata-guard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn policy() -> Retention {
        Retention::from_manifest(24, 30, 365)
    }

    /// Write several versions of one file, so pruning has something to remove.
    fn several_versions(strata: &mut Strata, dir: &Path) {
        let file = dir.join("doc.txt");
        for i in 0..5 {
            fs::write(&file, format!("version {i}\n")).unwrap();
            strata.snapshot(&file).unwrap();
        }
    }

    #[test]
    fn below_the_floor_prunes_and_reports_what_it_freed() {
        let base = scratch("low");
        let docs = base.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let mut s = Strata::open(base.join("store")).unwrap();
        several_versions(&mut s, &docs);

        // A floor of 100 is below any real filesystem, so the guard always fires; a `now`
        // far in the future makes every version older than the policy.
        let now = time::now_ns() + 1000 * 86_400 * time::SECOND;
        let report = guard(&mut s, 100, &policy(), now)
            .unwrap()
            .expect("a floor of 100 is below any filesystem, so the guard must run");
        assert!(report.versions_removed > 0, "{report:#?}");
        assert!(report.objects_removed > 0, "{report:#?}");
        assert!(report.bytes_freed > 0, "{report:#?}");

        // The newest version of the file is always kept, whatever was pruned.
        let history = s.index().history(&docs.join("doc.txt")).unwrap();
        assert_eq!(
            history.len(),
            1,
            "the newest version must survive: {history:#?}"
        );
    }

    #[test]
    fn at_or_above_the_floor_does_nothing() {
        let base = scratch("high");
        let docs = base.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let mut s = Strata::open(base.join("store")).unwrap();
        several_versions(&mut s, &docs);

        // A floor of 0 is always at or above, so the guard must not touch the store.
        let now = time::now_ns() + 1000 * 86_400 * time::SECOND;
        let outcome = guard(&mut s, 0, &policy(), now).unwrap();
        assert!(outcome.is_none(), "{outcome:#?}");
        assert_eq!(
            s.index().history(&docs.join("doc.txt")).unwrap().len(),
            5,
            "nothing may be pruned at or above the floor"
        );
    }

    #[test]
    fn free_percent_reads_the_store_filesystem() {
        let base = scratch("free");
        let s = Strata::open(base.join("store")).unwrap();
        let free = free_percent(s.root()).unwrap();
        assert!(free <= 100, "a percentage is 0–100, got {free}");
    }

    /// A full disk fails the index WRITE that records a prune, but the object
    /// deletes that free space need no room. A `BEFORE DELETE` trigger stands in
    /// for that full medium, portably and fast, so the test can prove the guard
    /// still recovers the space and does not fail the whole boot.
    #[test]
    fn frees_space_even_when_the_index_refuses_the_record() {
        let base = scratch("record-refused");
        let docs = base.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let mut s = Strata::open(base.join("store")).unwrap();
        several_versions(&mut s, &docs);
        let history = s.index().history(&docs.join("doc.txt")).unwrap();
        let newest = history.last().unwrap().content.clone();
        let oldest = history.first().unwrap().content.clone();
        assert_ne!(oldest, newest);

        // Refuse the index write the way a full disk would, after the objects exist.
        let db = rusqlite::Connection::open(s.root().join("index.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TRIGGER block_prune_record BEFORE DELETE ON versions \
             BEGIN SELECT RAISE(ABORT, 'disk is full'); END;",
        )
        .unwrap();

        // A floor of 100 is below any filesystem, so the guard runs; a far-future
        // `now` makes every version older than the policy, so there is work to do.
        let now = time::now_ns() + 1000 * 86_400 * time::SECOND;
        let report = guard(&mut s, 100, &policy(), now)
            .unwrap()
            .expect("the guard must run below the floor");
        // The space was recovered even though the index write was refused.
        assert!(report.objects_removed > 0, "{report:#?}");
        assert!(report.bytes_freed > 0, "{report:#?}");
        assert_eq!(
            report.versions_removed, 0,
            "the record was refused, so no index row was removed: {report:#?}"
        );

        // The newest version still restores; the pruned versions' objects are gone.
        assert!(s.store().read_file(&newest, &mut Vec::new()).is_ok());
        assert!(matches!(
            s.store().read_file(&oldest, &mut Vec::new()),
            Err(Error::NotFound(_))
        ));
    }

    /// The CLI's `prune` shares the guard's order: the object deletes happen before
    /// the index write, so a full disk cannot stop the space from being recovered.
    #[test]
    fn prune_frees_the_objects_before_it_records() {
        let base = scratch("prune-order");
        let docs = base.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let mut s = Strata::open(base.join("store")).unwrap();
        several_versions(&mut s, &docs);
        let history = s.index().history(&docs.join("doc.txt")).unwrap();
        let newest = history.last().unwrap().content.clone();
        let oldest = history.first().unwrap().content.clone();

        let db = rusqlite::Connection::open(s.root().join("index.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TRIGGER block_prune_record BEFORE DELETE ON versions \
             BEGIN SELECT RAISE(ABORT, 'disk is full'); END;",
        )
        .unwrap();

        let now = time::now_ns() + 1000 * 86_400 * time::SECOND;
        // The record write fails, but the objects were already deleted.
        assert!(matches!(s.prune(&policy(), now), Err(Error::Db(_))));
        assert!(s.store().read_file(&newest, &mut Vec::new()).is_ok());
        assert!(matches!(
            s.store().read_file(&oldest, &mut Vec::new()),
            Err(Error::NotFound(_))
        ));
    }

    /// The guard's planning reads and the object deletes that free space must not
    /// need to write anything next to the index (doc 20, S7): a full disk leaves no
    /// room for exactly that write, so the guard has to be able to read the index and
    /// free objects without it. A read-only store root stands in for that full disk,
    /// portably: the index can be read but nothing can be created next to it, while
    /// the object folders stay writable so the deletes still run. The record write is
    /// then refused, but the guard must still report the space it recovered instead
    /// of failing the boot.
    #[test]
    fn frees_space_when_the_index_directory_cannot_be_written() {
        let base = scratch("read-only-root");
        let docs = base.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let mut s = Strata::open(base.join("store")).unwrap();
        several_versions(&mut s, &docs);
        let history = s.index().history(&docs.join("doc.txt")).unwrap();
        let newest = history.last().unwrap().content.clone();
        let oldest = history.first().unwrap().content.clone();
        assert_ne!(oldest, newest);

        // Read-only root: as on a full disk, nothing can be created next to the
        // index. The object folders stay writable, so the deletes still run.
        fs::set_permissions(s.root(), fs::Permissions::from_mode(0o555)).unwrap();

        let now = time::now_ns() + 1000 * 86_400 * time::SECOND;
        let report = guard(&mut s, 100, &policy(), now)
            .unwrap()
            .expect("a floor of 100 is below any filesystem, so the guard must run");
        // Restore the root before asserting, so a failure leaves the scratch removable.
        fs::set_permissions(s.root(), fs::Permissions::from_mode(0o755)).unwrap();

        // The space was recovered even though the index write was refused.
        assert!(report.objects_removed > 0, "{report:#?}");
        assert!(report.bytes_freed > 0, "{report:#?}");
        assert_eq!(
            report.versions_removed, 0,
            "the record was refused, so no index row was removed: {report:#?}"
        );

        // The newest version still restores; the pruned versions' objects are gone.
        assert!(s.store().read_file(&newest, &mut Vec::new()).is_ok());
        assert!(matches!(
            s.store().read_file(&oldest, &mut Vec::new()),
            Err(Error::NotFound(_))
        ));
    }
}
