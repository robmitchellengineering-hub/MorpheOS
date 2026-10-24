//! Snapshot, restore, rewind and prune: the operations a person and the heal
//! engine actually use.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::detect::Alert;
use crate::index::{Index, Kind, State, Version};
use crate::retention::{Retention, prunable};
use crate::store::{GcReport, ObjectStore};
use crate::time;
use crate::{Error, Result, io};

/// Marks Strata's own temporary files, so the watcher ignores them.
pub const TMP_MARK: &str = ".strata-tmp-";

pub struct Strata {
    root: PathBuf,
    store: ObjectStore,
    index: Index,
}

/// What a file looks like on disk right now.
#[derive(Debug, Clone, PartialEq)]
enum OnDisk {
    File {
        content: String,
        mode: u32,
    },
    Absent,
    /// A directory, symlink or device where a file is expected. Left alone.
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Step {
    /// Put back the version that existed at the rewind point.
    Restore {
        #[serde(serialize_with = "lossy")]
        path: PathBuf,
        version: i64,
    },
    /// The file did not exist at the rewind point: move it to quarantine.
    MoveAway {
        #[serde(serialize_with = "lossy")]
        path: PathBuf,
    },
}

fn lossy<S: serde::Serializer>(p: &Path, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&p.to_string_lossy())
}

