//! The `storage.retention` applier: the policy file Strata prunes by.
//!
//! The retention policy is three tiers (keep every recent version, then one per hour, then
//! one per day). `morphd` writes the current generation's value to
//! `/persist/morph/strata-retention.json`; `strata prune` (with no flags) reads that file, so
//! the Manifest's `storage.retention` is the one place the policy is decided. The file is the
//! interface: the applier never calls Strata, and Strata never reads the Manifest.

use std::path::PathBuf;

use crate::Applier;

/// The three retention tiers, mirroring the Manifest's `storage.retention`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Retention {
    pub all_versions_hours: u32,
    pub hourly_days: u32,
    pub daily_days: u32,
}

/// The retention applier: writes the policy file Strata's `prune` reads when given no flags.
pub struct RetentionPolicy {
    file: PathBuf,
}

impl RetentionPolicy {
    /// The real policy file, on the data partition.
    pub fn new() -> Self {
        Self {
            file: PathBuf::from("/persist/morph/strata-retention.json"),
        }
    }

    /// Over an arbitrary file (a test).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { file: path.into() }
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl Applier for RetentionPolicy {
    type Value = Retention;

    fn apply(&self, value: &Retention) -> Result<(), String> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| format!("serialising the retention policy: {e}"))?;
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(&self.file, format!("{json}\n"))
            .map_err(|e| format!("writing {}: {e}", self.file.display()))
    }

    fn read_back(&self) -> Result<Retention, String> {
        let text = std::fs::read_to_string(&self.file)
            .map_err(|e| format!("reading {}: {e}", self.file.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a retention policy: {e}", self.file.display()))
    }

    fn undo(&self, previous: &Retention) -> Result<(), String> {
        self.apply(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn scratch() -> PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("morphd-retention-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn policy() -> Retention {
        Retention {
            all_versions_hours: 24,
            hourly_days: 30,
            daily_days: 365,
        }
    }

    #[test]
    fn apply_writes_the_policy_and_reads_it_back() {
        let applier = RetentionPolicy::at(scratch().join("morph/strata-retention.json"));
        applier.apply(&policy()).unwrap();
        assert_eq!(applier.read_back().unwrap(), policy());
    }

    #[test]
    fn undo_restores_the_previous_policy() {
        let applier = RetentionPolicy::at(scratch().join("morph/strata-retention.json"));
        let old = policy();
        applier.apply(&old).unwrap();
        let new = Retention {
            hourly_days: 7,
            ..policy()
        };
        applier.apply(&new).unwrap();
        applier.undo(&old).unwrap();
        assert_eq!(applier.read_back().unwrap(), old);
    }

    #[test]
    fn a_policy_file_whose_parent_is_a_file_is_refused() {
        // The parent of the policy file is a regular file, so `create_dir_all` refuses — a
        // real reason, the same shape as a hostname the kernel cannot take.
        let base = scratch();
        std::fs::write(base.join("blocker"), "a file, not a directory").unwrap();
        let applier = RetentionPolicy::at(base.join("blocker/strata-retention.json"));
        assert!(applier.apply(&policy()).is_err());
    }
}
