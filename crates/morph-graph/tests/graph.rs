//! The service graph: what it orders, what it refuses, and why a fixed order is not enough.

use morph_graph::{Error, Graph, plan_update};
use morph_manifest::model::Manifest;

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

fn manifest(extra: &str) -> Manifest {
    let value = morph_manifest::resolve_value(&format!("{BASE}\n{extra}"), "t").unwrap();
    morph_manifest::layer::from_value(value).unwrap()
}

/// One service, enabled, with the given dependencies.
fn service(name: &str, exec: &str, class: &str, after: &[&str]) -> String {
    service_with(name, exec, class, after, true)
}

fn service_with(name: &str, exec: &str, class: &str, after: &[&str], enabled: bool) -> String {
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
    if !enabled {
        s.push_str("enabled = false\n");
    }
    s
}

#[test]
fn a_valid_graph_orders_dependencies_first() {
    // zulu has no dependencies; alpha depends on zulu; bravo depends on alpha.
    let m = manifest(&format!(
        "{}{}{}",
        service("alpha", "/a", "background", &["zulu"]),
        service("bravo", "/b", "background", &["alpha"]),
        service("zulu", "/z", "background", &[]),
    ));
    let graph = Graph::build(&m).unwrap();
    assert_eq!(graph.start_order(), vec!["zulu", "alpha", "bravo"]);
    assert_eq!(graph.stop_order(), vec!["bravo", "alpha", "zulu"]);
}

#[test]
fn the_order_is_not_a_fixed_order() {
    // H17 negative control. The services are authored in ALPHABETICAL order (alpha, bravo,
    // zulu) — which is also a fixed, un-analysed order, the thing a trivial graph returns.
    // Alphabetical is wrong here: alpha depends on zulu, so zulu must come first. A graph
    // that returned services in the order they appear, or sorted by name, would fail this
    // test — which is what makes it a negative control rather than a smoke test.
    let m = manifest(&format!(
        "{}{}{}",
        service("alpha", "/a", "background", &["zulu"]),
        service("bravo", "/b", "background", &["alpha"]),
        service("zulu", "/z", "background", &[]),
    ));
    let graph = Graph::build(&m).unwrap();
    let order = graph.start_order();

    let authored: Vec<&str> = m.services.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        authored,
        vec!["alpha", "bravo", "zulu"],
        "the fixture is not as described"
    );
    assert_ne!(
        order, authored,
        "the graph returned the authored (alphabetical) order, which starts alpha before its \
         dependency zulu"
    );
    assert_eq!(order, vec!["zulu", "alpha", "bravo"]);
}

#[test]
fn a_cycle_is_refused_and_named() {
    let m = manifest(&format!(
        "{}{}",
        service("a", "/a", "background", &["b"]),
        service("b", "/b", "background", &["a"]),
    ));
    let err = Graph::build(&m).expect_err("a cycle must be refused");
    match &err {
        Error::Graph(message) => {
            assert!(message.contains("cycle"), "{message}");
            assert!(message.contains('a'), "{message}");
            assert!(message.contains('b'), "{message}");
        }
        other => panic!("expected a graph refusal, got {other}"),
    }
}

#[test]
fn a_missing_dependency_is_refused_and_named() {
    let m = manifest(&service("a", "/a", "background", &["ghost"]));
    let err = Graph::build(&m).expect_err("a missing dependency must be refused");
    match &err {
        Error::Graph(message) => {
            assert!(message.contains("'ghost'"), "{message}");
            assert!(message.contains("not a service"), "{message}");
        }
        other => panic!("expected a graph refusal, got {other}"),
    }
}

#[test]
fn an_invariant_break_is_refused_not_ordered() {
    // Structurally fine — one valid service — but the invariant "A/B boot slots must stay
    // enabled" is broken. The graph must refuse, not order a machine that cannot safely boot.
    let m = manifest(&format!(
        "{}{}",
        service("a", "/a", "background", &[]),
        "[system.boot]\nab_slots = false\n",
    ));
    let err = Graph::build(&m).expect_err("an invariant break must be refused");
    match &err {
        Error::Graph(message) => {
            assert!(
                message.contains("ab_slots") || message.contains("INV-BOOT"),
                "{message}"
            );
        }
        other => panic!("expected a graph refusal, got {other}"),
    }
}

