//! The non-Linux stand-in for the inotify watcher.
//!
//! It keeps the crate buildable where inotify does not exist, so that `cargo test
//! --all` still runs on a development Mac. Everything except watching works there: the
//! store, the index, detection, retention and rewind are all platform-neutral, and
//! their tests run for real.
//!
//! `Watcher::new` **always** returns [`Error::Refused`], naming the reason. That is
//! deliberate and has one consequence worth stating: the test
//! `the_store_cannot_live_inside_what_it_protects` asserts that `Watcher::new` fails,
//! so on this platform it would pass without checking its subject. It is therefore
//! `#[ignore]`d here rather than left to pass — see `AGENTS.md` H9.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::detect::Alert;
use crate::index::Version;
use crate::{Error, Result, Strata};

const WHY: &str = "the file watcher needs inotify, which only exists on Linux; \
                   the store, index, detection, retention and rewind all work here";

/// What the watcher reports, byte-for-byte the same shape as the Linux events.
///
/// Defined here as well as in `linux` so that code and tests that match on these
/// variants — `main.rs` does — type-check on every platform.
#[derive(Debug)]
pub enum Event {
    Recorded(Version),
    Alert(Alert),
    /// The kernel dropped events; every watched folder was rescanned.
    Rescanned,
    Error(String),
}

/// A watcher that cannot watch.
///
/// The lifetime is carried by a `PhantomData` rather than by a real `&Strata` because
/// there is nothing to hold: the type exists so that `Watcher<'a>` is interchangeable
/// with the Linux one at every call site, not because it has state.
pub struct Watcher<'a> {
    _handle: std::marker::PhantomData<&'a Strata>,
}

impl<'a> Watcher<'a> {
    /// Refuses, always. There is no inotify to start outside Linux.
    pub fn new(_strata: &'a Strata, _roots: &[PathBuf]) -> Result<Self> {
        Err(Error::Refused(WHY.to_string()))
    }

    /// Refuses, always, for the same reason as [`Watcher::new`].
    pub fn run(&mut self, _stop: &AtomicBool, _emit: &mut dyn FnMut(Event)) -> Result<()> {
        Err(Error::Refused(WHY.to_string()))
    }
}
