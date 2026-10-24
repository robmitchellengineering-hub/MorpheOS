//! `morphd`: make the Manifest generation in force real on the machine.
//!
//! This is the half the Manifest has been missing: `morph-manifest` validates and stores
//! generations, and `morphd` obeys the one in force. `boot` converges the machine to the newest
//! generation that parses, validates and matches its digest at every start (first resolving an
//! interrupted transaction, then rolling back a corrupt `current` — rung H2); `apply` runs one
//! transaction — arm a trial, apply the settings, health-check, commit or roll back.

use std::process::ExitCode;

use morph_manifest::Manifest;
use morph_transaction::Transaction;
use morphd::{Applier, Hostname, MinFreePercent, MinFreePercentPolicy, Retention, RetentionPolicy};

/// Where the store and the transaction live (doc 20, S11 note 1).
const STORE: &str = "/persist/morph";

const USAGE: &str = "\
usage: morphd <command>

commands:
  boot                                   converge the machine to the current generation
  apply <new.toml> [--approved-by WHO]   run one transaction applying <new.toml>";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("morphd: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().map(String::as_str).collect();
    match rest.as_slice() {
        ["boot"] => boot(),
        ["apply", new, opts @ ..] => apply(new, opts),
        _ => Err(format!("unknown command\n{USAGE}")),
    }
}

fn flag<'a>(args: &'a [&'a str], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| *a == name)
        .and_then(|i| args.get(i + 1).copied())
}

/// The current generation's Manifest, or an error naming what is missing.
fn current_manifest(transaction: &Transaction) -> Result<(u64, Manifest), String> {
    let current = transaction
        .store()
        .current()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "the store has no generation; seed it with `morph-manifest apply --store`".to_string()
        })?;
    let value = transaction
        .store()
        .load(current.number)
        .map_err(|e| e.to_string())?;
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|e| format!("generation {} is not a Manifest: {e}", current.number))?;
    Ok((current.number, manifest))
}

