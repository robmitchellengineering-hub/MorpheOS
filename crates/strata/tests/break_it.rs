//! "Try to Break It", Strata part: a folder survives editing, deletion and a
//! simulated ransomware attack, using the real inotify watcher.
//!
//! The watcher runs in its own thread with its own `Strata` handle, the way the
//! service and the `strata` command share one store in real use.
//!
//! **All three tests are `#[ignore]`d on non-Linux, and that is not laziness.** Two
//! need a working watcher, which inotify only provides on Linux. The third asserts that
//! `Watcher::new` **fails** when the store sits inside the tree it protects — and off
//! Linux the non-Linux stand-in fails for every input, so that assertion would hold
//! without checking its subject. Leaving it to pass would be worse than skipping it:
//! the test would look green and prove nothing. See `AGENTS.md` H9.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use strata::Strata;
use strata::detect::Alert;
use strata::time;
use strata::watch::{Event, Watcher};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("strata-breakit-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

/// Ordinary document text: low entropy, like real files.
fn document(n: usize) -> Vec<u8> {
    format!("Invoice {n}. Thank you for your business; payment is due within 30 days.\n")
        .repeat(80)
        .into_bytes()
}

/// What "encryption" looks like from the outside: bytes indistinguishable from random.
fn encrypt(data: &[u8], seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    data.iter()
        .map(|b| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            b ^ (x as u8)
        })
        .collect()
}

/// Every regular file below `dir`: relative path → (bytes, mode).
fn snapshot_tree(dir: &Path) -> BTreeMap<PathBuf, (Vec<u8>, u32)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() {
                let mode = e.metadata().unwrap().permissions().mode() & 0o7777;
                out.insert(
                    p.strip_prefix(dir).unwrap().to_path_buf(),
                    (fs::read(&p).unwrap(), mode),
                );
            }
        }
    }
    out
}

