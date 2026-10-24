//! `strata`: every saved version of every file, and a way back to any of them.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use strata::index::Kind;
use strata::retention::Retention;
use strata::watch::{Event, Watcher};
use strata::{Strata, time};

const USAGE: &str = "\
usage: strata [--store DIR] <command>

commands:
  watch <folder>...                  keep a version of every save (runs until stopped)
  scan <folder>                      record the current state of every file once
  history <file>                     list a file's versions
  restore <file> [--at TIME | --version ID] [--to PATH]
                                     put a version back (default: the newest)
  rewind <folder> --at TIME [--dry-run]
                                     put a whole folder back the way it was at TIME;
                                     nothing is deleted (newer files go to quarantine)
  prune [--keep-hours N] [--hourly-days N] [--daily-days N]
                                     thin out old versions (no flags: the policy `morphd`
                                     wrote, else 24 / 30 / 365)
  guard                              when free space is below the floor `morphd` wrote
                                     (else 10%), prune by the retention policy
  policy                            the retention policy in force
  alerts [--json]                    ransomware-like bursts seen by `watch`, oldest first,
                                     each with the rewind command that undoes it
  stats                              versions, files and store size

TIME: -90s | -15m | -2h | -3d | @<unix seconds> | 2026-10-10T13:05:09Z
store: --store, else $STRATA_STORE, else /var/lib/strata";

struct Args {
    store: PathBuf,
    rest: Vec<String>,
}

fn parse_args() -> Option<Args> {
    let mut store = std::env::var_os("STRATA_STORE").map(PathBuf::from);
    let mut rest = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--store" {
            store = Some(PathBuf::from(it.next()?));
        } else {
            rest.push(a);
        }
    }
    Some(Args {
        store: store.unwrap_or_else(|| PathBuf::from("/var/lib/strata")),
        rest,
    })
}

/// Absolute path without requiring the file to exist (it may have been deleted).
fn absolute(p: &str) -> Result<PathBuf, String> {
    let p = Path::new(p);
    if let Ok(c) = p.canonicalize() {
        return Ok(c);
    }
    let name = p
        .file_name()
        .ok_or_else(|| format!("{}: not a file path", p.display()))?;
    let parent = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    Ok(parent.join(name))
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Where `morphd` writes the Manifest's `storage.retention` (doc 20, S11). The file is the
/// interface between the two: `morphd` decides the policy, Strata obeys it, and neither reads
/// the other's crate.
const POLICY_FILE: &str = "/persist/morph/strata-retention.json";

/// Where `morphd` writes the Manifest's `storage.min_free_percent` (doc 20, S7). The same
/// interface as [`POLICY_FILE`]: the floor is decided in one place and obeyed in another.
const MIN_FREE_FILE: &str = "/persist/morph/strata-min-free.json";

/// The raw policy the Manifest named, as `morphd` wrote it.
#[derive(serde::Deserialize)]
struct WrittenPolicy {
    all_versions_hours: u32,
    hourly_days: u32,
    daily_days: u32,
}

/// The raw free-space floor the Manifest named, as `morphd` wrote it.
#[derive(serde::Deserialize)]
struct WrittenFloor {
    min_free_percent: u8,
}

/// The policy `morphd` wrote, or `None` when it has never been written (the machine has not
/// yet obeyed a generation's `storage.retention`).
fn written_policy() -> Result<Option<WrittenPolicy>, String> {
    let text = match std::fs::read_to_string(POLICY_FILE) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("reading {POLICY_FILE}: {e}")),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{POLICY_FILE} is not a retention policy: {e}"))
}

/// The floor `morphd` wrote, or `None` when it has never been written.
fn written_min_free() -> Result<Option<u8>, String> {
    let text = match std::fs::read_to_string(MIN_FREE_FILE) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("reading {MIN_FREE_FILE}: {e}")),
    };
    let floor: WrittenFloor = serde_json::from_str(&text)
        .map_err(|e| format!("{MIN_FREE_FILE} is not a free-space floor: {e}"))?;
    Ok(Some(floor.min_free_percent))
}

fn when(args: &[String]) -> Result<Option<i64>, String> {
    match flag(args, "--at") {
        None => Ok(None),
        Some(t) => time::parse(t, time::now_ns())
            .map(Some)
            .ok_or_else(|| format!("cannot read time '{t}'\n\n{USAGE}")),
    }
}

