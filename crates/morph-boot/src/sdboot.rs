//! UEFI PCs and VMs: systemd-boot.
//!
//! - Each slot has one Boot Loader Specification entry on the ESP,
//!   `loader/entries/morpheos-a.conf` and `morpheos-b.conf`, whose kernel
//!   command line carries `morph.slot=a|b`.
//! - The default slot is the `default` line of `loader/loader.conf`, a plain
//!   file on the ESP.
//! - A trial is `bootctl set-oneshot <entry>`, which sets the
//!   `LoaderEntryOneShot` EFI variable. systemd-boot clears it when it uses
//!   it, so any later reset boots the default again.
//! - The running slot is read from `morph.slot=` in `/proc/cmdline`.
//!
//! MorpheOS never sets `LoaderEntryDefault` (the EFI variable that would
//! override `loader.conf`), so the file on disk is always the truth.

use std::path::PathBuf;

use crate::{BootControl, Error, Result, Slot, io, write_atomic};

pub fn entry_id(slot: Slot) -> String {
    format!("morpheos-{slot}.conf")
}

pub struct SdBoot {
    /// Mounted EFI system partition (contains `loader/`).
    pub esp: PathBuf,
    /// Usually `/proc/cmdline`.
    pub cmdline: PathBuf,
    /// The `bootctl` binary.
    pub bootctl: PathBuf,
}

impl SdBoot {
    pub fn new(esp: impl Into<PathBuf>) -> Self {
        SdBoot {
            esp: esp.into(),
            cmdline: "/proc/cmdline".into(),
            bootctl: "bootctl".into(),
        }
    }

    fn loader_conf(&self) -> PathBuf {
        self.esp.join("loader/loader.conf")
    }
}

