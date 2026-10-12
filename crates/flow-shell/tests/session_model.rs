//! The shell builds on `flow-session`'s model — it does not redefine a session of its own.

use flow_shell::{Feel, Openable, Session, Shell};

#[test]
fn a_session_added_to_the_shell_is_the_session_model() {
    let mut shell = Shell::new(320, 200);
    let id = shell.add_session("a terminal");

    let view = &shell.sessions()[0];
    // The model type the shell carries IS `flow-session`'s `Session` (re-exported), so there
    // is exactly one session concept, and the shell adds only surface + terminal.
    let _: &Session = &view.model;

    assert_eq!(
        view.model.id(),
        id,
        "the id the shell hands out is the model's id"
    );
    assert_eq!(
        view.model.label(),
        "a terminal",
        "the label reaches the model"
    );
    assert_eq!(
        view.model.feel(),
        Feel::SessionShelf,
        "a session the shell has not described as opening anything is the default feel"
    );
    assert!(
        view.model.openables().is_empty(),
        "the shell has not been told what the session opens"
    );
}

#[test]
fn the_shell_infers_the_feel_from_what_a_session_opens() {
    let mut shell = Shell::new(320, 200);
    let id = shell.add_session_opening("Photos", vec![Openable::legacy_app("Photos")]);

    let view = &shell.sessions()[0];
    assert_eq!(view.model.id(), id);
    assert_eq!(
        view.model.feel(),
        Feel::Classic,
        "a lone legacy app infers Classic through the shell"
    );
    assert_eq!(view.model.openables().len(), 1);
}

#[test]
fn every_session_in_the_shell_carries_a_distinct_model_id() {
    let mut shell = Shell::new(320, 200);
    let first = shell.add_session("first");
    let second = shell.add_session("second");

    let ids: Vec<_> = shell.sessions().iter().map(|v| v.model.id()).collect();
    assert_eq!(
        ids,
        vec![first, second],
        "the shell's order is the model order"
    );
    assert_eq!(
        shell.focused(),
        Some(second),
        "adding a session still focuses it"
    );
}
