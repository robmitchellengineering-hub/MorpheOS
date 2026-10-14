//! The **service graph** and the **applier** that changes it transactionally.
//!
//! # The gap this closes
//!
//! A Manifest *validates* (`morph-manifest`), but until this crate nothing **applied** it.
//! `morph-manifest` can refuse an invalid generation and keep the ones it has been given,
//! and `morph-transaction` can arm a trial, apply, health-check and commit or roll back —
//! but the two were never joined to the Manifest's `services`. The machine is described by a
//! document that names what runs and in what order, and no component turned that description
//! into an order, let alone a change to one. A service that depends on a service that does
//! not exist, or a start order that loops back on itself, was a configuration mistake nobody
//! noticed.
//!
//! This crate is the half of `morphd` (doc 03's PID 1) that is buildable and provable before
//! the daemon exists:
//!
//! - [`graph::Graph`] reads a Manifest's `services`, resolves their `after` edges, and
//!   computes a valid start, stop and update order — refusing a **cycle**, a **missing
//!   dependency**, or a Manifest that fails its own **invariants**, rather than ordering
//!   around the problem.
//! - [`applier::apply`] takes that graph and changes it the way doc 03 says every state
//!   change happens: arm a trial, apply, health-check, commit or roll back — through the
//!   existing `morph-transaction` and the generation store.
//!
//! # What this crate is not
//!
//! It is **not `morphd`**, and deliberately so. It does not become PID 1, it does not start,
//! stop or supervise a process, and it does not replay boot I/O. A service's `exec` is read
//! only to *detect that the service changed*; it is never executed here. The health check is
//! **supplied by the caller**, exactly as in `morph-transaction`: this crate orders services
//! and transacts their configuration, and it does not know how to tell whether a running
//! service is healthy. `morphd` will own that check; until it does, a check that never ran
//! is not a pass, and the transaction treats a missing check as a reason to roll back.
//!
//! # Manifest fields this crate still does not enforce (H5)
//!
//! [`morph_manifest::validate`] flags a set of fields as *"accepted and enforced by nothing
//! yet"*, and this crate does not change that. It reads `manifest.services` (each service's
//! `name`, `exec`, `class`, `after` and `enabled`) and it reuses `validate` for the
//! invariants. The fields below remain **advisory — parsed, validated for shape, and read by
//! no component** — and setting them has no effect here:
//!
//! - `system.channel` — no component reads a release channel.
//! - `system.update_ring` — update rings are Fleet-tier and not built.
//! - `storage.retention` — the retention policy is advisory; nothing evicts on it yet.
//! - `heal.max_auto_rung` and `heal.ask_after_failed_rungs` — how far a heal engine may act
//!   on its own; the slot repair in `morph-heal` is triggered by boot history, not by these.
//! - everything else but `services`: the machine, the kernel and boot settings, free space,
//!   replicas, heal's switch, the network, the users and the capsules. Nothing on a machine
//!   reads the Manifest yet (doc 20, S11).
//!
//! They stay listed in [`morph_manifest::unenforced_settings`], which is the full list, and
//! the warning `validate` emits on every Manifest is still true of them.

pub mod applier;
pub mod graph;
pub mod plan;

pub use applier::{ApplyOutcome, apply};
pub use graph::Graph;
pub use plan::{ServicePlan, plan_update};

use std::fmt;

/// The service graph's own answer, or a pass-through of the layers below it.
#[derive(Debug)]
pub enum Error {
    /// The graph is broken: a cycle, a missing dependency, or a Manifest that fails its own
    /// invariants. The order is refused rather than guessed.
    Graph(String),
    /// The Manifest layer's answer (a document that does not fit the model), passed through
    /// unchanged.
    Manifest(morph_manifest::Error),
    /// The transaction layer's answer (a refusal or a defect mid-transaction), passed through
    /// unchanged.
    Transaction(morph_transaction::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Graph(message) => write!(f, "invalid service graph: {message}"),
            Error::Manifest(e) => write!(f, "{e}"),
            Error::Transaction(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<morph_manifest::Error> for Error {
    fn from(e: morph_manifest::Error) -> Error {
        Error::Manifest(e)
    }
}

impl From<morph_transaction::Error> for Error {
    fn from(e: morph_transaction::Error) -> Error {
        Error::Transaction(e)
    }
}
