//! Recovery, proven with real files and no root.
//!
//! The recovery partition, the ESP payload directory and the two slot roots are ordinary
//! files here and block devices in production; the code path is the same "stage the
//! recovery image as a payload, seal and sign it for this one run, write it into both
//! slots, read it back and verify". Every refusal is asserted as hard as a success, and
//! each one also asserts what it protected — the two slot roots are the things a recovery
//! must never touch unless it is the real recovery system.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use morph_boot::{BootControl, Slot};
use morph_heal::{Error, Heal};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("morph-recover-{}-{name}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

const RECOVERY_IMAGE: &[u8] = b"the factory recovery system image, which both slots must become";
const KERNEL: &[u8] = b"recovery kernel";
const INITRD: &[u8] = b"recovery initramfs";
const GARBAGE: &[u8] = &[0x5A; 256];
const RELEASE: u64 = 3;

const RECOVERY_CMDLINE: &str =
    "root=PARTLABEL=recovery ro morph.recovery=1 systemd.unit=morph-recovery.target panic=5";
const SLOT_CMDLINE: &str = "root=PARTLABEL=root-a ro morph.slot=a panic=5";

fn mountinfo() -> String {
    "1 1 8:5 / / rw,relatime - ext4 /dev/disk/by-partlabel/recovery rw\n".to_string()
}

struct Machine {
    heal: Heal,
    recovery_root: PathBuf,
    root_a: PathBuf,
    root_b: PathBuf,
}

impl Machine {
    /// A machine whose ESP carries the recovery kernel and initrd and whose two slots hold
    /// garbage, the way a disk whose slots were wiped would.
    fn new() -> Machine {
        let base = scratch("machine");
        let esp = base.join("esp");
        let recovery_dir = esp.join("morpheos/recovery");
        std::fs::create_dir_all(esp.join("morph-boot")).unwrap();
        std::fs::create_dir_all(&recovery_dir).unwrap();

        let recovery_root = base.join("recovery.img");
        std::fs::write(&recovery_root, RECOVERY_IMAGE).unwrap();
        let root_a = base.join("root-a.img");
        let root_b = base.join("root-b.img");
        std::fs::write(&root_a, GARBAGE).unwrap();
        std::fs::write(&root_b, GARBAGE).unwrap();

        // The ESP carries the recovery kernel and initrd beside the recovery entry; recover
        // stages them with the recovery partition as the root image.
        std::fs::write(recovery_dir.join("vmlinuz"), KERNEL).unwrap();
        std::fs::write(recovery_dir.join("initrd.img"), INITRD).unwrap();

        // The factory manifest (S4 self-check): at image build time the recovery payload is
        // sealed and signed with the factory key, and BLAKE3SUMS + .sig ride the ESP beside
        // the recovery kernel. The factory public key is what the recovery system trusts.
        let (secret, public) = morph_update::keygen().unwrap();
        let recovery_keys = base.join("recovery-keys");
        std::fs::create_dir_all(&recovery_keys).unwrap();
        std::fs::write(recovery_keys.join("build.pub"), format!("{public}\n")).unwrap();

        let payload = base.join("recovery-payload");
        std::fs::create_dir_all(&payload).unwrap();
        std::fs::copy(&recovery_root, payload.join("root.img")).unwrap();
        std::fs::write(payload.join("vmlinuz"), KERNEL).unwrap();
        std::fs::write(payload.join("initrd.img"), INITRD).unwrap();
        morph_update::seal(&payload, RELEASE).unwrap();
        morph_update::sign(&payload, &secret).unwrap();
        std::fs::copy(payload.join("BLAKE3SUMS"), recovery_dir.join("BLAKE3SUMS")).unwrap();
        std::fs::copy(
            payload.join("BLAKE3SUMS.sig"),
            recovery_dir.join("BLAKE3SUMS.sig"),
        )
        .unwrap();

        let release_file = base.join("release");
        std::fs::write(&release_file, format!("{RELEASE}\n")).unwrap();

        let heal = Heal {
            esp,
            history: base.join("esp/morph-boot/history.jsonl"),
            report: base.join("esp/morph-boot/repairs.json"),
            recovery_record: base.join("esp/morph-boot/recovery.json"),
            recovery_dir,
            recovery_keys,
            stage: base.join("stage"),
            release_file,
        };
        Machine {
            heal,
            recovery_root,
            root_a,
            root_b,
        }
    }

    fn slot_a_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.root_a).unwrap()
    }

    fn slot_b_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.root_b).unwrap()
    }
}

struct Running {
    default: Slot,
}

