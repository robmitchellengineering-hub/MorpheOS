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
                                     thin out old versions (defaults 24 / 30 / 365)
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
            let policy = Retention::from_manifest(
                num("--keep-hours", 24)?,
                num("--hourly-days", 30)?,
                num("--daily-days", 365)?,
            );
            let (versions, gc) = s.prune(&policy, time::now_ns()).map_err(err)?;
            println!(
                "{versions} old versions removed; {} objects ({} bytes) freed",
                gc.objects_removed, gc.bytes_freed
            );
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
