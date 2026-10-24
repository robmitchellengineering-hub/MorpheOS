//! The H2 heal: the generation in force no longer parses, validates, or matches its digest,
//! so the machine is taken back to the newest generation that does.
//!
//! This is rung H2 of the heal ladder (doc 15): roll back the **one** configuration change
//! that went bad, not the whole machine. Three checks gate a generation before it is obeyed —
//! it parses, it validates against the Manifest's invariants, and its stored file matches the
//! digest recorded when it was applied — and a generation that fails any of them is not.
//!
//! The heal records itself in `heal.jsonl` beside the store, so a later boot (or a person) can
//! read *from the store's own record* which generation was rolled back, why, and what is in
//! force now (AGENTS.md H8 — never from a log line). A store with only one generation has
//! nothing to roll back to, so [`converge`] refuses rather than inventing a default.

use std::io::Write;

use morph_manifest::{Manifest, Store};
use serde::Serialize;

/// What [`converge`] did: the generation now in force, and whether it took a rollback to get
/// there.
#[derive(Debug, Clone, PartialEq)]
pub struct Convergence {
    /// The generation the machine should obey — the one that passes all three checks.
    pub number: u64,
    /// That generation's Manifest, already judged good and ready to be applied.
    pub manifest: Manifest,
    /// The generation that was rolled back, when one was.
    pub rolled_back_from: Option<u64>,
    /// Why it was rolled back, when it was.
    pub reason: Option<String>,
}

/// One line of `heal.jsonl`: which generation was rolled back, why, and to where.
///
/// JSON Lines on purpose: each heal *appends* without rewriting the file, so a crash between
/// appends loses at most the record being written, and the file is greppable line by line.
#[derive(Serialize)]
struct HealRecord {
    rung: &'static str,
    bad: u64,
    reason: String,
    now: u64,
    at: u64,
}

/// Converge the machine to the newest generation that passes all three checks, rolling back
/// the one in force if it does not.
///
/// Returns an error when the generation in force is bad and no earlier generation passes — a
/// store with one generation, all of it corrupt. It does **not** apply a default: there is no
/// default generation, and inventing one would hide the fact that the machine's whole
/// configuration is unreadable.
pub fn converge(store: &Store) -> Result<Convergence, String> {
    let current = store.current().map_err(|e| e.to_string())?.ok_or_else(|| {
        "the store has no generation; seed it with `morph-manifest apply --store`".to_string()
    })?;

    match judge(store, current.number) {
        Ok(manifest) => Ok(Convergence {
            number: current.number,
            manifest,
            rolled_back_from: None,
            reason: None,
        }),
        Err(reason) => {
            let (to, manifest) = newest_good(store, current.number)?;
            // Move `current` back. This is `Store::rollback`: it points the link at the older
            // generation and deletes nothing, so the corrupt generation stays in the history
            // for a person to inspect — and H12 still holds, because the next generation is
            // `next_number()`, never `current + 1`.
            store.rollback(Some(to)).map_err(|e| e.to_string())?;
            append_heal(store, current.number, &reason, to)?;
            Ok(Convergence {
                number: to,
                manifest,
                rolled_back_from: Some(current.number),
                reason: Some(reason),
            })
        }
    }
}

/// The newest generation older than `bad` that passes all three checks, or an error when there
/// is none.
///
/// Only older generations are considered: a rollback goes *back*. A newer generation in the
/// history is one the machine already rejected (or left behind), and "the newest good
/// generation" does not mean re-applying it.
fn newest_good(store: &Store, bad: u64) -> Result<(u64, Manifest), String> {
    let history = store.history().map_err(|e| e.to_string())?;
    // `history` is oldest-first; walk it backwards so the first good one is the newest.
    for generation in history.iter().rev() {
        if generation.number >= bad {
            continue; // the corrupt one, and anything newer
        }
        if let Ok(manifest) = judge(store, generation.number) {
            return Ok((generation.number, manifest));
        }
    }
    Err(format!(
        "generation {bad} is corrupt and there is no earlier generation to roll back to; \
         refusing to apply a default"
    ))
}

/// One generation, judged by the three checks that gate it.
///
/// Returns the manifest the machine may obey, or a one-line reason naming which check failed.
fn judge(store: &Store, number: u64) -> Result<Manifest, String> {
    let value = match store.load(number) {
        Ok(value) => value,
        Err(morph_manifest::Error::Corrupt(e)) => {
            return Err(format!("does not match its digest ({e})"));
        }
        Err(e) => return Err(format!("does not parse ({e})")),
    };
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|e| format!("does not parse (generation {number} is not a Manifest: {e})"))?;
    let report = morph_manifest::validate(&manifest);
    if !report.is_ok() {
        let reasons: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.severity == morph_manifest::Severity::Error)
            .map(|f| f.message.as_str())
            .collect();
        return Err(format!("does not validate ({})", reasons.join("; ")));
    }
    Ok(manifest)
}

/// Append one `{rung:"H2", bad, reason, now, at}` record to `heal.jsonl` beside the store.
///
/// `at` is Unix seconds — the same clock-less convention the store's marks use, so the record
/// needs no timezone or clock dependency.
fn append_heal(store: &Store, bad: u64, reason: &str, now: u64) -> Result<(), String> {
    let path = store.root().join("heal.jsonl");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let record = HealRecord {
        rung: "H2",
        bad,
        reason: reason.to_string(),
        now,
        at: unix_seconds(),
    };
    let mut line = serde_json::to_string(&record)
        .map_err(|e| format!("the heal record does not serialise: {e}"))?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
