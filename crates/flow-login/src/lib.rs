//! `flow-login`: the sign-in screen on the Flow shell (doc 20, S5, account model 2). On a
//! first boot it asks for a name and a password and creates the one account; on every later
//! boot it asks for the same password to sign in. The account lives on `/persist`, so it
//! outlives a reboot and an update; the password is never stored, only a salted hash.

pub mod account;
pub mod login;
