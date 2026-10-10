//! MorpheOS Manifest: the typed, layered, validated description of a machine.
//!
//! - [`layer`] merges `base` → profile → machine file into one document.
//! - [`validate`] checks structure and the "can't brick" invariants.
//! - [`plan`] diffs two generations and classifies every change by risk.
//!
//! `morphd` applies only Manifests that pass validation, and the AI layer can
//! only propose new generations, which go through [`plan::plan_values`].

pub mod layer;
pub mod model;
pub mod plan;
pub mod validate;

pub use layer::{resolve, resolve_value};
pub use model::Manifest;
pub use plan::{Plan, Risk, plan_values};
pub use validate::{Finding, Report, Severity, validate};

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// A file is not valid TOML.
    Parse { origin: String, message: String },
    /// The merged document does not fit the Manifest model.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { origin, message } => write!(f, "{origin}: {message}"),
            Error::Invalid(message) => write!(f, "invalid manifest: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// JSON Schema for the resolved Manifest, for editors and AI planners.
pub fn json_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Manifest)).expect("schema serializes")
}
