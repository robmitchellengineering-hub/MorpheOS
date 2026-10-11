//! A generation **store**: the previous Manifests, and the way back to one.
//!
//! [`crate::apply`] writes the generation it is given and reports what it did. That is
//! enough to make a risk tier refuse something, and it is not enough to make the refusal
//! *recoverable*: an apply that overwrites the only copy of a machine's configuration has
//! destroyed the thing a person would want back.
//!
//! So a store keeps every generation it has been given, and `current` names one of them.
//! Three properties are the whole point, and each has a test that can fail:
//!
//! 1. **A refused apply leaves the store exactly as it was.** No generation file, no meta
//!    file, no change to `current`. The refusal is worth nothing if the store moves anyway.
//! 2. **`current` only ever names a complete generation.** A generation is written to a
//!    temporary name and renamed into place, and `current` moves by renaming a symlink over
//!    it — so an interruption leaves the previous generation current rather than pointing a
//!    reader at half a file.
//! 3. **Rollback cannot lose the present.** Rolling back *points* `current` at an older
//!    generation; it never deletes one. So a rollback to the wrong generation is itself
//!    undone by another rollback, which is the only property that makes rollback usable on
//!    a machine somebody is relying on.
//!
//! **What this is not.** It stores *Manifests* — configuration generations, a few kilobytes
//! each. Rolling one back changes which configuration is current; it does not roll back an
//! operating system, and nothing here boots anything. ADR 0009's rung **H3** rolls back to
//! the last known-good *system* generation and is built as boot policy (the A/B trial boot):
//! that is the other axis, and neither substitutes for the other. What a store gives a heal
//! ladder is somewhere a *configuration* can be put back — which today nothing does.
//!
//! **Deliberately not here: pruning.** Generation files are small text, so a store that
//! grows is a directory listing rather than a full disk, and a `--keep N` that silently
//! removed the generation somebody needed would be the one mistake this module exists to
//! prevent. Add it when a generation is big enough for the size to matter.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Applied, Error, Risk, apply};

/// A generation, as recorded beside it.
///
/// This is the meta file's shape as well as the return value: the number, how risky the
/// change was, how many paths it touched, and who took responsibility if anyone had to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generation {
    pub number: u64,
    pub risk: Risk,
    pub changes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
}

/// **A person's mark on a change** ([ADR 0012]). Written beside the generation it authorises
/// as `<number>.rob`, and read back by the applier.
///
/// The extension is the point: this is the machine's own record that a person, not the AI,
/// took responsibility for a change — so "what has somebody signed here?" is a directory
/// listing rather than an audit of a log.
///
/// It is a record of **consent**, not a cryptographic signature. Anyone who can write the
/// store can forge it, which is one more reason the store is single-writer by contract.
///
/// [ADR 0012]: ../../docs/adr/0012-the-name-in-the-machine.md
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    /// The generation this marks.
    pub generation: u64,
    /// Who took responsibility.
    pub approved_by: String,
    /// How risky the change they signed was.
    pub risk: Risk,
    /// How many paths it touched.
    pub changes: usize,
    /// When, in seconds since the Unix epoch. No clock dependency: the store has none.
    pub approved_at_unix: u64,
}

