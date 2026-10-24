//! A one-message notice window on the Flow shell (doc 20, S7).
//!
//! [`notice`] is the whole of what the notice shows — a message and one colour — with no
//! Wayland in it, tested anywhere. The space guard (S7) shows "how full the disk is and what
//! was freed"; S8 reuses the same crate to show which transaction a config rollback undid.
//! The window is `src/main.rs`.
//!
//! The API stays deliberately this small — a notice is one message and one colour — so every
//! caller of the crate (now S7, later S8) uses it unchanged rather than growing its own.

pub mod notice;
