//! The Manifest data model: one typed document describing a whole machine.
//!
//! A machine file is layered on top of `base` and its profile defaults
//! (see `layer`), so every field below is present after merging. Unknown
//! fields are rejected, so a typo can never silently do nothing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Manifest format version this crate understands.
pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u32,
    pub machine: Machine,
    pub system: System,
    pub storage: Storage,
    pub heal: Heal,
    pub network: Network,
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub capsules: Vec<Capsule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    pub name: String,
    pub profile: Profile,
    pub arch: Arch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    Seed,
    Desk,
    Studio,
    Fleet,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Seed => "seed",
            Profile::Desk => "desk",
            Profile::Studio => "studio",
            Profile::Fleet => "fleet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    Aarch64,
    X86_64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub channel: Channel,
    pub update_ring: UpdateRing,
    pub kernel: KernelVariant,
    pub boot: Boot,
    /// Immutable recovery image that can rebuild everything else (heal rung H4).
    pub recovery_image: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    Stable,
    Beta,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateRing {
    Canary,
    Early,
    Broad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KernelVariant {
    Standard,
    /// PREEMPT_RT, for the Studio real-time audio profile.
    Rt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    /// A/B system slots, so an update never overwrites the running system.
    pub ab_slots: bool,
    /// Failed boots before falling back to the previous slot (heal rung H3).
    pub max_boot_attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    pub retention: Retention,
    /// Free-space floor the system keeps by evicting caches and tiering out old versions.
    pub min_free_percent: u8,
    #[serde(default)]
    pub replicas: Vec<Replica>,
    /// Explicitly accept running with no second copy of data.
    pub accept_single_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    pub all_versions_hours: u32,
    pub hourly_days: u32,
    pub daily_days: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Replica {
    pub name: String,
    pub kind: ReplicaKind,
    /// Mesh device name, disk label, or hosted region, depending on `kind`.
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReplicaKind {
    MeshPeer,
    Disk,
    Hosted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Heal {
    pub enabled: bool,
    /// Highest healing-ladder rung (0 = restart … 4 = rebuild in place) the
    /// engine may apply without asking. Rung 5 (Phoenix) always needs a person.
    pub max_auto_rung: u8,
    /// Failed rungs before the engine stops and asks the user.
    pub ask_after_failed_rungs: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub ethernet: bool,
    /// Known Wi-Fi networks by SSID. Credentials live in the secret store, never here.
    #[serde(default)]
    pub wifi: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub name: String,
    #[serde(default)]
    pub admin: bool,
    /// The user holds a recovery key for encrypted data (e.g. printed code).
    #[serde(default)]
    pub recovery_key: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub name: String,
    pub exec: String,
    pub class: IntentClass,
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Resource Governor scheduling class (doc 03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IntentClass {
    Interactive,
    Realtime,
    ForegroundBatch,
    Background,
    Opportunistic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capsule {
    pub name: String,
    pub runtime: Runtime,
    /// Content hash, recipe id or package reference the capsule is built from.
    pub source: String,
    /// Capabilities granted, e.g. `files:documents`, `device:audio`, `net:outbound`.
    #[serde(default)]
    pub grants: Vec<String>,
}

/// Compatibility ladder rung (doc 05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Runtime {
    Native,
    Linux,
    Web,
    Wine,
    Android,
    Translated,
    Microvm,
}

fn default_true() -> bool {
    true
}
