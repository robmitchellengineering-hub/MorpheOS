//! The version index: one row per saved state of a path.
//!
//! Paths are stored as raw bytes (Linux file names need not be UTF-8), which
//! also makes "everything under this folder" a byte-range query: every path
//! below `/a/b` sorts between `/a/b/` and `/a/b0` (`'0'` is the byte after `'/'`).

use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Version {
    pub id: i64,
    #[serde(serialize_with = "path_lossy")]
    pub path: PathBuf,
    pub ts: i64,
    pub kind: Kind,
    pub size: u64,
    pub mode: u32,
    /// Object hash of the content (empty for a deletion).
    pub content: String,
    pub entropy: f64,
}

fn path_lossy<S: serde::Serializer>(p: &Path, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&p.to_string_lossy())
}

impl Version {
    /// True when `other` describes the same state (ignoring when it was seen).
    pub fn same_state(&self, kind: Kind, content: &str, mode: u32) -> bool {
        self.kind == kind && self.content == content && (kind == Kind::Deleted || self.mode == mode)
    }
}

/// The state of a path at one moment: what [`Index::record`] stores.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub kind: Kind,
    pub size: u64,
    pub mode: u32,
    pub content: String,
    pub entropy: f64,
}

impl State {
    pub fn file(size: u64, mode: u32, content: &str, entropy: f64) -> Self {
        State {
            kind: Kind::File,
            size,
            mode,
            content: content.into(),
            entropy,
        }
    }

    pub fn deleted() -> Self {
        State {
            kind: Kind::Deleted,
            size: 0,
            mode: 0,
            content: String::new(),
            entropy: 0.0,
        }
    }
}

