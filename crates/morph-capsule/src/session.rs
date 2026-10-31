//! A **Session** as the machine's unit: a task, its files, and its capsule — undone together.
//!
//! Doc 03 calls the Session "the core UX abstraction", and doc 03's transaction is
//! `plan → validate → snapshot → apply → health-check → commit | rollback`. What was missing was the thing a
//! person experiences as one action: a file in the session's own directory *and* state written by an
//! application inside its capsule, undone as a single unit.
//!
//! This module makes that real by putting both directories in **one Strata store**, so one moment in time
//! covers both, and one undo restores both. That is the whole trick, and it is why the capsule's layer and
//! the session's files must share a history rather than each having their own backup.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Capsule, Error, Result, StrataQuery};

/// A record of one transition, written into the session so the machine can say what was undone and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub label: String,
    /// The moment the state before the change was recorded (ns).
    pub at: i64,
    /// How many paths the undo restored or removed, across both the session and its capsule.
    pub restored: u64,
}

pub struct SessionLayer {
    capsule: Capsule,
    session_dir: PathBuf,
    strata_root: PathBuf,
    query: StrataQuery,
}

impl SessionLayer {
    /// Open a session at `root`: `<root>/session` holds the session's own files, `<root>/capsule` its
    /// application layer, and `<root>/.strata` the single history both are recorded in.
    pub fn open(root: impl Into<PathBuf>, label: &str, query: &str) -> Result<Self> {
        let root = root.into();
        let session_dir = root.join("session");
        std::fs::create_dir_all(&session_dir)
            .map_err(|source| Error::Io { what: format!("creating {}", session_dir.display()), source })?;
        let strata_root = root.join(".strata");
        std::fs::create_dir_all(&strata_root)
            .map_err(|source| Error::Io { what: format!("creating {}", strata_root.display()), source })?;

        // The capsule reuses the session's store, so one rewind covers both. `Capsule::open` would create its
        // own history beside it, which is precisely the split that makes "undo the session" a lie.
        let capsule = Capsule::open_with_store(root.join("capsule"), label, &strata_root)?;
        let query = StrataQuery::parse(query)?;
        Ok(Self { capsule, session_dir, strata_root, query })
    }

    pub fn capsule(&self) -> &Capsule {
        &self.capsule
    }

    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    pub fn query(&self) -> &StrataQuery {
        &self.query
    }

    /// Write a file into the session's own directory (a person's document, not an app's state).
    pub fn write(&self, rel: &str, content: &[u8]) -> Result<PathBuf> {
        let path = self.session_dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                what: format!("creating {}", parent.display()),
                source,
            })?;
        }
        std::fs::write(&path, content).map_err(|source| Error::Io {
            what: format!("writing {}", path.display()),
            source,
        })?;
        Ok(path)
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        std::fs::read(self.session_dir.join(rel)).map_err(|source| Error::Io {
            what: format!("reading {}", rel),
            source,
        })
    }

    fn strata(&self) -> Result<strata::Strata> {
        strata::Strata::open(&self.strata_root).map_err(|e| Error::Strata(format!("{e}")))
    }

    /// Record the state of both the session's files and the capsule's layer, and keep the moment. The
    /// returned value is what a later [`SessionLayer::undo_to`] means by "as things were".
    pub fn record(&self) -> Result<i64> {
        let strata = self.strata()?;
        strata
            .scan(&self.session_dir)
            .map_err(|e| Error::Strata(format!("{e}")))?;
        strata.scan(self.capsule.layer()).map_err(|e| Error::Strata(format!("{e}")))?;
        Ok(self.capsule.now())
    }

    /// The files a person sees in this session: query results from the session directory, resolved against
    /// the same history.
    pub fn files(&self) -> Result<Vec<crate::ResolvedFile>> {
        let strata = self.strata()?;
        let mut query = self.query.clone();
        // The query's root is relative to the session, so `under=.` means the session directory.
        if query.under == PathBuf::from(".") {
            query.under = self.session_dir.clone();
        }
        query.resolve(&strata)
    }

    /// Undo the session **and** its capsule to a moment, as one transition. This is the unit the product
    /// promises: a change a person made and a change an application made, taken back together.
    pub fn undo_to(&self, at: i64) -> Result<TransitionRecord> {
        let strata = self.strata()?;
        let session_rewind = strata
            .rewind(&self.session_dir, at, false)
            .map_err(|e| Error::Strata(format!("{e}")))?;
        let capsule_rewind = strata
            .rewind(self.capsule.layer(), at, false)
            .map_err(|e| Error::Strata(format!("{e}")))?;
        Ok(TransitionRecord {
            label: format!("undo session {}", self.capsule.manifest().label),
            at,
            restored: session_rewind.restored + capsule_rewind.restored,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("morph-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// THE P1 FALSIFIER, extended: one transition spanning a person's file and an app's state.
    #[test]
    fn one_undo_takes_back_a_session_file_and_an_application_change_together() {
        let dir = scratch("both");
        let s = SessionLayer::open(&dir, "report", "strata://query?under=.").unwrap();

        s.write("report.md", b"draft one").unwrap();
        s.capsule().write("home/prefs.json", br#"{"theme":"light"}"#).unwrap();
        s.record().unwrap();
        let at = s.record().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        s.write("report.md", b"draft two").unwrap();
        s.capsule().write("home/prefs.json", br#"{"theme":"dark"}"#).unwrap();
        s.record().unwrap();

        assert_eq!(s.read("report.md").unwrap(), b"draft two");
        assert_eq!(s.capsule().read("home/prefs.json").unwrap(), br#"{"theme":"dark"}"#);

        let record = s.undo_to(at).unwrap();

        assert_eq!(s.read("report.md").unwrap(), b"draft one", "the session file must come back");
        assert_eq!(
            s.capsule().read("home/prefs.json").unwrap(),
            br#"{"theme":"light"}"#,
            "the application's state must come back in the SAME undo"
        );
        assert!(record.restored >= 2, "both paths must be reported as restored: {}", record.restored);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_session_query_lists_its_files_and_refuses_what_it_cannot_answer() {
        let dir = scratch("query");
        let s = SessionLayer::open(&dir, "report", "strata://query?under=.").unwrap();
        s.write("a.md", b"a").unwrap();
        s.write("b.md", b"b").unwrap();
        s.record().unwrap();
        let files = s.files().unwrap();
        assert_eq!(files.len(), 2, "the query must resolve both files: {files:?}");
        assert!(files.iter().all(|f| f.path.starts_with(s.session_dir())));

        // A query the machine cannot answer is refused where the session is opened, not silently empty.
        let refused = SessionLayer::open(&scratch("q2"), "report", "strata://query?project=q3");
        assert!(refused.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

}
