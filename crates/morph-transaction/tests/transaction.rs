//! A transaction: what it commits, what it takes back, and what it does when nobody is looking.

use std::time::{Duration, Instant};

use morph_manifest::Store;
use morph_transaction::{Error, HealthCheck, Transaction, is_refusal};

const V1: &str = r#"
[machine]
name = "t"
profile = "desk"
arch = "x86_64"

[[users]]
name = "a"
admin = true
recovery_key = true

[[storage.replicas]]
name = "usb"
kind = "disk"
target = "USB"
"#;

/// A tier-1 change: the smallest thing that may be applied unattended.
fn free(percent: u32) -> serde_json::Value {
    value(&format!("{V1}\n[storage]\nmin_free_percent = {percent}\n"))
}

/// Security-critical, so no unattended caller may apply it — the refusal this layer has to
/// pass through without recording anything.
const CAPSULE: &str = "[[capsules]]\nname = \"crm\"\nruntime = \"wine\"\nsource = \"recipe:acme-crm-2012\"\ngrants = [\"files:documents\"]\n";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("morph-transaction-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn value(source: &str) -> serde_json::Value {
    morph_manifest::resolve_value(source, "t").unwrap()
}

/// A store with generation 1 established and committed. Returns the root, generation 1's
/// manifest, and the next generation's.
fn established(name: &str) -> (std::path::PathBuf, serde_json::Value, serde_json::Value) {
    let root = scratch(name);
    let v1 = value(V1);
    let v2 = free(12);
    Store::open(&root).apply(&v1, &v2, None).unwrap();
    (root, v2, free(15))
}

#[test]
fn a_healthy_transaction_commits() {
    let (root, from, to) = established("healthy");
    let transaction = Transaction::open(&root);

    let outcome = transaction
        .apply(&from, &to, None, &HealthCheck::new("true"))
        .unwrap();
    assert!(outcome.committed, "{outcome:?}");
    assert_eq!(outcome.from, 1);
    assert_eq!(outcome.trialled, 2);
    assert_eq!(outcome.generation, 2);
    assert!(outcome.reason.is_none());
    assert_eq!(transaction.status().unwrap().0.unwrap().number, 2);
    assert_eq!(
        transaction.trial().unwrap(),
        None,
        "the marker survived a commit"
    );
}

#[test]
fn an_unhealthy_transaction_takes_itself_back() {
    let (root, from, to) = established("unhealthy");
    let transaction = Transaction::open(&root);

    let outcome = transaction
        .apply(&from, &to, None, &HealthCheck::new("false"))
        .unwrap();
    assert!(!outcome.committed, "{outcome:?}");
    assert_eq!(outcome.trialled, 2);
    // The machine is on the generation it was on before, and the marker is gone.
    assert_eq!(outcome.generation, 1);
    assert_eq!(transaction.status().unwrap().0.unwrap().number, 1);
    assert_eq!(transaction.trial().unwrap(), None);
    assert!(
        outcome.reason.as_deref().unwrap_or("").contains("exited 1"),
        "the reason must say what the check did: {outcome:?}"
    );

    // ⚠️ The generation that was taken back is STILL IN THE HISTORY. A change somebody made
    // and the machine rejected is a decision worth keeping, and deleting it would make the
    // next person re-derive why it is not in force.
    let numbers: Vec<u64> = transaction
        .store()
        .history()
        .unwrap()
        .iter()
        .map(|g| g.number)
        .collect();
    assert_eq!(numbers, vec![1, 2]);
}

#[test]
fn a_trial_nobody_resolved_is_taken_back_by_the_next_transaction() {
    let (root, from, to) = established("interrupted");
    let v3 = free(15);

    // A process that armed a trial and died before deciding. Nothing else happens — no
    // cleanup, no signal handler — because the marker IS the record of the unfinished work.
    let armed = Transaction::open(&root);
    let trial = armed.begin(&from, &to, None).unwrap();
    assert_eq!(trial.to, 2);
    drop(armed);

    // A fresh process, as after a reboot.
    let next = Transaction::open(&root);
    assert_eq!(next.trial().unwrap().map(|t| t.from), Some(1));
    assert_eq!(next.store().current().unwrap().unwrap().number, 2);

    let outcome = next
        .apply(&from, &v3, None, &HealthCheck::new("true"))
        .unwrap();
    assert_eq!(
        outcome.recovered,
        Some(1),
        "the abandoned trial was not resolved"
    );
    assert!(outcome.committed);
    // ⚠️ The new generation is 3, not 2. The abandoned trial consumed number 2, and numbers
    // are append-only even when the pointer goes back — so a caller that assumed
    // `current + 1` would arm a trial for a number the store will not use.
    assert_eq!(outcome.generation, 3);
    assert_eq!(next.trial().unwrap(), None);
    // Generation 2 is the one that was abandoned, and it is still there.
    assert!(
        next.store()
            .history()
            .unwrap()
            .iter()
            .any(|g| g.number == 2),
        "the abandoned generation was deleted"
    );
}

