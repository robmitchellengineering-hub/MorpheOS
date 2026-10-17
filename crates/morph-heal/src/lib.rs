//! Notice that the spare slot is broken, and rebuild it from the running system.
//!
//! # The gap this closes
//!
//! The A/B design gives every machine two slots so that a bad update costs a reboot
//! ([ADR 0003](../../docs/adr/0003-unbreakable-wedge.md)). When a trial fails, the boot
//! policy falls back to the good slot — and that is where it stops. **The machine is left
//! with one working slot and one broken spare, and it stays that way until the next
//! update.** So the redundancy the whole design rests on is missing exactly when it has
//! just been needed: the next update overwrites the only other copy of the system, and if
//! *that* one fails there is nothing left to fall back to.
//!
//! This is the repair half, and it needs no recovery image. The boot history already
//! records which slot failed its trial; the running system is already a good image; and
//! `crates/morph-update` already writes a slot **and verifies what it wrote**. Repairing
//! the spare is those three facts joined.
//!
//! # Why the history is the trigger
//!
//! Not "does slot B look healthy" — that is a guess dressed as a check. The history is the
//! record the system keeps about itself, and it says exactly one thing that matters here:
//! **this slot was trialled and did not survive its health check.** A repair triggered by
//! anything else would be a repair that cannot say why it ran.
//!
//! # Why it forgets
//!
//! A repair must not run on every boot forever, and it must not skip a *later* failure
//! either. So each repair records the **index of the history entry it repaired**. The
//! spare is rebuilt only when the newest failure is one that has not been repaired yet:
//! idempotent for the same failure, and resets itself when a new one happens. Nothing here
//! assumes the machine boots only once.

use std::path::{Path, PathBuf};

use morph_boot::{BootControl, Slot};
use morph_update::{Layout, Payload, TrustedKeys};

pub use morph_update::Outcome;

/// Where the pieces of this machine live.
#[derive(Debug, Clone)]
pub struct Heal {
    /// The EFI system partition: boot files under `morpheos/<slot>/`, and the report under
    /// `morph-boot/`.
    pub esp: PathBuf,
    /// The boot history the image writes, one JSON object per boot.
    pub history: PathBuf,
    /// Where repairs are recorded, so the same failure is not repaired twice.
    pub report: PathBuf,
    /// Where a successful recovery is recorded, naming both slots as rebuilt.
    pub recovery_record: PathBuf,
    /// Scratch space for assembling the payload. Must not be the read-only root.
    pub stage: PathBuf,
    /// Which release the running system is: the repair reinstalls exactly that one.
    pub release_file: PathBuf,
    /// The recovery image's payload directory on the ESP: its kernel and initrd, which
    /// `recover` stages with the recovery partition as the root image.
    pub recovery_dir: PathBuf,
}

impl Heal {
    /// The conventional layout, given the ESP. Everything else is derived, so a caller
    /// cannot configure the pieces inconsistently with each other.
    pub fn new(esp: impl Into<PathBuf>) -> Heal {
        let esp = esp.into();
        Heal {
            history: esp.join("morph-boot/history.jsonl"),
            report: esp.join("morph-boot/repairs.json"),
            recovery_record: esp.join("morph-boot/recovery.json"),
            stage: PathBuf::from("/run/morph-heal"),
            release_file: PathBuf::from(morph_update::RELEASE_FILE),
            recovery_dir: esp.join("morpheos/recovery"),
            esp,
        }
    }
}

/// A slot that was trialled and did not survive its health check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Failure {
    /// The slot that failed.
    pub slot: Slot,
    /// The slot the machine fell back to — the one that is still good.
    pub good: Slot,
    /// 1-based position of the history entry, which is what makes a repair idempotent.
    pub index: usize,
}

