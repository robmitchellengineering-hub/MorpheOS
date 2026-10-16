//! The Flow shell as a Wayland compositor.
//!
//! The library holds what is platform-independent and testable everywhere: the window
//! manager's decisions ([`wm`]), which display card shows the shell ([`display`]), and
//! the boot splash's timing and mark ([`splash`]). The `flow-compositor` binary carries
//! them out with Smithay, on Linux.

pub mod display;
pub mod splash;
pub mod wm;