/// A directory of generations, with `current` naming the one in force.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Name a store. **This does no I/O and creates nothing** — a store that has never been
    /// applied to is an empty directory, not a defect, and `rollback` on one says so.
    pub fn open(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn generations(&self) -> PathBuf {
        self.root.join("generations")
    }

    fn current_link(&self) -> PathBuf {
        self.root.join("current")
    }

    /// Where generation `number`'s manifest lives. Zero-padded so that the directory
    /// listing sorts in the order the generations were made.
    pub fn path_of(&self, number: u64) -> PathBuf {
        self.generations().join(format!("{number:04}.json"))
    }

    fn meta_of(&self, number: u64) -> PathBuf {
        self.generations().join(format!("{number:04}.meta.json"))
    }

    /// Where generation `number`'s mark lives, when a person made one.
    ///
    /// `.rob` and not `.json`, deliberately: the name belongs to the machine, and a file
    /// the OS writes about a person should say whose it is.
    pub fn mark_of(&self, number: u64) -> PathBuf {
        self.generations().join(format!("{number:04}.rob"))
    }

    /// The mark on a generation, or `None` when no person approved it.
    ///
    /// A mark that exists but cannot be read is **an error naming it** rather than `None`:
    /// the only way that file exists is somebody having written it, and a mark that quietly
    /// reads as absent is worse than one that reads as broken.
    pub fn mark(&self, number: u64) -> Result<Option<Mark>, Error> {
        let path = self.mark_of(number);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
        let mark: Mark = serde_json::from_str(&text).map_err(|e| {
            Error::Invalid(format!("{} is not a readable mark: {e}", path.display()))
        })?;
        Ok(Some(mark))
    }

    /// Every mark the store holds, oldest generation first.
    pub fn marks(&self) -> Result<Vec<Mark>, Error> {
        let mut marks = Vec::new();
        for generation in self.history()? {
            if let Some(mark) = self.mark(generation.number)? {
                marks.push(mark);
            }
        }
        Ok(marks)
    }

    fn write_mark(&self, generation: &Generation) -> Result<(), Error> {
        let mark = Mark {
            generation: generation.number,
            approved_by: generation.approved_by.clone().unwrap_or_default(),
            risk: generation.risk,
            changes: generation.changes,
            approved_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let bytes = serde_json::to_vec_pretty(&mark)
            .map_err(|e| Error::Invalid(format!("the mark does not serialise: {e}")))?;
        write_atomic(&self.mark_of(generation.number), &bytes)
    }

    /// Every generation, oldest first.
    ///
    /// A file without a readable meta record is **an error naming it**, not a generation to
    /// skip quietly: the only way that file exists is a store somebody has edited, and a
    /// history with a hole in it is worse than no history.
    pub fn history(&self) -> Result<Vec<Generation>, Error> {
        let dir = self.generations();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut numbers: Vec<u64> = Vec::new();
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| Error::Invalid(format!("{}: {e}", dir.display())))?
        {
            let entry = entry.map_err(|e| Error::Invalid(format!("{}: {e}", dir.display())))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".meta.json")
                && let Ok(number) = stem.parse::<u64>()
            {
                numbers.push(number);
            }
        }
        numbers.sort_unstable();
        numbers.into_iter().map(|n| self.meta(n)).collect()
    }

    fn meta(&self, number: u64) -> Result<Generation, Error> {
        let path = self.meta_of(number);
        let bytes = std::fs::read(&path)
            .map_err(|e| Error::Invalid(format!("generation {number}: {}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| {
            Error::Invalid(format!(
                "generation {number}: {} is not readable: {e}",
                path.display()
            ))
        })
    }

    /// The generation `current` names, or `None` when the store has never been applied to.
    pub fn current(&self) -> Result<Option<Generation>, Error> {
        let link = self.current_link();
        let target = match std::fs::read_link(&link) {
            Ok(target) => target,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Invalid(format!("{}: {e}", link.display()))),
        };
        // A dangling `current` is a defect, not an empty store: something removed the
        // generation that was in force, and saying "nothing is current" would hide it.
        let resolved = self.root.join(&target);
        if !resolved.is_file() {
            return Err(Error::Invalid(format!(
                "{} names {target:?}, which is not a file — the generation in force is missing",
                link.display()
            )));
        }
        let name = target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let number = name
            .strip_suffix(".json")
            .and_then(|stem| stem.parse::<u64>().ok())
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "{} names {target:?}, which is not a generation",
                    link.display()
                ))
            })?;
        self.meta(number).map(Some)
    }

    /// Read one generation's record without reading its Manifest.
    ///
    /// The transaction layer needs this after a commit: it has to report *what* is now in
    /// force (tier, how many paths, who approved it) without re-reading a document it did not
    /// write.
    pub fn generation(&self, number: u64) -> Result<Generation, Error> {
        self.meta(number)
    }

    /// Read a stored generation's manifest.
    pub fn load(&self, number: u64) -> Result<Value, Error> {
        let path = self.path_of(number);
        let bytes = std::fs::read(&path)
            .map_err(|e| Error::Invalid(format!("generation {number}: {}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| {
            Error::Invalid(format!(
                "generation {number}: {} is not readable: {e}",
                path.display()
            ))
        })
    }

    /// Validate `new`, refuse it if its tier demands a person, store it, and make it current.
    ///
    /// Every refusal is [`crate::apply`]'s, unchanged — this adds a place to keep the
    /// generation, not a second opinion about whether it may be applied. **The store is
    /// written only after the refusal has not happened**, which is what makes "nothing was
    /// written" true of the whole store rather than only of one file.
    ///
    /// A plan is **a move from the generation in force**, not a document to file: if `old` is
    /// not what the store currently holds, the plan was written against a machine that has
    /// since moved on, and applying it would replace a generation it never saw. That is a
    /// refusal, not a defect — a person edited the Manifest by hand, or two appliers ran.
    ///
    /// **Single-writer, by contract.** Two applies that both find current = 3 would both
    /// choose generation 4, and one would overwrite the other's file. A lock that a crash
    /// could leave behind would be worse — it would block every later apply forever — so the
    /// constraint is stated here instead of half-enforced.
    pub fn apply(
        &self,
        old: &Value,
        new: &Value,
        approved_by: Option<&str>,
    ) -> Result<Generation, Error> {
        if let Some(current) = self.current()? {
            let in_force = self.load(current.number)?;
            if &in_force != old {
                return Err(Error::Refused(format!(
                    "generation {} is in force and this plan starts from a different manifest, \
                     so applying it would replace a generation it was not written for. \
                     **Nothing was written.** Re-plan against generation {}.",
                    current.number, current.number
                )));
            }
        }

        let number = self.next_number()?;
        let dir = self.generations();
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::Invalid(format!("{}: {e}", dir.display())))?;

        // The generation itself, then its record, then `current`. Order matters at every
        // step: a crash can leave an unreferenced generation (harmless, and visible in the
        // history) but cannot leave `current` pointing at something unfinished.
        let applied: Applied = apply(old, new, &self.path_of(number), approved_by)?;

        let generation = Generation {
            number,
            risk: applied.risk,
            changes: applied.changes,
            approved_by: applied.approved_by,
        };
        let meta = serde_json::to_vec_pretty(&generation).map_err(|e| {
            Error::Invalid(format!("the generation record does not serialise: {e}"))
        })?;
        write_atomic(&self.meta_of(number), &meta)?;

        // THE PERSON'S MARK (ADR 0012). Written after the record and before `current`, for
        // the same reason as every other step in this order: a crash can leave an UNSIGNED
        // generation, which is honest, but never a signed one that does not exist.
        //
        // Only a change a person took responsibility for gets a mark. That is what makes
        // the absence of one mean something: `ls generations/*.rob` is the list of changes
        // somebody stood behind.
        if generation.approved_by.is_some() {
            self.write_mark(&generation)?;
        }

        sync_dir(&dir);
        self.point_current_at(number)?;
        Ok(generation)
    }

    /// Make an **existing** generation the current one, without changing anything else.
    ///
    /// `--to` a number is how a rollback is undone: rolling back is a pointer move in both
    /// directions, so the generation that was in force is still there to go back to.
    pub fn rollback(&self, to: Option<u64>) -> Result<Generation, Error> {
        let current = self.current()?;
        let history = self.history()?;
        if history.is_empty() {
            return Err(Error::Refused(
                "the store has no generations, so there is nothing to roll back to".into(),
            ));
        }

        let target = match to {
            Some(number) => {
                if !history.iter().any(|g| g.number == number) {
                    return Err(Error::Refused(format!(
                        "generation {number} is not in the store; the history is {}",
                        numbers(&history)
                    )));
                }
                number
            }
            None => {
                let from = current.as_ref().map(|g| g.number).unwrap_or_default();
                let mut older: Vec<u64> = history
                    .iter()
                    .map(|g| g.number)
                    .filter(|n| *n < from)
                    .collect();
                match older.pop() {
                    Some(number) => number,
                    None => {
                        return Err(Error::Refused(format!(
                            "generation {from} is the oldest in the store, so there is nothing \
                             to roll back to; the history is {}",
                            numbers(&history)
                        )));
                    }
                }
            }
        };

        self.point_current_at(target)?;
        self.meta(target)
    }

    /// The number the next generation will be given.
    ///
    /// Public because it is **not** `current + 1`: generations are append-only and a rollback
    /// moves `current` backwards, so after rolling back to 1 in a store that holds 1 and 2, the
    /// next generation is 3. A caller that assumes otherwise arms a trial for a number the
    /// store will not use, which is how the transaction layer found this.
    pub fn next_number(&self) -> Result<u64, Error> {
        Ok(self.history()?.last().map(|g| g.number + 1).unwrap_or(1))
    }

    /// Point `current` at a generation by renaming a link over it, so a reader either sees
    /// the old generation or the new one and never a missing or half-written link.
    fn point_current_at(&self, number: u64) -> Result<(), Error> {
        let link = self.current_link();
        let tmp = self.root.join("current.morph-new");
        let _ = std::fs::remove_file(&tmp);
        std::os::unix::fs::symlink(format!("generations/{:04}.json", number), &tmp)
            .map_err(|e| Error::Invalid(format!("{}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, &link)
            .map_err(|e| Error::Invalid(format!("{}: {e}", link.display())))?;
        sync_dir(&self.root);
        Ok(())
    }
}

fn numbers(history: &[Generation]) -> String {
    history
        .iter()
        .map(|g| g.number.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Write a file that is either fully there or not there at all.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".morph-new");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| Error::Invalid(format!("{}: {e}", tmp.display())))?;
    if let Ok(f) = std::fs::File::open(&tmp) {
        let _ = f.sync_all();
    }
    std::fs::rename(&tmp, path).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))
}

/// Flush a directory's entries, so a rename is durable and not only in the cache.
///
/// Best-effort on purpose: the store's guarantees are about *which* file a reader finds, and
/// a platform that cannot fsync a directory should not turn a completed apply into an error.
/// The link is still created atomically either way.
fn sync_dir(path: &Path) {
    if let Ok(dir) = std::fs::File::open(path) {
        let _ = dir.sync_all();
    }
}