/// What `recover` did: both slots rebuilt from the recovery image, and the new default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Recovered {
    /// The release the recovery image wrote into both slots.
    pub release: u64,
    /// Slot A, rebuilt and verified.
    pub a: Outcome,
    /// Slot B, rebuilt and verified.
    pub b: Outcome,
    /// The slot the firmware boots next — always A.
    pub default: Slot,
}

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    /// The history could not be understood.
    History(String),
    /// There is nothing to repair, or this process is not the one to do it.
    Refused(String),
    /// The boot policy would not answer, or refused: kept separate from `Refused` because
    /// "you asked for the wrong slot" and "the machine will not tell me its state" are
    /// different problems with different fixes.
    Boot(morph_boot::Error),
    /// `morph-update` refused or failed: carried through so the reason survives.
    Update(morph_update::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::History(m) => write!(f, "boot history: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
            Error::Boot(e) => write!(f, "boot policy: {e}"),
            Error::Update(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<morph_update::Error> for Error {
    fn from(e: morph_update::Error) -> Self {
        Error::Update(e)
    }
}

impl From<morph_boot::Error> for Error {
    fn from(e: morph_boot::Error) -> Self {
        Error::Boot(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

/// The newest entry in the boot history that records a failed trial.
///
/// Absent history is not an error: a machine that has never had a trial failure has
/// nothing to repair, and saying so is the correct answer rather than a failure.
pub fn last_failure(history: &Path) -> Result<Option<Failure>> {
    let text = match std::fs::read_to_string(history) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(format!("reading {}", history.display()))(e)),
    };
    let mut found = None;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| Error::History(format!("{}:{}: {e}", history.display(), n + 1)))?;
        let action = entry
            .get("decision")
            .and_then(|d| d.get("action"))
            .and_then(|a| a.as_str())
            .unwrap_or("");
        if action != "fallback" {
            continue;
        }
        // `status.booted` is the slot that was being trialled; `status.default` is the
        // slot the machine went back to. Both come from the system's own record.
        let slot = entry
            .get("status")
            .and_then(|s| s.get("booted"))
            .and_then(|s| s.as_str())
            .and_then(Slot::parse);
        let good = entry
            .get("status")
            .and_then(|s| s.get("default"))
            .and_then(|s| s.as_str())
            .and_then(Slot::parse);
        match (slot, good) {
            (Some(slot), Some(good)) if slot != good => {
                found = Some(Failure {
                    slot,
                    good,
                    index: n + 1,
                });
            }
            // A fallback entry that does not name two different slots is a history this
            // code does not understand, and guessing at it would repair the wrong slot.
            _ => {
                return Err(Error::History(format!(
                    "{}:{}: a fallback that does not name a trialled slot and a different \
                     default; refusing to guess which to repair",
                    history.display(),
                    n + 1
                )));
            }
        }
    }
    Ok(found)
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct Repairs {
    /// History index of the newest failure that has been repaired.
    #[serde(default)]
    repaired_index: usize,
    /// The slot last rebuilt, for the report and for a human reading it.
    #[serde(default)]
    spare: Option<String>,
}

fn read_repairs(path: &Path) -> Result<Repairs> {
    match std::fs::read_to_string(path) {
        Ok(t) => {
            serde_json::from_str(&t).map_err(|e| Error::History(format!("{}: {e}", path.display())))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Repairs::default()),
        Err(e) => Err(io(format!("reading {}", path.display()))(e)),
    }
}

impl Heal {
    /// The failure that still needs repairing, or `None` if there is nothing to do.
    pub fn needed(&self) -> Result<Option<Failure>> {
        let Some(failure) = last_failure(&self.history)? else {
            return Ok(None);
        };
        let done = read_repairs(&self.report)?;
        Ok((failure.index > done.repaired_index).then_some(failure))
    }

    /// Rebuild the failed slot from the running one, verify it, and record the repair.
    ///
    /// `running_root` is the running slot's root — read as an **image**, so the spare
    /// becomes a copy of a system that is known to boot, rather than a copy of the files
    /// inside it. `spare_root` is the failed slot's root, which is written and verified by
    /// `morph-update`.
    ///
    /// The policy still decides: `morph_update::install` asks
    /// [`morph_boot::update_target`] which slot may be written, and this refuses earlier if
    /// the failure names the running slot, so a corrupted history cannot talk it into
    /// overwriting the system it is running on.
    pub fn repair(
        &self,
        bc: &dyn BootControl,
        running: Slot,
        running_root: &Path,
        spare_root: &Path,
    ) -> Result<(Failure, Outcome)> {
        let failure = self.needed()?.ok_or_else(|| {
            Error::Refused("no unrepaired failed trial in the boot history".into())
        })?;
        if failure.slot == running {
            return Err(Error::Refused(format!(
                "the history says slot {} failed, but this system is running from slot {}; \
                 refusing to rebuild the slot in use",
                failure.slot, running
            )));
        }

        // Stage the running system as a payload. The root is **referenced, not copied**:
        // a block device is a file, and `morph-update` hashes and streams whatever path it
        // is given, so a symlink stages the whole system image at no cost in space.
        //
        // ⚠️ THIS WAS A COPY, AND A COPY IS WRONG TWICE OVER. The scratch directory is a
        // tmpfs on a real machine, so copying a 1.2 GiB system image into it asks for
        // 1.2 GiB of RAM — the VM test has 1 GiB of it, which is how this was found — and
        // on a machine with more RAM it is still a pointless full-image write before the
        // write that matters. The image is read twice (hashed, then written) and neither
        // read needs a second copy to exist.
        //
        // Reading the running root is safe precisely because it is mounted read-only; the
        // image cannot change under the read. If that ever stops being true, this is the
        // line that becomes a lie.
        let stage = self.stage.join("payload");
        let _ = std::fs::remove_dir_all(&self.stage);
        std::fs::create_dir_all(&stage).map_err(io(format!("creating {}", stage.display())))?;
        let image = stage.join(morph_update::ROOT_IMAGE);
        std::os::unix::fs::symlink(running_root, &image).map_err(io(format!(
            "referencing the running root {}",
            running_root.display()
        )))?;
        for name in [morph_update::KERNEL, morph_update::INITRD] {
            let from = self.esp.join("morpheos").join(running.as_str()).join(name);
            std::fs::copy(&from, stage.join(name))
                .map_err(io(format!("reading {}", from.display())))?;
        }
        // The spare becomes the running system again, so it is the running release: equal,
        // which `install` allows, never older.
        let release = morph_update::read_release(&self.release_file)?;
        morph_update::seal(&stage, release)?;
        // There is no unsigned way to open a payload (doc 20, S2), and this one needs no
        // vendor's signature: it is the machine's own running system, read from its own
        // read-only root. So it is signed with a key made for this one repair and opened
        // trusting only that key, which proves the manifest is the one written just above.
        let (secret, public) = morph_update::keygen()?;
        morph_update::sign(&stage, &secret)?;
        let trusted = TrustedKeys::from_public_hex("this repair's own key", &public)?;
        let mut payload = Payload::open(&stage, &trusted)?;
        let layout = Layout {
            esp: self.esp.clone(),
            root_device: spare_root.to_path_buf(),
            running_release: release,
        };
        let outcome = morph_update::install(bc, &mut payload, &layout, Some(failure.slot))?;

        // Recorded only AFTER the write verified, so an interrupted repair is retried
        // rather than remembered as done.
        let repairs = Repairs {
            repaired_index: failure.index,
            spare: Some(failure.slot.to_string()),
        };
        let report = self.report.clone();
        if let Some(dir) = report.parent() {
            std::fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
        }
        std::fs::write(
            &report,
            serde_json::to_string_pretty(&repairs).unwrap_or_default(),
        )
        .map_err(io(format!("writing {}", report.display())))?;

        // The scratch copy is a full system image; it has no business outliving the repair.
        let _ = std::fs::remove_dir_all(&self.stage);
        Ok((failure, outcome))
    }

    /// Rebuild both slots from the recovery image, make slot A the default, and record it.
    ///
    /// This is the rung below the A/B pair (S4, H4): when a slot's root is lost and neither
    /// slot can boot, the machine starts the recovery system and runs this. It is the same
    /// verified write as an update, but with the recovery partition as the source and both
    /// slots as targets — exactly what the boot policy would never allow — so it writes
    /// through `morph_update::write_slot` (no policy), never `install`.
    ///
    /// Two independent checks are made before anything is written:
    ///
    /// 1. **We are the recovery system.** `morph.recovery=1` on the command line **and** `/`
    ///    mounted from the recovery partition ([`morph_boot::is_recovery`]). Either alone is
    ///    forgeable; the pair is what stops a slot that merely resembles recovery from
    ///    overwriting the very slot it is running from.
    /// 2. **The slot roots are not the recovery partition itself**, so the source is not
    ///    overwritten by one of its own targets.
    ///
    /// The payload is sealed and signed for this one run, exactly as [`repair`] does — there
    /// is no way to open an unsigned payload, and this one is the machine's own running image,
    /// read from its own recovery partition. (Checking the recovery image against its
    /// factory-signed manifest is the corrupt-recovery refusal, a later rung.)
    ///
    /// "Slot A is the factory image" is a policy property — the recovery image is built as
    /// slot A's tree — not a proof that A boots on this machine. It is chosen because it is
    /// the slot the factory image ships as the committed default, and it is said plainly
    /// here rather than dressed up as a guarantee.
    pub fn recover(
        &self,
        bc: &mut dyn BootControl,
        cmdline: &str,
        mountinfo: &str,
        recovery_root: &Path,
        root_a: &Path,
        root_b: &Path,
    ) -> Result<Recovered> {
        if !morph_boot::is_recovery(cmdline, mountinfo) {
            return Err(Error::Refused(
                "recover runs only from the recovery system: the command line must carry \
                 morph.recovery=1 and / must be the recovery partition"
                    .into(),
            ));
        }
        if root_a == recovery_root || root_b == recovery_root {
            return Err(Error::Refused(format!(
                "refusing to write the recovery partition {} as a slot root",
                recovery_root.display()
            )));
        }

        // Stage the recovery image as a payload: the recovery partition is the root image,
        // **referenced, not copied** (it is a block device; a copy would need a whole slot's
        // worth of scratch, the same reason `repair` symlinks). The kernel and initrd come
        // from the ESP, beside the recovery entry.
        let stage = self.stage.join("recovery");
        let _ = std::fs::remove_dir_all(&self.stage);
        std::fs::create_dir_all(&stage).map_err(io(format!("creating {}", stage.display())))?;
        let image = stage.join(morph_update::ROOT_IMAGE);
        std::os::unix::fs::symlink(recovery_root, &image).map_err(io(format!(
            "referencing the recovery partition {}",
            recovery_root.display()
        )))?;
        for name in [morph_update::KERNEL, morph_update::INITRD] {
            let from = self.recovery_dir.join(name);
            std::fs::copy(&from, stage.join(name))
                .map_err(io(format!("reading {}", from.display())))?;
        }
        // The release the recovery image says it is, read from the recovery system itself;
        // both slots are rebuilt at that release.
        let release = morph_update::read_release(&self.release_file)?;
        morph_update::seal(&stage, release)?;
        // There is no unsigned way to open a payload (doc 20, S2), and this one needs no
        // vendor's signature: it is the machine's own running recovery image, read from its
        // own recovery partition. So it is signed with a key made for this one recovery and
        // opened trusting only that key.
        let (secret, public) = morph_update::keygen()?;
        morph_update::sign(&stage, &secret)?;
        let trusted = TrustedKeys::from_public_hex("this recovery's own key", &public)?;
        let mut payload = Payload::open(&stage, &trusted)?;

        // Write both slots, A then B. `write_slot` has no policy by design; the recovery-only
        // guard above is what makes writing the running system impossible here.
        let outcome_a = morph_update::write_slot(
            &mut payload,
            &Layout {
                esp: self.esp.clone(),
                root_device: root_a.to_path_buf(),
                running_release: release,
            },
            Slot::A,
        )?;
        let outcome_b = morph_update::write_slot(
            &mut payload,
            &Layout {
                esp: self.esp.clone(),
                root_device: root_b.to_path_buf(),
                running_release: release,
            },
            Slot::B,
        )?;

        // Slot A is the factory image, so it is the default the machine returns to.
        bc.set_default_slot(Slot::A)?;

        // The recovery record names both slots, and is written only after both verified.
        let record = self.recovery_record.clone();
        if let Some(dir) = record.parent() {
            std::fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
        }
        let text = serde_json::json!({
            "action": "recovered",
            "from": "recovery",
            "release": release,
            "slots": ["a", "b"],
            "default": "a",
        });
        std::fs::write(
            &record,
            serde_json::to_string_pretty(&text).unwrap_or_default(),
        )
        .map_err(io(format!("writing {}", record.display())))?;

        // The scratch image is a full system image; it has no business outliving the recovery.
        let _ = std::fs::remove_dir_all(&self.stage);
        Ok(Recovered {
            release,
            a: outcome_a,
            b: outcome_b,
            default: Slot::A,
        })
    }
}
