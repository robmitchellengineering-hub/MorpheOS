//! Watching a folder for changes: the contract, and which platform backs it.
//!
//! [`Event`] and [`Watcher`] are the contract, not the implementation. On Linux they
//! are the real inotify watcher in [`linux`]; on every other platform the same names
//! exist in [`unsupported`] so the crate, the `strata` command and the integration
//! tests still **compile** there — but `Watcher::new` refuses with a reason instead of
//! pretending to watch.
//!
//! This is not a claim that Strata runs on macOS. Strata targets Linux: doc 04 models a
//! Linux filesystem and MorpheOS ships on Pi and x86. A Mac is a **development**
//! machine, and the fallback exists to keep the documented verification bar honest
//! there — a developer on a Mac can still run the full workspace suite minus the three
//! watcher tests, rather than being unable to build the crate at all.
//!
//! **The trap this arrangement invites, and how it is handled:** the three tests in
//! `tests/break_it.rs` are marked `#[ignore]` on non-Linux *even though they compile*.
//! Two of them need a working watcher, and one of them asserts that `Watcher::new`
//! **fails** — which on a platform where it *always* fails would pass without checking
//! anything. They are ignored rather than adapted so the skip is visible in the test
//! count; a fallback that silently satisfies a negative assertion is not a fallback.
//! See `AGENTS.md` H9.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Event, Watcher};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Event, Watcher};
