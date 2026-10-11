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

/// The tier a path gets when **no rule matches it**.
///
/// ⚠️ **This constant is the whole point of this function's shape, and it used to be a
/// silent `PersistentConfig`.** Tier 2 is the middle of the range, and because it was the
/// fallback it was also the tier applied to every path the table did not anticipate — so a
/// setting added to the Manifest arrived already classified, by nobody. The risk tier is what
/// decides whether a change may be applied without a person, so a tier that was never chosen
/// is worse than a wrong one: a wrong tier can be argued with.
///
/// It is now the **most conservative** tier, which is the safe direction: an unclassified
/// change is never auto-applied, so the failure mode is "a person is asked" rather than "an
/// unclassified change is applied alone".
///
/// And it is close to unreachable: `every_leaf_in_the_schema_is_classified` walks
/// [`crate::schema_leaf_paths`] and fails if any leaf the schema can produce lands here.
/// **Reaching this constant with a real Manifest path is a bug, not a policy.**
pub const UNCLASSIFIED: Risk = Risk::SecurityCritical;

/// Classify a changed path. Rules are checked in order; the first match wins.
///
/// Returns `None` when no rule matches, so that "the table classified this" and "the table had
/// nothing to say" are distinguishable. [`risk_of`] is the version callers want.
pub fn risk_rule_of(path: &str) -> Option<Risk> {
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
        // ── classified deliberately, when the table was checked against the schema ─────
        //
        // Every rule below was added on 2026-10-11 after `every_leaf_in_the_schema_is_
        // classified_by_a_rule` found **16 of 34 schema leaves matching no rule** — which
        // meant sixteen settings had a tier nobody chose. The tier is now a decision and the
        // comment is the reason. Tiers are doc 06's: 0 cosmetic (auto, silent), 1 reversible
        // local (auto, with Undo), 2 persistent config (ask, one tap), 3 data-affecting
        // (always ask, show a diff), 4 security-critical (strong auth; the AI cannot do it
        // alone).
        //
        // 4 — what code runs, and how much the machine may do without a person. Same class
        // as `/system/boot` and `/heal/enabled`, which were already here.
        ("/manifest_version", Risk::SecurityCritical), // changes how every other value is read
        ("/system/kernel", Risk::SecurityCritical),    // selects the code booted at the top layer
        ("/services[*]/exec", Risk::SecurityCritical), // arbitrary code, as the service user
        ("/heal/max_auto_rung", Risk::SecurityCritical), // how far it may repair alone
        ("/heal/ask_after_failed_rungs", Risk::SecurityCritical), // when it must ask instead
        // 2 — preferences that change behaviour and want a person's nod. Doc 06 puts both
        // "install an app" and "change a network profile" here.
        ("/system/channel", Risk::PersistentConfig), // which software you are offered
        ("/system/update_ring", Risk::PersistentConfig),
        ("/machine/profile", Risk::PersistentConfig), // moves many defaults at once
        ("/network/ethernet", Risk::PersistentConfig),
        ("/network/wifi", Risk::PersistentConfig),
        ("/capsules[*]/source", Risk::PersistentConfig), // what gets installed
        ("/services[*]/enabled", Risk::PersistentConfig),
        ("/services[*]/after", Risk::PersistentConfig), // startup order; wrong is a bad boot
        // 1 — names, and the capsule recipe. Doc 06 names "capsule recipe" as a tier 1
        // example, and `/machine/name` was already here for the same reason.
        ("/capsules[*]/name", Risk::ReversibleLocal),
        ("/capsules[*]/runtime", Risk::ReversibleLocal),
        ("/services[*]/name", Risk::ReversibleLocal),
    ];
    RULES
        .iter()
        .find(|(prefix, _)| {
            p == *prefix
                || p.starts_with(&format!("{prefix}/"))
                || p.starts_with(&format!("{prefix}["))
        })
        .map(|(_, r)| *r)
}

/// The tier for a changed path: the table's answer, or [`UNCLASSIFIED`].
pub fn risk_of(path: &str) -> Risk {
    risk_rule_of(path).unwrap_or(UNCLASSIFIED)
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
        assert_eq!(risk_of("/network/wifi"), Risk::PersistentConfig);
        // A prefix must match a whole segment, not part of a name.
        assert_eq!(risk_of("/usersettings"), UNCLASSIFIED);
        // ⚠️ **This line used to assert `PersistentConfig`, and it was wrong in a way worth
        // keeping.** `/services[*]/exec` matched no rule, so it inherited the silent default —
        // and the test recorded that accident as if it were the decision. A service's `exec`
        // is the command run as that service, frequently as root: it is arbitrary code, which
        // is what tier 4 is for. The assertion was correct about the code and wrong about the
        // intent, which is exactly what a test of a default cannot tell you.
        assert_eq!(risk_of("/services[mixer]/exec"), Risk::SecurityCritical);
        // And the tiers the schema check forced us to choose:
        assert_eq!(risk_of("/system/kernel"), Risk::SecurityCritical);
        assert_eq!(risk_of("/capsules[daw]/runtime"), Risk::ReversibleLocal);
        assert_eq!(risk_of("/capsules[daw]/source"), Risk::PersistentConfig);
        assert_eq!(risk_of("/system/channel"), Risk::PersistentConfig);
    }

    /// **The check the risk table has never had.** It used to be a hand-written table with a
    /// silent default, so "is every path classified?" could only be answered by reading the
    /// table and the schema side by side and hoping. This walks the schema and asks.
    #[test]
    fn every_leaf_in_the_schema_is_classified_by_a_rule() {
        let leaves = crate::schema_leaf_paths();
        assert!(
            !leaves.is_empty(),
            "the schema walker found no leaves at all"
        );
        let unclassified: Vec<&String> = leaves
            .iter()
            .filter(|p| risk_rule_of(p).is_none())
            .collect();
        assert!(
            unclassified.is_empty(),
            "{} of {} schema leaves match no rule, so they fall to {:?} — a tier nobody chose \
             for them. Add a rule for each:\n{:#?}",
            unclassified.len(),
            leaves.len(),
            UNCLASSIFIED,
            unclassified
        );
    }
}