#[test]
fn a_disabled_service_is_ordered_but_not_started() {
    // 'plugin' depends on 'audio' but is disabled. Its edge still orders it (a disabled
    // service may be started by hand later), but it is not in the start order.
    let m = manifest(&format!(
        "{}{}",
        service("audio", "/audio", "realtime", &[]),
        service_with("plugin", "/plugin", "realtime", &["audio"], false),
    ));
    let graph = Graph::build(&m).unwrap();
    assert_eq!(graph.full_order(), vec!["audio", "plugin"]);
    assert_eq!(graph.start_order(), vec!["audio"]);
    assert_eq!(graph.stop_order(), vec!["audio"]);
}

#[test]
fn plan_update_names_what_starts_stops_and_restarts() {
    let old_m = manifest(&format!(
        "{}{}{}",
        service("audio", "/audio-v1", "realtime", &[]),
        service("plugin", "/plugin", "realtime", &["audio"]),
        service("logger", "/logger", "background", &[]),
    ));
    // audio's exec changed → restart; plugin unchanged; logger removed → stop; network
    // added → start.
    let new_m = manifest(&format!(
        "{}{}{}",
        service("audio", "/audio-v2", "realtime", &[]),
        service("plugin", "/plugin", "realtime", &["audio"]),
        service("network", "/network", "background", &[]),
    ));
    let plan = plan_update(
        &Graph::build(&old_m).unwrap(),
        &Graph::build(&new_m).unwrap(),
    );
    assert_eq!(plan.start, vec!["network"]);
    assert_eq!(plan.stop, vec!["logger"]);
    assert_eq!(plan.restart, vec!["audio"]);
    assert!(!plan.is_empty());
}

#[test]
fn a_restart_orders_dependencies_first() {
    let old_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio-v1", "realtime", &[]),
        service("plugin", "/plugin-v1", "realtime", &["audio"]),
    ));
    let new_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio-v2", "realtime", &[]),
        service("plugin", "/plugin-v2", "realtime", &["audio"]),
    ));
    let plan = plan_update(
        &Graph::build(&old_m).unwrap(),
        &Graph::build(&new_m).unwrap(),
    );
    // Start phase: dependencies first (audio before plugin). The stop phase is this list
    // reversed — dependents (plugin) stop before their dependencies.
    assert_eq!(plan.restart, vec!["audio", "plugin"]);
}

#[test]
fn a_plan_with_no_changes_is_empty() {
    let m = manifest(&service("audio", "/audio", "realtime", &[]));
    let plan = plan_update(&Graph::build(&m).unwrap(), &Graph::build(&m).unwrap());
    assert!(plan.is_empty());
}

#[test]
fn turning_a_service_off_is_a_stop_not_a_restart() {
    let old_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio", "realtime", &[]),
        service("plugin", "/plugin", "realtime", &["audio"]),
    ));
    let new_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio", "realtime", &[]),
        service_with("plugin", "/plugin", "realtime", &["audio"], false),
    ));
    let plan = plan_update(
        &Graph::build(&old_m).unwrap(),
        &Graph::build(&new_m).unwrap(),
    );
    assert_eq!(plan.stop, vec!["plugin"]);
    assert!(plan.start.is_empty());
    assert!(plan.restart.is_empty());
}

#[test]
fn turning_a_service_on_is_a_start_not_a_restart() {
    let old_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio", "realtime", &[]),
        service_with("plugin", "/plugin", "realtime", &["audio"], false),
    ));
    let new_m = manifest(&format!(
        "{}{}",
        service("audio", "/audio", "realtime", &[]),
        service("plugin", "/plugin", "realtime", &["audio"]),
    ));
    let plan = plan_update(
        &Graph::build(&old_m).unwrap(),
        &Graph::build(&new_m).unwrap(),
    );
    assert_eq!(plan.start, vec!["plugin"]);
    assert!(plan.stop.is_empty());
    assert!(plan.restart.is_empty());
}
