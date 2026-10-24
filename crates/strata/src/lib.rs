//! Strata v0: every saved version of every file, and a way back to any of them.
//!
//! This is the phase-1 slice of doc 04, built to make "you never lose a file"
//! true before the rest of Strata exists:
//!
//! - [`store`]: a content-addressed object store. Files are cut into
//!   content-defined chunks (FastCDC), named by their BLAKE3 hash, and stored
//!   zstd-compressed, so unchanged parts of a file, and identical files, are
//!   stored once however many versions exist.
//! - [`index`]: the version history. One row per saved state of a path, including
//!   deletions, in SQLite.
//! - [`Strata`]: snapshot, history, restore, **rewind** a folder to a moment, and
//!   retention pruning. Rewind never destroys anything: the current state is
//!   snapshotted first, and files that did not exist at that moment are moved to
//!   a quarantine folder, not deleted. A rewind can itself be undone.
//! - [`watch`]: an inotify watcher that snapshots files as they are saved, moved
//!   and deleted, and feeds [`detect`], which spots ransomware-like bursts.
//! - [`guard`]: the space guard — when the store's filesystem falls below the
//!   Manifest's `storage.min_free_percent`, thin out old versions by the retention
//!   policy so a full disk cannot break the machine (doc 20, S7).
//!
//! v0 runs on top of any Linux filesystem. Its store must live outside the
//! folders it watches and be writable only by the service (doc 09): software
//! that can reach your files must not be able to reach their history.

pub mod detect;
pub mod guard;
pub mod index;
pub mod retention;
pub mod store;
pub mod time;
pub mod watch;

mod strata;

pub use strata::{RewindReport, Strata};

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    Db(rusqlite::Error),
    /// Stored data does not match its hash: the store is damaged.
    Corrupt(String),
    NotFound(String),
    Refused(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::Db(e) => write!(f, "version index: {e}"),
            Error::Corrupt(m) => write!(f, "store is damaged: {m}"),
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}
