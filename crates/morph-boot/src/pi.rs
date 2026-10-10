//! Raspberry Pi 4/5: `autoboot.txt` + `tryboot`.
//!
//! This follows the A/B flow in the official Raspberry Pi documentation
//! (`documentation/asciidoc/computers/config_txt/autoboot.adoc`):
//!
//! - partition 1 is a small FAT partition holding only `autoboot.txt`;
//! - partitions 2 and 3 are the boot partitions of slots A and B, each with
//!   its own `cmdline.txt` pointing at its own root;
//! - `autoboot.txt` names the default partition under `[all]`, and the trial
//!   partition under `[tryboot]`, with `tryboot_a_b=1` so the normal
//!   `config.txt` is used in both;
//! - `reboot "0 tryboot"` boots the `[tryboot]` partition once; the firmware
//!   clears the flag after use, so any later reset (crash, watchdog, power)
//!   boots the default partition again;
//! - committing swaps the two partition numbers in `autoboot.txt`.
//!
//! The running partition is read from `/proc/device-tree/chosen/bootloader/partition`,
//! a raw 32-bit big-endian integer.

use std::path::{Path, PathBuf};

use crate::{BootControl, Error, Result, Slot, io, write_atomic};

/// Boot partition number of each slot (partition 1 is the boot selector).
pub const SLOT_A_PARTITION: u32 = 2;
pub const SLOT_B_PARTITION: u32 = 3;

pub fn partition_of(slot: Slot) -> u32 {
    match slot {
        Slot::A => SLOT_A_PARTITION,
        Slot::B => SLOT_B_PARTITION,
    }
}

pub fn slot_of(partition: u32) -> Option<Slot> {
    match partition {
        SLOT_A_PARTITION => Some(Slot::A),
        SLOT_B_PARTITION => Some(Slot::B),
        _ => None,
    }
}

/// The parts of `autoboot.txt` MorpheOS controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Autoboot {
    pub default: Slot,
}

impl Autoboot {
    /// Render the file exactly in the documented A/B form. The trial
    /// partition is always the other slot.
    pub fn render(&self) -> String {
        format!(
            "# Managed by morph-boot. Do not edit: see docs/15-self-healing.md\n\
             [all]\n\
             tryboot_a_b=1\n\
             boot_partition={}\n\
             [tryboot]\n\
             boot_partition={}\n",
            partition_of(self.default),
            partition_of(self.default.other()),
        )
    }

    pub fn parse(text: &str) -> Result<Autoboot> {
        let mut section = "all".to_string();
        let mut default = None;
        let mut trial = None;
        let mut tryboot_a_b = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match (section.as_str(), key) {
                ("all", "boot_partition") => default = Some(parse_partition(value)?),
                ("tryboot", "boot_partition") => trial = Some(parse_partition(value)?),
                ("all", "tryboot_a_b") => tryboot_a_b = value == "1",
                _ => {}
            }
        }
        let default = default.ok_or_else(|| corrupt("no [all] boot_partition"))?;
        let trial = trial.ok_or_else(|| corrupt("no [tryboot] boot_partition"))?;
        if !tryboot_a_b {
            return Err(corrupt("tryboot_a_b=1 is missing"));
        }
        if trial != default.other() {
            return Err(corrupt(
                "default and tryboot partitions must be the two different slots",
            ));
        }
        Ok(Autoboot { default })
    }
}

fn parse_partition(value: &str) -> Result<Slot> {
    let n: u32 = value
        .parse()
        .map_err(|_| corrupt(&format!("boot_partition '{value}' is not a number")))?;
    slot_of(n).ok_or_else(|| corrupt(&format!("boot_partition {n} is not a MorpheOS slot")))
}

fn corrupt(m: &str) -> Error {
    Error::Corrupt(format!("autoboot.txt: {m}"))
}

pub struct Pi {
    /// Path to `autoboot.txt` on the mounted partition 1.
    pub autoboot: PathBuf,
    /// Path to `/proc/device-tree/chosen/bootloader`.
    pub bootloader_dt: PathBuf,
}