struct Running {
    stop: Arc<AtomicBool>,
    events: mpsc::Receiver<String>,
    alerts: mpsc::Receiver<Alert>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Running {
    fn start(store: &Path, dir: &Path) -> Running {
        let stop = Arc::new(AtomicBool::new(false));
        let (etx, events) = mpsc::channel();
        let (atx, alerts) = mpsc::channel();
        let (ready_tx, ready) = mpsc::channel();
        let (store, dir, stop2) = (store.to_path_buf(), dir.to_path_buf(), stop.clone());
        let thread = std::thread::spawn(move || {
            let s = Strata::open(&store).unwrap();
            let mut w = Watcher::new(&s, &[dir]).unwrap();
            ready_tx.send(()).unwrap();
            w.run(&stop2, &mut |e| match e {
                Event::Recorded(v) => {
                    let _ = etx.send(v.path.to_string_lossy().into_owned());
                }
                Event::Alert(a) => {
                    let _ = atx.send(a);
                }
                Event::Rescanned => {}
                Event::Error(m) => panic!("watcher error: {m}"),
            })
            .unwrap();
        });
        ready
            .recv_timeout(Duration::from_secs(20))
            .expect("watcher started");
        Running {
            stop,
            events,
            alerts,
            thread: Some(thread),
        }
    }

    /// Write a marker file and wait until the watcher has recorded it. Events
    /// arrive in order, so everything before the marker has been recorded too.
    fn sync(&self, dir: &Path, n: u32) {
        let marker = dir.join(format!(".sync-{n}"));
        fs::write(&marker, format!("sync {n}")).unwrap();
        let want = marker.to_string_lossy().into_owned();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(p) if p == want => return,
                Ok(_) => continue,
                Err(_) => panic!("watcher did not record {want} in time"),
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs the Linux inotify watcher")]
fn a_folder_survives_ransomware_and_the_rewind_can_be_undone() {
    let base = scratch("ransom");
    let docs = base.join("docs");
    let store = base.join("store");
    fs::create_dir_all(docs.join("2026/clients")).unwrap();

    // Some files exist before the watcher starts: the initial scan must catch them.
    for i in 0..5 {
        fs::write(docs.join(format!("early-{i}.txt")), document(i)).unwrap();
    }
    let watch = Running::start(&store, &docs);

    // Ordinary work: create, edit, delete, rename, change permissions.
    for i in 0..12 {
        let sub = if i % 2 == 0 { "" } else { "2026/clients" };
        fs::write(docs.join(sub).join(format!("invoice-{i}.txt")), document(i)).unwrap();
    }
    fs::write(docs.join("invoice-0.txt"), document(100)).unwrap();
    fs::remove_file(docs.join("early-4.txt")).unwrap();
    fs::rename(
        docs.join("early-3.txt"),
        docs.join("2026/early-3-renamed.txt"),
    )
    .unwrap();
    fs::set_permissions(
        docs.join("invoice-2.txt"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::write(docs.join("invoice-2.txt"), document(102)).unwrap(); // close-write records the mode
    // A photo: already high entropy, and legitimately so.
    fs::write(docs.join("photo.jpg"), encrypt(&vec![0u8; 50_000], 9)).unwrap();
    watch.sync(&docs, 1);

    let good = snapshot_tree(&docs);
    let good_at = time::now_ns();
    std::thread::sleep(Duration::from_millis(1500));

    // The attack.
    let mut victims: usize = 0;
    for (rel, (data, _)) in &good {
        let p = docs.join(rel);
        let name = rel.file_name().unwrap().to_string_lossy();
        if name.starts_with('.') || name.ends_with(".jpg") {
            continue;
        }
        victims += 1;
        if victims.is_multiple_of(2) {
            fs::write(&p, encrypt(data, victims as u64)).unwrap(); // in place
        } else {
            fs::write(
                p.with_file_name(format!("{name}.locked")),
                encrypt(data, victims as u64),
            )
            .unwrap();
            fs::remove_file(&p).unwrap(); // replace
        }
    }
    fs::set_permissions(
        docs.join("invoice-2.txt"),
        fs::Permissions::from_mode(0o644),
    )
    .ok();
    fs::write(
        docs.join("READ_ME_TO_RECOVER.txt"),
        b"Send 1 BTC to ...".repeat(20),
    )
    .unwrap();
    watch.sync(&docs, 2);
    assert!(victims >= 10, "attack touched only {victims} files");

    // The detector noticed, and its rewind point is after the good state and before the attack.
    let alert = watch.alerts.try_recv().expect("ransomware alert");
    assert!(
        alert.rewind_to > good_at,
        "rewind point is before the good state"
    );
    assert!(alert.files >= 8);

    // The alert is in the store, not only in the event stream: the heal engine and
    // the shell read it later, possibly after a reboot, from a fresh handle.
    let s = Strata::open(&store).unwrap();
    assert_eq!(s.alerts().unwrap(), vec![alert.clone()], "alert not kept");

    // Preview first: a dry run changes nothing.
    let attacked = snapshot_tree(&docs);
    let preview = s.rewind(&docs, alert.rewind_to, true).unwrap();
    assert!(preview.steps.len() >= victims, "{preview:#?}");
    assert_eq!(snapshot_tree(&docs), attacked, "dry run changed files");

    // Rewind.
    let before_rewind = time::now_ns();
    std::thread::sleep(Duration::from_millis(50));
    let report = s.rewind(&docs, alert.rewind_to, false).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    watch.sync(&docs, 3);
    let mut after = snapshot_tree(&docs);
    after.remove(Path::new(".sync-3"));
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        good.keys().collect::<Vec<_>>()
    );
    for (rel, want) in &good {
        assert_eq!(&after[rel], want, "{} differs after rewind", rel.display());
    }
    // Nothing the attack wrote was destroyed: it is in quarantine.
    let q = &report.quarantine;
    let note = q.join(
        docs.join("READ_ME_TO_RECOVER.txt")
            .strip_prefix("/")
            .unwrap(),
    );
    assert!(
        note.is_file(),
        "ransom note not in quarantine at {}",
        note.display()
    );

    // And the rewind itself can be undone: back to the attacked state...
    let undo = s.rewind(&docs, before_rewind, false).unwrap();
    assert!(undo.failed.is_empty(), "{:?}", undo.failed);
    let mut undone = snapshot_tree(&docs);
    undone.remove(Path::new(".sync-3"));
    let mut attacked_now = attacked.clone();
    attacked_now.remove(Path::new(".sync-3"));
    assert_eq!(
        undone, attacked_now,
        "undoing the rewind did not bring back the attacked state"
    );

    // ...and forward again.
    let redo = s.rewind(&docs, alert.rewind_to, false).unwrap();
    assert!(redo.failed.is_empty(), "{:?}", redo.failed);
    let mut again = snapshot_tree(&docs);
    again.retain(|rel, _| !rel.to_string_lossy().starts_with(".sync-3"));
    assert_eq!(again, good);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs the Linux inotify watcher")]
fn ordinary_work_raises_no_alert() {
    let base = scratch("calm");
    let docs = base.join("docs");
    let store = base.join("store");
    fs::create_dir_all(&docs).unwrap();
    let watch = Running::start(&store, &docs);
    for round in 0..3 {
        for i in 0..15 {
            fs::write(docs.join(format!("note-{i}.md")), document(i * 10 + round)).unwrap();
        }
        // Compressed archives are high entropy, and that is fine.
        fs::write(
            docs.join(format!("backup-{round}.zip")),
            encrypt(&vec![7u8; 20_000], round as u64 + 1),
        )
        .unwrap();
    }
    watch.sync(&docs, 1);
    assert!(
        watch.alerts.try_recv().is_err(),
        "false alarm on ordinary work"
    );
    assert!(
        Strata::open(&store).unwrap().alerts().unwrap().is_empty(),
        "false alarm kept in the store"
    );
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "vacuous off Linux: the non-Linux stand-in fails for every input, so this would pass unchecked"
)]
fn the_store_cannot_live_inside_what_it_protects() {
    let base = scratch("inside");
    let s = Strata::open(base.join("store")).unwrap();
    assert!(s.check_watchable(&base).is_err());
    assert!(Watcher::new(&s, std::slice::from_ref(&base)).is_err());
}
