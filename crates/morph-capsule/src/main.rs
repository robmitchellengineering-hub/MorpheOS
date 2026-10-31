//! `morph-session` — a session you can drive from a shell, and whose history you can check.
//!
//! The library is only half the capability: a person cannot use a test. This binary is the other half, and it
//! is deliberately small — open, run, undo, history — because those four verbs are the whole promise:
//! "the thing I was doing, its state, and the ability to take it back".
//!
//!   morph-session keygen  --out PATH
//!   morph-session open    --root DIR --label L [--query URL] --key PATH
//!   morph-session run     --root DIR --key PATH [--isolated] -- <command...>
//!   morph-session undo    --root DIR --key PATH [--at NS]
//!   morph-session history --root DIR
//!
//! Exit codes: 0 done, 1 refused, 2 misuse. `history` refuses a history that does not verify, because a history
//! a tool cannot check is not evidence.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ed25519_dalek::SigningKey;
use morph_attest::unhex;
use morph_capsule::{Error, SealedSession};

fn arg(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

fn key_from(path: &Path) -> Result<SigningKey, String> {
    let raw = std::fs::read(path).map_err(|e| format!("no key at {}: {e}", path.display()))?;
    let bytes = if raw.len() == 32 {
        raw
    } else {
        unhex(String::from_utf8_lossy(&raw).trim()).ok_or("key is neither 32 bytes nor hex")?
    };
    let arr: [u8; 32] = bytes.try_into().map_err(|_| "key is not 32 bytes")?;
    Ok(SigningKey::from_bytes(&arr))
}

fn open(args: &[String]) -> Result<SealedSession, String> {
    let root = arg(args, "--root").ok_or("--root is required")?;
    let key_path = arg(args, "--key").ok_or("--key is required: a session without a key cannot be sealed")?;
    let label = arg(args, "--label").unwrap_or_else(|| "session".into());
    let query = arg(args, "--query").unwrap_or_else(|| "strata://query?under=.".into());
    let key = key_from(Path::new(&key_path))?;
    SealedSession::open(root, &label, &query, key).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "keygen" => {
            let Some(out) = arg(&args, "--out") else {
                eprintln!("morph-session keygen --out PATH");
                return ExitCode::from(2);
            };
            let out = PathBuf::from(out);
            if out.exists() {
                eprintln!("morph-session: {} already exists; refusing to replace a key", out.display());
                return ExitCode::from(1);
            }
            let mut seed = [0u8; 32];
            if std::fs::File::open("/dev/urandom")
                .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut seed))
                .is_err()
            {
                eprintln!("morph-session: could not read /dev/urandom");
                return ExitCode::from(1);
            }
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&out, morph_attest::hex(&seed)).is_err() {
                eprintln!("morph-session: could not write {}", out.display());
                return ExitCode::from(1);
            }
            let key = SigningKey::from_bytes(&seed);
            println!(
                "key: {}\nverifying key (give this to whoever must check the history):\n{}",
                out.display(),
                morph_attest::hex(key.verifying_key().as_bytes())
            );
            ExitCode::SUCCESS
        }
        "open" => match open(&args) {
            Ok(s) => {
                println!("session opened; verifying key: {}", s.verifying_key());
                ExitCode::SUCCESS
            }
            Err(why) => {
                eprintln!("morph-session: {why}");
                ExitCode::from(1)
            }
        },
        "run" => {
            let Some(split) = args.iter().position(|a| a == "--") else {
                eprintln!("morph-session run ... -- <command...>");
                return ExitCode::from(2);
            };
            let command = args[split + 1..].join(" ");
            if command.trim().is_empty() {
                eprintln!("morph-session: no command after --");
                return ExitCode::from(2);
            }
            let isolated = args.iter().any(|a| a == "--isolated");
            let session = match open(&args) {
                Ok(s) => s,
                Err(why) => {
                    eprintln!("morph-session: {why}");
                    return ExitCode::from(1);
                }
            };
            let result = session.change(&command, |layer| {
                if isolated {
                    // The command runs inside the capsule's own root and namespaces, so it cannot reach the
                    // machine — and whatever it writes lands in the layer this change seals.
                    let (code, out, err) = layer.capsule().run_isolated(&command)?;
                    print!("{out}");
                    eprint!("{err}");
                    match code {
                        Some(0) => Ok(()),
                        Some(c) => Err(Error::Refused(format!("the command exited {c}"))),
                        None => Err(Error::Refused("the command was killed".into())),
                    }
                } else {
                    let (code, out, err) = layer.capsule().run(&command)?;
                    print!("{out}");
                    eprint!("{err}");
                    match code {
                        Some(0) => Ok(()),
                        Some(c) => Err(Error::Refused(format!("the command exited {c}"))),
                        None => Err(Error::Refused("the command was killed".into())),
                    }
                }
            });
            match result {
                Ok(committed) => {
                    println!(
                        "sealed: {} (transition {})",
                        committed.sealed.record.label, committed.sealed.record.sealed_at
                    );
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("morph-session: {why}");
                    ExitCode::from(1)
                }
            }
        }
        "undo" => {
            let session = match open(&args) {
                Ok(s) => s,
                Err(why) => {
                    eprintln!("morph-session: {why}");
                    return ExitCode::from(1);
                }
            };
            let at = match arg(&args, "--at") {
                Some(v) => match v.parse::<i64>() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("morph-session: --at is not a number");
                        return ExitCode::from(2);
                    }
                },
                None => session
                    .history()
                    .ok()
                    .and_then(|h| h.last().map(|s| s.record.at))
                    .unwrap_or(0),
            };
            match session.undo_to(at) {
                Ok(committed) => {
                    println!(
                        "undone and sealed: {} paths restored (transition {})",
                        committed.restored, committed.sealed.record.sealed_at
                    );
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("morph-session: {why}");
                    ExitCode::from(1)
                }
            }
        }
        "history" => {
            let root = arg(&args, "--root").unwrap_or_else(|| ".".into());
            let key_path = arg(&args, "--key");
            // A history can be listed without a key (verification is by the recorded key), but a session is
            // opened with one; use a throwaway key purely to reach the layer when none was given.
            let key = match key_path {
                Some(p) => match key_from(Path::new(&p)) {
                    Ok(k) => k,
                    Err(why) => {
                        eprintln!("morph-session: {why}");
                        return ExitCode::from(1);
                    }
                },
                None => SigningKey::from_bytes(&[0u8; 32]),
            };
            match SealedSession::open(&root, "session", "strata://query?under=.", key)
                .and_then(|s| s.history())
            {
                Ok(history) => {
                    for s in &history {
                        println!(
                            "{}  {}  verified  key {}",
                            s.record.sealed_at, s.record.label, s.key
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("morph-session: {why}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("morph-session: unknown command '{other}' (keygen|open|run|undo|history)");
            ExitCode::from(2)
        }
    }
}