impl Pi {
    pub fn new(autoboot: impl Into<PathBuf>, bootloader_dt: impl Into<PathBuf>) -> Self {
        Pi {
            autoboot: autoboot.into(),
            bootloader_dt: bootloader_dt.into(),
        }
    }

    fn read_autoboot(&self) -> Result<Autoboot> {
        let text = std::fs::read_to_string(&self.autoboot)
            .map_err(io(format!("reading {}", self.autoboot.display())))?;
        Autoboot::parse(&text)
    }

    /// `true` when the firmware's tryboot flag was set for this boot.
    pub fn tryboot_flag(&self) -> Result<bool> {
        Ok(read_dt_u32(&self.bootloader_dt.join("tryboot"))? == 1)
    }
}

/// Read a device-tree property holding one big-endian u32.
pub fn read_dt_u32(path: &Path) -> Result<u32> {
    let bytes = std::fs::read(path).map_err(io(format!("reading {}", path.display())))?;
    let arr: [u8; 4] = bytes.as_slice().try_into().map_err(|_| {
        Error::Corrupt(format!(
            "{} holds {} bytes, expected 4",
            path.display(),
            bytes.len()
        ))
    })?;
    Ok(u32::from_be_bytes(arr))
}

impl BootControl for Pi {
    fn default_slot(&self) -> Result<Slot> {
        Ok(self.read_autoboot()?.default)
    }

    fn set_default_slot(&mut self, slot: Slot) -> Result<()> {
        write_atomic(&self.autoboot, &Autoboot { default: slot }.render())
    }

    fn booted_slot(&self) -> Result<Slot> {
        let n = read_dt_u32(&self.bootloader_dt.join("partition"))?;
        slot_of(n).ok_or_else(|| {
            Error::Corrupt(format!(
                "booted from partition {n}, which is not a MorpheOS slot"
            ))
        })
    }

    fn arm_trial(&mut self, slot: Slot) -> Result<Vec<String>> {
        // autoboot.txt always names the non-default slot under [tryboot], so
        // arming is just the reboot flag. Re-render to repair a hand-edited file.
        let current = self.read_autoboot()?;
        if current.default.other() != slot {
            return Err(Error::Refused(format!(
                "slot {slot} is the default; trials are only for the other slot"
            )));
        }
        write_atomic(&self.autoboot, &current.render())?;
        Ok(vec![
            "systemctl".into(),
            "reboot".into(),
            "0 tryboot".into(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_matches_the_documented_initial_file() {
        let text = Autoboot { default: Slot::A }.render();
        let body: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(
            body,
            [
                "[all]",
                "tryboot_a_b=1",
                "boot_partition=2",
                "[tryboot]",
                "boot_partition=3"
            ]
        );
    }

    #[test]
    fn render_matches_the_documented_committed_file() {
        let text = Autoboot { default: Slot::B }.render();
        assert!(
            text.contains("[all]\ntryboot_a_b=1\nboot_partition=3\n[tryboot]\nboot_partition=2\n")
        );
    }

    #[test]
    fn parse_roundtrips_and_tolerates_comments_and_spacing() {
        for slot in [Slot::A, Slot::B] {
            let a = Autoboot { default: slot };
            assert_eq!(Autoboot::parse(&a.render()).unwrap(), a);
        }
        let spaced = "[all]\n tryboot_a_b = 1 # comment\nboot_partition = 3\n\n[tryboot]\nboot_partition=2\n";
        assert_eq!(Autoboot::parse(spaced).unwrap().default, Slot::B);
    }

    #[test]
    fn parse_rejects_unsafe_files() {
        let no_ab = "[all]\nboot_partition=2\n[tryboot]\nboot_partition=3\n";
        assert!(Autoboot::parse(no_ab).is_err());
        let same = "[all]\ntryboot_a_b=1\nboot_partition=2\n[tryboot]\nboot_partition=2\n";
        assert!(Autoboot::parse(same).is_err());
        let foreign = "[all]\ntryboot_a_b=1\nboot_partition=5\n[tryboot]\nboot_partition=3\n";
        assert!(Autoboot::parse(foreign).is_err());
        assert!(Autoboot::parse("").is_err());
    }
}
