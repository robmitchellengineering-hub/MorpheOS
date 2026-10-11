//! `morph-manifest`: validate, render and plan MorpheOS Manifests.

use std::process::ExitCode;

use morph_manifest::{Error, Severity, apply, plan_values, resolve_value, validate};

const USAGE: &str = "\
usage:
  morph-manifest validate <machine.toml>       check structure and invariants
  morph-manifest render   <machine.toml>       print the merged manifest as JSON
  morph-manifest plan     <old.toml> <new.toml> show changes, risk, and whether it may auto-apply
  morph-manifest apply    <old.toml> <new.toml> --to <file> [--approved-by <who>]
                                                apply it, or refuse and say why
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

/// `apply <old> <new> --to <file> [--approved-by <who>]`
///
/// A refusal is a **decision**, not a defect: the reason goes to stderr and the exit code is 1,
/// so a caller can tell "this must not happen" from "this could not be attempted". A usage error
/// stays exit 2 — and in particular `--to` is required rather than defaulted, because a default
/// target is a way to write a generation somewhere nobody chose.
fn cmd_apply(args: &[&str]) -> Result<bool, Error> {
    let mut positional: Vec<&str> = Vec::new();
    let mut to: Option<&str> = None;
    let mut approved_by: Option<&str> = None;
    let mut rest = args.iter().copied();
    while let Some(arg) = rest.next() {
        match arg {
            "--to" => {
                to = Some(rest.next().ok_or_else(|| usage("--to needs a file"))?);
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
    let to = to.ok_or_else(|| usage("apply needs --to <file>"))?;

    match apply(
        &load(old)?,
        &load(new)?,
        std::path::Path::new(to),
        approved_by,
    ) {
        Ok(applied) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&applied).expect("applied serializes")
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

/// A usage error is a defect in the invocation (exit 2), not a refusal (exit 1).
fn usage(message: &str) -> Error {
    Error::Usage(format!("{message}\n\n{USAGE}"))
}
