//! The feel is inferred, not asked — and the negative control that proves the rule bites.

use flow_session::{Capability, Feel, Openable, OpenableKind, Session, SessionId, infer_feel};

fn id(n: u64) -> SessionId {
    SessionId(n)
}

#[test]
fn a_single_legacy_app_is_classic() {
    let feel = infer_feel(&[Openable::legacy_app("Photos")]);
    assert_eq!(feel, Feel::Classic);
}

#[test]
fn several_legacy_apps_are_classic() {
    // Decision 3: "run these legacy apps in windows" is itself a Classic desktop session.
    let feel = infer_feel(&[
        Openable::legacy_app("browser"),
        Openable::legacy_app("mail"),
    ]);
    assert_eq!(feel, Feel::Classic);
}

#[test]
fn a_task_with_a_few_tools_is_session_shelf() {
    let feel = infer_feel(&[
        Openable::tool("editor"),
        Openable::tool("terminal"),
        Openable::tool("build"),
    ]);
    assert_eq!(feel, Feel::SessionShelf);
}

#[test]
fn a_legacy_app_beside_tools_is_a_task() {
    // A legacy app among tools is one tool among several: the task is the unit, not the app.
    let feel = infer_feel(&[
        Openable::legacy_app("Photos"),
        Openable::tool("scanner"),
        Openable::tool("editor"),
    ]);
    assert_eq!(feel, Feel::SessionShelf);
}

#[test]
fn nothing_app_shaped_is_flow() {
    // Empty, bare surfaces, and documents are task-shaped, not app-shaped.
    assert_eq!(infer_feel(&[]), Feel::Flow);
    assert_eq!(
        infer_feel(&[Openable::new(OpenableKind::None, "a bare surface")]),
        Feel::Flow
    );
    assert_eq!(infer_feel(&[Openable::surface("that letter")]), Feel::Flow);
}

#[test]
fn a_fixed_feel_would_fail() {
    // THE NEGATIVE CONTROL (H17). Each single-case test above would be passed by a fake
    // `infer_feel` that returned one feel regardless of contents — this one fails it. The three
    // canonical inputs must map to three *different* feels, so a constant function cannot pass
    // all three assertions at once.
    let lone_app = infer_feel(&[Openable::legacy_app("Photos")]);
    let tools = infer_feel(&[Openable::tool("editor"), Openable::tool("terminal")]);
    let empty = infer_feel(&[]);

    assert_ne!(lone_app, tools, "a lone app and a tool-set must differ");
    assert_ne!(tools, empty, "a tool-set and nothing must differ");
    assert_ne!(lone_app, empty, "a lone app and nothing must differ");
}

#[test]
fn the_override_wins_over_inference() {
    let mut session = Session::opening(id(1), "Photos", vec![Openable::legacy_app("Photos")]);
    assert_eq!(
        session.feel(),
        Feel::Classic,
        "inference says Classic for a lone app"
    );

    session.set_feel(Feel::Flow);
    assert_eq!(
        session.feel(),
        Feel::Flow,
        "the override replaces the inferred feel"
    );
    assert!(session.is_feel_overridden(), "the override is visible");

    // A later content change would re-infer (a tool → Session + shelf), but the override holds.
    session.add_openable(Openable::tool("scanner"));
    assert_eq!(
        session.feel(),
        Feel::Flow,
        "an override is not silently re-inferred away"
    );
}

#[test]
fn clearing_the_override_returns_to_inference() {
    let mut session = Session::opening(id(1), "Dev", vec![Openable::tool("editor")]);
    assert_eq!(session.feel(), Feel::SessionShelf);

    session.set_feel(Feel::Classic);
    assert_eq!(session.feel(), Feel::Classic);

    session.clear_feel_override();
    assert_eq!(
        session.feel(),
        Feel::SessionShelf,
        "dropping the override re-infers"
    );
    assert!(!session.is_feel_overridden());
}

#[test]
fn the_default_and_the_empty_inference_are_two_different_things() {
    // Pins the Decision 2 / Decision 7 distinction (see the module header): a new session that
    // has opened nothing is Session + shelf (the default); a session *described* as opening
    // nothing is Flow (nothing app-shaped). Both are deliberate, so both are asserted.
    let fresh = Session::new(id(1), "a new session");
    assert_eq!(fresh.feel(), Feel::SessionShelf);
    assert_eq!(fresh.feel(), Feel::default());

    let described_empty = Session::opening(id(2), "an empty session", vec![]);
    assert_eq!(described_empty.feel(), Feel::Flow);
    assert_eq!(infer_feel(&[]), Feel::Flow);
}

#[test]
fn a_session_carries_its_identity_label_and_contents() {
    let session = Session::opening(id(7), "Dev", vec![Openable::tool("editor")]);
    assert_eq!(session.id(), id(7));
    assert_eq!(session.label(), "Dev");
    assert_eq!(session.openables().len(), 1);
    assert_eq!(session.openables()[0].kind, OpenableKind::Tool);
    assert_eq!(session.openables()[0].label, "editor");
}

#[test]
fn a_feel_scales_down_to_what_a_profile_can_run() {
    // Decision 6's seam: a feel is reduced or off, never forked into three codebases.
    assert!(Capability::Seed.can_run(Feel::Flow));
    assert!(!Capability::Seed.can_run(Feel::SessionShelf));
    assert!(!Capability::Seed.can_run(Feel::Classic));

    assert!(Capability::Frame.can_run(Feel::SessionShelf));
    assert!(!Capability::Frame.can_run(Feel::Classic));

    assert!(Capability::Windowed.can_run(Feel::Classic));

    assert_eq!(Capability::Seed.reduce(Feel::Classic), Some(Feel::Flow));
    assert_eq!(
        Capability::Frame.reduce(Feel::Classic),
        Some(Feel::SessionShelf)
    );
    assert_eq!(
        Capability::Windowed.reduce(Feel::Classic),
        Some(Feel::Classic)
    );
    assert_eq!(Capability::Seed.reduce(Feel::Flow), Some(Feel::Flow));
}
