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
pub use plan::{Applied, Plan, Risk, apply, plan_values};
pub use validate::{Finding, Report, Severity, unenforced_settings, validate};

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// A file is not valid TOML.
    Parse { origin: String, message: String },
    /// The merged document does not fit the Manifest model.
    Invalid(String),
    /// The change was understood and **refused**: it is invalid, or above the tier the caller
    /// is allowed to apply. Distinct from the two above because it is a *decision*, not a
    /// defect — the manifest parsed, the model accepted it, and something said no.
    Refused(String),
    /// The command line itself was wrong. A fourth kind because it is the only one that is
    /// *no statement about a manifest at all*: `invalid manifest: --to needs a file` names a
    /// problem that does not exist, and a message that misdescribes the situation is worse than
    /// a blunt one. Distinct from [`Error::Invalid`], which is a defect *in a manifest or in
    /// writing one*.
    Usage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { origin, message } => write!(f, "{origin}: {message}"),
            Error::Invalid(message) => write!(f, "invalid manifest: {message}"),
            Error::Refused(message) => write!(f, "refused: {message}"),
            Error::Usage(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {}

/// JSON Schema for the resolved Manifest, for editors and AI planners.
pub fn json_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Manifest)).expect("schema serializes")
}

/// Every leaf path the Manifest schema can produce, sorted and de-duplicated.
///
/// A **leaf** is a place a value can actually sit: a string, number, boolean or enum. Object
/// members recurse, `Option<T>` (which schemars writes as `anyOf: [T, null]`) recurses into
/// its non-null arm, and maps and arrays both generalise their key or index to `[*]` — the
/// same shape [`plan::risk_of`] genericises a real path into, so the two are directly
/// comparable.
///
/// It exists because **the risk table was checked against nothing.** `risk_of` classified
/// paths by a prefix table and fell back to a default for anything the table did not
/// anticipate, so a setting added to the Manifest silently acquired a risk tier nobody chose.
/// Enumerating the schema is what turns "is every path classified?" from an opinion into a
/// test.
pub fn schema_leaf_paths() -> Vec<String> {
    let schema = json_schema();
    let defs = schema.get("$defs").cloned().unwrap_or_default();
    let mut out = Vec::new();
    walk_schema(&schema, &defs, "", &mut out, 0);
    out.sort();
    out.dedup();
    out
}

fn walk_schema(
    node: &serde_json::Value,
    defs: &serde_json::Value,
    path: &str,
    out: &mut Vec<String>,
    depth: usize,
) {
    // A recursive type would otherwise walk forever. The Manifest has none today; the guard
    // is here so that adding one is a visible truncation rather than a hung test.
    if depth > 32 {
        return;
    }
    if let Some(r) = node.get("$ref").and_then(|r| r.as_str()) {
        if let Some(target) = r.strip_prefix("#/$defs/").and_then(|name| defs.get(name)) {
            walk_schema(target, defs, path, out, depth + 1);
        }
        return;
    }
    // `Option<T>` is `anyOf: [T, null]`: the null arm is not a path of its own.
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(alts) = node.get(key).and_then(|a| a.as_array()) {
            for alt in alts {
                let is_null = alt.get("type").and_then(|t| t.as_str()) == Some("null");
                if !is_null {
                    walk_schema(alt, defs, path, out, depth + 1);
                }
            }
            return;
        }
    }
    if let Some(props) = node.get("properties").and_then(|p| p.as_object()) {
        for (name, child) in props {
            walk_schema(child, defs, &format!("{path}/{name}"), out, depth + 1);
        }
        return;
    }
    // A map: keys generalise, exactly as `risk_of` generalises `[nas]` to `[*]`.
    if let Some(add) = node.get("additionalProperties") {
        walk_schema(add, defs, &format!("{path}[*]"), out, depth + 1);
        return;
    }
    if let Some(items) = node.get("items") {
        walk_schema(items, defs, &format!("{path}[*]"), out, depth + 1);
        return;
    }
    // Anything else is a value that can sit somewhere: a leaf.
    if !path.is_empty() {
        out.push(path.to_string());
    }
}
