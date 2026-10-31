//! P2: the seal becomes mandatory.
//!
//! P1 could seal a transition, but sealing was something a caller *chose* to do. A promise that can be skipped
//! is not a promise, so this module makes the seal the only way to change a session:
//!
//! * the session is opened **with a key**, and opening it without one is refused — there is no "unsigned mode"
//!   to fall into by forgetting a flag;
//! * every change goes through [`SealedSession::change`], which records the moment, runs the change, seals the
//!   result, and — if the change fails — takes the state back to that moment and writes **no seal**. A failed
//!   change must not leave a signed record claiming it happened.
//! * [`SealedSession::history`] verifies every seal it reads, so an edited history is refused rather than
//!   believed.
//!
//! WHAT THIS DOES NOT DO, stated so it cannot be implied: it makes changes that go through `change()` sealed.
//! A caller that writes to the layer by other means is outside the promise, and the honest defence is that the
//! layer's owner is this type — the API does not expose an unsealed write. Detecting out-of-band modification
//! needs the store to be read-only to everyone else, which is a mount-level property (next step).

use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::{Error, Result, SealedTransition, SessionLayer, TransitionRecord};

/// A session whose every change is signed. Holding one means every change went through [`SealedSession::change`].
pub struct SealedSession {
    layer: SessionLayer,
    key: SigningKey,
}

/// What a change produced: the sealed record, and how many paths it touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Committed {
    pub sealed: SealedTransition,
    pub restored: u64,
}

impl SealedSession {
    /// Open a session that requires a seal. `key` is the ed25519 key that signs its transitions — the same
    /// kind of key `morph-attest` uses, so one public key can check what the machine is and what it did.
    pub fn open(
        root: impl Into<PathBuf>,
        label: &str,
        query: &str,
        key: SigningKey,
    ) -> Result<Self> {
        let layer = SessionLayer::open(root, label, query)?;
        Ok(Self { layer, key })
    }

