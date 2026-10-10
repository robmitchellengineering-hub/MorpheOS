//! Planning: what changes between two Manifest generations, and how risky is it.
//!
//! This is the Verifier's first job (doc 06): whoever proposed the change
//! (a person, the AI, an update), the plan classifies every changed path into a
//! risk tier, and only a valid plan at tier 1 or below may auto-apply.

use serde::Serialize;
use serde_json::Value;

use crate::model::Manifest;
use crate::validate::{Report, validate};
use crate::{Error, layer};

/// Risk tiers from doc 06.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Risk {
    Cosmetic = 0,
    ReversibleLocal = 1,
    PersistentConfig = 2,
    DataAffecting = 3,
    SecurityCritical = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
    pub risk: Risk,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub changes: Vec<Change>,
    /// Highest risk of any change (Cosmetic when nothing changed).
    pub risk: Risk,
    /// Validation of the proposed (new) generation.
    pub report: Report,
    /// True only when the new generation is valid and every change is tier 0–1.
    pub auto_apply: bool,
}

/// Plan the move from `old` to `new`, both given as resolved Manifest values.
pub fn plan_values(old: &Value, new: &Value) -> Result<Plan, Error> {
    let manifest: Manifest = layer::from_value(new.clone())?;
    let report = validate(&manifest);
    let mut changes = Vec::new();
    diff(old, new, String::new(), &mut changes);
    let risk = changes
        .iter()
        .map(|c| c.risk)
        .max()
        .unwrap_or(Risk::Cosmetic);
    let auto_apply = report.is_ok() && risk <= Risk::ReversibleLocal;
    Ok(Plan {
        changes,
        risk,
        report,
        auto_apply,
    })
}

fn diff(old: &Value, new: &Value, path: String, out: &mut Vec<Change>) {
    if old == new {
        return;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = format!("{path}/{k}");
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => diff(x, y, p, out),
                    (Some(x), None) => out.push(change(p, ChangeKind::Removed, Some(x), None)),
                    (None, Some(y)) => out.push(change(p, ChangeKind::Added, None, Some(y))),
                    (None, None) => unreachable!(),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if named(a) && named(b) => {
            for x in a {
                let n = name(x).unwrap_or_default();
                let p = format!("{path}[{n}]");
                match b.iter().find(|y| name(y) == Some(n)) {
                    Some(y) => diff(x, y, p, out),
                    None => out.push(change(p, ChangeKind::Removed, Some(x), None)),
                }
            }
            for y in b {
                let n = name(y).unwrap_or_default();
                if !a.iter().any(|x| name(x) == Some(n)) {
                    out.push(change(
                        format!("{path}[{n}]"),
                        ChangeKind::Added,
                        None,
                        Some(y),
                    ));
                }
            }
        }
        _ => out.push(change(path, ChangeKind::Changed, Some(old), Some(new))),
    }
}

fn named(items: &[Value]) -> bool {
    items.iter().all(|v| name(v).is_some())
}

fn name(v: &Value) -> Option<&str> {
    v.as_object()?.get("name")?.as_str()
}

fn change(path: String, kind: ChangeKind, before: Option<&Value>, after: Option<&Value>) -> Change {
    Change {
        risk: risk_of(&path),
        path,
        kind,
        before: before.cloned(),
        after: after.cloned(),
    }
}

/// Classify a changed path. Rules are checked in order; the first match wins.
pub fn risk_of(path: &str) -> Risk {
    // `/services[mixer]/class` → `/services[*]/class`
    let mut generic = String::with_capacity(path.len());
    let mut in_name = false;
    for ch in path.chars() {
        match ch {
            '[' => {
                in_name = true;
                generic.push_str("[*]");
            }
            ']' => in_name = false,
            _ if in_name => {}
            _ => generic.push(ch),
        }
    }
    let p = generic.as_str();

    const RULES: &[(&str, Risk)] = &[
        ("/users", Risk::SecurityCritical),
        ("/system/boot", Risk::SecurityCritical),
        ("/system/recovery_image", Risk::SecurityCritical),
        ("/heal/enabled", Risk::SecurityCritical),
        ("/machine/arch", Risk::SecurityCritical),
        ("/capsules[*]/grants", Risk::DataAffecting),
        ("/storage/retention", Risk::DataAffecting),
        ("/storage/replicas", Risk::DataAffecting),
        ("/storage/accept_single_copy", Risk::DataAffecting),
        ("/services[*]/class", Risk::ReversibleLocal),
        ("/storage/min_free_percent", Risk::ReversibleLocal),
        ("/machine/name", Risk::ReversibleLocal),
    ];
    RULES
        .iter()
        .find(|(prefix, _)| {
            p == *prefix
                || p.starts_with(&format!("{prefix}/"))
                || p.starts_with(&format!("{prefix}["))
        })
        .map(|(_, r)| *r)
        .unwrap_or(Risk::PersistentConfig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_rules() {
        assert_eq!(risk_of("/users[rob]/admin"), Risk::SecurityCritical);
        assert_eq!(risk_of("/system/boot/ab_slots"), Risk::SecurityCritical);
        assert_eq!(risk_of("/capsules[daw]/grants"), Risk::DataAffecting);
        assert_eq!(risk_of("/storage/replicas[nas]"), Risk::DataAffecting);
        assert_eq!(risk_of("/services[mixer]/class"), Risk::ReversibleLocal);
        assert_eq!(risk_of("/services[mixer]/exec"), Risk::PersistentConfig);
        assert_eq!(risk_of("/network/wifi"), Risk::PersistentConfig);
        // A prefix must match a whole segment, not part of a name.
        assert_eq!(risk_of("/usersettings"), Risk::PersistentConfig);
    }
}
