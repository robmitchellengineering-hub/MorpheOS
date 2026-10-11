//! `morph-manifest`: validate, render and plan MorpheOS Manifests.

use std::process::ExitCode;

use morph_manifest::{Error, Severity, Store, apply, plan_values, resolve_value, validate};

const USAGE: &str = "\
usage:
  morph-manifest validate <machine.toml>       check structure and invariants
  morph-manifest render   <machine.toml>       print the merged manifest as JSON
  morph-manifest plan     <old.toml> <new.toml> show changes, risk, and whether it may auto-apply
  morph-manifest apply    <old.toml> <new.toml> (--to <file> | --store <dir>) [--approved-by <who>]
                                                apply it, or refuse and say why
  morph-manifest history  --store <dir>        every stored generation, oldest first
  morph-manifest rollback --store <dir> [--to <number>]
                                                make an older generation current again
  morph-manifest schema                        print the JSON Schema

exit codes:
  0  applied, or the command succeeded
  1  REFUSED — a decision, not a defect: the new generation is invalid, or it is above the tier
     an unattended caller may apply. The reason is on stderr and nothing was written.
  2  a defect or a usage error (unparseable file, missing argument)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["validate", file] => cmd_validate(file),
        ["render", file] => cmd_render(file),
        ["plan", old, new] => cmd_plan(old, new),
        ["apply", rest @ ..] => cmd_apply(rest),
        ["history", rest @ ..] => cmd_history(rest),
        ["rollback", rest @ ..] => cmd_rollback(rest),
        ["schema"] => {
            println!("{:#}", morph_manifest::json_schema());
            Ok(true)
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn load(file: &str) -> Result<serde_json::Value, Error> {
    let source = std::fs::read_to_string(file).map_err(|e| Error::Parse {
        origin: file.to_string(),
        message: e.to_string(),
    })?;
    resolve_value(&source, file)
}

fn cmd_validate(file: &str) -> Result<bool, Error> {
    let manifest = morph_manifest::layer::from_value(load(file)?)?;
    let report = validate(&manifest);
    for f in &report.findings {
        let tag = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{tag} [{}] {}: {}", f.code, f.path, f.message);
    }
    if report.is_ok() {
        println!(
            "ok: {} ({})",
            manifest.machine.name,
            manifest.machine.profile.as_str()
        );
    }
    Ok(report.is_ok())
}

fn cmd_render(file: &str) -> Result<bool, Error> {
    let value = load(file)?;
    morph_manifest::layer::from_value(value.clone())?;
    println!("{value:#}");
    Ok(true)
}

fn cmd_plan(old: &str, new: &str) -> Result<bool, Error> {
    let plan = plan_values(&load(old)?, &load(new)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&plan).expect("plan serializes")
    );
    Ok(plan.report.is_ok())
}