    /// The session's files — read-only through this type, because a write that skips the seal is exactly what
    /// this module exists to prevent.
    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        self.layer.read(rel)
    }

    pub fn files(&self) -> Result<Vec<crate::ResolvedFile>> {
        self.layer.files()
    }

    /// The capsule's layer, for a caller that must run an application inside it.
    pub fn capsule_layer(&self) -> &std::path::Path {
        self.layer.capsule().layer()
    }

    /// Make a change under the seal. `change` receives the layer and does the work; whatever it does is one
    /// transition.
    ///
    /// On success: the new state is recorded and sealed, and the seal is returned.
    /// On failure: the state is taken back to the moment before the change and **no seal is written**, so the
    /// history never claims a change that did not happen.
    pub fn change<F>(&self, label: &str, change: F) -> Result<Committed>
    where
        F: FnOnce(&SessionLayer) -> Result<()>,
    {
        // The moment before: what a rollback returns to.
        let before = self.layer.record()?;
        match change(&self.layer) {
            Ok(()) => {
                self.layer.record()?;
                let record = TransitionRecord {
                    label: format!("{label} (sealed)"),
                    at: before,
                    // The record describes the transition FROM `before`; the seal is what makes it checkable.
                    restored: 0,
                    // What the state IS now. A later digest that differs means something wrote around the seal.
                    digest: self.layer.digest()?,
                    sealed_at: 0, // stamped by `SessionLayer::seal`
                };
                let sealed = self.layer.seal(&record, &self.key)?;
                Ok(Committed { sealed, restored: 0 })
            }
            Err(why) => {
                // Take it back. If the rollback itself fails, say both things: a half-applied change that
                // cannot be undone is the most important state to report accurately.
                match self.layer.undo_to(before) {
                    Ok(record) => Err(Error::Refused(format!(
                        "the change failed ({why}) and was taken back ({} paths restored); no seal was written",
                        record.restored
                    ))),
                    Err(rollback) => Err(Error::Refused(format!(
                        "the change failed ({why}) AND the rollback failed ({rollback}): this session needs \
                         attention"
                    ))),
                }
            }
        }
    }

    /// Undo the session (and its capsule) to a moment, sealing the undo as a transition of its own. An undo is
    /// a change: it must be as checkable as the change it reverses.
    pub fn undo_to(&self, at: i64) -> Result<Committed> {
        let record = self.layer.undo_to(at)?;
        let sealed = self.layer.seal(&record, &self.key)?;
        Ok(Committed { sealed, restored: record.restored })
    }

    /// Compare the state on disk with the digest of the last sealed transition. Anything that differs — added,
    /// changed or removed — is a write that happened outside a sealed change.
    ///
    /// This is the software answer to a limit permissions cannot close: root can always write to a file. What
    /// root cannot do is write and stay invisible.
    pub fn drift(&self) -> Result<Vec<String>> {
        let history = self.layer.transitions()?;
        let Some(last) = history.last() else {
            // No sealed transition yet: the state has never been claimed, so nothing can have drifted from it.
            return Ok(Vec::new());
        };
        let now = self.layer.digest()?;
        let then = &last.record.digest;
        let mut out = Vec::new();
        for (path, hash) in &now {
            match then.get(path) {
                Some(old) if old == hash => {}
                Some(_) => out.push(format!("changed outside a seal: {path}")),
                None => out.push(format!("added outside a seal: {path}")),
            }
        }
        for path in then.keys() {
            if !now.contains_key(path) {
                out.push(format!("removed outside a seal: {path}"));
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every sealed transition, verified. An edited record is refused here, not discovered later.
    pub fn history(&self) -> Result<Vec<SealedTransition>> {
        self.layer.transitions()
    }

    /// The key's public half, in hex — what a third party needs to check this session's seals.
    pub fn verifying_key(&self) -> String {
        morph_attest::hex(self.key.verifying_key().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("morph-sealed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[21u8; 32])
    }

    #[test]
    fn every_change_is_sealed_and_the_seal_verifies() {
        let dir = scratch("commit");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        let committed = s
            .change("write report.md", |layer| {
                layer.write("report.md", b"draft one")?;
                Ok(())
            })
            .unwrap();
        assert_eq!(s.read("report.md").unwrap(), b"draft one");
        assert!(
            committed.sealed.record.label.contains("write report.md"),
            "the seal must name the change: {:?}",
            committed.sealed.record
        );
        let history = s.history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].key, s.verifying_key());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE P2 FALSIFIER: a change that fails must leave the state as it was and must NOT leave a seal claiming
    /// otherwise. A signed lie is worse than no seal at all.
    #[test]
    fn a_failed_change_is_taken_back_and_leaves_no_seal() {
        let dir = scratch("fail");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        s.change("first", |layer| {
                layer.write("a.md", b"original")?;
                Ok(())
            }).unwrap();
        let before = s.read("a.md").unwrap();
        assert_eq!(before, b"original");

        let refused = s.change("doomed", |layer| {
            layer.write("a.md", b"half-written")?;
            Err(Error::Refused("the application died mid-write".into()))
        });
        assert!(refused.is_err());
        let message = format!("{}", refused.unwrap_err());
        assert!(message.contains("taken back"), "unexpected: {message}");
        assert!(message.contains("no seal was written"), "unexpected: {message}");

        assert_eq!(s.read("a.md").unwrap(), before, "the failed change must be reverted");
        assert_eq!(s.history().unwrap().len(), 1, "only the successful change may be sealed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_undo_is_itself_a_sealed_change() {
        let dir = scratch("undo");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        s.change("one", |layer| {
                layer.write("a.md", b"one")?;
                Ok(())
            }).unwrap();
        let at = s.change("two", |layer| {
                layer.write("a.md", b"two")?;
                Ok(())
            }).unwrap();
        // Undo the second change, then seal it: an undo must be as checkable as the change it reverses.
        let undone = s.undo_to(at.sealed.record.at).unwrap();
        assert_eq!(s.read("a.md").unwrap(), b"one");
        assert!(undone.sealed.record.label.contains("undo"));
        assert_eq!(s.history().unwrap().len(), 3, "two changes and the undo, all sealed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The promise that permissions cannot keep: root can write to any file. What root cannot do is write and
    /// stay invisible — a write that bypasses the seal shows up as drift, with the path named.
    #[test]
    fn a_write_around_the_seal_is_reported_with_its_path() {
        let dir = scratch("drift");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        s.change("sealed", |layer| { layer.write("a.md", b"sealed content")?; Ok(()) }).unwrap();
        assert_eq!(s.drift().unwrap(), Vec::<String>::new(), "a clean session must report no drift");

        // Write directly into the layer, exactly as a process that does not know about sealing would.
        let sneaky = dir.join("capsule/layer/home/sneaky.txt");
        std::fs::create_dir_all(sneaky.parent().unwrap()).unwrap();
        std::fs::write(&sneaky, b"written around the seal").unwrap();

        let drift = s.drift().unwrap();
        assert_eq!(drift.len(), 1, "expected exactly one drifted path: {drift:?}");
        assert!(drift[0].contains("added outside a seal"), "{drift:?}");
        assert!(drift[0].contains("sneaky.txt"), "the path must be named: {drift:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_removed_file_is_drift_too() {
        let dir = scratch("drift-remove");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        s.change("sealed", |layer| { layer.write("a.md", b"here")?; Ok(()) }).unwrap();
        std::fs::remove_file(dir.join("session/a.md")).unwrap();
        let drift = s.drift().unwrap();
        assert_eq!(drift.len(), 1, "{drift:?}");
        assert!(drift[0].contains("removed outside a seal"), "{drift:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sealed_change_then_undo_leaves_no_drift() {
        let dir = scratch("drift-undo");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        let at = s.change("one", |layer| { layer.write("a.md", b"one")?; Ok(()) }).unwrap();
        s.undo_to(at.sealed.record.at).unwrap();
        assert_eq!(s.drift().unwrap(), Vec::<String>::new(), "a sealed undo is not drift");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_foreign_key_cannot_vouch_for_this_history() {
        let dir = scratch("foreign");
        let s = SealedSession::open(&dir, "report", "strata://query?under=.", key()).unwrap();
        s.change("one", |layer| {
                layer.write("a.md", b"one")?;
                Ok(())
            }).unwrap();
        let history = s.history().unwrap();
        let recorded = &history[0];

        // Someone else's key: the signature must not verify against it.
        let other = SigningKey::from_bytes(&[22u8; 32]);
        let payload = serde_json::to_vec(&recorded.record).unwrap();
        assert!(
            morph_attest::verify_payload(
                &morph_attest::hex(other.verifying_key().as_bytes()),
                &payload,
                &recorded.signature
            )
            .is_err(),
            "a foreign key verified this session's record"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
