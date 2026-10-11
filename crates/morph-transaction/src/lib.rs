//! A **transaction**: `plan → validate → snapshot → apply → health-check → commit | rollback`.
//!
//! That line is doc 03's description of how every state change happens in MorpheOS, and this
//! crate is the part of it that can be built and proven before the machine consumes the
//! Manifest at all. It is the difference between applying a generation and *being allowed to
//! keep* it:
//!
//! - [`crate::Store`] (in `morph-manifest`) can already say no before anything is written, and
//!   can keep the generations it has been given. What it cannot do is decide that a change it
//!   *accepted* should be taken back — that needs a health check, and a health check needs
//!   something running to check.
//! - So a transaction **arms a trial before it applies**, and clears the trial only on commit.
//!   If the health check fails, or the process dies between the two, the generation in force
//!   goes back to the one that was there before. This is the boot path's rule — a bad update
//!   is booted at most once (ADR 0008) — carried to configuration.
//!
//! Three properties, and each has a test that can fail:
//!
//! 1. **A trial that is not committed is not kept.** Health check fails → the previous
//!    generation is current again, and the failed generation is still in the history, so the
//!    decision is visible rather than erased.
//! 2. **An interrupted transaction is resolved, not ignored.** The marker survives the process
//!    that wrote it, so the next transaction rolls it back before doing anything else. Nothing
//!    waits for a human to notice.
//! 3. **A health check that did not run is not a pass.** An empty command is an error, not a
//!    reason to commit: the entire decision here is the check, and a transaction that cannot
//!    fail is just an apply with extra steps.
//!
//! **What this is not.** It is not `morphd`. Doc 03 describes PID 1, a service graph, boot I/O
//! replay and a settings store; this is the transaction state machine alone, and it changes
//! *which Manifest generation is current*. Nothing on the machine reads the Manifest yet (H5
//! in `AGENTS.md` lists the fields no code consumes), so the health check is supplied by the
//! caller rather than derived from the Manifest. When something does consume it, the check
//! becomes this daemon's job — the ordering and the rollback do not change.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use morph_manifest::{Generation, Store};

/// How long a health check is given before it counts as a failure.
///
/// A check with no bound is the shape of incident H11: one command that never returns and a
/// machine that waits for a verdict that cannot come. The check is *given* time, not trusted
/// to end.
pub const DEFAULT_HEALTH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub enum Error {
    /// The Manifest layer's own answer, passed through unchanged so that a refusal is a
    /// refusal in either binary: exit 1 means a decision, exit 2 a defect.
    Manifest(morph_manifest::Error),
    /// A decision by this layer: there is no trial to commit, or nothing to fall back to.
    Refused(String),
    /// A defect: a marker that cannot be read, a store that cannot be written.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Manifest(e) => write!(f, "{e}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
            Error::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl Error {
    /// The message without the `refused:` prefix, for a caller that prints its own marker.
    /// Without this the CLI says `REFUSED: refused: ...`, which reads like two decisions.
    pub fn reason(&self) -> String {
        match self {
            Error::Refused(m) => m.clone(),
            Error::Manifest(morph_manifest::Error::Refused(m)) => m.clone(),
            other => other.to_string(),
        }
    }
}

impl From<morph_manifest::Error> for Error {
    fn from(e: morph_manifest::Error) -> Error {
        Error::Manifest(e)
    }
}

/// True when the error is a decision rather than a defect — the exit-code contract, in one
/// place so both binaries agree about it.
pub fn is_refusal(error: &Error) -> bool {
    matches!(
        error,
        Error::Refused(_) | Error::Manifest(morph_manifest::Error::Refused(_))
    )
}

/// A trial in progress: the generation to go back to if this one is not committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    /// The generation that was in force when the trial was armed.
    pub from: u64,
    /// The generation the trial put in force.
    pub to: u64,
}

/// A command that answers "is this machine still healthy?" by exiting zero.
#[derive(Debug, Clone)]
pub struct HealthCheck {
    command: String,
    timeout: Duration,
}

impl HealthCheck {
    pub fn new(command: impl Into<String>) -> HealthCheck {
        HealthCheck {
            command: command.into(),
            timeout: DEFAULT_HEALTH_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> HealthCheck {
        self.timeout = timeout;
        self
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    /// Run the check. `Ok(())` means the machine said yes; `Err(reason)` is the reason to roll
    /// back, phrased for the log rather than for a stack trace.
    pub fn run(&self) -> Result<(), String> {
        if self.command.trim().is_empty() {
            // A check that never ran is not a pass. Committing here would mean the
            // transaction had no way to fail, which is an apply with extra steps.
            return Err("no health check was given, so there is nothing to decide on".into());
        }

        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&self.command)
            .spawn()
            .map_err(|e| format!("could not run the health check: {e}"))?;

        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    return Err(match status.code() {
                        Some(code) => format!("the health check exited {code}"),
                        None => "the health check was killed by a signal".to_string(),
                    });
                }
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "the health check did not finish within {} s",
                        self.timeout.as_secs()
                    ));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("could not wait for the health check: {e}")),
            }
        }
    }
}

/// What a transaction did.
#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    /// The generation in force when it finished.
    pub generation: u64,
    /// The generation that was trialled.
    pub trialled: u64,
    /// The generation it was trialled from.
    pub from: u64,
    pub committed: bool,
    /// Why it was taken back. Present only when it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// An earlier trial that was unresolved and was rolled back before this one started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovered: Option<u64>,
}

/// A transaction over a generation store.
pub struct Transaction {
    store: Store,
}

