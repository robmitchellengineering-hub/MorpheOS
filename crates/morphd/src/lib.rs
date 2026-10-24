//! `morphd`: apply the Manifest generation in force to the machine.
//!
//! The Manifest is validated by `morph-manifest` and stored as generations, but nothing has
//! ever obeyed it — `validate.rs` keeps an `unenforced_settings()` list of the fields nothing
//! reads. This crate is the other half: one *applier* per setting, each a small reversible
//! operation, so a generation becomes machine state through `morph-transaction` (plan, apply,
//! health check, commit or roll back).
//!
//! # The applier contract
//!
//! An applier has exactly three methods, and they are the transaction's whole vocabulary for
//! one setting:
//!
//! - [`Applier::apply`] puts the new value in force, refusing *before* mutating anything if it
//!   cannot;
//! - [`Applier::read_back`] reads the value that is actually in force, so the health check
//!   compares real state, never intent;
//! - [`Applier::undo`] puts the previous value back, for a refused apply or a rollback.
//!
//! An applier is deliberately small and side-effect-free beyond its one setting: the reason a
//! change is safe to take back is that taking it back is the same operation in reverse.

pub mod heal;
pub mod hostname;
pub mod min_free;
pub mod retention;

pub use heal::{Convergence, converge};
pub use hostname::{Hostname, Kernel};
pub use min_free::{MinFreePercent, MinFreePercentPolicy};
pub use retention::{Retention, RetentionPolicy};

/// A single reversible setting change.
pub trait Applier {
    /// The value the setting takes (a hostname string, a retention policy, …).
    type Value;

    /// Put `value` in force.
    fn apply(&self, value: &Self::Value) -> Result<(), String>;

    /// Read the value in force on the machine.
    fn read_back(&self) -> Result<Self::Value, String>;

    /// Put `previous` back.
    fn undo(&self, previous: &Self::Value) -> Result<(), String>;
}
