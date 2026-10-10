//! Layering: `base` → profile defaults → machine file → (later) user overlays.
//!
//! Merge rules, applied left to right:
//! - tables merge key by key, recursively;
//! - arrays whose elements are all tables with a `name` merge element by
//!   element on `name` (a matching element merges, a new one is appended);
//! - anything else in the overlay replaces the base value.

use serde_json::Value;

use crate::Error;
use crate::model::{Manifest, Profile};

const BASE: &str = include_str!("../profiles/base.toml");
const SEED: &str = include_str!("../profiles/seed.toml");
const DESK: &str = include_str!("../profiles/desk.toml");
const STUDIO: &str = include_str!("../profiles/studio.toml");
const FLEET: &str = include_str!("../profiles/fleet.toml");

fn profile_source(profile: Profile) -> &'static str {
    match profile {
        Profile::Seed => SEED,
        Profile::Desk => DESK,
        Profile::Studio => STUDIO,
        Profile::Fleet => FLEET,
    }
}

pub fn parse_toml(source: &str, origin: &str) -> Result<Value, Error> {
    toml::from_str::<Value>(source).map_err(|e| Error::Parse {
        origin: origin.to_string(),
        message: e.to_string(),
    })
}

/// Merge `overlay` into `base` in place, following the module's merge rules.
pub fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (Value::Array(base), Value::Array(overlay))
            if is_named_list(base) && is_named_list(&overlay) =>
        {
            for item in overlay {
                let name = item_name(&item).map(str::to_owned);
                match base.iter_mut().find(|b| item_name(b) == name.as_deref()) {
                    Some(existing) => merge(existing, item),
                    None => base.push(item),
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn is_named_list(items: &[Value]) -> bool {
    items.iter().all(|v| item_name(v).is_some())
}

fn item_name(v: &Value) -> Option<&str> {
    v.as_object()?.get("name")?.as_str()
}

/// Read the profile a machine file asks for, before anything is merged.
fn requested_profile(machine: &Value) -> Result<Profile, Error> {
    let raw = machine
        .pointer("/machine/profile")
        .cloned()
        .ok_or_else(|| Error::Invalid("machine.profile is required".into()))?;
    serde_json::from_value(raw).map_err(|e| Error::Invalid(format!("machine.profile: {e}")))
}

/// Merge a machine file over `base` and its profile, returning the merged
/// document (still untyped, so it can be diffed and rendered).
pub fn resolve_value(machine_source: &str, origin: &str) -> Result<Value, Error> {
    let machine = parse_toml(machine_source, origin)?;
    let profile = requested_profile(&machine)?;
    let mut merged = parse_toml(BASE, "profiles/base.toml")?;
    merge(
        &mut merged,
        parse_toml(
            profile_source(profile),
            &format!("profiles/{}.toml", profile.as_str()),
        )?,
    );
    merge(&mut merged, machine);
    Ok(merged)
}

/// Resolve a machine file into a typed, fully populated `Manifest`.
pub fn resolve(machine_source: &str, origin: &str) -> Result<Manifest, Error> {
    from_value(resolve_value(machine_source, origin)?)
}

pub fn from_value(value: Value) -> Result<Manifest, Error> {
    serde_json::from_value(value).map_err(|e| Error::Invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tables_merge_recursively() {
        let mut base = json!({"a": {"x": 1, "y": 2}, "b": 1});
        merge(&mut base, json!({"a": {"y": 3}, "c": 4}));
        assert_eq!(base, json!({"a": {"x": 1, "y": 3}, "b": 1, "c": 4}));
    }

    #[test]
    fn named_lists_merge_by_name() {
        let mut base = json!([{"name": "a", "v": 1}, {"name": "b", "v": 1}]);
        merge(
            &mut base,
            json!([{"name": "b", "v": 2}, {"name": "c", "v": 3}]),
        );
        assert_eq!(
            base,
            json!([{"name": "a", "v": 1}, {"name": "b", "v": 2}, {"name": "c", "v": 3}])
        );
    }

    #[test]
    fn plain_lists_are_replaced() {
        let mut base = json!({"wifi": ["home", "work"]});
        merge(&mut base, json!({"wifi": ["cafe"]}));
        assert_eq!(base, json!({"wifi": ["cafe"]}));
    }

    #[test]
    fn missing_profile_is_an_error() {
        let err = resolve("manifest_version = 1\n[machine]\nname = \"x\"\n", "t").unwrap_err();
        assert!(err.to_string().contains("machine.profile"));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let src = "[machine]\nname = \"x\"\nprofile = \"desk\"\narch = \"x86_64\"\n[storage]\nreplica = []\n";
        let err = resolve(src, "t").unwrap_err();
        assert!(err.to_string().contains("replica"), "{err}");
    }
}
