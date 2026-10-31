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

/// A transition record together with the signature that makes it checkable later by someone who was not
/// there. The seal is ed25519 over the record's canonical JSON — the same primitive `morph-attest` uses for
/// the machine's self-description, so one public key can check both what the machine is and what it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedTransition {
    pub record: TransitionRecord,
    pub signature: String,
    pub key: String,
}

/// A record of one transition, written into the session so the machine can say what was undone and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub label: String,
    /// The moment the state before the change was recorded (ns). This is what an undo returns to.
    pub at: i64,
    /// How many paths the undo restored or removed, across both the session and its capsule.
    pub restored: u64,
    /// When the record itself was made (ns). Distinct per transition — the first version of this struct had
    /// no such field, so a change and the undo of it shared a filename (both used `at`) and one silently
    /// overwrote the other. A history that loses entries without saying so is the bug this field prevents.
    #[serde(default)]
    pub sealed_at: i64,
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

    /// Where a session keeps its sealed transitions.
    pub fn transitions_dir(&self) -> PathBuf {
        self.session_dir.join(".transitions")
    }

    /// Seal a transition with `key` and keep it. A transition nobody sealed is a claim with no author, and the
    /// product's promise ("prove what it did") needs the author.
    pub fn seal(&self, record: &TransitionRecord, key: &ed25519_dalek::SigningKey) -> Result<SealedTransition> {
        // Stamp the record with its own moment, so two transitions that share an `at` cannot collide.
        let mut record = record.clone();
        record.sealed_at = self.capsule.now();
        let payload = serde_json::to_vec(&record).map_err(|e| Error::Refused(format!("{e}")))?;
        let sealed = SealedTransition {
            record: record.clone(),
            signature: morph_attest::sign_payload(key, &payload),
            key: morph_attest::hex(key.verifying_key().as_bytes()),
        };
        std::fs::create_dir_all(self.transitions_dir()).map_err(|source| Error::Io {
            what: format!("creating {}", self.transitions_dir().display()),
            source,
        })?;
        let path = self
            .transitions_dir()
            .join(format!("{}.json", record.sealed_at));
        std::fs::write(&path, serde_json::to_vec_pretty(&sealed).map_err(|e| Error::Refused(format!("{e}")))?)
            .map_err(|source| Error::Io { what: format!("writing {}", path.display()), source })?;
        Ok(sealed)
    }

    /// Read the sealed transitions, and **refuse** any whose signature does not cover its record. The refusal
    /// is the point: a history that silently accepts an edited record is a history nobody can use as evidence.
    pub fn transitions(&self) -> Result<Vec<SealedTransition>> {
        let dir = self.transitions_dir();
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(source) => {
                return Err(Error::Io { what: format!("reading {}", dir.display()), source })
            }
        };
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io { what: format!("reading {}", dir.display()), source })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|source| Error::Io { what: format!("reading {}", path.display()), source })?;
            let sealed: SealedTransition = serde_json::from_slice(&bytes)
                .map_err(|e| Error::Refused(format!("{} is not a sealed transition: {e}", path.display())))?;
            let payload = serde_json::to_vec(&sealed.record)
                .map_err(|e| Error::Refused(format!("{e}")))?;
            morph_attest::verify_payload(&sealed.key, &payload, &sealed.signature).map_err(|why| {
                Error::Refused(format!("{} is not trustworthy: {why}", path.display()))
            })?;
            out.push(sealed);
        }
        out.sort_by_key(|s| s.record.sealed_at);
        Ok(out)
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
            // `steps` is what actually happened to a path (restored or removed); `unchanged` is not
            // a restore, so counting it would inflate the record.
            restored: (session_rewind.steps.len() + capsule_rewind.steps.len()) as u64,
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
    fn a_sealed_transition_verifies_and_a_tampered_one_is_refused() {
        let dir = scratch("seal");
        let s = SessionLayer::open(&dir, "report", "strata://query?under=.").unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
        s.write("a.md", b"one").unwrap();
        let at = s.record().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.write("a.md", b"two").unwrap();
        s.record().unwrap();
        let record = s.undo_to(at).unwrap();
        s.seal(&record, &key).unwrap();

        let loaded = s.transitions().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].record, record, "the same record must verify");

        // Now edit the stored record the way an attacker editing history would: one field.
        let path = s.transitions_dir().join(format!("{}.json", record.at));
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["record"]["restored"] = serde_json::json!(999);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        match s.transitions() {
            Err(Error::Refused(why)) => assert!(why.contains("not trustworthy"), "unexpected: {why}"),
            Ok(v) => panic!("an edited transition was accepted: {v:?}"),
            Err(other) => panic!("unexpected error: {other:?}"),
        }
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
