//! The Flow shell as a Wayland compositor.
//!
//! The library holds what is platform-independent and testable everywhere: the window
//! manager's decisions ([`wm`]). The `flow-compositor` binary carries them out with
//! Smithay, on Linux.

pub mod wm;
