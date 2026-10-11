//! `morph-transaction`: apply a generation, check the machine, and keep it only if the machine
//! agrees.

use std::process::ExitCode;
use std::time::Duration;

use morph_manifest::Error as ManifestError;
use morph_transaction::{Error, HealthCheck, Transaction};

const USAGE: &str = "\
usage:
  morph-transaction apply  --store <dir> --old <a.toml> --new <b.toml> --health <command>
                           [--approved-by <who>] [--timeout <seconds>]
                                                apply, check, then commit or roll back
  morph-transaction begin  --store <dir> --old <a.toml> --new <b.toml> [--approved-by <who>]
                                                arm a trial and apply, without deciding yet
  morph-transaction commit --store <dir>        keep the trial in progress
  morph-transaction rollback --store <dir>      take the trial back
  morph-transaction status --store <dir>        what is in force, and whether it is on trial

exit codes:
  0  committed, or the command succeeded
  1  REFUSED — a decision: the Manifest layer said no, or the health check failed and the
     transaction took itself back. The reason is on stderr and the output says what is in force.
  2  a defect or a usage error

`--health` runs through `/bin/sh -c`, so it can be a command line. A transaction with nothing
to check has no way to fail, so `--health` is required rather than optional.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["apply", rest @ ..] => cmd_apply(rest),
        ["begin", rest @ ..] => cmd_begin(rest),
        ["commit", rest @ ..] => cmd_commit(rest),
        ["rollback", rest @ ..] => cmd_rollback(rest),
        ["status", rest @ ..] => cmd_status(rest),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            // One line, and the marker printed once: a refusal's message must not arrive
            // wearing its own `refused:` prefix underneath this one.
            if morph_transaction::is_refusal(&e) {
                eprintln!("REFUSED: {}", e.reason());
            } else {
                eprintln!("error: {e}");
            }
            // The exit code is the contract: a decision is 1, a defect is 2.
            ExitCode::from(if morph_transaction::is_refusal(&e) {
                1
            } else {
                2
            })
        }
    }
}

fn usage(message: &str) -> Error {
    Error::Manifest(ManifestError::Usage(format!("{message}\n\n{USAGE}")))
}

/// The flags every subcommand shares, collected in one pass so no subcommand invents its own
/// spelling of `--store`.
#[derive(Default)]
struct Flags {
    store: Option<String>,
    old: Option<String>,
    new: Option<String>,
    health: Option<String>,
    approved_by: Option<String>,
    timeout: Option<u64>,
}

fn parse(args: &[&str], allowed: &[&str]) -> Result<Flags, Error> {
    let mut flags = Flags::default();
    let mut rest = args.iter().copied();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().map(str::to_string);
        match arg {
            "--store" => {
                flags.store = Some(value().ok_or_else(|| usage("--store needs a directory"))?)
            }
            "--old" => flags.old = Some(value().ok_or_else(|| usage("--old needs a file"))?),
            "--new" => flags.new = Some(value().ok_or_else(|| usage("--new needs a file"))?),
            "--health" => {
                flags.health = Some(value().ok_or_else(|| usage("--health needs a command"))?)
            }
            "--approved-by" => {
                flags.approved_by =
                    Some(value().ok_or_else(|| usage("--approved-by needs a name"))?)
            }
            "--timeout" => {
                let raw = value().ok_or_else(|| usage("--timeout needs a number of seconds"))?;
                flags.timeout = Some(
                    raw.parse()
                        .map_err(|_| usage(&format!("--timeout needs a number, not {raw:?}")))?,
                );
            }
            _ => return Err(usage(&format!("unknown option {arg}"))),
        }
        if !allowed.contains(&arg) {
            return Err(usage(&format!("{arg} is not an option of this command")));
        }
    }
    Ok(flags)
}

fn store_of(flags: &Flags, command: &str) -> Result<Transaction, Error> {
    let dir = flags
        .store
        .clone()
        .ok_or_else(|| usage(&format!("{command} needs --store <dir>")))?;
    Ok(Transaction::open(dir))
}

fn load(file: &str) -> Result<serde_json::Value, Error> {
    let source = std::fs::read_to_string(file).map_err(|e| ManifestError::Parse {
        origin: file.to_string(),
        message: e.to_string(),
    })?;
    Ok(morph_manifest::resolve_value(&source, file)?)
}

fn pair(flags: &Flags, command: &str) -> Result<(serde_json::Value, serde_json::Value), Error> {
    let old = flags
        .old
        .as_deref()
        .ok_or_else(|| usage(&format!("{command} needs --old <file>")))?;
    let new = flags
        .new
        .as_deref()
        .ok_or_else(|| usage(&format!("{command} needs --new <file>")))?;
    Ok((load(old)?, load(new)?))
}

fn report<T: serde::Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("the report serializes")
    );
}

fn cmd_apply(args: &[&str]) -> Result<bool, Error> {
    let flags = parse(
        args,
        &[
            "--store",
            "--old",
            "--new",
            "--health",
            "--approved-by",
            "--timeout",
        ],
    )?;
    let transaction = store_of(&flags, "apply")?;
    let command = flags.health.clone().ok_or_else(|| {
        usage(
            "apply needs --health <command>: the whole point of a transaction is the check, and \
             one that cannot fail is an apply with extra steps",
        )
    })?;
    let (old, new) = pair(&flags, "apply")?;

    let mut health = HealthCheck::new(command);
    if let Some(seconds) = flags.timeout {
        health = health.with_timeout(Duration::from_secs(seconds));
    }

    let outcome = transaction.apply(&old, &new, flags.approved_by.as_deref(), &health)?;
    report(&outcome);
    if !outcome.committed {
        eprintln!(
            "ROLLED BACK: {} — generation {} was trialled and taken back; generation {} is in \
             force",
            outcome
                .reason
                .as_deref()
                .unwrap_or("the health check failed"),
            outcome.trialled,
            outcome.generation
        );
    }
    Ok(outcome.committed)
}

fn cmd_begin(args: &[&str]) -> Result<bool, Error> {
    let flags = parse(args, &["--store", "--old", "--new", "--approved-by"])?;
    let transaction = store_of(&flags, "begin")?;
    let (old, new) = pair(&flags, "begin")?;
    report(&transaction.begin(&old, &new, flags.approved_by.as_deref())?);
    Ok(true)
}

fn cmd_commit(args: &[&str]) -> Result<bool, Error> {
    let flags = parse(args, &["--store"])?;
    report(&store_of(&flags, "commit")?.commit()?);
    Ok(true)
}

fn cmd_rollback(args: &[&str]) -> Result<bool, Error> {
    let flags = parse(args, &["--store"])?;
    report(&store_of(&flags, "rollback")?.rollback()?);
    Ok(true)
}

fn cmd_status(args: &[&str]) -> Result<bool, Error> {
    let flags = parse(args, &["--store"])?;
    let (current, trial) = store_of(&flags, "status")?.status()?;
    report(&serde_json::json!({ "current": current, "trial": trial }));
    Ok(true)
}