impl BootControl for Running {
    fn default_slot(&self) -> morph_boot::Result<Slot> {
        Ok(self.default)
    }
    fn set_default_slot(&mut self, slot: Slot) -> morph_boot::Result<()> {
        self.default = slot;
        Ok(())
    }
    fn booted_slot(&self) -> morph_boot::Result<Slot> {
        Ok(Slot::A)
    }
    fn arm_trial(&mut self, _slot: Slot) -> morph_boot::Result<Vec<String>> {
        Ok(vec!["reboot".into()])
    }
}

#[test]
fn recover_rebuilds_both_slots_and_sets_a_as_default() {
    let m = Machine::new();
    let mut bc = Running { default: Slot::B };
    let r = m
        .heal
        .recover(
            &mut bc,
            RECOVERY_CMDLINE,
            &mountinfo(),
            &m.recovery_root,
            &m.root_a,
            &m.root_b,
        )
        .unwrap();

    assert_eq!(r.a.slot, Slot::A);
    assert_eq!(r.b.slot, Slot::B);
    assert_eq!(r.default, Slot::A);
    assert_eq!(r.release, RELEASE);
    assert_eq!(
        m.slot_a_bytes(),
        RECOVERY_IMAGE,
        "slot A must become the recovery image"
    );
    assert_eq!(
        m.slot_b_bytes(),
        RECOVERY_IMAGE,
        "slot B must become the recovery image"
    );
    assert_eq!(bc.default, Slot::A, "the default is A, not the slot it was");

    // Both slots get the recovery kernel and initrd, not the files they held.
    for s in ["a", "b"] {
        let boot = m.heal.esp.join("morpheos").join(s);
        assert_eq!(std::fs::read(boot.join("vmlinuz")).unwrap(), KERNEL);
        assert_eq!(std::fs::read(boot.join("initrd.img")).unwrap(), INITRD);
    }

    // A recovery record on the ESP names both slots as rebuilt.
    let record = std::fs::read_to_string(&m.heal.recovery_record).unwrap();
    assert!(record.contains("\"action\": \"recovered\""), "{record}");
    assert!(record.contains("\"slots\": ["), "{record}");
    assert!(record.contains("\"default\": \"a\""), "{record}");
}

#[test]
fn recover_refuses_outside_the_recovery_system() {
    let m = Machine::new();
    let mut bc = Running { default: Slot::B };
    // A normal slot's command line, even with / on a recovery-looking device, is not enough.
    let err = m
        .heal
        .recover(
            &mut bc,
            SLOT_CMDLINE,
            &mountinfo(),
            &m.recovery_root,
            &m.root_a,
            &m.root_b,
        )
        .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
    assert_eq!(m.slot_a_bytes(), GARBAGE, "nothing may be written");
    assert_eq!(m.slot_b_bytes(), GARBAGE);
    assert_eq!(bc.default, Slot::B, "the default is untouched");
}

#[test]
fn recover_refuses_to_write_the_recovery_partition_as_a_slot() {
    let m = Machine::new();
    let mut bc = Running { default: Slot::B };
    let err = m
        .heal
        .recover(
            &mut bc,
            RECOVERY_CMDLINE,
            &mountinfo(),
            &m.recovery_root,
            &m.recovery_root,
            &m.root_b,
        )
        .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
    assert!(
        Path::new(&m.recovery_root).exists(),
        "the recovery partition must survive"
    );
    assert_eq!(m.slot_b_bytes(), GARBAGE);
}

#[test]
fn the_recovery_record_does_not_exist_until_a_recovery_has_succeeded() {
    let m = Machine::new();
    assert!(!Path::new(&m.heal.recovery_record).exists());
}

#[test]
fn recover_refuses_a_corrupted_recovery_image_and_writes_nothing() {
    let m = Machine::new();
    let mut bc = Running { default: Slot::B };

    // Flip one byte of the recovery partition. The factory manifest still verifies, but the
    // recovery image it describes no longer matches what is on the device.
    let mut corrupted = std::fs::read(&m.recovery_root).unwrap();
    corrupted[0] ^= 0xFF;
    std::fs::write(&m.recovery_root, &corrupted).unwrap();

    let err = m
        .heal
        .recover(
            &mut bc,
            RECOVERY_CMDLINE,
            &mountinfo(),
            &m.recovery_root,
            &m.root_a,
            &m.root_b,
        )
        .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
    assert_eq!(m.slot_a_bytes(), GARBAGE, "nothing may be written");
    assert_eq!(m.slot_b_bytes(), GARBAGE, "nothing may be written");
    assert_eq!(bc.default, Slot::B, "the default is untouched");
    assert!(
        !Path::new(&m.heal.recovery_record).exists(),
        "a refused recovery must not record itself as having recovered"
    );
}
