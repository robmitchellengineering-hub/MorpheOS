//! The Wi-Fi picker (doc 20, E5): the list a person chooses a network from, the password
//! they type, and the join, over NetworkManager (A6).
//!
//! [`picker`] is the whole of what the person sees and does, with no Wayland and no D-Bus in
//! it, tested anywhere. [`backend`] is where networks come from: [`nm`] on a machine, or a
//! fake for the compositor's tests. The window is `src/main.rs`.

pub mod backend;
#[cfg(target_os = "linux")]
pub mod nm;
pub mod picker;