/// What a rewind would do: the changes, how many files are already right, and
/// the files it cannot touch (path, reason).
struct Plan {
    steps: Vec<Step>,
    unchanged: usize,
    failed: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RewindReport {
    pub at: String,
    pub dry_run: bool,
    pub steps: Vec<Step>,
    pub unchanged: usize,
    #[serde(serialize_with = "lossy")]
    pub quarantine: PathBuf,
    pub failed: Vec<(String, String)>,
}

impl Strata {
    /// Open (or create) a store. The directory is made private to its owner:
    /// whatever can reach your files must not be able to reach their history.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(io(format!("creating {}", root.display())))?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(io(format!("securing {}", root.display())))?;
        let root = root
            .canonicalize()
            .map_err(io(format!("resolving {}", root.display())))?;
        Ok(Strata {
            store: ObjectStore::open(&root)?,
            index: Index::open(&root.join("index.sqlite"))?,
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn store(&self) -> &ObjectStore {
        &self.store
    }

    /// Keep an alert in the store, one JSON line per alert in `alerts.jsonl`.
    ///
    /// WHY IN THE STORE. An alert printed to a log is gone once the log rotates, and
    /// it can be read only by scraping text. The heal engine, the shell and the VM test
    /// all need the rewind point later, from the record the system itself keeps
    /// (AGENTS.md H8). The store is private to the service, so software that can
    /// write your files cannot erase the alert about it.
    pub fn record_alert(&self, alert: &Alert) -> Result<()> {
        let path = self.root.join("alerts.jsonl");
        let mut line = serde_json::to_string(alert).expect("alert serializes");
        line.push('\n');
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io(format!("opening {}", path.display())))?;
        f.write_all(line.as_bytes())
            .and_then(|()| f.sync_all())
            .map_err(io(format!("writing {}", path.display())))
    }

    /// One alert per burst, oldest burst first, each in its latest version: a burst
    /// is re-recorded as it reaches new folders (see [`Alert`]), and acting on an
    /// earlier version would leave those folders encrypted.
    pub fn alerts(&self) -> Result<Vec<Alert>> {
        let mut out: Vec<Alert> = Vec::new();
        for a in self.alert_records()? {
            match out.iter_mut().find(|o| o.first_ts == a.first_ts) {
                Some(o) => *o = a,
                None => out.push(a),
            }
        }
        Ok(out)
    }

    /// Every line of `alerts.jsonl`, as written.
    fn alert_records(&self) -> Result<Vec<Alert>> {
        let path = self.root.join("alerts.jsonl");
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Error::Io {
                    what: format!("reading {}", path.display()),
                    source: e,
                });
            }
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str(l)
                    .map_err(|e| Error::Corrupt(format!("{}: {e}: {l}", path.display())))
            })
            .collect()
    }

    /// Refuse to version a folder that contains the store itself.
    pub fn check_watchable(&self, dir: &Path) -> Result<()> {
        if self.root.starts_with(dir) {
            return Err(Error::Refused(format!(
                "{} contains the Strata store ({}); keep the store outside the folders it protects",
                dir.display(),
                self.root.display()
            )));
        }
        Ok(())
    }

    fn on_disk(&self, path: &Path) -> Result<OnDisk> {
        match fs::symlink_metadata(path) {
            Ok(m) if m.is_file() => {
                let f = fs::File::open(path).map_err(io(format!("reading {}", path.display())))?;
                // Hash only: planning (and a dry run) must not write to the store.
                let content = self.store.hash_file(f)?.content;
                Ok(OnDisk::File {
                    content,
                    mode: m.permissions().mode() & 0o7777,
                })
            }
            Ok(_) => Ok(OnDisk::Other),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OnDisk::Absent),
            Err(e) => Err(Error::Io {
                what: format!("inspecting {}", path.display()),
                source: e,
            }),
        }
    }

    /// Record the current state of `path`. Returns (the version recorded before,
    /// the new version), the second being `None` when nothing changed.
    pub fn snapshot(&self, path: &Path) -> Result<(Option<Version>, Option<Version>)> {
        let prev = self.index.latest(path)?;
        let now = time::now_ns();
        let new = match fs::symlink_metadata(path) {
            Ok(m) if m.is_file() => {
                let f = match fs::File::open(path) {
                    Ok(f) => f,
                    // Gone between the event and now: fall through to a deletion.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return self.record_deleted(path, prev, now);
                    }
                    Err(e) => {
                        return Err(Error::Io {
                            what: format!("reading {}", path.display()),
                            source: e,
                        });
                    }
                };
                let s = self.store.put_file(f)?;
                let mode = m.permissions().mode() & 0o7777;
                self.index
                    .record(path, now, &State::file(s.size, mode, &s.content, s.entropy))?
            }
            Ok(_) => None,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return self.record_deleted(path, prev, now);
            }
            Err(e) => {
                return Err(Error::Io {
                    what: format!("inspecting {}", path.display()),
                    source: e,
                });
            }
        };
        Ok((prev, new))
    }

    fn record_deleted(
        &self,
        path: &Path,
        prev: Option<Version>,
        now: i64,
    ) -> Result<(Option<Version>, Option<Version>)> {
        let new = match &prev {
            Some(v) if v.kind == Kind::File => self.index.record(path, now, &State::deleted())?,
            _ => None,
        };
        Ok((prev, new))
    }

    /// Snapshot every file below `dir`. Returns (files seen, versions recorded).
    pub fn scan(&self, dir: &Path) -> Result<(u64, u64)> {
        self.check_watchable(dir)?;
        let mut seen = 0;
        let mut recorded = 0;
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let entries = match fs::read_dir(&d) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(Error::Io {
                        what: format!("listing {}", d.display()),
                        source: e,
                    });
                }
            };
            for entry in entries {
                let entry = entry.map_err(io(format!("listing {}", d.display())))?;
                let path = entry.path();
                let ft = entry
                    .file_type()
                    .map_err(io(format!("inspecting {}", path.display())))?;
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() && !is_tmp(&path) {
                    seen += 1;
                    if self.snapshot(&path)?.1.is_some() {
                        recorded += 1;
                    }
                }
            }
        }
        Ok((seen, recorded))
    }

    /// Write `version` to `to`, atomically, with its original permissions.
    pub fn restore_version(&self, version: &Version, to: &Path) -> Result<()> {
        if version.kind != Kind::File {
            return Err(Error::NotFound(format!(
                "version {} of {} is a deletion; there is no content to restore",
                version.id,
                version.path.display()
            )));
        }
        let dir = to
            .parent()
            .ok_or_else(|| Error::Refused(format!("{} has no parent", to.display())))?;
        fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
        let name = to
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let tmp = dir.join(format!(".{name}{TMP_MARK}{}", std::process::id()));
        let result = (|| {
            let mut f = fs::File::create(&tmp).map_err(io(format!("writing {}", tmp.display())))?;
            self.store.read_file(&version.content, &mut f)?;
            f.flush().map_err(io("flushing"))?;
            f.set_permissions(fs::Permissions::from_mode(version.mode))
                .map_err(io(format!("setting permissions on {}", tmp.display())))?;
            f.sync_all().map_err(io("syncing"))?;
            fs::rename(&tmp, to).map_err(io(format!("replacing {}", to.display())))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Work out what a rewind of `dir` to `at` would change, without changing it.
    fn plan_rewind(&self, dir: &Path, at: i64) -> Result<Plan> {
        let mut steps = Vec::new();
        let mut unchanged = 0;
        let mut failed = Vec::new();
        for path in self.index.paths_under(dir)? {
            if is_tmp(&path) {
                continue;
            }
            let target = self.index.at(&path, at)?.filter(|v| v.kind == Kind::File);
            let disk = match self.on_disk(&path) {
                Ok(d) => d,
                Err(e) => {
                    failed.push((path.to_string_lossy().into_owned(), e.to_string()));
                    continue;
                }
            };
            match (target, disk) {
                (_, OnDisk::Other) => failed.push((
                    path.to_string_lossy().into_owned(),
                    "not a regular file now; left alone".into(),
                )),
                (Some(t), OnDisk::File { content, mode })
                    if t.content == content && t.mode == mode =>
                {
                    unchanged += 1
                }
                (Some(t), _) => steps.push(Step::Restore {
                    path,
                    version: t.id,
                }),
                (None, OnDisk::File { .. }) => steps.push(Step::MoveAway { path }),
                (None, OnDisk::Absent) => unchanged += 1,
            }
        }
        Ok(Plan {
            steps,
            unchanged,
            failed,
        })
    }

    /// Put every file below `dir` back the way it was at `at`.
    ///
    /// Nothing is destroyed: each file's current state is snapshotted before it
    /// is touched, and files that did not exist at `at` are moved into
    /// `<store>/quarantine/<time>/…` instead of being deleted. Rewinding to the
    /// moment just before the rewind undoes it.
    pub fn rewind(&self, dir: &Path, at: i64, dry_run: bool) -> Result<RewindReport> {
        self.check_watchable(dir)?;
        let Plan {
            steps,
            unchanged,
            mut failed,
        } = self.plan_rewind(dir, at)?;
        let quarantine = self
            .root
            .join("quarantine")
            .join(time::format(time::now_ns()));
        let mut report = RewindReport {
            at: time::format(at),
            dry_run,
            steps: Vec::new(),
            unchanged,
            quarantine: quarantine.clone(),
            failed: Vec::new(),
        };
        if dry_run {
            report.steps = steps;
            report.failed = failed;
            return Ok(report);
        }
        for step in steps {
            let r = match &step {
                Step::Restore { path, version } => self.apply_restore(path, *version),
                Step::MoveAway { path } => self.apply_move_away(path, &quarantine),
            };
            match r {
                Ok(()) => report.steps.push(step),
                Err(e) => {
                    let path = match &step {
                        Step::Restore { path, .. } | Step::MoveAway { path } => path,
                    };
                    failed.push((path.to_string_lossy().into_owned(), e.to_string()));
                }
            }
        }
        report.failed = failed;
        Ok(report)
    }

    fn apply_restore(&self, path: &Path, version: i64) -> Result<()> {
        let v = self
            .index
            .get(version)?
            .ok_or_else(|| Error::NotFound(format!("version {version}")))?;
        self.snapshot(path)?;
        self.restore_version(&v, path)?;
        self.snapshot(path)?;
        Ok(())
    }

    fn apply_move_away(&self, path: &Path, quarantine: &Path) -> Result<()> {
        self.snapshot(path)?;
        let rel = path.strip_prefix("/").unwrap_or(path);
        let dest = quarantine.join(rel);
        let parent = dest.parent().expect("quarantine path has a parent");
        fs::create_dir_all(parent).map_err(io(format!("creating {}", parent.display())))?;
        if fs::rename(path, &dest).is_err() {
            // Different filesystem: copy, then remove the original only once the copy is safe.
            fs::copy(path, &dest)
                .map_err(io(format!("copying {} to quarantine", path.display())))?;
            fs::File::open(&dest)
                .and_then(|f| f.sync_all())
                .map_err(io("syncing quarantine copy"))?;
            fs::remove_file(path).map_err(io(format!("removing {}", path.display())))?;
        }
        self.snapshot(path)?;
        Ok(())
    }

    /// The version ids the retention policy will prune — a read, so the space
    /// guard can plan on a full disk without writing anything to the index.
    pub fn prune_plan(&self, policy: &Retention, now: i64) -> Result<Vec<i64>> {
        let mut by_path: HashMap<Vec<u8>, Vec<(i64, i64)>> = HashMap::new();
        for (id, path, ts) in self.index.all_versions()? {
            by_path.entry(path).or_default().push((id, ts));
        }
        Ok(by_path
            .values()
            .flat_map(|v| prunable(v, now, policy))
            .collect())
    }

    /// Delete the objects only the doomed versions refer to. This is the step
    /// that frees space, and it writes nothing to the index, so it works on a
    /// full disk: the guard frees the space before it ever needs to record.
    pub fn collect(&self, doomed: &[i64], now: i64) -> Result<GcReport> {
        let doomed: HashSet<i64> = doomed.iter().copied().collect();
        // The surviving content hashes, computed from a read. No index row is
        // deleted yet, so this cannot depend on the delete having happened first.
        let keep: HashSet<String> = self
            .index
            .file_contents()?
            .into_iter()
            .filter(|(id, _)| !doomed.contains(id))
            .map(|(_, content)| content)
            .collect();
        // Objects written in the last hour are spared: a snapshot in progress
        // may have stored chunks it has not recorded yet.
        self.store.collect_garbage(&keep, now - 3600 * time::SECOND)
    }

    /// Remove the doomed versions from the index — the "record" half of a prune,
    /// and the only step that writes the index.
    pub fn record_prune(&mut self, doomed: &[i64]) -> Result<usize> {
        self.index.delete(doomed)
    }

    /// Thin out old versions by `policy`: free the objects, then record the prune.
    ///
    /// The objects are deleted first because that is the step that frees space: a
    /// full disk leaves no room for the index write, so the write can only succeed
    /// once the deletes have made room. `guard` uses the two steps separately so it
    /// can still report the recovered space when the record write is refused.
    pub fn prune(&mut self, policy: &Retention, now: i64) -> Result<(usize, GcReport)> {
        let doomed = self.prune_plan(policy, now)?;
        let gc = self.collect(&doomed, now)?;
        let removed = self.record_prune(&doomed)?;
        Ok((removed, gc))
    }
}

pub(crate) fn is_tmp(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().contains(TMP_MARK))
        .unwrap_or(false)
}
