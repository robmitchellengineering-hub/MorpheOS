//! The rewind timeline (doc 20, S6): the moments a person can go back to, and the rewind
//! that undoes a ransomware burst, over Strata.
//!
//! [`timeline`] is the whole of what the person sees and does, with no Wayland and no Strata
//! in it, tested anywhere. [`backend`] is where the moments come from: Strata on a machine,
//! or a fake for the compositor's tests. The window is `src/main.rs`.

pub mod backend;
pub mod timeline;