/// `apply <old> <new> (--to <file> | --store <dir>) [--approved-by <who>]`
///
/// A refusal is a **decision**, not a defect: the reason goes to stderr and the exit code is 1,
/// so a caller can tell "this must not happen" from "this could not be attempted". A usage error
/// stays exit 2 — and in particular a destination is required rather than defaulted, because a
/// default target is a way to write a generation somewhere nobody chose.
///
/// `--to` writes one generation over a file; `--store` keeps every generation and makes the new
/// one current. Exactly one, because "write it here" and "keep it here" are different promises
/// and a command that silently picked one would be wrong for whoever meant the other.
fn cmd_apply(args: &[&str]) -> Result<bool, Error> {
    let mut positional: Vec<&str> = Vec::new();
    let mut to: Option<&str> = None;
    let mut store: Option<&str> = None;
    let mut approved_by: Option<&str> = None;
    let mut rest = args.iter().copied();
    while let Some(arg) = rest.next() {
        match arg {
            "--to" => {
                to = Some(rest.next().ok_or_else(|| usage("--to needs a file"))?);
            }
            "--store" => {
                store = Some(
                    rest.next()
                        .ok_or_else(|| usage("--store needs a directory"))?,
                );
            }
            "--approved-by" => {
                approved_by = Some(
                    rest.next()
                        .ok_or_else(|| usage("--approved-by needs a name"))?,
                );
            }
            _ if arg.starts_with("--") => {
                return Err(usage(&format!("unknown option {arg}")));
            }
            _ => positional.push(arg),
        }
    }
    let [old, new] = positional.as_slice() else {
        return Err(usage("apply needs exactly <old> and <new>"));
    };
    match (to, store) {
        (Some(_), Some(_)) => return Err(usage("apply takes --to or --store, not both")),
        (None, None) => return Err(usage("apply needs --to <file> or --store <dir>")),
        _ => {}
    }
    let (old, new) = (load(old)?, load(new)?);

    let outcome = match store {
        Some(dir) => {
            let store = Store::open(dir);
            match store.apply(&old, &new, approved_by) {
                Ok(g) => {
                    let mut report = serde_json::json!({
                        "store": store.root().display().to_string(),
                        "generation": g.number,
                        "risk": g.risk,
                        "changes": g.changes,
                        "approved_by": g.approved_by,
                    });
                    // Absent rather than null when nobody had to approve, matching `Applied`.
                    if report["approved_by"].is_null() {
                        report
                            .as_object_mut()
                            .expect("an object")
                            .remove("approved_by");
                    }
                    Ok(report)
                }
                Err(e) => Err(e),
            }
        }
        None => apply(
            &old,
            &new,
            std::path::Path::new(to.expect("exactly one destination is set")),
            approved_by,
        )
        .map(|applied| serde_json::to_value(applied).expect("applied serializes")),
    };

    match outcome {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("the report serializes")
            );
            Ok(true)
        }
        Err(Error::Refused(reason)) => {
            eprintln!("REFUSED: {reason}");
            Ok(false)
        }
        Err(other) => Err(other),
    }
}

/// `history --store <dir>` — every stored generation, oldest first.
///
/// JSON, like `plan` and `apply`, so the same output can be read by a person and by `jq`.
fn cmd_history(args: &[&str]) -> Result<bool, Error> {
    let dir = only_store_flag(args, "history")?;
    let history = Store::open(dir).history()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&history).expect("the history serializes")
    );
    Ok(true)
}

/// `rollback --store <dir> [--to <number>]` — make an older generation current again.
///
/// Prints where it came from as well as where it went, because a rollback a caller cannot
/// check is a rollback it has to trust.
fn cmd_rollback(args: &[&str]) -> Result<bool, Error> {
    let mut dir: Option<&str> = None;
    let mut to: Option<u64> = None;
    let mut rest = args.iter().copied();
    while let Some(arg) = rest.next() {
        match arg {
            "--store" => {
                dir = Some(
                    rest.next()
                        .ok_or_else(|| usage("--store needs a directory"))?,
                );
            }
            "--to" => {
                let value = rest.next().ok_or_else(|| usage("--to needs a number"))?;
                to = Some(
                    value
                        .parse()
                        .map_err(|_| usage(&format!("--to needs a number, not {value:?}")))?,
                );
            }
            _ => return Err(usage(&format!("unknown option {arg}"))),
        }
    }
    let store = Store::open(dir.ok_or_else(|| usage("rollback needs --store <dir>"))?);

    let from = store.current()?.map(|g| g.number);
    match store.rollback(to) {
        Ok(generation) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "from": from,
                    "to": generation.number,
                    "risk": generation.risk,
                    "changes": generation.changes,
                }))
                .expect("the rollback report serializes")
            );
            Ok(true)
        }
        Err(Error::Refused(reason)) => {
            eprintln!("REFUSED: {reason}");
            Ok(false)
        }
        Err(other) => Err(other),
    }
}

fn only_store_flag<'a>(args: &[&'a str], command: &str) -> Result<&'a str, Error> {
    match args {
        ["--store", dir] => Ok(dir),
        _ => Err(usage(&format!("{command} takes only --store <dir>"))),
    }
}

/// A usage error is a defect in the invocation (exit 2), not a refusal (exit 1).
fn usage(message: &str) -> Error {
    Error::Usage(format!("{message}\n\n{USAGE}"))
}
