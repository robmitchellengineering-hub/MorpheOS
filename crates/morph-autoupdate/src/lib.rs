//! The unattended-update entry point (doc 20, S3): check the server, decide, fetch, install
//! and trial — the whole flow the timer runs, so the timer's unit stays one line and the
//! parts that are pure enough to test stay testable.
//!
//! The decision itself lives in `morph-update`'s [`morph_update::decide`], and the fetch in
//! [`morph_update::fetch_with_ca`]; this crate is the glue that reads the machine, records
//! the armed trial, and orders the pieces. The one piece of state it owns is the record of
//! what was armed, so the *next* timer fire — which happens before the machine has rebooted
//! into the trial — skips instead of rewriting the spare slot and re-arming in a loop
//! (doc 20, S3 note 5).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where the record of the last armed update lives, on the shared data partition (doc 20, S3
/// note 3): the root is read-only, and this must survive the reboot into the trial.
pub const STATE_FILE: &str = "/persist/morph-update/state.json";
/// Where the boot history lives — the path `morph-boot-check` writes through its drop-in.
pub const HISTORY_FILE: &str = "/efi/morph-boot/history.jsonl";

/// The record written just before a trial is armed: which release is in which slot, and that
/// its trial is armed. After a fallback the boot history names the slot and this names the
/// release, so that release can be marked tried (doc 20, S3 note 3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub release: u64,
    /// `a` or `b`, the slot the release was written into.
    pub slot: String,
    pub state: StateKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StateKind {
    /// Installed and the trial armed; the machine has not yet rebooted into it (or, if the
    /// trial later fell back, this is how that release is remembered as tried).
    Armed,
}

/// What the recorded state means for this check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived {
    /// A trial was armed and the machine has not rebooted into it: skip, or the spare slot
    /// would be rewritten and re-armed in a loop (doc 20, S3 note 5).
    pub trial_armed: bool,
    /// Releases whose trial already fell back: never fetch them again.
    pub tried: Vec<u64>,
}

/// Derive `trial_armed` and `tried` from the recorded state and whether the slot in it has
/// since fallen back. Pure, so a table of tests covers every leaf (AGENTS.md H4).
///
/// `slot_fell_back` is true when the boot history records a `fallback` for `state.slot`; it
/// is the caller's job to read that, so this function needs no filesystem.
pub fn derive(state: Option<&State>, running_release: u64, slot_fell_back: bool) -> Derived {
    match state {
        Some(s) if s.state == StateKind::Armed => {
            if s.release > running_release {
                // The armed release is newer than what is running: either the trial has not
                // rebooted yet, or it fell back. The boot history distinguishes the two.
                if slot_fell_back {
                    Derived {
                        trial_armed: false,
                        tried: vec![s.release],
                    }
                } else {
                    Derived {
                        trial_armed: true,
                        tried: vec![],
                    }
                }
            } else {
                // The trial committed: the running system is now at least that release.
                Derived {
                    trial_armed: false,
                    tried: vec![],
                }
            }
        }
        _ => Derived {
            trial_armed: false,
            tried: vec![],
        },
    }
}

/// Whether the boot history records a fallback of `slot` (its trial failed). Each line is
/// the JSON `morph-boot-check` writes: `{"status":{"booted":"b",...},"decision":{"action":"fallback",...}}`.
/// A line that does not parse is skipped — the history is append-only and this only needs to
/// find the one fallback line that matters, not to validate the whole file.
pub fn slot_fell_back(history: &str, slot: &str) -> bool {
    #[derive(Deserialize)]
    struct Booted {
        booted: String,
    }
    #[derive(Deserialize)]
    struct Decision {
        action: String,
    }
    #[derive(Deserialize)]
    struct Line {
        status: Booted,
        decision: Decision,
    }
    history.lines().any(|line| {
        serde_json::from_str::<Line>(line)
            .map(|l| l.status.booted == slot && l.decision.action == "fallback")
            .unwrap_or(false)
    })
}

