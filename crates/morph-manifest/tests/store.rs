//! The generation store: what it keeps, what it refuses, and what a rollback may not lose.

use morph_manifest::{Error, Risk, Store};

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

fn with(extra: &str) -> String {
    format!("{V1}\n{extra}")
}

/// A tier-1 change: the smallest thing that may be applied unattended.
const FREE: &str = "[storage]\nmin_free_percent = 12\n";

/// A capsule block. Adding one is `SecurityCritical` (arbitrary code and its grants), which
/// is what a refusal test needs: a change nobody may apply alone.
fn capsule(grants: &str) -> String {
    format!(
        "[[capsules]]\nname = \"crm\"\nruntime = \"wine\"\nsource = \"recipe:acme-crm-2012\"\ngrants = [{grants}]\n"
    )
}

/// A directory of its own per test. The name is the test's, so a failure names the case.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morph-manifest-store-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn value(source: &str) -> serde_json::Value {
    morph_manifest::resolve_value(source, "t").unwrap()
}

fn example(name: &str) -> String {
    let path = format!(
        "{}/../../manifest/examples/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Everything in the store: every file's bytes, the `current` link's target, and nothing
/// else. Used to assert that a refusal changed *nothing* — a test that only checks the
/// generation count would pass while a refusal rewrote the link or left a stray file.
fn snapshot(root: &std::path::Path) -> Vec<String> {
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

#[test]
fn a_stored_generation_becomes_current_and_can_be_read_back() {
    let root = scratch("stored");
    let store = Store::open(&root);
    let (v1, v2) = (
        value(V1),
        value(&with("[storage]\nmin_free_percent = 12\n")),
    );

    let first = store.apply(&v1, &v2, None).unwrap();
    assert_eq!(first.number, 1);
    assert_eq!(first.risk, Risk::ReversibleLocal);
    assert_eq!(first.changes, 1);

    let current = store.current().unwrap().expect("a generation is in force");
    assert_eq!(current, first);
    // What is stored is the RESOLVED manifest, and it reads back equal to what was applied.
    assert_eq!(store.load(1).unwrap(), v2);
    assert_eq!(
        std::fs::read_link(root.join("current")).unwrap(),
        std::path::Path::new("generations/0001.json")
    );

    // A second generation supersedes it without destroying it.
    let v3 = value(&with("[storage]\nmin_free_percent = 15\n"));
    let second = store.apply(&v2, &v3, None).unwrap();
    assert_eq!(second.number, 2);
    assert_eq!(store.current().unwrap().unwrap().number, 2);
    assert_eq!(store.load(1).unwrap(), v2, "generation 1 was overwritten");
    assert_eq!(
        store
            .history()
            .unwrap()
            .iter()
            .map(|g| g.number)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn a_refused_apply_leaves_the_store_exactly_as_it_was() {
    let root = scratch("refused");
    let store = Store::open(&root);
    let v1 = value(V1);
    let v2 = value(&with(FREE));
    store.apply(&v1, &v2, None).unwrap();
    let before = snapshot(&root);
    assert!(
        !before.is_empty(),
        "the fixture must have something to protect"
    );

    // A tier-3 change with nobody taking responsibility for it. apply() refuses it — and the
    // store must not have acquired a generation, a record, or a moved link.
    //
    // ⚠️ The plan starts from the generation **in force**, and the assertion is on the
    // REASON. Written the obvious way — starting from some other Manifest — the staleness
    // check refuses it first and this test passes for a reason that is not its subject.
    let widened = value(&with(&format!(
        "{FREE}\n{}",
        capsule("\"files:documents\"")
    )));
    let err = store
        .apply(&v2, &widened, None)
        .expect_err("a tier-3 change needs a person");
    assert!(
        matches!(err, Error::Refused(_)),
        "expected a refusal: {err}"
    );
    assert!(
        err.to_string().contains("above the tier"),
        "refused, but not for the tier: {err}"
    );

    assert_eq!(snapshot(&root), before, "a refused apply changed the store");
    assert_eq!(store.current().unwrap().unwrap().number, 1);
    assert_eq!(store.history().unwrap().len(), 1);
}

#[test]
fn a_plan_that_starts_from_another_generation_is_refused() {
    let root = scratch("stale");
    let store = Store::open(&root);
    let v1 = value(V1);
    let v2 = value(&with("[storage]\nmin_free_percent = 12\n"));
    let v3 = value(&with("[storage]\nmin_free_percent = 15\n"));
    store.apply(&v1, &v2, None).unwrap();
    let before = snapshot(&root);

    // v1 -> v3 is a perfectly good plan, written against the generation the store USED to
    // hold. Applying it would silently replace generation 1, which the plan never saw.
    let err = store
        .apply(&v1, &v3, None)
        .expect_err("the store has moved past v1");
    assert!(
        matches!(err, Error::Refused(_)),
        "expected a refusal: {err}"
    );
    assert!(err.to_string().contains("generation 1"), "{err}");
    assert_eq!(snapshot(&root), before);
}

#[test]
fn rollback_points_at_an_older_generation_and_keeps_the_newer_one() {
    let root = scratch("rollback");
    let store = Store::open(&root);
    let v1 = value(V1);
    let v2 = value(&with("[storage]\nmin_free_percent = 12\n"));
    let v3 = value(&with("[storage]\nmin_free_percent = 15\n"));
    store.apply(&v1, &v2, None).unwrap();
    store.apply(&v2, &v3, None).unwrap();

    let rolled_back = store.rollback(None).unwrap();
    assert_eq!(rolled_back.number, 1);
    assert_eq!(store.current().unwrap().unwrap().number, 1);
    // ⚠️ The generation that was in force is STILL THERE. A rollback that deleted it would
    // make the mistake irreversible, which is the one thing a rollback must not do.
    assert_eq!(store.load(2).unwrap(), v3);

    // And it goes forward again: rolling back is a pointer move in both directions.
    assert_eq!(store.rollback(Some(2)).unwrap().number, 2);
    assert_eq!(store.rollback(None).unwrap().number, 1);

    // Nothing older: a refusal, and the pointer does not move.
    let err = store
        .rollback(None)
        .expect_err("generation 1 is the oldest");
    assert!(
        matches!(err, Error::Refused(_)),
        "expected a refusal: {err}"
    );
    assert_eq!(store.current().unwrap().unwrap().number, 1);
}

#[test]
fn a_rollback_to_a_generation_that_is_not_stored_is_refused() {
    let root = scratch("missing");
    let store = Store::open(&root);
    let v1 = value(V1);
    store
        .apply(
            &v1,
            &value(&with("[storage]\nmin_free_percent = 12\n")),
            None,
        )
        .unwrap();
    let before = snapshot(&root);

    let err = store
        .rollback(Some(7))
        .expect_err("generation 7 does not exist");
    assert!(
        matches!(err, Error::Refused(_)),
        "expected a refusal: {err}"
    );
    assert_eq!(snapshot(&root), before);
}

#[test]
fn an_empty_store_is_empty_rather_than_broken() {
    let root = scratch("empty");
    let store = Store::open(root.join("does-not-exist-yet"));
    assert_eq!(store.history().unwrap(), vec![]);
    assert_eq!(store.current().unwrap(), None);

    // ...and a rollback into an empty store says why rather than failing obscurely.
    let err = store.rollback(None).expect_err("nothing to roll back to");
    assert!(
        matches!(err, Error::Refused(_)),
        "expected a refusal: {err}"
    );
}

#[test]
fn a_dangling_current_is_a_defect_and_not_an_empty_store() {
    let root = scratch("dangling");
    let store = Store::open(&root);
    let v1 = value(V1);
    store
        .apply(
            &v1,
            &value(&with("[storage]\nmin_free_percent = 12\n")),
            None,
        )
        .unwrap();

    // Something removed the generation that was in force. "Nothing is current" would be a
    // comfortable answer and a false one — the difference between an empty store and a
    // broken one is exactly what an operator needs to know.
    std::fs::remove_file(root.join("generations/0001.json")).unwrap();
    let err = store
        .current()
        .expect_err("current names a missing generation");
    assert!(matches!(err, Error::Invalid(_)), "expected a defect: {err}");
    assert!(err.to_string().contains("0001"), "{err}");
}

#[test]
fn a_record_with_an_unknown_tier_is_an_error_rather_than_a_default() {
    let root = scratch("unknown-tier");
    let store = Store::open(&root);
    let v1 = value(V1);
    store
        .apply(
            &v1,
            &value(&with("[storage]\nmin_free_percent = 12\n")),
            None,
        )
        .unwrap();

    // A tier nobody chose is a hazard this enum has already produced once, so a record that
    // names one is unreadable rather than quietly rounded to something safe-looking.
    std::fs::write(
        root.join("generations/0001.meta.json"),
        br#"{"number": 1, "risk": "probably-fine", "changes": 1}"#,
    )
    .unwrap();
    assert!(store.history().is_err());
}

#[test]
fn the_history_records_who_took_responsibility() {
    let root = scratch("approved");
    let store = Store::open(&root);
    let old = value(&example("desk-laptop.toml"));
    let new = value(&example("desk-laptop-proposed.toml"));

    let generation = store.apply(&old, &new, Some("rob")).unwrap();
    assert_eq!(generation.risk, Risk::DataAffecting);
    assert_eq!(generation.approved_by.as_deref(), Some("rob"));

    // Read back from disk, not from the return value: the point of the record is that it
    // survives the process that wrote it.
    let history = store.history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].approved_by.as_deref(), Some("rob"));
    assert_eq!(history[0].risk, Risk::DataAffecting);
}

// ---------------------------------------------------------------------------
// The person's mark (ADR 0012): the store's own record that somebody stood behind a
// change — and, just as importantly, that nobody did when nobody did.
// ---------------------------------------------------------------------------

#[test]
fn a_person_who_signs_a_change_leaves_a_mark_beside_it() {
    let root = scratch("mark-signed");
    let store = Store::open(&root);
    let v1 = value(V1);
    // A capsule is SecurityCritical: a change nobody may apply alone.
    let v2 = value(&with(&capsule("\"clipboard\"")));

    let generation = store
        .apply(&v1, &v2, Some("Rob Mitchell"))
        .expect("a person signed it, so it may be applied");
    assert_eq!(generation.approved_by.as_deref(), Some("Rob Mitchell"));

    let mark = store
        .mark(generation.number)
        .unwrap()
        .expect("a signed change leaves a mark");
    assert_eq!(mark.generation, generation.number);
    assert_eq!(mark.approved_by, "Rob Mitchell");
    assert_eq!(mark.risk, generation.risk);
    assert_eq!(mark.changes, generation.changes);
    assert!(
        mark.approved_at_unix > 0,
        "a mark records when, not only who"
    );

    // It is a file in the machine's own state, beside the generation it authorises, so
    // "what has a person signed here?" is a directory listing.
    let path = store.mark_of(generation.number);
    assert!(path.ends_with("generations/0001.rob"), "{}", path.display());
    assert!(path.is_file(), "{}", path.display());
    assert_eq!(store.marks().unwrap().len(), 1);
}

#[test]
fn a_change_nobody_signed_has_no_mark() {
    // The absence is the claim. A tier-1 change may be applied unattended, and no person
    // stood behind it, so there is nothing to find.
    let root = scratch("mark-unsigned");
    let store = Store::open(&root);
    let v1 = value(V1);
    let v2 = value(&with(FREE));

    let generation = store
        .apply(&v1, &v2, None)
        .expect("tier 1 may be applied alone");
    assert!(generation.approved_by.is_none());
    assert!(store.mark(generation.number).unwrap().is_none());
    assert!(!store.mark_of(generation.number).exists());
    assert!(store.marks().unwrap().is_empty());
}

#[test]
fn a_refused_apply_leaves_no_mark() {
    let root = scratch("mark-refused");
    let store = Store::open(&root);
    let v1 = value(V1);
    let v2 = value(&with(&capsule("\"clipboard\"")));

    let err = store
        .apply(&v1, &v2, None)
        .expect_err("a capsule change needs a person to take responsibility");
    assert!(matches!(err, Error::Refused(_)), "{err:?}");

    assert!(
        !store.mark_of(1).exists(),
        "a refusal must not leave a mark"
    );
    assert!(store.marks().unwrap().is_empty());
    assert!(store.history().unwrap().is_empty(), "nor a generation");
}

#[test]
fn a_mark_belongs_to_its_generation_and_survives_a_rollback() {
    let root = scratch("mark-rollback");
    let store = Store::open(&root);
    let capsule_src = with(&capsule("\"clipboard\""));
    let v1 = value(V1);
    let v2 = value(&capsule_src);
    let v3 = value(&format!("{capsule_src}\n{FREE}"));

    let signed = store.apply(&v1, &v2, Some("Rob Mitchell")).unwrap();
    let unsigned = store.apply(&v2, &v3, None).unwrap();

    store.rollback(Some(signed.number)).unwrap();
    assert_eq!(store.current().unwrap().unwrap().number, signed.number);

    // A mark belongs to the change it authorises, not to whatever is current: rolling back
    // does not erase the fact that somebody signed generation 1, and generation 2 was never
    // signed at all.
    assert!(store.mark(signed.number).unwrap().is_some());
    assert!(store.mark(unsigned.number).unwrap().is_none());
    assert_eq!(store.marks().unwrap().len(), 1);
}