pub struct Index {
    db: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS versions (
    id      INTEGER PRIMARY KEY,
    path    BLOB    NOT NULL,
    ts      INTEGER NOT NULL,
    kind    TEXT    NOT NULL CHECK (kind IN ('file', 'deleted')),
    size    INTEGER NOT NULL,
    mode    INTEGER NOT NULL,
    content TEXT    NOT NULL,
    entropy REAL    NOT NULL
);
CREATE INDEX IF NOT EXISTS versions_path_ts ON versions (path, ts);
";

fn bytes(p: &Path) -> &[u8] {
    p.as_os_str().as_bytes()
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Version> {
    let path: Vec<u8> = r.get(1)?;
    let kind: String = r.get(3)?;
    Ok(Version {
        id: r.get(0)?,
        path: PathBuf::from(std::ffi::OsStr::from_bytes(&path)),
        ts: r.get(2)?,
        kind: if kind == "file" {
            Kind::File
        } else {
            Kind::Deleted
        },
        size: r.get::<_, i64>(4)? as u64,
        mode: r.get::<_, i64>(5)? as u32,
        content: r.get(6)?,
        entropy: r.get(7)?,
    })
}

const COLS: &str = "id, path, ts, kind, size, mode, content, entropy";

impl Index {
    pub fn open(file: &Path) -> Result<Self> {
        let db = Connection::open(file)?;
        // WAL lets the watcher write while the CLI reads; the timeout makes a
        // concurrent writer wait instead of failing.
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        db.execute_batch(SCHEMA)?;
        Ok(Index { db })
    }

    pub fn latest(&self, path: &Path) -> Result<Option<Version>> {
        Ok(self
            .db
            .query_row(
                &format!(
                    "SELECT {COLS} FROM versions WHERE path = ?1 ORDER BY ts DESC, id DESC LIMIT 1"
                ),
                params![bytes(path)],
                row,
            )
            .optional()?)
    }

    /// The state `path` was in at time `at`: the newest version not after it.
    pub fn at(&self, path: &Path, at: i64) -> Result<Option<Version>> {
        Ok(self
            .db
            .query_row(
                &format!(
                    "SELECT {COLS} FROM versions WHERE path = ?1 AND ts <= ?2 ORDER BY ts DESC, id DESC LIMIT 1"
                ),
                params![bytes(path), at],
                row,
            )
            .optional()?)
    }

    pub fn get(&self, id: i64) -> Result<Option<Version>> {
        Ok(self
            .db
            .query_row(
                &format!("SELECT {COLS} FROM versions WHERE id = ?1"),
                params![id],
                row,
            )
            .optional()?)
    }

    /// Record a state, unless it is the state already recorded last.
    /// Returns the new version, or `None` when nothing changed.
    pub fn record(&self, path: &Path, ts: i64, state: &State) -> Result<Option<Version>> {
        if let Some(last) = self.latest(path)?
            && last.same_state(state.kind, &state.content, state.mode)
        {
            return Ok(None);
        }
        let kind_s = match state.kind {
            Kind::File => "file",
            Kind::Deleted => "deleted",
        };
        self.db.execute(
            "INSERT INTO versions (path, ts, kind, size, mode, content, entropy) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                bytes(path),
                ts,
                kind_s,
                state.size as i64,
                state.mode as i64,
                state.content,
                state.entropy
            ],
        )?;
        self.get(self.db.last_insert_rowid())
    }

    pub fn history(&self, path: &Path) -> Result<Vec<Version>> {
        let mut st = self.db.prepare(&format!(
            "SELECT {COLS} FROM versions WHERE path = ?1 ORDER BY ts, id"
        ))?;
        let rows = st.query_map(params![bytes(path)], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every distinct path ever recorded at or below `dir`.
    pub fn paths_under(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut lo = bytes(dir).to_vec();
        if lo.last() != Some(&b'/') {
            lo.push(b'/');
        }
        let mut hi = lo.clone();
        *hi.last_mut().expect("non-empty") = b'0';
        let mut st = self.db.prepare(
            "SELECT DISTINCT path FROM versions WHERE path >= ?1 AND path < ?2 ORDER BY path",
        )?;
        let rows = st.query_map(params![lo, hi], |r| r.get::<_, Vec<u8>>(0))?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|b| PathBuf::from(std::ffi::OsStr::from_bytes(&b)))
            .collect())
    }

    /// (id, path, ts) of every version, for retention decisions.
    pub fn all_versions(&self) -> Result<Vec<(i64, Vec<u8>, i64)>> {
        let mut st = self
            .db
            .prepare("SELECT id, path, ts FROM versions ORDER BY path, ts, id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete(&mut self, ids: &[i64]) -> Result<usize> {
        let tx = self.db.transaction()?;
        let mut n = 0;
        {
            let mut st = tx.prepare("DELETE FROM versions WHERE id = ?1")?;
            for id in ids {
                n += st.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Every content hash some version still refers to.
    pub fn referenced_contents(&self) -> Result<std::collections::HashSet<String>> {
        let mut st = self
            .db
            .prepare("SELECT DISTINCT content FROM versions WHERE kind = 'file'")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn counts(&self) -> Result<(u64, u64)> {
        Ok(self.db.query_row(
            "SELECT COUNT(*), COUNT(DISTINCT path) FROM versions",
            [],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(name: &str) -> Index {
        let p = std::env::temp_dir().join(format!("strata-index-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        Index::open(&p).unwrap()
    }

    #[test]
    fn records_skip_unchanged_states() {
        let i = idx("dedup");
        let p = Path::new("/d/a.txt");
        assert!(
            i.record(p, 1, &State::file(3, 0o644, "h1", 4.0))
                .unwrap()
                .is_some()
        );
        assert!(
            i.record(p, 2, &State::file(3, 0o644, "h1", 4.0))
                .unwrap()
                .is_none()
        );
        assert!(
            i.record(p, 3, &State::file(3, 0o600, "h1", 4.0))
                .unwrap()
                .is_some(),
            "mode change"
        );
        assert!(i.record(p, 4, &State::deleted()).unwrap().is_some());
        assert!(i.record(p, 5, &State::deleted()).unwrap().is_none());
        assert_eq!(i.history(p).unwrap().len(), 3);
    }

    #[test]
    fn at_finds_the_state_at_a_moment() {
        let i = idx("at");
        let p = Path::new("/d/a.txt");
        i.record(p, 10, &State::file(1, 0o644, "v1", 4.0)).unwrap();
        i.record(p, 20, &State::file(1, 0o644, "v2", 4.0)).unwrap();
        assert!(i.at(p, 5).unwrap().is_none());
        assert_eq!(i.at(p, 10).unwrap().unwrap().content, "v1");
        assert_eq!(i.at(p, 19).unwrap().unwrap().content, "v1");
        assert_eq!(i.at(p, 99).unwrap().unwrap().content, "v2");
    }

    #[test]
    fn paths_under_is_exact_about_folder_boundaries() {
        let i = idx("under");
        for p in ["/d/a", "/d/sub/b", "/d-other/c", "/d0/e", "/dx", "/d"] {
            i.record(Path::new(p), 1, &State::file(1, 0o644, p, 4.0))
                .unwrap();
        }
        let got: Vec<_> = i.paths_under(Path::new("/d")).unwrap();
        assert_eq!(got, [PathBuf::from("/d/a"), PathBuf::from("/d/sub/b")]);
        assert_eq!(i.paths_under(Path::new("/d/")).unwrap().len(), 2);
    }

    #[test]
    fn non_utf8_names_survive() {
        let i = idx("bytes");
        let raw = std::ffi::OsStr::from_bytes(b"/d/caf\xe9.txt");
        i.record(Path::new(raw), 1, &State::file(1, 0o644, "x", 4.0))
            .unwrap();
        assert_eq!(
            i.paths_under(Path::new("/d")).unwrap(),
            [PathBuf::from(raw)]
        );
    }
}
