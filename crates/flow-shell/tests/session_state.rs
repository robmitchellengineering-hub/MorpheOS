//! A session's state belongs to the session, and closing the view does not lose the work.
//!
//! This is item 4 of the P1 objective: the Session — not a window list — is the unit of the session manager.
//! The shell already held sessions as its unit for layout and focus; what was missing was that a session owns
//! *state*. These tests pin the ownership without linking a storage engine into the compositor: the shell
//! carries the layer root, and `morph-capsule` owns what a layer is.

use std::path::PathBuf;

use flow_session::{Openable, Session};
use flow_shell::Shell;

fn shell() -> Shell {
    Shell::new(1280, 800)
}

#[test]
fn a_session_has_no_state_until_it_is_given_some() {
    let mut shell = shell();
    let id = shell.add_session_opening("Q3 report", vec![Openable::tool("editor")]);
    assert_eq!(shell.layer_root(id), None, "a session with no state is a real state, not an error");
    assert!(shell.attach_layer(id, "/persist/sessions/q3"));
    assert_eq!(shell.layer_root(id), Some(&PathBuf::from("/persist/sessions/q3")));
}

#[test]
fn attaching_state_to_a_session_that_does_not_exist_is_reported() {
    let mut shell = shell();
    let id = shell.add_session_opening("gone", Vec::new());
    assert!(shell.close_session(id));
    assert!(!shell.attach_layer(id, "/persist/sessions/gone"), "a caller bug must not be silent");
}

#[test]
fn the_state_root_survives_focus_changes_and_relayout() {
    let mut shell = shell();
    let a = shell.add_session_opening("Q3 report", vec![Openable::tool("editor")]);
    let b = shell.add_session_opening("call", vec![Openable::legacy_app("meet")]);
    assert!(shell.attach_layer(a, "/persist/sessions/q3"));
    // Focus moves to b; a's state must still be a's.
    assert!(shell.focus(b), "focusing a live session must succeed");
    assert_eq!(shell.layer_root(a), Some(&PathBuf::from("/persist/sessions/q3")));
    assert_eq!(shell.layer_root(b), None);
}

#[test]
fn closing_a_session_hands_back_its_state_instead_of_dropping_it() {
    let mut shell = shell();
    let id = shell.add_session_opening("Q3 report", vec![Openable::tool("editor")]);
    assert!(shell.attach_layer(id, "/persist/sessions/q3"));
    let kept = shell.close_session_keeping_state(id);
    assert_eq!(kept, Some(PathBuf::from("/persist/sessions/q3")));
    // The view is gone; the work is not.
    assert_eq!(shell.layer_root(id), None);
    assert!(shell.sessions().is_empty());
}