/// The arguments the timer's unit passes, in one place so the unit stays a single line and
/// the test drop-in overrides only what a test changes.
#[derive(Debug, Clone)]
pub struct Config {
    /// The update server's base URL, e.g. `https://10.0.2.2:8123`.
    pub base: String,
    /// An extra CA certificate to trust, the autoupdate test image's throwaway CA (doc 20,
    /// S3 note 4). `None` in production.
    pub ca: Option<PathBuf>,
    pub trust: PathBuf,
    pub release_file: PathBuf,
    pub esp: PathBuf,
    pub cmdline: PathBuf,
    pub state_file: PathBuf,
    pub history_file: PathBuf,
    pub staging: PathBuf,
    /// Reboot right after arming (the test drop-in) rather than at the next boot a person
    /// makes (doc 20, S3 note 6).
    pub reboot_now: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base: String::new(),
            ca: None,
            trust: morph_update::TRUSTED_KEYS_DIR.into(),
            release_file: morph_update::RELEASE_FILE.into(),
            esp: "/efi".into(),
            cmdline: "/proc/cmdline".into(),
            state_file: STATE_FILE.into(),
            history_file: HISTORY_FILE.into(),
            staging: "/persist/morph-update/staging".into(),
            reboot_now: false,
        }
    }
}

fn read_state(path: &Path) -> Option<State> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_state(path: &Path, state: &State) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Print a reason and succeed. "Skip" is a success, never a failed unit: a machine that cannot
/// reach the server, has nothing to install, or refuses a payload is not broken, and a failed
/// timer unit would read as an unhealthy boot (doc 20, S3 note 3).
fn skip(reason: impl std::fmt::Display) -> Result<(), String> {
    eprintln!("morph-autoupdate: skipping: {reason}");
    Ok(())
}

/// Run the whole check. Returns `Ok(())` for both "nothing to do" and "installed and armed";
/// a skip is printed and the process exits 0, so a refused or unreachable update never fails
/// the unit. `Err` is only for a genuine inconsistency after the payload was already verified
/// (the install or the arm could not happen), which the main maps to exit 1.
pub fn run(cfg: &Config) -> Result<(), String> {
    let mut bc = morph_boot::backend(&morph_boot::BackendOptions {
        backend: None,
        autoboot: "/bootfs/autoboot.txt".into(),
        dt: "/proc/device-tree/chosen/bootloader".into(),
        esp: cfg.esp.clone(),
        cmdline: cfg.cmdline.clone(),
    });

    let status = match morph_boot::status(bc.as_ref()) {
        Ok(s) => s,
        Err(e) => return skip(format!("cannot read the boot state: {e}")),
    };
    let running = match morph_update::read_release(&cfg.release_file) {
        Ok(r) => r,
        Err(e) => return skip(format!("cannot read the running release: {e}")),
    };
    let state = read_state(&cfg.state_file);
    let history = std::fs::read_to_string(&cfg.history_file).unwrap_or_default();
    let fell_back = state
        .as_ref()
        .is_some_and(|s| slot_fell_back(&history, &s.slot));
    let derived = derive(state.as_ref(), running, fell_back);
    let server = match morph_update::latest(&cfg.base, cfg.ca.as_deref()) {
        Ok(r) => r,
        Err(e) => return skip(format!("cannot read the server's release: {e}")),
    };

    let release = match morph_update::decide(
        status.phase,
        running,
        server,
        &derived.tried,
        derived.trial_armed,
    ) {
        morph_update::AutoDecision::Skip { reason } => return skip(reason),
        morph_update::AutoDecision::Fetch { release } => release,
    };

    let trusted = match morph_update::TrustedKeys::load(&cfg.trust) {
        Ok(k) => k,
        Err(e) => return Err(format!("the image accepts no update: {e}")),
    };
    let dir = cfg.staging.join(release.to_string());
    // A refusal here (untrusted, mis-advertised, truncated) is a skip, not a failure: the
    // machine did its job, and a failed unit would read as an unhealthy boot (doc 20, S3).
    let mut payload =
        match morph_update::fetch_with_ca(&cfg.base, release, &dir, &trusted, cfg.ca.as_deref()) {
            Ok(p) => p,
            Err(e) => return skip(format!("the payload was refused: {e}")),
        };

    let target = match morph_boot::update_target(bc.as_ref()) {
        Ok(t) => t,
        Err(e) => return Err(format!("no slot may be updated: {e}")),
    };
    let layout = morph_update::Layout {
        esp: cfg.esp.clone(),
        root_device: PathBuf::from(format!("/dev/disk/by-partlabel/root-{}", target.as_str())),
        running_release: running,
    };
    let outcome = morph_update::install(bc.as_ref(), &mut payload, &layout, None)
        .map_err(|e| format!("install failed: {e}"))?;

    // Record BEFORE arming: the next timer fire reads this and skips, instead of rewriting
    // the slot and re-arming (doc 20, S3 note 5).
    let state = State {
        release: outcome.release,
        slot: outcome.slot.as_str().to_string(),
        state: StateKind::Armed,
    };
    write_state(&cfg.state_file, &state)?;

    let reboot = morph_boot::start_trial(bc.as_mut(), outcome.slot)
        .map_err(|e| format!("arming the trial failed: {e}"))?;
    eprintln!(
        "morph-autoupdate: installed release {} into slot {} and armed its trial",
        outcome.release, outcome.slot
    );
    if cfg.reboot_now {
        run_command(&reboot)?;
    }
    Ok(())
}