#[test]
fn a_health_check_that_never_ran_is_not_a_pass() {
    let (root, from, to) = established("no-check");
    let transaction = Transaction::open(&root);

    assert!(
        HealthCheck::new("   ").run().is_err(),
        "an empty check must not pass"
    );

    // And through the transaction: no check means no reason to commit, so it rolls back
    // rather than keeping a change nothing looked at.
    let outcome = transaction
        .apply(&from, &to, None, &HealthCheck::new(""))
        .unwrap();
    assert!(!outcome.committed, "{outcome:?}");
    assert_eq!(outcome.generation, 1);
}

#[test]
fn a_health_check_that_hangs_counts_as_a_failure_and_does_not_hang_the_transaction() {
    let (root, from, to) = established("timeout");
    let transaction = Transaction::open(&root);

    let started = Instant::now();
    let outcome = transaction
        .apply(
            &from,
            &to,
            None,
            &HealthCheck::new("sleep 30").with_timeout(Duration::from_millis(300)),
        )
        .unwrap();
    let waited = started.elapsed();

    assert!(!outcome.committed, "{outcome:?}");
    assert!(
        waited < Duration::from_secs(5),
        "the transaction waited {waited:?} for a check that cannot finish"
    );
    assert!(
        outcome
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("did not finish"),
        "{outcome:?}"
    );
    assert_eq!(transaction.status().unwrap().0.unwrap().number, 1);
}

#[test]
fn nothing_to_fall_back_to_is_refused_rather_than_promised() {
    let root = scratch("no-fallback");
    let transaction = Transaction::open(&root);
    let (from, to) = (value(V1), free(12));

    // A first generation is not a transaction: there is nothing to roll back to, and calling
    // it one would promise a rollback that cannot happen.
    let err = transaction
        .begin(&from, &to, None)
        .expect_err("an empty store has nothing to fall back to");
    assert!(is_refusal(&err), "expected a refusal: {err}");
    assert!(
        err.reason().contains("no generation to fall back to"),
        "{err}"
    );
    assert_eq!(transaction.trial().unwrap(), None);
}

#[test]
fn a_trial_in_progress_blocks_another_one_and_says_how_to_resolve_it() {
    let (root, from, to) = established("two-trials");
    let v3 = free(15);
    let transaction = Transaction::open(&root);

    transaction.begin(&from, &to, None).unwrap();
    let err = transaction
        .begin(&from, &v3, None)
        .expect_err("a trial is already open");
    assert!(is_refusal(&err), "expected a refusal: {err}");
    // No dead ends: the message names both ways out.
    assert!(err.reason().contains("commit"), "{err}");
    assert!(err.reason().contains("rollback"), "{err}");

    transaction.rollback().unwrap();
    transaction.begin(&from, &v3, None).unwrap();
}

#[test]
fn a_generation_the_manifest_layer_refuses_leaves_no_trial_behind() {
    let (root, from, _) = established("refused");
    let transaction = Transaction::open(&root);
    let dangerous = value(&format!(
        "{V1}\n[storage]\nmin_free_percent = 12\n{CAPSULE}"
    ));

    // No approver, and adding a capsule is security-critical.
    let err = transaction
        .begin(&from, &dangerous, None)
        .expect_err("a security-critical change needs a person");
    assert!(is_refusal(&err), "expected a refusal: {err}");
    assert!(
        err.reason().contains("above the tier"),
        "wrong reason: {err}"
    );

    // ⚠️ The marker must be gone. Left behind, it would describe a trial that never started,
    // and the next transaction would roll a machine back for no reason at all.
    assert_eq!(
        transaction.trial().unwrap(),
        None,
        "a refused change left a trial marker"
    );
    assert_eq!(transaction.store().current().unwrap().unwrap().number, 1);
    // And the checked-in failure path did not add a generation either.
    assert_eq!(transaction.store().history().unwrap().len(), 1);
}

#[test]
fn commit_and_rollback_without_a_trial_are_refused() {
    let (root, _, _) = established("nothing-in-progress");
    let transaction = Transaction::open(&root);
    assert!(is_refusal(&transaction.commit().unwrap_err()));
    assert!(is_refusal(&transaction.rollback().unwrap_err()));
    // A defect is not a refusal, and the distinction is the exit-code contract.
    assert!(!is_refusal(&Error::Invalid("a broken store".into())));
}
