//! Snapshot files as they change, using inotify.
//!
//! A file is recorded when it is closed after writing, moved in, moved out or
//! deleted. New folders are watched as they appear and scanned at once, so
//! files written into them before their watch existed are not missed. If the
//! kernel's event queue overflows, everything is rescanned.
//!
//! Known v0 limits, stated rather than hidden: a file written and deleted
//! faster than it can be read is recorded only as deleted; writes through a
//! memory map or a file kept open are recorded when it is closed; the watcher
//! sees *what* changed, not *who* changed it (fanotify, with the heal engine).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};

use crate::detect::{Alert, Detector};
use crate::index::Version;
use crate::strata::is_tmp;
use crate::{Error, Result, Strata, io};

#[derive(Debug)]
pub enum Event {
    Recorded(Version),
    Alert(Alert),
    /// The kernel dropped events; every watched folder was rescanned.
    Rescanned,
    Error(String),
}

pub struct Watcher<'a> {
    strata: &'a Strata,
    roots: Vec<PathBuf>,
    inotify: Inotify,
    dirs: HashMap<WatchDescriptor, PathBuf>,
    detector: Detector,
}

fn mask() -> WatchMask {
    WatchMask::CLOSE_WRITE
        | WatchMask::MOVED_TO
        | WatchMask::MOVED_FROM
        | WatchMask::DELETE
        | WatchMask::CREATE
        | WatchMask::ONLYDIR
}

impl<'a> Watcher<'a> {
    /// Watch `roots`, after snapshotting everything already in them.
    pub fn new(strata: &'a Strata, roots: &[PathBuf]) -> Result<Self> {
        let inotify = Inotify::init().map_err(io("starting inotify"))?;
        let mut w = Watcher {
            strata,
            roots: Vec::new(),
            inotify,
            dirs: HashMap::new(),
            detector: Detector::default(),
        };
        for root in roots {
            let root = root
                .canonicalize()
                .map_err(io(format!("resolving {}", root.display())))?;
            strata.check_watchable(&root)?;
            w.add_tree(&root)?;
            strata.scan(&root)?;
            w.roots.push(root);
        }
        Ok(w)
    }

    fn add_tree(&mut self, dir: &Path) -> Result<()> {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            if d.starts_with(self.strata.root()) {
                continue;
            }
            match self.inotify.watches().add(&d, mask()) {
                Ok(wd) => {
                    self.dirs.insert(wd, d.clone());
                }
                // Removed again before we got to it.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(Error::Io {
                        what: format!("watching {}", d.display()),
                        source: e,
                    });
                }
            }
            if let Ok(entries) = std::fs::read_dir(&d) {
                for e in entries.flatten() {
                    if e.file_type().is_ok_and(|t| t.is_dir()) {
                        stack.push(e.path());
                    }
                }
            }
        }
        Ok(())
    }

    fn record(&mut self, path: &Path, emit: &mut dyn FnMut(Event)) {
        if is_tmp(path) {
            return;
        }
        match self.strata.snapshot(path) {
            Ok((prev, Some(new))) => {
                if let Some(alert) = self.detector.observe(prev.as_ref(), &new) {
                    emit(Event::Recorded(new));
                    emit(Event::Alert(alert));
                } else {
                    emit(Event::Recorded(new));
                }
            }
            Ok((_, None)) => {}
            Err(e) => emit(Event::Error(format!("{}: {e}", path.display()))),
        }
    }

    /// Everything Strata knows below `dir` is gone, or moved out of view.
    fn record_tree_gone(&mut self, dir: &Path, emit: &mut dyn FnMut(Event)) {
        match self.strata.index().paths_under(dir) {
            Ok(paths) => {
                for p in paths {
                    self.record(&p, emit);
                }
            }
            Err(e) => emit(Event::Error(format!("{}: {e}", dir.display()))),
        }
    }

    fn handle(
        &mut self,
        dir: PathBuf,
        name: Option<&OsStr>,
        mask: EventMask,
        emit: &mut dyn FnMut(Event),
    ) {
        let Some(name) = name else { return };
        let path = dir.join(name);
        if mask.contains(EventMask::ISDIR) {
            if mask.intersects(EventMask::CREATE | EventMask::MOVED_TO) {
                if let Err(e) = self.add_tree(&path) {
                    emit(Event::Error(e.to_string()));
                }
                match self.strata.scan(&path) {
                    Ok(_) => {}
                    Err(e) => emit(Event::Error(e.to_string())),
                }
                // A folder moved in may hold files Strata has history for.
                self.record_tree_gone(&path, emit);
            } else if mask.intersects(EventMask::DELETE | EventMask::MOVED_FROM) {
                self.record_tree_gone(&path, emit);
            }
            return;
        }
        if mask.intersects(
            EventMask::CLOSE_WRITE
                | EventMask::MOVED_TO
                | EventMask::MOVED_FROM
                | EventMask::DELETE,
        ) {
            self.record(&path, emit);
        }
    }

    /// Process events until `stop` is set. Safe to kill at any time: objects
    /// are written atomically and the index is a WAL-mode SQLite database.
    pub fn run(&mut self, stop: &AtomicBool, emit: &mut dyn FnMut(Event)) -> Result<()> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let batch: Vec<(PathBuf, Option<std::ffi::OsString>, EventMask)> =
                match self.inotify.read_events(&mut buf) {
                    Ok(events) => {
                        let mut out = Vec::new();
                        for ev in events {
                            if ev.mask.contains(EventMask::Q_OVERFLOW) {
                                out.push((PathBuf::new(), None, EventMask::Q_OVERFLOW));
                                continue;
                            }
                            if ev.mask.contains(EventMask::IGNORED) {
                                self.dirs.remove(&ev.wd);
                                continue;
                            }
                            if let Some(dir) = self.dirs.get(&ev.wd) {
                                out.push((dir.clone(), ev.name.map(|n| n.to_os_string()), ev.mask));
                            }
                        }
                        out
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if stop.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        continue;
                    }
                    Err(e) => {
                        return Err(Error::Io {
                            what: "reading inotify events".into(),
                            source: e,
                        });
                    }
                };
            for (dir, name, mask) in batch {
                if mask.contains(EventMask::Q_OVERFLOW) {
                    for root in self.roots.clone() {
                        if let Err(e) = self.add_tree(&root).and_then(|_| self.strata.scan(&root)) {
                            emit(Event::Error(e.to_string()));
                        }
                        self.record_tree_gone(&root, emit);
                    }
                    emit(Event::Rescanned);
                    continue;
                }
                self.handle(dir, name.as_deref(), mask, emit);
            }
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
        }
    }
}
