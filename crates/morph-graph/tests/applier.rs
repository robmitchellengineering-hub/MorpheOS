//! The applier: what it commits, what it takes back, and what a refusal may not touch.

use std::path::Path;

use morph_graph::{Error, apply};
use morph_manifest::Store;
use morph_transaction::HealthCheck;

/// A valid Manifest with no services, ready to have services layered on.
const BASE: &str = r#"
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

fn value(source: &str) -> serde_json::Value {
    morph_manifest::resolve_value(source, "t").unwrap()
}

fn service(name: &str, exec: &str, class: &str, after: &[&str]) -> String {
    let mut s =
        format!("[[services]]\nname = \"{name}\"\nexec = \"{exec}\"\nclass = \"{class}\"\n");
    if !after.is_empty() {
        let list = after
            .iter()
            .map(|a| format!("\"{a}\""))
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!("after = [{list}]\n"));
    }
    s
}

/// A directory of its own per test, so a failure names the case.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("morph-graph-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Everything in the store: every file's bytes, the `current` link's target, and nothing
/// else. Used to assert that a refusal changed *nothing* — a test that only checks the
/// generation count would pass while a refusal rewrote a link or left a stray file.
fn snapshot(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut walk = vec![root.to_path_buf()];
    while let Some(dir) = walk.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if meta.file_type().is_symlink() {
                out.push(format!(
                    "{name} -> {}",
                    std::fs::read_link(&path).unwrap().display()
                ));
            } else if meta.is_dir() {
                walk.push(path);
            } else {
                out.push(format!(
                    "{name} = {}",
                    std::fs::read_to_string(&path).unwrap()
                ));
            }
        }
    }
    out.sort();
    out
}

/// A store with generation 1 established — one service, committed — ready for a
/// transactional change. Returns the root and the two resolved documents the applier is
/// given: the generation in force (`from`) and the change (`to`, a service that depends on
/// the first).
///
/// Generation 1 is established **with a person's name** rather than as a transaction: adding
/// a service is security-critical (its `exec` is arbitrary code), which is a tier nobody may
/// apply alone, and a first generation has nothing to fall back to — so it is established the
/// way `morph-transaction` documents, not as a trial.
fn established(name: &str) -> (std::path::PathBuf, serde_json::Value, serde_json::Value) {
    let root = scratch(name);
    let v1 = value(BASE);
    let v2 = value(&format!(
        "{BASE}\n{}",
        service("audio", "/audio", "background", &[])
    ));
    Store::open(&root).apply(&v1, &v2, Some("rob")).unwrap();
    let v3 = value(&format!(
        "{BASE}\n{}{}",
        service("audio", "/audio", "background", &[]),
        service("plugin", "/plugin", "background", &["audio"]),
    ));
    (root, v2, v3)
}

#[test]
fn a_valid_graph_applies_and_commits() {
    let (root, from, to) = established("commits");

    let outcome = apply(&root, &from, &to, Some("rob"), &HealthCheck::new("true")).unwrap();
    assert!(outcome.transaction.committed, "{outcome:?}");
    assert_eq!(outcome.transaction.generation, 2);
    // The plan is part of the answer, not a side note: the one new service starts, in
    // dependency order, and nothing stops or restarts.
    assert_eq!(outcome.plan.start, vec!["plugin"]);
    assert!(outcome.plan.stop.is_empty());
    assert!(outcome.plan.restart.is_empty());

    assert_eq!(Store::open(&root).current().unwrap().unwrap().number, 2);
}

#[test]
fn a_failed_health_check_takes_the_machine_back() {
    let (root, from, to) = established("rollback");

    let outcome = apply(&root, &from, &to, Some("rob"), &HealthCheck::new("false")).unwrap();
    assert!(!outcome.transaction.committed, "{outcome:?}");
    assert_eq!(
        outcome.transaction.generation, 1,
        "the machine is not back on generation 1"
    );
    // The graph was valid — the health check is what failed — so the plan is still computed.
    assert_eq!(outcome.plan.start, vec!["plugin"]);

    let store = Store::open(&root);
    assert_eq!(store.current().unwrap().unwrap().number, 1);
    // The generation in force is byte-identical to what it was before the change.
    assert_eq!(store.load(1).unwrap(), from);
    // The rejected generation is retained, not erased: a change the machine refused is a
    // decision worth keeping (the transaction's documented contract — the store is
    // append-only, so "identical" is about the state in force, not the history).
    assert_eq!(store.history().unwrap().len(), 2);
}

