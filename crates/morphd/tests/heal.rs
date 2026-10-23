//! The H2 heal: a corrupt `current` rolls back to the newest generation that parses,
//! validates and matches its digest — and a store with nothing to fall back to refuses.

use std::path::Path;

use morph_manifest::Store;

/// A valid Manifest with no services, ready to have settings layered on.
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

/// A directory of its own per test, so a failure names the case.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("morphd-heal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A store with three good generations, `current` on 3.
fn three_generations(name: &str) -> (std::path::PathBuf, Store) {
    let root = scratch(name);
    let store = Store::open(&root);
    let v1 = value(BASE);
    let v2 = value(&format!("{BASE}\n[storage]\nmin_free_percent = 12\n"));
    let v3 = value(&format!("{BASE}\n[storage]\nmin_free_percent = 15\n"));
    let v4 = value(&format!("{BASE}\n[storage]\nmin_free_percent = 18\n"));
    store.apply(&v1, &v2, None).unwrap();
    store.apply(&v2, &v3, None).unwrap();
    store.apply(&v3, &v4, None).unwrap();
    (root, store)
}

/// Rewrite a generation's recorded digest to match its file's current bytes — the state a
/// "the validator changed since this was applied" scenario leaves: the file is intact (digest
/// matches) but its content no longer passes today's checks.
fn rewrite_digest(root: &Path, number: u64) {
    let file = root.join(format!("generations/{number:04}.json"));
    let bytes = std::fs::read(&file).unwrap();
    let digest = blake3::hash(&bytes).to_hex().to_string();
    let meta_path = root.join(format!("generations/{number:04}.meta.json"));
    let mut meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    meta.as_object_mut()
        .unwrap()
        .insert("digest".into(), serde_json::json!(digest));
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();
}

/// Every heal record the store has written, oldest first.
fn heal_records(root: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(root.join("heal.jsonl")).unwrap();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_corrupt_manifest_rolls_back_and_names_the_generation() {
    let (root, store) = three_generations("corrupt-toml");
    // Corrupt generation 3's stored file: it no longer parses.
    std::fs::write(root.join("generations/0003.json"), b"not json").unwrap();

    let converged = morphd::converge(&store).unwrap();
    assert_eq!(converged.rolled_back_from, Some(3), "names N");
    assert_eq!(converged.number, 2, "now M");
    let reason = converged.reason.as_ref().expect("a reason is recorded");
    assert!(reason.contains("parse"), "{reason}");

    // The store's own record names the generation and what is in force now (H8).
    assert_eq!(store.current().unwrap().unwrap().number, 2);
    let records = heal_records(&root);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["rung"], "H2");
    assert_eq!(records[0]["bad"], 3);
    assert_eq!(records[0]["now"], 2);
    // Do not delete N, and the next generation is still `next_number()`, never `current + 1`.
    assert_eq!(store.history().unwrap().len(), 3, "generation 3 is not deleted");
    assert_eq!(store.next_number().unwrap(), 4);
}

#[test]
fn a_manifest_that_no_longer_validates_rolls_back() {
    let (root, store) = three_generations("invalid");
    // Generation 3's file now holds a Manifest that deserialises but breaks an invariant
    // (A/B slots off). It is valid JSON and its digest still matches, so validation is the
    // check that must refuse it — not the digest.
    let invalid = value(&format!("{BASE}\n[system.boot]\nab_slots = false\n"));
    std::fs::write(
        root.join("generations/0003.json"),
        serde_json::to_vec_pretty(&invalid).unwrap(),
    )
    .unwrap();
    rewrite_digest(&root, 3);

    let converged = morphd::converge(&store).unwrap();
    assert_eq!(converged.rolled_back_from, Some(3));
    assert_eq!(converged.number, 2);
    let reason = converged.reason.as_ref().unwrap();
    assert!(reason.contains("validate"), "{reason}");
    assert_eq!(store.current().unwrap().unwrap().number, 2);
    let records = heal_records(&root);
    assert_eq!(records[0]["bad"], 3);
    assert_eq!(records[0]["now"], 2);
}

#[test]
fn a_manifest_with_the_wrong_digest_rolls_back() {
    let (root, store) = three_generations("wrong-digest");
    // A different, still-valid manifest, written not through the store: the digest recorded at
    // apply no longer matches. It parses and validates, so the digest is what refuses it.
    let tampered = value(&format!("{BASE}\n[storage]\nmin_free_percent = 21\n"));
    std::fs::write(
        root.join("generations/0003.json"),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();

    let converged = morphd::converge(&store).unwrap();
    assert_eq!(converged.rolled_back_from, Some(3));
    assert_eq!(converged.number, 2);
    let reason = converged.reason.as_ref().unwrap();
    assert!(reason.contains("digest"), "{reason}");
    assert_eq!(store.current().unwrap().unwrap().number, 2);
    let records = heal_records(&root);
    assert_eq!(records[0]["bad"], 3);
    assert_eq!(records[0]["now"], 2);
}

#[test]
fn a_store_with_one_corrupt_generation_refuses_instead_of_applying_a_default() {
    let root = scratch("single");
    let store = Store::open(&root);
    let v1 = value(BASE);
    let v2 = value(&format!("{BASE}\n[storage]\nmin_free_percent = 12\n"));
    store.apply(&v1, &v2, None).unwrap();
    std::fs::write(root.join("generations/0001.json"), b"not json").unwrap();

    let err = morphd::converge(&store).expect_err("no generation to fall back to");
    assert!(err.contains("refusing to apply a default"), "{err}");
    // Nothing moved, and nothing was recorded as healed: there is nothing to record a rollback
    // to, and no default was invented.
    assert_eq!(store.current().unwrap().unwrap().number, 1);
    assert!(!root.join("heal.jsonl").exists());
}