/// Replace (or add) the `default` line, keeping every other line as it is.
pub fn with_default(loader_conf: &str, slot: Slot) -> String {
    let wanted = format!("default {}", entry_id(slot));
    let mut out = Vec::new();
    let mut replaced = false;
    for line in loader_conf.lines() {
        if line.split_whitespace().next() == Some("default") {
            if !replaced {
                out.push(wanted.clone());
                replaced = true;
            }
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        out.insert(0, wanted);
    }
    out.join("\n") + "\n"
}

pub fn parse_default(loader_conf: &str) -> Result<Slot> {
    let value = loader_conf
        .lines()
        .find_map(|l| {
            let mut words = l.split_whitespace();
            (words.next() == Some("default")).then(|| words.next().unwrap_or(""))
        })
        .ok_or_else(|| Error::Corrupt("loader.conf has no default line".into()))?;
    [Slot::A, Slot::B]
        .into_iter()
        .find(|s| entry_id(*s) == value)
        .ok_or_else(|| {
            Error::Corrupt(format!(
                "loader.conf default '{value}' is not a MorpheOS slot"
            ))
        })
}

pub fn parse_cmdline_slot(cmdline: &str) -> Result<Slot> {
    cmdline
        .split_whitespace()
        .find_map(|w| w.strip_prefix("morph.slot="))
        .and_then(Slot::parse)
        .ok_or_else(|| Error::Corrupt("kernel command line has no valid morph.slot=".into()))
}

impl BootControl for SdBoot {
    fn default_slot(&self) -> Result<Slot> {
        let path = self.loader_conf();
        let text =
            std::fs::read_to_string(&path).map_err(io(format!("reading {}", path.display())))?;
        match parse_default(&text) {
            Ok(slot) => Ok(slot),
            // A default that is not `morpheos-a.conf`/`morpheos-b.conf` is a CUSTOM ENTRY, not a broken
            // table: our own Phase 0 kernel boots from `morpheos-our.conf`, and a person may point the
            // loader at a kernel of their own. The machine still knows which slot it is running — the
            // kernel command line carries `morph.slot=` — so answer from that instead of refusing.
            //
            // This was found by booting our kernel in a VM: morph-boot reported
            // "boot state is not readable: loader.conf default 'morpheos-our.conf' is not a MorpheOS slot"
            // on a machine that was perfectly healthy, which would have made every rollback decision
            // guess.
            Err(Error::Corrupt(_)) => self.booted_slot().map_err(|_| {
                Error::Corrupt(format!(
                    "loader.conf default is not a MorpheOS slot and the command line names no slot: {}",
                    text.trim()
                ))
            }),
            Err(e) => Err(e),
        }
    }

    fn set_default_slot(&mut self, slot: Slot) -> Result<()> {
        let path = self.loader_conf();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        write_atomic(&path, &with_default(&text, slot))
    }

    fn booted_slot(&self) -> Result<Slot> {
        let text = std::fs::read_to_string(&self.cmdline)
            .map_err(io(format!("reading {}", self.cmdline.display())))?;
        parse_cmdline_slot(&text)
    }

    fn arm_trial(&mut self, slot: Slot) -> Result<Vec<String>> {
        let entry = self.esp.join("loader/entries").join(entry_id(slot));
        if !entry.exists() {
            return Err(Error::Refused(format!(
                "{} does not exist",
                entry.display()
            )));
        }
        let out = std::process::Command::new(&self.bootctl)
            .arg(format!("--esp-path={}", self.esp.display()))
            .arg("set-oneshot")
            .arg(entry_id(slot))
            .output()
            .map_err(io(format!("running {}", self.bootctl.display())))?;
        if !out.status.success() {
            return Err(Error::Refused(format!(
                "bootctl set-oneshot failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(vec!["systemctl".into(), "reboot".into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_line_is_replaced_in_place() {
        let conf = "timeout 3\ndefault morpheos-a.conf\nconsole-mode keep\n";
        assert_eq!(
            with_default(conf, Slot::B),
            "timeout 3\ndefault morpheos-b.conf\nconsole-mode keep\n"
        );
        assert_eq!(
            parse_default(&with_default(conf, Slot::B)).unwrap(),
            Slot::B
        );
    }

    #[test]
    fn default_line_is_added_when_missing() {
        assert_eq!(
            with_default("timeout 0\n", Slot::A),
            "default morpheos-a.conf\ntimeout 0\n"
        );
        assert_eq!(with_default("", Slot::A), "default morpheos-a.conf\n");
    }

    #[test]
    fn foreign_default_is_corrupt() {
        assert!(parse_default("default ubuntu.conf\n").is_err());
        assert!(parse_default("timeout 3\n").is_err());
    }

    #[test]
    fn cmdline_slot() {
        let c = "root=PARTLABEL=root-b ro morph.slot=b quiet\n";
        assert_eq!(parse_cmdline_slot(c).unwrap(), Slot::B);
        assert!(parse_cmdline_slot("root=/dev/sda2 quiet").is_err());
        assert!(parse_cmdline_slot("morph.slot=c").is_err());
    }
}


#[cfg(test)]
mod custom_entry_tests {
    use super::*;

    /// A loader default that names a kernel of our own is not a broken table. Found by booting the Phase 0
    /// kernel in a VM: morph-boot refused a healthy machine because `morpheos-our.conf` is not a slot entry.
    #[test]
    fn a_custom_default_is_read_as_the_running_slot() {
        let esp = std::env::temp_dir().join("morph-boot-custom-entry");
        let _ = std::fs::remove_dir_all(&esp);
        std::fs::create_dir_all(esp.join("loader")).unwrap();
        std::fs::write(
            esp.join("loader/loader.conf"),
            "default morpheos-our.conf\ntimeout 0\n",
        )
        .unwrap();
        let cmdline = esp.join("cmdline");
        std::fs::write(&cmdline, "root=PARTLABEL=root-b ro morph.slot=b").unwrap();

        let backend = SdBoot {
            esp: esp.clone(),
            cmdline,
            bootctl: std::path::PathBuf::from("bootctl"),
        };
        assert_eq!(backend.default_slot().unwrap(), Slot::B, "the command line is the truth");
        let _ = std::fs::remove_dir_all(&esp);
    }

    /// And with no slot on the command line there is nothing true to say, so it must still refuse rather
    /// than invent a slot.
    #[test]
    fn a_custom_default_with_no_slot_on_the_cmdline_is_still_an_error() {
        let esp = std::env::temp_dir().join("morph-boot-custom-entry-noslot");
        let _ = std::fs::remove_dir_all(&esp);
        std::fs::create_dir_all(esp.join("loader")).unwrap();
        std::fs::write(esp.join("loader/loader.conf"), "default morpheos-our.conf\n").unwrap();
        let cmdline = esp.join("cmdline");
        std::fs::write(&cmdline, "root=PARTLABEL=root-a ro").unwrap();

        let backend = SdBoot {
            esp: esp.clone(),
            cmdline,
            bootctl: std::path::PathBuf::from("bootctl"),
        };
        assert!(backend.default_slot().is_err(), "no cmdline slot means no answer");
        let _ = std::fs::remove_dir_all(&esp);
    }
}
