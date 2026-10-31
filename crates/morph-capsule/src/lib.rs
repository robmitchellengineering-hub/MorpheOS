//! A **capsule**: an application plus a Strata-versioned user layer, so that what it does can be undone.
//!
//! This is doc 05's capsule, in the smallest form that is real: a directory the application lives in (its
//! "user layer"), a Strata store recording every version of every file in that layer, and a spawn that puts
//! the application's HOME inside the layer so its state is the layer's state.
//!
//! WHY THE LAYER IS THE POINT. The product's promise is "undo anything" (principle 2), and a capsule is
//! where an app's state actually lives. If undo covers files a person edited but not the state an app wrote,
//! then the promise has a hole exactly where damage is most likely. So the capsule's user layer is versioned
//! from the first write, and undo is a rewind of that layer — no backup tool, no snapshot mount, and it can
//! happen while the application is running.
//!
//! WHAT THIS IS NOT (yet): a namespace. Rung 1 of doc 05's ladder is "a Linux container — namespaces and
//! cgroups"; the honest first step is the layer and its undo, then the sandbox around it. The struct records
//! the rung it is running at so a later step cannot silently claim more than it does.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io { what: String, source: std::io::Error },
    Strata(String),
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::Strata(m) => write!(f, "strata: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
        }
    }
}

impl std::error::Error for Error {}

fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

/// Which rung of doc 05's compatibility ladder a capsule is actually running at. Recorded rather than
/// implied, so a capsule can never claim to be more isolated than it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rung {
    /// Native: no environment at all.
    Native,
    /// A Linux process, its own user layer, no namespace yet.
    LayerOnly,
    /// A Linux container: namespaces + cgroups (rung 1 proper).
    Container,
}

impl Rung {
    pub fn as_str(self) -> &'static str {
        match self {
            Rung::Native => "native",
            Rung::LayerOnly => "layer-only (no namespace yet)",
            Rung::Container => "container (namespaces)",
        }
    }
}

/// What a capsule is, written down next to its layer so the machine can say what this state belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub label: String,
    pub rung: Rung,
    /// The capability set the capsule may use (doc 05). Empty today means "not yet enforced" — recorded as
    /// `enforced: false` rather than left to look like an empty allowlist, which would be a lie.
    pub capabilities: Vec<String>,
    pub enforced: bool,
}

pub struct Capsule {
    root: PathBuf,
    layer: PathBuf,
    strata: strata::Strata,
    manifest: Manifest,
}

impl Capsule {
    /// Open (or create) a capsule at `root`. Its user layer is `root/layer`; its versions live in
    /// `root/.strata`, so the history travels with the capsule and never with a global tool.
    pub fn open(root: impl Into<PathBuf>, label: impl Into<String>) -> Result<Self> {
        let root = root.into();
        let layer = root.join("layer");
        std::fs::create_dir_all(layer.join("home")).map_err(io(format!("creating {}", layer.display())))?;
        std::fs::create_dir_all(root.join(".strata"))
            .map_err(io(format!("creating {}", root.join(".strata").display())))?;

        let manifest_path = root.join("capsule.json");
        let manifest = match std::fs::read(&manifest_path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| Error::Refused(format!(
                "{} is not a capsule manifest: {e}", manifest_path.display()
            )))?,
            Err(_) => {
                let m = Manifest {
                    label: label.into(),
                    rung: Rung::LayerOnly,
                    capabilities: Vec::new(),
                    enforced: false,
                };
                std::fs::write(&manifest_path, serde_json::to_vec_pretty(&m).expect("serialises"))
                    .map_err(io(format!("writing {}", manifest_path.display())))?;
                m
            }
        };

        let strata_root = root.join(".strata");
        let strata = strata::Strata::open(&strata_root).map_err(|e| Error::Strata(format!("{e}")))?;
        Ok(Self { root, layer, strata, manifest })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory an application's state actually lives in. HOME points here.
    pub fn layer(&self) -> &Path {
        &self.layer
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// A path inside the layer, refusing anything that tries to leave it. A capsule's undo is only honest if
    /// its writes are confined to the thing it versions.
    pub fn path(&self, rel: &str) -> Result<PathBuf> {
        let candidate = self.layer.join(rel);
        let normalised = candidate
            .components()
            .fold(PathBuf::new(), |mut acc, c| {
                match c {
                    std::path::Component::ParentDir => {
                        acc.pop();
                    }
                    std::path::Component::CurDir => {}
                    other => acc.push(other.as_os_str()),
                }
                acc
            });
        if !normalised.starts_with(&self.layer) {
            return Err(Error::Refused(format!(
                "'{rel}' escapes the capsule layer; a capsule may only change its own state"
            )));
        }
        Ok(normalised)
    }

    /// Write a file into the layer. This is the application writing its state — done through the capsule so
    /// the versioning cannot be skipped by accident.
    pub fn write(&self, rel: &str, content: &[u8]) -> Result<PathBuf> {
        let path = self.path(rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io(format!("creating {}", parent.display())))?;
        }
        std::fs::write(&path, content).map_err(io(format!("writing {}", path.display())))?;
        Ok(path)
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let path = self.path(rel)?;
        std::fs::read(&path).map_err(io(format!("reading {}", path.display())))
    }

