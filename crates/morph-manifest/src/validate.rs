//! Validation: structural checks plus the "unbreakable" invariants (doc 15).
//!
//! An `Error` finding blocks a Manifest from being applied. A `Warning` is
//! shown to the user (or the AI planner) but does not block.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::model::{
    IntentClass, KernelVariant, MANIFEST_VERSION, Manifest, Profile, ReplicaKind, Runtime,
};

/// Highest rung the heal engine may ever take on its own. Rung 5 (Phoenix
/// restore onto other hardware) always needs a person.
pub const MAX_AUTONOMOUS_RUNG: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    pub fn has(&self, code: &str) -> bool {
        self.findings.iter().any(|f| f.code == code)
    }

    fn push(&mut self, severity: Severity, code: &'static str, path: &str, message: String) {
        self.findings.push(Finding {
            severity,
            code,
            path: path.to_string(),
            message,
        });
    }

    fn error(&mut self, code: &'static str, path: &str, message: impl Into<String>) {
        self.push(Severity::Error, code, path, message.into());
    }

    fn warn(&mut self, code: &'static str, path: &str, message: impl Into<String>) {
        self.push(Severity::Warning, code, path, message.into());
    }
}

pub fn validate(m: &Manifest) -> Report {
    let mut r = Report::default();
    structure(m, &mut r);
    invariants(m, &mut r);
    advisories(m, &mut r);
    r
}

fn structure(m: &Manifest, r: &mut Report) {
    if m.manifest_version != MANIFEST_VERSION {
        r.error(
            "E-VERSION",
            "/manifest_version",
            format!(
                "manifest_version {} is not supported (expected {MANIFEST_VERSION})",
                m.manifest_version
            ),
        );
    }
    if m.machine.name.trim().is_empty() {
        r.error("E-NAME", "/machine/name", "machine.name must not be empty");
    }
    unique(m.users.iter().map(|u| u.name.as_str()), "/users", r);
    unique(m.services.iter().map(|s| s.name.as_str()), "/services", r);
    unique(m.capsules.iter().map(|c| c.name.as_str()), "/capsules", r);
    unique(
        m.storage.replicas.iter().map(|x| x.name.as_str()),
        "/storage/replicas",
        r,
    );

    let known: HashSet<&str> = m.services.iter().map(|s| s.name.as_str()).collect();
    for s in &m.services {
        for dep in &s.after {
            if !known.contains(dep.as_str()) {
                r.error(
                    "E-UNKNOWN-DEP",
                    &format!("/services[{}]/after", s.name),
                    format!("service '{}' starts after unknown service '{dep}'", s.name),
                );
            }
        }
    }
    if let Some(cycle) = find_cycle(m) {
        r.error(
            "E-DEP-CYCLE",
            "/services",
            format!("service start order has a cycle: {}", cycle.join(" → ")),
        );
    }
}

fn unique<'a>(names: impl Iterator<Item = &'a str>, path: &str, r: &mut Report) {
    let mut seen = HashSet::new();
    for n in names {
        if !seen.insert(n) {
            r.error(
                "E-DUPLICATE",
                path,
                format!("name '{n}' is used more than once"),
            );
        }
    }
}

/// Depth-first search over `after` edges; returns one cycle if any exists.
fn find_cycle(m: &Manifest) -> Option<Vec<String>> {
    let edges: HashMap<&str, Vec<&str>> = m
        .services
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.after.iter().map(String::as_str).collect(),
            )
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }

    fn visit<'a>(
        node: &'a str,
        edges: &HashMap<&'a str, Vec<&'a str>>,
        marks: &mut HashMap<&'a str, Mark>,
        stack: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        match marks.get(node) {
            Some(Mark::Done) => return None,
            Some(Mark::Visiting) => {
                let start = stack.iter().position(|n| *n == node).unwrap_or(0);
                let mut cycle: Vec<String> = stack[start..].iter().map(|s| s.to_string()).collect();
                cycle.push(node.to_string());
                return Some(cycle);
            }
            None => {}
        }
        marks.insert(node, Mark::Visiting);
        stack.push(node);
        for next in edges.get(node).into_iter().flatten() {
            if edges.contains_key(next)
                && let Some(c) = visit(next, edges, marks, stack)
            {
                return Some(c);
            }
        }
        stack.pop();
        marks.insert(node, Mark::Done);
        None
    }

    let mut marks = HashMap::new();
    let mut names: Vec<&str> = edges.keys().copied().collect();
    names.sort_unstable();
    for name in names {
        let mut stack = Vec::new();
        if let Some(c) = visit(name, &edges, &mut marks, &mut stack) {
            return Some(c);
        }
    }
    None
}