fn run(a: Args) -> Result<(), String> {
    let err = |e: strata::Error| e.to_string();
    let rest: Vec<&str> = a.rest.iter().map(String::as_str).collect();
    match rest.as_slice() {
        ["watch", folders @ ..] if !folders.is_empty() => {
            let s = Strata::open(&a.store).map_err(err)?;
            let roots: Vec<PathBuf> = folders.iter().map(PathBuf::from).collect();
            let mut w = Watcher::new(&s, &roots).map_err(err)?;
            eprintln!(
                "strata: watching {} folder(s); store {}",
                roots.len(),
                s.root().display()
            );
            let stop = AtomicBool::new(false);
            w.run(&stop, &mut |e| match e {
                Event::Recorded(v) => println!(
                    "{} {} {}",
                    time::format(v.ts),
                    if v.kind == Kind::File {
                        "saved  "
                    } else {
                        "deleted"
                    },
                    v.path.display()
                ),
                Event::Alert(al) => {
                    println!(
                        "ALERT ransomware-like activity: {} files turned into ciphertext since {}",
                        al.files,
                        time::format(al.first_ts)
                    );
                    for f in &al.folders {
                        println!(
                            "ALERT   get them back: strata rewind {} --at {}",
                            f.display(),
                            time::format(al.rewind_to)
                        );
                    }
                }
                Event::Rescanned => eprintln!("strata: event queue overflowed; rescanned"),
                Event::Error(m) => eprintln!("strata: {m}"),
            })
            .map_err(err)
        }
        ["scan", folder] => {
            let s = Strata::open(&a.store).map_err(err)?;
            let (seen, rec) = s.scan(&absolute(folder)?).map_err(err)?;
            println!("{seen} files seen, {rec} new versions recorded");
            Ok(())
        }
        ["history", file] => {
            let s = Strata::open(&a.store).map_err(err)?;
            let path = absolute(file)?;
            let h = s.index().history(&path).map_err(err)?;
            if h.is_empty() {
                return Err(format!("no history for {}", path.display()));
            }
            for v in h {
                match v.kind {
                    Kind::File => println!(
                        "{:>6}  {}  {:>10} bytes  mode {:o}  entropy {:.2}",
                        v.id,
                        time::format(v.ts),
                        v.size,
                        v.mode,
                        v.entropy
                    ),
                    Kind::Deleted => println!("{:>6}  {}  deleted", v.id, time::format(v.ts)),
                }
            }
            Ok(())
        }
        ["restore", file, opts @ ..] => {
            let s = Strata::open(&a.store).map_err(err)?;
            let opts: Vec<String> = opts.iter().map(|o| o.to_string()).collect();
            let path = absolute(file)?;
            let v = if let Some(id) = flag(&opts, "--version") {
                let id: i64 = id.parse().map_err(|_| format!("bad version '{id}'"))?;
                s.index()
                    .get(id)
                    .map_err(err)?
                    .filter(|v| v.path == path)
                    .ok_or_else(|| format!("{} has no version {id}", path.display()))?
            } else if let Some(at) = when(&opts)? {
                s.index().at(&path, at).map_err(err)?.ok_or_else(|| {
                    format!("{} did not exist at {}", path.display(), time::format(at))
                })?
            } else {
                s.index()
                    .history(&path)
                    .map_err(err)?
                    .into_iter()
                    .rev()
                    .find(|v| v.kind == Kind::File)
                    .ok_or_else(|| format!("no saved version of {}", path.display()))?
            };
            let to = match flag(&opts, "--to") {
                Some(t) => absolute(t)?,
                None => path.clone(),
            };
            if to == path {
                s.snapshot(&path).map_err(err)?; // keep what is there now, too
            }
            s.restore_version(&v, &to).map_err(err)?;
            println!(
                "restored version {} ({}) to {}",
                v.id,
                time::format(v.ts),
                to.display()
            );
            Ok(())
        }
        ["rewind", folder, opts @ ..] => {
            let s = Strata::open(&a.store).map_err(err)?;
            let opts: Vec<String> = opts.iter().map(|o| o.to_string()).collect();
            let at = when(&opts)?.ok_or_else(|| format!("rewind needs --at TIME\n\n{USAGE}"))?;
            let dry = opts.iter().any(|o| o == "--dry-run");
            let r = s.rewind(&absolute(folder)?, at, dry).map_err(err)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&r).expect("report serializes")
            );
            if r.failed.is_empty() {
                Ok(())
            } else {
                Err(format!("{} file(s) could not be rewound", r.failed.len()))
            }
        }
        ["prune", opts @ ..] => {
            let mut s = Strata::open(&a.store).map_err(err)?;
            let opts: Vec<String> = opts.iter().map(|o| o.to_string()).collect();
            let num = |name: &str, default: u32| -> Result<u32, String> {
                flag(&opts, name).map_or(Ok(default), |v| {
                    v.parse().map_err(|_| format!("bad {name} '{v}'"))
                })
            };
            // Flags override the policy file; with none, the Manifest's `storage.retention`
            // (written by `morphd`) is the policy in force — pruning obeys the generation,
            // not a hard-coded default.
            let policy = if opts.iter().any(|o| o.starts_with("--")) {
                Retention::from_manifest(
                    num("--keep-hours", 24)?,
                    num("--hourly-days", 30)?,
                    num("--daily-days", 365)?,
                )
            } else {
                match written_policy()? {
                    Some(p) => {
                        Retention::from_manifest(p.all_versions_hours, p.hourly_days, p.daily_days)
                    }
                    None => Retention::from_manifest(24, 30, 365),
                }
            };
            let (versions, gc) = s.prune(&policy, time::now_ns()).map_err(err)?;
            println!(
                "{versions} old versions removed; {} objects ({} bytes) freed",
                gc.objects_removed, gc.bytes_freed
            );
            Ok(())
        }
        ["guard"] => {
            let mut s = Strata::open(&a.store).map_err(err)?;
            // The floor is the Manifest's `storage.min_free_percent`, written by `morphd`;
            // 10 is the base profile's default, used only before `morphd` has run once.
            let floor = written_min_free()?.unwrap_or(10);
            let policy = match written_policy()? {
                Some(p) => {
                    Retention::from_manifest(p.all_versions_hours, p.hourly_days, p.daily_days)
                }
                None => Retention::from_manifest(24, 30, 365),
            };
            match strata::guard::guard(&mut s, floor, &policy, time::now_ns()).map_err(err)? {
                Some(r) => {
                    // Say "now", not only "was": the shell's warning reports how full the disk
                    // is after the guard ran, and whether pruning could get back above the
                    // floor — when it could not, that is the "why not" the notice shows rather
                    // than deleting beyond the policy (doc 20, S7 note 5).
                    let after = if r.free_percent_after >= r.floor_percent {
                        format!("back above the {}% floor", r.floor_percent)
                    } else {
                        format!("still below the {}% floor", r.floor_percent)
                    };
                    println!(
                        "free space {}% was below the {}% floor; pruned {} old versions ({} objects, {} bytes freed); now {}% free, {after}",
                        r.free_percent,
                        r.floor_percent,
                        r.versions_removed,
                        r.objects_removed,
                        r.bytes_freed,
                        r.free_percent_after
                    );
                }
                None => {
                    let free = strata::guard::free_percent(s.root()).map_err(err)?;
                    println!("free space {free}% is at or above the {floor}% floor; nothing to do");
                }
            }
            Ok(())
        }
        ["policy"] => match written_policy()? {
            Some(p) => {
                println!(
                    "keep every version under {} hours; then one per hour for {} days; then one per day for {} days",
                    p.all_versions_hours, p.hourly_days, p.daily_days
                );
                Ok(())
            }
            None => {
                println!("no retention policy written (default: 24 hours / 30 days / 365 days)");
                Ok(())
            }
        },
        ["alerts", opts @ ..] if opts.iter().all(|o| *o == "--json") => {
            let s = Strata::open(&a.store).map_err(err)?;
            let alerts = s.alerts().map_err(err)?;
            let json = !opts.is_empty();
            if alerts.is_empty() && !json {
                println!("no alerts");
            }
            for al in &alerts {
                let rewind_at = time::format(al.rewind_to);
                if json {
                    // The alert as stored, plus the rewind point in the form `--at` reads,
                    // so a script can pass it straight back without date arithmetic.
                    let mut v = serde_json::to_value(al).expect("alert serializes");
                    v["rewind_at"] = rewind_at.into();
                    println!("{v}");
                    continue;
                }
                println!(
                    "{}  {} files turned into ciphertext",
                    time::format(al.first_ts),
                    al.files
                );
                for f in &al.folders {
                    println!("    strata rewind {} --at {rewind_at}", f.display());
                }
            }
            Ok(())
        }
        ["stats"] => {
            let s = Strata::open(&a.store).map_err(err)?;
            let (versions, paths) = s.index().counts().map_err(err)?;
            println!(
                "{versions} versions of {paths} files in {}",
                s.root().display()
            );
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let Some(args) = parse_args() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("strata: {e}");
            ExitCode::FAILURE
        }
    }
}
