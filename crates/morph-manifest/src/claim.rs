//! A claim the Conductor makes about a machine, and the discipline attached to it
//! (doc 27 §2.1).
//!
//! A claim is only as strong as the falsifier it names: a claim marked
//! `demonstrated` with no falsifier is refused, not trusted — *"a check that
//! cannot fail in the way you fear is not a check"*. This is the smallest real
//! piece of the pacland verification discipline, ported to Rust.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a claim is settled — the kind of observation that could prove or refute it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OracleClass {
    /// The bootloader's failed-boot counter (the A/B trial boot's own oracle).
    BootCount,
    /// A log line that must (or must not) appear.
    LogLine,
    /// A probe: a command, a network check, a sensor read.
    Probe,
    /// A re-run of the same operation, byte-compared to a stored result.
    Rerun,
}

impl OracleClass {
    pub fn as_str(self) -> &'static str {
        match self {
            OracleClass::BootCount => "boot-count",
            OracleClass::LogLine => "log-line",
            OracleClass::Probe => "probe",
            OracleClass::Rerun => "rerun",
        }
    }
}

/// The strength of a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimStatus {
    /// A falsifier is named and the claim has survived the check against it.
    Demonstrated,
    /// The claim is asserted but no check has been run (a statement, not a proof).
    Stated,
    /// The claim is honestly not demonstrated yet.
    NotDemonstrated,
}

impl ClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ClaimStatus::Demonstrated => "demonstrated",
            ClaimStatus::Stated => "stated",
            ClaimStatus::NotDemonstrated => "not-demonstrated",
        }
    }
}

/// One claim about the machine, with the discipline attached (doc 27 §2.1).
///
/// Fields, and why each is required by the discipline:
/// - `oracle_class` — *what kind of check could settle this*; a claim without a
///   way to be checked is decoration.
/// - `falsifier` — *the concrete observation that would refute it*, written
///   before the claim is acted on. Required to call the claim `demonstrated`.
/// - `remedy` — *what to do when the falsifier fires* (roll back, re-trial, ask).
/// - `status` — the claim's strength; the judge refuses `demonstrated` without a
///   falsifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// A short stable id, e.g. `boot-a-committed`. Two claims with one id are a
    /// tension, not a silent overwrite.
    pub id: String,
    pub oracle_class: OracleClass,
    /// The observation that would refute the claim. `None` means the claim has
    /// no falsifier, so it can never be `demonstrated`.
    #[serde(default)]
    pub falsifier: Option<String>,
    pub remedy: String,
    pub status: ClaimStatus,
}