/// The "can't brick" invariants. Every one of these must hold for every
/// Manifest generation, whoever (user, AI, update) proposed it.
fn invariants(m: &Manifest, r: &mut Report) {
    // A bad update must never be able to strand the machine.
    if !m.system.boot.ab_slots {
        r.error(
            "INV-BOOT",
            "/system/boot/ab_slots",
            "A/B boot slots must stay enabled: without them a bad update can leave no bootable system",
        );
    }
    if !m.system.recovery_image {
        r.error(
            "INV-RECOVERY",
            "/system/recovery_image",
            "the immutable recovery image must stay enabled (heal rung H4)",
        );
    }

    // Someone must always be able to administer and unlock the machine.
    if !m.users.iter().any(|u| u.admin) {
        r.error("INV-ADMIN", "/users", "at least one user must be an admin");
    }
    if !m.users.iter().any(|u| u.recovery_key) {
        r.error(
            "INV-RECOVERY-KEY",
            "/users",
            "at least one user must hold a recovery key, or encrypted data can become unrecoverable",
        );
    }

    // Healing can't be switched off, and never acts beyond what's safe unattended.
    if !m.heal.enabled {
        r.error(
            "INV-HEAL",
            "/heal/enabled",
            "the heal engine cannot be disabled",
        );
    }
    if m.heal.max_auto_rung > MAX_AUTONOMOUS_RUNG {
        r.error(
            "INV-HEAL",
            "/heal/max_auto_rung",
            format!(
                "max_auto_rung {} is too high: rung 5 (Phoenix restore) always needs a person",
                m.heal.max_auto_rung
            ),
        );
    }
    if m.heal.ask_after_failed_rungs == 0 {
        r.error(
            "INV-HEAL",
            "/heal/ask_after_failed_rungs",
            "ask_after_failed_rungs must be at least 1",
        );
    }

    // Data must exist in two places, unless someone has knowingly accepted otherwise.
    if m.storage.replicas.is_empty() {
        if m.storage.accept_single_copy {
            r.warn(
                "W-SINGLE-COPY",
                "/storage/replicas",
                "only one copy of your data exists; a dead disk will lose it",
            );
        } else {
            r.error(
                "INV-COPIES",
                "/storage/replicas",
                "add a replica (mesh peer, disk or hosted), or set accept_single_copy = true",
            );
        }
    }
    if !(5..=50).contains(&m.storage.min_free_percent) {
        r.error(
            "INV-FREE-SPACE",
            "/storage/min_free_percent",
            "min_free_percent must be 5–50 so the disk can never fill completely",
        );
    }
}