fn run_command(cmd: &[String]) -> Result<(), String> {
    let status = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .status()
        .map_err(|e| format!("{}: {e}", cmd[0]))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{} exited with {status}", cmd.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armed(release: u64, slot: &str) -> State {
        State {
            release,
            slot: slot.to_string(),
            state: StateKind::Armed,
        }
    }

    /// The full table for the state derivation: every leaf of [`derive`].
    #[test]
    fn derive_table() {
        let cases = [
            // (state, running, fell_back, trial_armed, tried)
            (None, 1, false, false, vec![]),
            (Some(armed(2, "b")), 1, false, true, vec![]),
            (Some(armed(2, "b")), 1, true, false, vec![2]),
            // The trial committed: running is now the armed release.
            (Some(armed(2, "b")), 2, false, false, vec![]),
            // Committed and then a later release came and went: running past the armed one.
            (Some(armed(2, "b")), 3, false, false, vec![]),
        ];
        for (state, running, fell_back, want_armed, want_tried) in cases {
            let d = derive(state.as_ref(), running, fell_back);
            assert_eq!(
                d.trial_armed, want_armed,
                "running={running} fell_back={fell_back}"
            );
            assert_eq!(
                d.tried, want_tried,
                "running={running} fell_back={fell_back}"
            );
        }
    }

    #[test]
    fn a_fallback_for_the_armed_slot_marks_it_tried() {
        let history = r#"{"state":"running","status":{"booted":"a","default":"a","phase":"committed"},"decision":{"action":"none"}}
{"state":"degraded","status":{"booted":"b","default":"a","phase":"trial"},"decision":{"action":"fallback","to":"a","reboot":["systemctl","reboot"]}}
"#;
        assert!(slot_fell_back(history, "b"));
        assert!(!slot_fell_back(history, "a"));
    }

    #[test]
    fn a_commit_is_not_a_fallback() {
        let history = r#"{"state":"running","status":{"booted":"b","default":"a","phase":"trial"},"decision":{"action":"committed","slot":"b"}}
"#;
        assert!(!slot_fell_back(history, "b"));
    }

    #[test]
    fn unparseable_history_lines_are_skipped_not_fatal() {
        assert!(!slot_fell_back("garbage\nnot json\n", "b"));
    }
}