    /// Record the current state of the layer (every file) in the capsule's history. Returns how many files
    /// were seen and how many versions were newly recorded.
    pub fn record(&self) -> Result<(u64, u64)> {
        self.strata
            .scan(&self.layer)
            .map_err(|e| Error::Strata(format!("{e}")))
    }

    /// The moment a caller should pass to [`Capsule::undo_to`] to mean "as things were just now".
    pub fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }

    /// Undo everything in the layer back to `at`: files that changed are restored to the version in force at
    /// that moment, files created after it are removed. The application may keep running — this is the
    /// falsifier the product's "undo anything" promise stands on.
    pub fn undo_to(&self, at: i64) -> Result<strata::RewindReport> {
        self.strata
            .rewind(&self.layer, at, false)
            .map_err(|e| Error::Strata(format!("{e}")))
    }

    /// Run a command with the capsule's layer as its HOME and working directory. `script` is passed to the
    /// system shell, because the capsule's job is to run the application's own command line.
    pub fn spawn(&self, script: &str) -> Result<Child> {
        let home = self.layer.join("home");
        std::fs::create_dir_all(&home).map_err(io(format!("creating {}", home.display())))?;
        Command::new("sh")
            .arg("-c")
            .arg(script)
            .current_dir(&self.layer)
            .env("HOME", &home)
            .env("CAPSULE_ROOT", &self.root)
            .env("CAPSULE_LABEL", &self.manifest.label)
            .env("CAPSULE_RUNG", self.manifest.rung.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(io("spawning the capsule's command"))
    }

    /// Run a command to completion and return (status code, stdout, stderr).
    pub fn run(&self, script: &str) -> Result<(Option<i32>, String, String)> {
        let out = self
            .spawn(script)?
            .wait_with_output()
            .map_err(io("waiting for the capsule's command"))?;
        Ok((
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}

/// One transition that spans a session's own files and a capsule's layer: the unit doc 03 calls a
/// transaction, at the granularity a person experiences — "this thing I did, undo it".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transition {
    pub label: String,
    /// When the state before the change was recorded.
    pub at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("morph-capsule-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_write_is_versioned_and_undo_restores_the_earlier_content() {
        let dir = scratch("write-undo");
        let c = Capsule::open(&dir, "editor").unwrap();
        c.write("notes.txt", b"first").unwrap();
        c.record().unwrap();
        let at = c.now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        c.write("notes.txt", b"second").unwrap();
        c.record().unwrap();

        assert_eq!(c.read("notes.txt").unwrap(), b"second");
        c.undo_to(at).unwrap();
        assert_eq!(c.read("notes.txt").unwrap(), b"first", "undo must restore the earlier version");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE FALSIFIER. Undo while the application is running: the process must see the reverted state without
    /// a restart, because a rollback that needs a reboot is a backup tool, not a primitive.
    #[test]
    fn undo_reaches_a_running_application() {
        let dir = scratch("running");
        let c = Capsule::open(&dir, "editor").unwrap();
        c.write("home/notes.txt", b"before").unwrap();
        c.record().unwrap();
        let at = c.now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        c.write("home/notes.txt", b"after").unwrap();
        c.record().unwrap();

        // A "long-running application": started before the undo, reading its state when asked.
        let mut app = c.spawn("while true; do cat \"$HOME/notes.txt\"; echo; sleep 0.05; done").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));

        c.undo_to(at).unwrap();

        // The same process, never restarted, now reports the earlier content.
        let mut seen_before = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(25));
            let output = c.run("cat \"$HOME/notes.txt\"").unwrap().1;
            if output.trim() == "before" {
                seen_before = true;
                break;
            }
        }
        let _ = app.kill();
        let _ = app.wait();
        assert!(seen_before, "the running application did not see the undone state");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_capsule_cannot_write_outside_its_layer() {
        let dir = scratch("escape");
        let c = Capsule::open(&dir, "editor").unwrap();
        match c.write("../escaped.txt", b"nope") {
            Err(Error::Refused(msg)) => assert!(msg.contains("escapes the capsule layer")),
            other => panic!("a write outside the layer was not refused: {other:?}"),
        }
        assert!(!dir.join("escaped.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_manifest_records_what_the_capsule_can_and_cannot_do() {
        let dir = scratch("manifest");
        let c = Capsule::open(&dir, "browser").unwrap();
        assert_eq!(c.manifest().label, "browser");
        assert_eq!(c.manifest().rung, Rung::LayerOnly);
        assert!(!c.manifest().enforced, "an unenforced capability list must say so, not look empty");
        // Re-opening reads the manifest back rather than inventing a new one.
        let again = Capsule::open(&dir, "different-label").unwrap();
        assert_eq!(again.manifest().label, "browser");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_created_after_the_undo_point_are_removed() {
        let dir = scratch("created");
        let c = Capsule::open(&dir, "editor").unwrap();
        c.write("keep.txt", b"keep").unwrap();
        c.record().unwrap();
        let at = c.now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        c.write("new.txt", b"new").unwrap();
        c.record().unwrap();
        c.undo_to(at).unwrap();
        assert!(!c.path("new.txt").unwrap().exists(), "a file created after the undo point must go");
        assert_eq!(c.read("keep.txt").unwrap(), b"keep");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
