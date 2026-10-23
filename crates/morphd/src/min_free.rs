//! The `storage.min_free_percent` applier: the floor Strata's space guard prunes by.
//!
//! The Manifest promises the disk never fills completely: when free space falls below
//! `storage.min_free_percent`, Strata thins out old versions (doc 20, S7). `morphd` writes
//! the current generation's floor to `/persist/morph/strata-min-free.json`; `strata guard`
//! reads that file, so the Manifest is the one place the floor is decided. The file is the
//! interface: the applier never calls Strata, and Strata never reads the Manifest.

use std::path::PathBuf;

use crate::Applier;

/// The free-space floor, mirroring the Manifest's `storage.min_free_percent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MinFreePercent {
    pub min_free_percent: u8,
}

/// The min-free applier: writes the floor file Strata's `guard` reads.
pub struct MinFreePercentPolicy {
    file: PathBuf,
}

impl MinFreePercentPolicy {
    /// The real floor file, on the data partition.
    pub fn new() -> Self {
        Self {
            file: PathBuf::from("/persist/morph/strata-min-free.json"),
        }
    }

    /// Over an arbitrary file (a test).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { file: path.into() }
    }
}

impl Default for MinFreePercentPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl Applier for MinFreePercentPolicy {
    type Value = MinFreePercent;

    fn apply(&self, value: &MinFreePercent) -> Result<(), String> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| format!("serialising the free-space floor: {e}"))?;
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(&self.file, format!("{json}\n"))
            .map_err(|e| format!("writing {}: {e}", self.file.display()))
    }

    fn read_back(&self) -> Result<MinFreePercent, String> {
        let text = std::fs::read_to_string(&self.file)
            .map_err(|e| format!("reading {}: {e}", self.file.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a free-space floor: {e}", self.file.display()))
    }

    fn undo(&self, previous: &MinFreePercent) -> Result<(), String> {
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
        let p = std::env::temp_dir().join(format!("morphd-min-free-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn apply_writes_the_floor_and_reads_it_back() {
        let applier = MinFreePercentPolicy::at(scratch().join("morph/strata-min-free.json"));
        let floor = MinFreePercent {
            min_free_percent: 15,
        };
        applier.apply(&floor).unwrap();
        assert_eq!(applier.read_back().unwrap(), floor);
    }

    #[test]
    fn undo_restores_the_previous_floor() {
        let applier = MinFreePercentPolicy::at(scratch().join("morph/strata-min-free.json"));
        let old = MinFreePercent {
            min_free_percent: 10,
        };
        applier.apply(&old).unwrap();
        let new = MinFreePercent {
            min_free_percent: 15,
        };
        applier.apply(&new).unwrap();
        applier.undo(&old).unwrap();
        assert_eq!(applier.read_back().unwrap(), old);
    }

    #[test]
    fn a_floor_file_whose_parent_is_a_file_is_refused() {
        let base = scratch();
        std::fs::write(base.join("blocker"), "a file, not a directory").unwrap();
        let applier = MinFreePercentPolicy::at(base.join("blocker/strata-min-free.json"));
        assert!(
            applier
                .apply(&MinFreePercent {
                    min_free_percent: 15
                })
                .is_err()
        );
    }
}