impl Transaction {
    pub fn open(root: impl Into<PathBuf>) -> Transaction {
        Transaction {
            store: Store::open(root),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Where the "a trial is in progress" marker lives.
    ///
    /// **Beside the generations, not inside them.** An unresolved trial is state *about* the
    /// store, and a marker inside `generations/` would be picked up by the history walk and
    /// read as a generation somebody made.
    pub fn marker(&self) -> PathBuf {
        self.store.root().join(".morph-trial")
    }

    /// The trial in progress, if any. `None` means the generation in force is committed.
    pub fn trial(&self) -> Result<Option<Trial>, Error> {
        let marker = self.marker();
        let bytes = match std::fs::read(&marker) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Invalid(format!("{}: {e}", marker.display()))),
        };
        serde_json::from_slice(&bytes).map(Some).map_err(|e| {
            Error::Invalid(format!(
                "{} is not a trial record: {e} — this is the state an interrupted transaction \
                 leaves, and it must be resolved by hand rather than guessed at",
                marker.display()
            ))
        })
    }

    /// What is in force, and whether it is committed or still on trial.
    pub fn status(&self) -> Result<(Option<Generation>, Option<Trial>), Error> {
        Ok((self.store.current()?, self.trial()?))
    }

    /// Arm a trial and apply `new`.
    ///
    /// **The marker is written before the change, never after.** If this process dies at any
    /// point from here on, the next transaction finds the marker and takes the machine back —
    /// so the failure mode is "a change nobody committed is undone", not "a change nobody
    /// committed is in force".
    ///
    /// Refuses when there is no generation to fall back to: a first generation is not a
    /// transaction, and calling it one would promise a rollback that cannot happen. Establish
    /// it with `morph-manifest apply --store`.
    pub fn begin(
        &self,
        old: &Value,
        new: &Value,
        approved_by: Option<&str>,
    ) -> Result<Trial, Error> {
        if let Some(trial) = self.trial()? {
            return Err(Error::Refused(format!(
                "generation {} is on trial and was never committed (it replaced generation \
                 {}). Nothing new may be trialled until that is resolved: `commit` it, or \
                 `rollback` it — or use `apply`, which rolls it back first.",
                trial.to, trial.from
            )));
        }

        let current = self.store.current()?.ok_or_else(|| {
            Error::Refused(
                "the store has no generation to fall back to, so this cannot be a transaction. \
                 Establish the first generation with `morph-manifest apply --store`."
                    .into(),
            )
        })?;

        // The store's own next number, not `current + 1`: a rollback moves `current`
        // backwards while the generations stay, so those two are not the same thing.
        let to = self.store.next_number()?;
        let trial = Trial {
            from: current.number,
            to,
        };
        write_atomic(
            &self.marker(),
            &serde_json::to_vec(&trial).expect("a trial serialises"),
        )?;

        match self.store.apply(old, new, approved_by) {
            Ok(generation) => {
                // The store's own numbering is the authority; if a generation was added
                // between the read above and the apply, say so rather than record a number
                // that is not the one in force.
                if generation.number != to {
                    return Err(Error::Invalid(format!(
                        "the trial was armed for generation {to} but the store made {}",
                        generation.number
                    )));
                }
                Ok(trial)
            }
            Err(e) => {
                // The change was refused before anything was written. The marker describes a
                // trial that never started, so it goes — otherwise a refused apply would leave
                // a machine that rolls itself back on the next transaction.
                let _ = std::fs::remove_file(self.marker());
                Err(e.into())
            }
        }
    }

    /// Accept the trial. The health check has already passed, or a person has decided the
    /// machine is fine; either way this is the only step that makes the change permanent.
    pub fn commit(&self) -> Result<Generation, Error> {
        let trial = self.trial()?.ok_or_else(|| {
            Error::Refused("there is no trial to commit; nothing is in progress".into())
        })?;
        std::fs::remove_file(self.marker())
            .map_err(|e| Error::Invalid(format!("{}: {e}", self.marker().display())))?;
        Ok(self.store.generation(trial.to)?)
    }

    /// Take the trial back. The generation it replaced is current again, and the trialled one
    /// stays in the history — a change that was taken back is a decision worth keeping.
    pub fn rollback(&self) -> Result<Generation, Error> {
        let trial = self.trial()?.ok_or_else(|| {
            Error::Refused("there is no trial to roll back; nothing is in progress".into())
        })?;
        let restored = self.store.rollback(Some(trial.from))?;
        std::fs::remove_file(self.marker())
            .map_err(|e| Error::Invalid(format!("{}: {e}", self.marker().display())))?;
        Ok(restored)
    }

    /// Resolve a trial left behind by a process that died. `Ok(None)` means there was nothing
    /// to resolve, which is the normal case.
    pub fn recover(&self) -> Result<Option<Generation>, Error> {
        match self.trial()? {
            None => Ok(None),
            Some(_) => self.rollback().map(Some),
        }
    }

    /// The whole thing, in the order doc 03 gives it: recover, plan and validate (the store
    /// refuses what it must), snapshot (the store keeps what was there), apply, health-check,
    /// then commit or roll back.
    pub fn apply(
        &self,
        old: &Value,
        new: &Value,
        approved_by: Option<&str>,
        health: &HealthCheck,
    ) -> Result<Outcome, Error> {
        let recovered = self.recover()?.map(|g| g.number);
        let trial = self.begin(old, new, approved_by)?;
        let checked = health.run();

        match checked {
            Ok(()) => Ok(Outcome {
                generation: self.commit()?.number,
                trialled: trial.to,
                from: trial.from,
                committed: true,
                reason: None,
                recovered,
            }),
            Err(reason) => {
                let restored = self.rollback()?;
                Ok(Outcome {
                    generation: restored.number,
                    trialled: trial.to,
                    from: trial.from,
                    committed: false,
                    reason: Some(reason),
                    recovered,
                })
            }
        }
    }
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
