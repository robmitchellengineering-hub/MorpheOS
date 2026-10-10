//! End-to-end checks over the example machines in `manifest/examples`.

use morph_manifest::{Risk, plan_values, resolve, resolve_value, validate};

fn example(name: &str) -> String {
    let path = format!(
        "{}/../../manifest/examples/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

const DESK_MINIMAL: &str = r#"
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
    format!("{DESK_MINIMAL}\n{extra}")
}

#[test]
fn every_example_is_valid() {
    for name in [
        "seed-shed-pi.toml",
        "desk-laptop.toml",
        "desk-laptop-proposed.toml",
        "studio-pc.toml",
    ] {
        let m = resolve(&example(name), name).unwrap();
        let report = validate(&m);
        assert!(report.is_ok(), "{name}: {:#?}", report.findings);
    }
}

#[test]
fn profiles_apply_their_defaults() {
    let seed = resolve(&example("seed-shed-pi.toml"), "seed").unwrap();
    assert_eq!(seed.storage.min_free_percent, 15);
    assert_eq!(seed.storage.retention.all_versions_hours, 6);
    let studio = resolve(&example("studio-pc.toml"), "studio").unwrap();
    assert_eq!(
        studio.system.kernel,
        morph_manifest::model::KernelVariant::Rt
    );
    assert!(
        validate(&studio).findings.is_empty(),
        "{:#?}",
        validate(&studio).findings
    );
}

#[test]
fn disabling_ab_boot_is_blocked() {
    let m = resolve(&with("[system.boot]\nab_slots = false\n"), "t").unwrap();
    let r = validate(&m);
    assert!(!r.is_ok());
    assert!(r.has("INV-BOOT"));
}

#[test]
fn heal_engine_cannot_be_disabled_or_overreach() {
    let m = resolve(&with("[heal]\nenabled = false\nmax_auto_rung = 5\n"), "t").unwrap();
    let r = validate(&m);
    assert_eq!(
        r.findings.iter().filter(|f| f.code == "INV-HEAL").count(),
        2
    );
}

#[test]
fn machine_needs_admin_and_recovery_key() {
    let src = DESK_MINIMAL.replace("admin = true\nrecovery_key = true\n", "");
    let r = validate(&resolve(&src, "t").unwrap());
    assert!(r.has("INV-ADMIN"));
    assert!(r.has("INV-RECOVERY-KEY"));
}

#[test]
fn single_copy_is_an_error_unless_accepted() {
    let no_replica = DESK_MINIMAL
        .split("[[storage.replicas]]")
        .next()
        .unwrap()
        .to_string();
    let r = validate(&resolve(&no_replica, "t").unwrap());
    assert!(r.has("INV-COPIES"));

    let accepted = format!("{no_replica}\n[storage]\naccept_single_copy = true\n");
    let r = validate(&resolve(&accepted, "t").unwrap());
    assert!(r.is_ok());
    assert!(r.has("W-SINGLE-COPY"));
}

#[test]
fn service_dependency_errors() {
    let src = with(
        r#"
[[services]]
name = "a"
exec = "/a"
class = "background"
after = ["b"]

[[services]]
name = "b"
exec = "/b"
class = "background"
after = ["a", "ghost"]
"#,
    );
    let r = validate(&resolve(&src, "t").unwrap());
    assert!(r.has("E-DEP-CYCLE"), "{:#?}", r.findings);
    assert!(r.has("E-UNKNOWN-DEP"));
}

#[test]
fn ai_proposal_is_planned_and_held_for_approval() {
    let old = resolve_value(&example("desk-laptop.toml"), "old").unwrap();
    let new = resolve_value(&example("desk-laptop-proposed.toml"), "new").unwrap();
    let plan = plan_values(&old, &new).unwrap();
    let paths: Vec<&str> = plan.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["/capsules[crm]/grants", "/network/wifi"]);
    assert_eq!(plan.risk, Risk::DataAffecting);
    assert!(plan.report.is_ok());
    assert!(!plan.auto_apply, "a tier-3 change must not auto-apply");
}

#[test]
fn low_risk_change_may_auto_apply() {
    let old = resolve_value(DESK_MINIMAL, "old").unwrap();
    let new = resolve_value(&with("[storage]\nmin_free_percent = 12\n"), "new").unwrap();
    let plan = plan_values(&old, &new).unwrap();
    assert_eq!(plan.risk, Risk::ReversibleLocal);
    assert!(plan.auto_apply);
}

#[test]
fn invalid_proposal_never_auto_applies() {
    let old = resolve_value(DESK_MINIMAL, "old").unwrap();
    let new = resolve_value(&with("[storage]\nmin_free_percent = 0\n"), "new").unwrap();
    let plan = plan_values(&old, &new).unwrap();
    assert_eq!(plan.risk, Risk::ReversibleLocal);
    assert!(!plan.report.is_ok());
    assert!(!plan.auto_apply);
}

#[test]
fn schema_names_every_section() {
    let schema = morph_manifest::json_schema();
    let props = schema["properties"].as_object().unwrap();
    for key in [
        "machine", "system", "storage", "heal", "network", "users", "services", "capsules",
    ] {
        assert!(props.contains_key(key), "schema is missing {key}");
    }
}
