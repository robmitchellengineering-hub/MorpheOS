//! The Flow shell as a Wayland compositor.
//!
//! The library holds what is platform-independent and testable everywhere: the window
//! manager's decisions ([`wm`]) and which display card shows the shell ([`display`]). The
//! `flow-compositor` binary carries them out with Smithay, on Linux.

pub mod display;
pub mod wm;