/// Resolve an interrupted transaction: the marker outlives the process, so the next `boot` or
/// `apply` takes the machine back rather than leaving a nobody-decided change in force.
fn resolve_interrupted(transaction: &Transaction) -> Result<(), String> {
    if transaction.trial().map_err(|e| e.to_string())?.is_some() {
        transaction.rollback().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Apply the hostname, retention and free-space floor a Manifest names.
fn apply_settings(manifest: &Manifest) -> Result<(), String> {
    let hostname = Hostname::new();
    hostname.apply(&manifest.machine.name)?;
    let retention = RetentionPolicy::new();
    retention.apply(&Retention {
        all_versions_hours: manifest.storage.retention.all_versions_hours,
        hourly_days: manifest.storage.retention.hourly_days,
        daily_days: manifest.storage.retention.daily_days,
    })?;
    let min_free = MinFreePercentPolicy::new();
    min_free.apply(&MinFreePercent {
        min_free_percent: manifest.storage.min_free_percent,
    })?;
    Ok(())
}

/// Whether the machine reports the values `manifest` names, and the Strata service is active.
fn healthy(manifest: &Manifest) -> bool {
    let hostname_ok = Hostname::new()
        .read_back()
        .map(|n| n == manifest.machine.name)
        .unwrap_or(false);
    let retention_ok = RetentionPolicy::new()
        .read_back()
        .map(|r| {
            r.all_versions_hours == manifest.storage.retention.all_versions_hours
                && r.hourly_days == manifest.storage.retention.hourly_days
                && r.daily_days == manifest.storage.retention.daily_days
        })
        .unwrap_or(false);
    let min_free_ok = MinFreePercentPolicy::new()
        .read_back()
        .map(|f| f.min_free_percent == manifest.storage.min_free_percent)
        .unwrap_or(false);
    // `strata.service` active is the health check the card names (note 1); a systemd call is
    // the one thing this process does that the appliers do not.
    let strata = std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", "strata.service"])
        .status()
        .is_ok_and(|s| s.success());
    hostname_ok && retention_ok && min_free_ok && strata
}

/// Converge the machine to the newest good generation, first undoing any interrupted change.
///
/// If the generation in force no longer parses, validates, or matches its digest, `converge`
/// rolls `current` back to the newest generation that does and records the heal; then that
/// generation's settings are applied. A store with one corrupt generation refuses and reports.
fn boot() -> Result<(), String> {
    let transaction = Transaction::open(STORE);
    resolve_interrupted(&transaction)?;
    let converged = morphd::converge(transaction.store())?;
    if let Some((from, reason)) = converged.rolled_back_from.zip(converged.reason.as_ref()) {
        eprintln!(
            "morphd: generation {from} was rolled back ({reason}); generation {} is now in force",
            converged.number
        );
        // The shell shows the rollback (doc 20, S8). morphd runs before the compositor is up,
        // so it stages the notice under /run — a tmpfs writable however early this is — and
        // flow-rollback-notice.service shows it once the shell is on screen. Staging is
        // best-effort: the rollback itself is the recovery, and a notice that could not be
        // written must not turn a successful rollback into a failed boot (S11's trap).
        if let Err(e) = stage_rollback_notice(from, reason, converged.number) {
            eprintln!("morphd: could not stage the rollback notice: {e}");
        }
    }
    apply_settings(&converged.manifest)?;
    Ok(())
}

/// Stage the notice the shell shows when a generation was rolled back (doc 20, S8).
///
/// The file is two lines — the message, then the colour name — the same shape the space guard
/// stages (`show-space-notice`, S7), so the notice's reader stays one contract. The colour is
/// `warn`: flow-notice's `colours::WARN` is documented as "a change was rolled back (S8)".
fn stage_rollback_notice(from: u64, reason: &str, now: u64) -> Result<(), String> {
    let path = "/run/morpheos/rollback-notice";
    let message = format!(
        "configuration generation {from} was rolled back ({reason}); generation {now} is now in force"
    );
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    std::fs::write(path, format!("{message}\nwarn\n"))
        .map_err(|e| format!("writing {path}: {e}"))
}

/// Run one transaction: parse + validate the new Manifest, arm the trial, apply the settings,
/// health-check, then commit or roll back.
fn apply(new_toml: &str, opts: &[&str]) -> Result<(), String> {
    let approved_by = flag(opts, "--approved-by");
    let transaction = Transaction::open(STORE);
    resolve_interrupted(&transaction)?;

    let (old_number, old_manifest) = current_manifest(&transaction)?;
    let old_value = transaction
        .store()
        .load(old_number)
        .map_err(|e| e.to_string())?;

    let source =
        std::fs::read_to_string(new_toml).map_err(|e| format!("reading {new_toml}: {e}"))?;
    let new_value = morph_manifest::resolve_value(&source, new_toml).map_err(|e| e.to_string())?;
    let manifest: Manifest = serde_json::from_value(new_value.clone())
        .map_err(|e| format!("{new_toml} is not a Manifest: {e}"))?;
    let report = morph_manifest::validate(&manifest);
    if !report.is_ok() {
        return Err(format!(
            "{new_toml} does not validate: {}",
            report
                .findings
                .iter()
                .filter(|f| f.severity == morph_manifest::Severity::Error)
                .map(|f| f.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    // Arm the trial — the marker is written before anything on the machine changes.
    let trial = transaction
        .begin(&old_value, &new_value, approved_by)
        .map_err(|e| e.to_string())?;

    if let Err(e) = apply_settings(&manifest) {
        // The change could not be applied: take it back, store and machine.
        transaction.rollback().map_err(|err| err.to_string())?;
        undo_settings(&old_manifest)?;
        return Err(format!(
            "applying the change failed; it was rolled back: {e}"
        ));
    }

    if healthy(&manifest) {
        transaction.commit().map_err(|e| e.to_string())?;
        eprintln!(
            "generation {} is in force (replaced generation {})",
            trial.to, trial.from
        );
        Ok(())
    } else {
        transaction.rollback().map_err(|e| e.to_string())?;
        undo_settings(&old_manifest)?;
        Err("the change did not pass its health check and was rolled back".to_string())
    }
}

/// Put the previous generation's settings back, for a refused apply or a rollback.
fn undo_settings(old: &Manifest) -> Result<(), String> {
    Hostname::new().undo(&old.machine.name)?;
    RetentionPolicy::new().undo(&Retention {
        all_versions_hours: old.storage.retention.all_versions_hours,
        hourly_days: old.storage.retention.hourly_days,
        daily_days: old.storage.retention.daily_days,
    })?;
    MinFreePercentPolicy::new().undo(&MinFreePercent {
        min_free_percent: old.storage.min_free_percent,
    })?;
    Ok(())
}