#[test]
fn an_always_commit_fake_would_keep_a_change_the_machine_refused() {
    // H17 negative control. The assertions below are the exact inverse of what an applier
    // that ignored the health check and always committed would produce: that fake leaves
    // `current` on the trialled generation (2) with the new manifest in force. A correct
    // applier leaves `current` on generation 1 with the old bytes in force. If this test
    // could pass while the applier always committed, it would prove nothing — which is the
    // point of a negative control.
    let (root, from, to) = established("always-commit");

    let outcome = apply(&root, &from, &to, Some("rob"), &HealthCheck::new("false")).unwrap();
    assert!(!outcome.transaction.committed);

    let store = Store::open(&root);
    assert_eq!(
        store.current().unwrap().unwrap().number,
        1,
        "an always-commit applier would leave generation 2 in force here"
    );
    assert_eq!(
        store.load(1).unwrap(),
        from,
        "the old generation was not restored"
    );
    // No open trial: a commit or a rollback both clear the marker; leaving one would mean
    // the next transaction takes the machine back for a decision it already made.
    assert_eq!(
        morph_transaction::Transaction::open(&root).trial().unwrap(),
        None
    );
}

#[test]
fn a_cycle_is_refused_before_anything_is_written() {
    let (root, from, _) = established("cycle");
    let before = snapshot(&root);

    let cyclic = value(&format!(
        "{BASE}\n{}{}",
        service("a", "/a", "background", &["b"]),
        service("b", "/b", "background", &["a"]),
    ));
    // A person is named, to show the refusal is the graph's and not the tier's: a cycle
    // cannot be applied by anyone.
    let err = apply(
        &root,
        &from,
        &cyclic,
        Some("rob"),
        &HealthCheck::new("true"),
    )
    .expect_err("a cycle must be refused before the transaction arms");
    match &err {
        Error::Graph(message) => assert!(message.contains("cycle"), "{message}"),
        other => panic!("expected a graph refusal, got {other}"),
    }

    assert_eq!(snapshot(&root), before, "a refused graph changed the store");
    assert_eq!(Store::open(&root).current().unwrap().unwrap().number, 1);
}

#[test]
fn a_missing_dependency_is_refused_before_anything_is_written() {
    let (root, from, _) = established("missing-dep");
    let before = snapshot(&root);

    let broken = value(&format!(
        "{BASE}\n{}",
        service("plugin", "/plugin", "background", &["ghost"]),
    ));
    let err = apply(
        &root,
        &from,
        &broken,
        Some("rob"),
        &HealthCheck::new("true"),
    )
    .expect_err("a missing dependency must be refused");
    match &err {
        Error::Graph(message) => assert!(message.contains("'ghost'"), "{message}"),
        other => panic!("expected a graph refusal, got {other}"),
    }

    assert_eq!(snapshot(&root), before, "a refused graph changed the store");
}

#[test]
fn an_invariant_break_is_refused_before_anything_is_written() {
    let (root, from, _) = established("invariant");
    let before = snapshot(&root);

    // Structurally fine — one valid service — but it turns off A/B boot slots, the invariant
    // the whole wedge rests on. A person is named, to show the refusal is the invariant's and
    // not the tier's: a machine that cannot boot cannot be configured by anyone.
    let broken = value(&format!(
        "{BASE}\n{}[system.boot]\nab_slots = false\n",
        service("plugin", "/plugin", "background", &[]),
    ));
    let err = apply(
        &root,
        &from,
        &broken,
        Some("rob"),
        &HealthCheck::new("true"),
    )
    .expect_err("an invariant break must be refused");
    match &err {
        Error::Graph(message) => assert!(
            message.contains("ab_slots") || message.contains("INV-BOOT"),
            "{message}"
        ),
        other => panic!("expected a graph refusal, got {other}"),
    }

    assert_eq!(snapshot(&root), before, "a refused graph changed the store");
}
