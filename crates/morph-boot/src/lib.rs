//! MorpheOS A/B boot control.
//!
//! Every platform is reduced to the same two facts:
//!
//! - a **default slot**, stored persistently, which is always a system that
//!   has passed its health check;
//! - a **one-shot trial**, held by the firmware, which boots the other slot
//!   exactly once and is cleared as soon as it is used.
//!
//! An update is written to the slot that isn't running, booted once as a
//! trial, and only becomes the default after it proves healthy. If the trial
//! crashes, hangs (the watchdog resets it) or fails its health check, the
//! next boot is the default slot again: the firmware already forgot the trial.
//!
//! Backends: [`pi`] (Raspberry Pi 4/5 `autoboot.txt` + `tryboot`) and
//! [`sdboot`] (systemd-boot `loader.conf` default + `LoaderEntryOneShot`).

pub mod pi;
pub mod sdboot;
pub mod sim;

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    A,
    B,
}

impl Slot {
    pub fn other(self) -> Slot {
        match self {
            Slot::A => Slot::B,
            Slot::B => Slot::A,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Slot::A => "a",
            Slot::B => "b",
        }
    }

    pub fn parse(s: &str) -> Option<Slot> {
        match s.trim().to_ascii_lowercase().as_str() {
            "a" => Some(Slot::A),
            "b" => Some(Slot::B),
            _ => None,
        }
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    Corrupt(String),
    Refused(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::Corrupt(m) => write!(f, "boot state is not readable: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

/// What a platform has to provide. Everything above this trait is shared.
pub trait BootControl {
    /// The slot the firmware boots when no trial is pending.
    fn default_slot(&self) -> Result<Slot>;
    /// Make `slot` the default. Only called for a slot that is running and healthy.
    fn set_default_slot(&mut self, slot: Slot) -> Result<()>;
    /// The slot this boot came from.
    fn booted_slot(&self) -> Result<Slot>;
    /// Arm a one-shot boot of `slot` and return the command that reboots into it.
    fn arm_trial(&mut self, slot: Slot) -> Result<Vec<String>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Running the default slot.
    Committed,
    /// Running a one-shot trial of the non-default slot.
    Trial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    pub booted: Slot,
    pub default: Slot,
    pub phase: Phase,
}

pub fn status(bc: &dyn BootControl) -> Result<Status> {
    let booted = bc.booted_slot()?;
    let default = bc.default_slot()?;
    let phase = if booted == default {
        Phase::Committed
    } else {
        Phase::Trial
    };
    Ok(Status {
        booted,
        default,
        phase,
    })
}

/// The slot an update may be written to: whichever one isn't running.
///
/// Refused during a trial: the running system hasn't proven itself yet, and
/// the other slot is the only known-good system, so it must not be touched.
pub fn update_target(bc: &dyn BootControl) -> Result<Slot> {
    let s = status(bc)?;
    if s.phase == Phase::Trial {
        return Err(Error::Refused(format!(
            "running an unconfirmed trial of slot {}; confirm or reboot before updating slot {}",
            s.booted, s.default
        )));
    }
    Ok(s.booted.other())
}

/// Whether this boot is the recovery system, not a slot (S4, rung H4).
///
/// Recovery is the third, immutable system that rebuilds both slots when their roots are
/// lost. Anything that runs there — `morph-heal recover` — must be certain it *is* there,
/// because the one mistake that cannot be undone is overwriting a slot that is running.
/// Three facts must agree:
///
/// - `morph.recovery=1` on the kernel command line — the recovery boot entry's marker;
/// - `root=PARTLABEL=recovery` on the same line — the recovery entry is the only one whose
///   root is the recovery partition (the slots say `root-a`/`root-b`); and
/// - `/` actually mounted from a block device, read from `/proc/self/mountinfo`.
///
/// The middle fact is what names the partition: `/proc/self/mountinfo` records the
/// *resolved* device (e.g. `/dev/vda5`), not the label the boot entry used, so the mount
/// source alone cannot say "recovery". But `root=PARTLABEL=recovery` is what the kernel
/// mounted — if `/` is mounted at all, it is mounted from that partition. The marker and
/// the mounted root together separate "I am the recovery system" from "a slot that
/// resembles one".
pub fn is_recovery(cmdline: &str, mountinfo: &str) -> bool {
    cmdline_has_recovery(cmdline) && cmdline_roots_recovery(cmdline) && root_is_mounted(mountinfo)
}

fn cmdline_has_recovery(cmdline: &str) -> bool {
    cmdline
        .split_whitespace()
        .find_map(|w| w.strip_prefix("morph.recovery="))
        == Some("1")
}

/// The recovery boot entry is the only one whose `root=` is `PARTLABEL=recovery`.
fn cmdline_roots_recovery(cmdline: &str) -> bool {
    cmdline
        .split_whitespace()
        .any(|w| w == "root=PARTLABEL=recovery")
}

/// The mount source of `/` (the field after the `- <fstype>` pair), if `/` is mounted.
fn root_mount_source(mountinfo: &str) -> Option<&str> {
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // mountinfo: id parent major:minor root mountpoint options [optional..] - fstype source superopts
        if fields.get(4) != Some(&"/") {
            continue;
        }
        let dash = fields.iter().position(|f| *f == "-")?;
        return fields.get(dash + 2).copied();
    }
    None
}

/// `/` is mounted from a real block device, not still in the initramfs or on a tmpfs/overlay.
fn root_is_mounted(mountinfo: &str) -> bool {
    root_mount_source(mountinfo).is_some_and(|src| src.starts_with("/dev/"))
}

/// Where this machine's boot control lives, as the command line describes it.
///
/// This exists so that every tool that needs the boot policy — `morph-boot` itself
/// and `morph-update` — reaches it through **one** construction path. Two copies of
/// "which slot is safe to write" would be two chances to disagree, and the one that
/// disagreed would be the one writing to the running system.
#[derive(Debug, Clone)]
pub struct BackendOptions {
    /// `pi`, `sdboot`, or `None` to detect.
    pub backend: Option<String>,
    /// pi: `autoboot.txt`.
    pub autoboot: PathBuf,
    /// pi: the bootloader device-tree node.
    pub dt: PathBuf,
    /// sdboot: the EFI system partition.
    pub esp: PathBuf,
    /// sdboot: the kernel command line to read the running slot from.
    pub cmdline: PathBuf,
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions {
            backend: None,
            autoboot: "/bootfs/autoboot.txt".into(),
            dt: "/proc/device-tree/chosen/bootloader".into(),
            esp: "/efi".into(),
            cmdline: "/proc/cmdline".into(),
        }
    }
}

/// Build the boot control for this machine.
///
/// Detection is by the device-tree node the Pi firmware publishes; anything else is
/// treated as systemd-boot.
pub fn backend(o: &BackendOptions) -> Box<dyn BootControl> {
    let kind = o.backend.clone().unwrap_or_else(|| {
        if Path::new(&o.dt).join("partition").exists() {
            "pi".into()
        } else {
            "sdboot".into()
        }
    });
    if kind == "pi" {
        Box::new(pi::Pi::new(&o.autoboot, &o.dt))
    } else {
        let mut s = sdboot::SdBoot::new(&o.esp);
        s.cmdline = o.cmdline.clone();
        Box::new(s)
    }
}

/// Arm a one-shot trial of a freshly written slot. Returns the reboot command.
pub fn start_trial(bc: &mut dyn BootControl, slot: Slot) -> Result<Vec<String>> {
    let target = update_target(bc)?;
    if slot != target {
        return Err(Error::Refused(format!(
            "slot {slot} is the running system; only slot {target} can be trialled"
        )));
    }
    bc.arm_trial(slot)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Decision {
    /// Nothing to do: this boot was the default slot.
    None,
    /// The trial passed and is now the default.
    Committed { slot: Slot },
    /// The trial failed: reboot, and the firmware returns to the default slot.
    Fallback { to: Slot, reboot: Vec<String> },
}

/// Called once per boot, after the health check, with its result.
pub fn confirm(bc: &mut dyn BootControl, healthy: bool) -> Result<Decision> {
    let s = status(bc)?;
    match (s.phase, healthy) {
        (Phase::Committed, _) => Ok(Decision::None),
        (Phase::Trial, true) => {
            bc.set_default_slot(s.booted)?;
            Ok(Decision::Committed { slot: s.booted })
        }
        (Phase::Trial, false) => Ok(Decision::Fallback {
            to: s.default,
            reboot: vec!["systemctl".into(), "reboot".into()],
        }),
    }
}

/// Write `contents` to `path` as atomically as the filesystem allows:
/// write a sibling temp file, flush it to disk, then rename over the target.
pub(crate) fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("morph-tmp");
    let what = format!("writing {}", path.display());
    let mut f = std::fs::File::create(&tmp).map_err(io(what.clone()))?;
    f.write_all(contents.as_bytes()).map_err(io(what.clone()))?;
    f.sync_all().map_err(io(what.clone()))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(io(what.clone()))?;
    if let Some(dir) = path.parent()
        && let Ok(d) = std::fs::File::open(dir)
    {
        // Best effort: FAT (the Pi boot partition) may not support syncing a directory.
        let _ = d.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECOVERY_CMDLINE: &str =
        "root=PARTLABEL=recovery ro morph.recovery=1 systemd.unit=morph-recovery.target panic=5";
    const SLOT_A_CMDLINE: &str = "root=PARTLABEL=root-a ro morph.slot=a panic=5";

    fn mountinfo(root_source: &str) -> String {
        format!("1 1 8:5 / / rw,relatime - ext4 {root_source} rw\n")
    }

    #[test]
    fn recovery_requires_the_cmdline_and_the_mounted_root() {
        assert!(is_recovery(
            RECOVERY_CMDLINE,
            &mountinfo("/dev/disk/by-partlabel/recovery")
        ));
        // A slot boot, however recovery-marked, is not recovery: its root is a slot.
        assert!(!is_recovery(
            "root=PARTLABEL=root-a ro morph.recovery=1 panic=5",
            &mountinfo("/dev/disk/by-partlabel/root-a")
        ));
        // The recovery root without the marker is not recovery either.
        assert!(!is_recovery(
            SLOT_A_CMDLINE,
            &mountinfo("/dev/disk/by-partlabel/recovery")
        ));
    }

    #[test]
    fn the_resolved_device_is_still_recovery() {
        // /proc/self/mountinfo records the RESOLVED device (e.g. /dev/vda5), not the
        // PARTLABEL the boot entry used. The cmdline's root=PARTLABEL=recovery is what
        // names it recovery, so the resolved name must not break the check.
        assert!(is_recovery(RECOVERY_CMDLINE, &mountinfo("/dev/vda5")));
        assert!(is_recovery(RECOVERY_CMDLINE, &mountinfo("/dev/sda5")));
    }

    #[test]
    fn a_claim_on_the_cmdline_that_does_not_say_1_is_not_recovery() {
        for cmdline in [
            "root=PARTLABEL=recovery morph.recovery=0",
            "root=PARTLABEL=recovery morph.recovery=yes",
            "root=PARTLABEL=recovery morph.slot=a",
            "",
        ] {
            assert!(
                !is_recovery(cmdline, &mountinfo("/dev/disk/by-partlabel/recovery")),
                "cmdline {cmdline:?} must not read as recovery"
            );
        }
    }

    #[test]
    fn the_root_must_actually_be_mounted() {
        // A mountinfo with no / mount at all cannot prove anything.
        assert!(!is_recovery(
            RECOVERY_CMDLINE,
            "1 1 8:1 /efi /efi rw - vfat /dev/sda1 rw\n"
        ));
        // / mounted on a tmpfs (still in the initramfs) is not the recovery partition.
        assert!(!is_recovery(
            RECOVERY_CMDLINE,
            "1 1 0:1 / / rw - tmpfs tmpfs rw\n"
        ));
    }

    #[test]
    fn mountinfo_optional_fields_do_not_shift_the_source() {
        // `master:1` and `propagate_from` are optional fields before the "-".
        let mi = "1 1 8:5 / / rw,relatime master:1 propagate_from:2 - ext4 /dev/disk/by-partlabel/recovery rw\n";
        assert!(is_recovery(RECOVERY_CMDLINE, mi));
    }
}