/// Manifest settings that are parsed and validated and **read by no component**.
///
/// WHY THIS IS A LIST AND NOT A SILENCE. A configuration field that nothing obeys is
/// worse than a missing one: this Manifest is the contract between the operator, the AI
/// and the system, so a field that looks implemented and does nothing is a promise the
/// system does not keep. Six had accumulated here before anything noticed (2026-10-10).
/// One of them, `boot.max_boot_attempts`, backed doc 15's *"3 failed boots trigger an
/// automatic fallback"* while `morph-boot` falls back after **one**, because the
/// firmware's trial is one-shot by construction. Listing it here is what turned that
/// into a decision: ADR 0008 keeps the one try and removed the field.
///
/// ONE GROUPED WARNING, not one per field: a warning per field on every validate would train
/// people to ignore warnings, which is its own failure mode.
///
/// **Deleting a line here is part of implementing the field.** `tests/manifests.rs`
/// fails if a path stops matching the schema, so a rename cannot silently retire a
/// warning — the check is on the path, and the path is the thing that rots.
pub fn unenforced_settings() -> &'static [(&'static str, &'static str)] {
    &[
        ("/system/channel", "no component reads a release channel"),
        (
            "/system/update_ring",
            "update rings are Fleet-tier and not built",
        ),
        ("/heal/max_auto_rung", "there is no heal engine yet"),
        (
            "/heal/ask_after_failed_rungs",
            "there is no heal engine yet",
        ),
        // Found on 2026-10-14 by reading what each component reads: nothing on a machine
        // reads the Manifest at all yet (doc 20, S11), and these had no line here. The three
        // that are now obeyed — /machine/name (morphd's hostname applier), /storage/retention
        // (morphd's retention applier + strata prune) and /storage/min_free_percent (morphd's
        // min-free applier + strata guard, doc 20 S7) — have been removed from this list.
        ("/machine/profile", "no component reads a scaling profile"),
        (
            "/machine/arch",
            "the architecture is the image's, not read from it",
        ),
        (
            "/system/kernel",
            "the kernel is the image's, not chosen from it",
        ),
        (
            "/system/boot",
            "the boot path is the image's (ADR 0006), not read from it",
        ),
        (
            "/system/recovery_image",
            "there is no recovery image yet (doc 20, S4)",
        ),
        (
            "/storage/replicas",
            "nothing replicates yet (doc 20, P1); the invariants on them are checked, no more",
        ),
        (
            "/storage/accept_single_copy",
            "nothing replicates yet, so nothing can be a single copy",
        ),
        ("/heal/enabled", "there is no heal engine yet"),
        (
            "/network",
            "NetworkManager is configured by the image, not from it",
        ),
        (
            "/users",
            "nothing creates an account from it (doc 20, S5); the invariants are checked, no more",
        ),
        ("/capsules", "there are no capsules yet (doc 20, M5)"),
    ]
}

fn advisories(m: &Manifest, r: &mut Report) {
    let networked = m.network.ethernet || !m.network.wifi.is_empty();
    for x in &m.storage.replicas {
        if matches!(x.kind, ReplicaKind::MeshPeer | ReplicaKind::Hosted) && !networked {
            r.warn(
                "W-REPLICA-UNREACHABLE",
                &format!("/storage/replicas[{}]", x.name),
                format!(
                    "replica '{}' needs a network, but none is configured",
                    x.name
                ),
            );
        }
    }

    // What this Manifest says that nothing acts on. One line, and it is true.
    let unenforced = unenforced_settings();
    let names: Vec<&str> = unenforced.iter().map(|(p, _)| *p).collect();
    r.warn(
        "W-UNENFORCED",
        "/",
        format!(
            "{} setting(s) in this Manifest are accepted and enforced by nothing yet: {} — they describe behaviour the system does not have. Setting them is harmless; nothing reads them today.",
            unenforced.len(),
            names.join(", ")
        ),
    );

    let rt = m.system.kernel == KernelVariant::Rt;
    if rt && m.machine.profile != Profile::Studio {
        r.warn(
            "W-RT-KERNEL",
            "/system/kernel",
            "the real-time kernel is meant for the studio profile and costs battery elsewhere",
        );
    }
    for s in &m.services {
        if s.class == IntentClass::Realtime && !rt {
            r.warn(
                "W-REALTIME-CLASS",
                &format!("/services[{}]/class", s.name),
                format!(
                    "service '{}' asks for realtime scheduling without the rt kernel",
                    s.name
                ),
            );
        }
    }

    let r_ = &m.storage.retention;
    if r_.all_versions_hours == 0 && r_.hourly_days == 0 && r_.daily_days == 0 {
        r.warn(
            "W-NO-HISTORY",
            "/storage/retention",
            "no version history is kept, so files cannot be rewound",
        );
    }

    for c in &m.capsules {
        if m.machine.profile == Profile::Seed && c.runtime == Runtime::Microvm {
            r.warn(
                "W-SEED-MICROVM",
                &format!("/capsules[{}]/runtime", c.name),
                format!(
                    "capsule '{}' needs a microVM, which won't fit a seed device's memory",
                    c.name
                ),
            );
        }
        for g in &c.grants {
            if g == "*" || g == "files:*" || g == "files:/" {
                r.warn(
                    "W-BROAD-GRANT",
                    &format!("/capsules[{}]/grants", c.name),
                    format!(
                        "capsule '{}' is granted '{g}', which is all of your files",
                        c.name
                    ),
                );
            }
        }
    }
}
