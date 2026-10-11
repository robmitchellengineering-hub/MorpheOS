//! Real files, no root, no mocks of the thing under test.
//!
//! The two roots are ordinary files here and block devices in production; the code path is
//! the same "read the running system as an image, write it into the spare, read it back and
//! hash it". Every refusal is asserted as hard as a repair, and each one also asserts what
//! it protected — the running slot is the one thing this must never overwrite.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use morph_boot::{BootControl, Slot};
use morph_heal::{Error, Heal};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("morph-heal-{}-{name}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A committed boot, then a trial of `failed` that fell back to `good`.
fn history_with_failure(failed: &str, good: &str) -> String {
    let committed = format!(
        r#"{{"state":"running","status":{{"booted":"{good}","default":"{good}","phase":"committed"}},"decision":{{"action":"none"}}}}"#
    );
    let fallback = format!(
        r#"{{"state":"degraded","status":{{"booted":"{failed}","default":"{good}","phase":"trial"}},"decision":{{"action":"fallback","to":"{good}","reboot":["systemctl","reboot"]}}}}"#
    );
    format!("{committed}\n{fallback}\n")
}

const RUNNING_IMAGE: &[u8] = b"the running slot's system image, which the spare must become";
const OLD_SPARE: &[u8] = &[0x5A; 256];

struct Machine {
    base: PathBuf,
    heal: Heal,
    running_root: PathBuf,
    spare_root: PathBuf,
}

impl Machine {
    fn new(history: &str) -> Machine {
        let base = scratch("machine");
        let esp = base.join("esp");
        std::fs::create_dir_all(esp.join("morph-boot")).unwrap();
        std::fs::create_dir_all(esp.join("morpheos/a")).unwrap();
        std::fs::write(esp.join("morph-boot/history.jsonl"), history).unwrap();
        // Slot A's boot files, which a repair copies into the spare.
        std::fs::write(esp.join("morpheos/a/vmlinuz"), b"slot a kernel").unwrap();
        std::fs::write(esp.join("morpheos/a/initrd.img"), b"slot a initramfs").unwrap();

        let running_root = base.join("root-a.img");
        std::fs::write(&running_root, RUNNING_IMAGE).unwrap();
        let spare_root = base.join("root-b.img");
        std::fs::write(&spare_root, OLD_SPARE).unwrap();

        let heal = Heal {
            esp,
            history: base.join("esp/morph-boot/history.jsonl"),
            report: base.join("esp/morph-boot/repairs.json"),
            stage: base.join("stage"),
        };
        Machine {
            base,
            heal,
            running_root,
            spare_root,
        }
    }

    fn spare_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.spare_root).unwrap()
    }

    fn append_failure(&self, failed: &str, good: &str) {
        let mut h = std::fs::read_to_string(&self.heal.history).unwrap();
        h.push_str(history_with_failure(failed, good).lines().last().unwrap());
        h.push('\n');
        std::fs::write(&self.heal.history, h).unwrap();
    }
}

/// Slot A running and committed: slot B is the spare.
struct Running {
    booted: Slot,
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
        Ok(self.booted)
    }
    fn arm_trial(&mut self, _slot: Slot) -> morph_boot::Result<Vec<String>> {
        Ok(vec!["reboot".into()])
    }
}

fn running_a() -> Running {
    Running {
        booted: Slot::A,
        default: Slot::A,
    }
}

#[test]
fn a_failed_trial_is_read_out_of_the_history() {
    let m = Machine::new(&history_with_failure("b", "a"));
    let f = morph_heal::last_failure(&m.heal.history).unwrap().unwrap();
    assert_eq!(f.slot, Slot::B, "the trialled slot is the one that failed");
    assert_eq!(f.good, Slot::A);
    assert_eq!(f.index, 2, "the index is what makes a repair idempotent");
}

#[test]
fn a_machine_that_has_never_failed_a_trial_owes_nothing() {
    let m = Machine::new(
        "{\"state\":\"running\",\"status\":{\"booted\":\"a\",\"default\":\"a\",\"phase\":\"committed\"},\"decision\":{\"action\":\"none\"}}\n",
    );
    assert_eq!(morph_heal::last_failure(&m.heal.history).unwrap(), None);
    assert_eq!(m.heal.needed().unwrap(), None);
}

#[test]
fn missing_history_is_nothing_to_do_and_not_an_error() {
    let m = Machine::new("");
    std::fs::remove_file(&m.heal.history).unwrap();
    assert_eq!(m.heal.needed().unwrap(), None);
}

#[test]
fn a_fallback_that_names_no_slots_is_refused_rather_than_guessed() {
    let m = Machine::new(
        "{\"state\":\"degraded\",\"status\":{},\"decision\":{\"action\":\"fallback\"}}\n",
    );
    let err = morph_heal::last_failure(&m.heal.history).unwrap_err();
    assert!(matches!(err, Error::History(_)), "got {err}");
}

#[test]
fn repair_rebuilds_the_spare_from_the_running_system_and_records_it() {
    let m = Machine::new(&history_with_failure("b", "a"));
    let (failure, outcome) = m
        .heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap();

    assert_eq!(failure.slot, Slot::B);
    assert_eq!(outcome.slot, Slot::B);
    assert_eq!(
        m.spare_bytes(),
        RUNNING_IMAGE,
        "the spare must become the running system's image, not the old one"
    );

    // The spare's boot files come from the running slot too, or it would boot a kernel
    // that does not match its root.
    let boot = m.heal.esp.join("morpheos/b");
    assert_eq!(
        std::fs::read(boot.join("vmlinuz")).unwrap(),
        b"slot a kernel"
    );
    assert_eq!(
        std::fs::read(boot.join("initrd.img")).unwrap(),
        b"slot a initramfs"
    );

    // And the repair is remembered, so it does not run again on every boot.
    assert!(m.heal.report.exists(), "the repair must be recorded");
    assert_eq!(
        m.heal.needed().unwrap(),
        None,
        "the same failure is not repaired twice"
    );

    // The full system image staged for the copy does not outlive the repair.
    assert!(
        !m.heal.stage.exists(),
        "the scratch image must be cleaned up"
    );
}

#[test]
fn a_new_failure_after_a_repair_is_repaired_again() {
    let m = Machine::new(&history_with_failure("b", "a"));
    m.heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap();
    assert_eq!(m.heal.needed().unwrap(), None);

    // The same slot fails again on a later boot: a repair that only remembered "B was
    // repaired once" would skip it, and the machine would stay unredundant.
    m.append_failure("b", "a");
    let owed = m
        .heal
        .needed()
        .unwrap()
        .expect("a new failure owes a repair");
    assert_eq!(owed.slot, Slot::B);
    assert_eq!(owed.index, 3);
}

#[test]
fn repair_refuses_when_the_history_names_the_slot_we_are_running_from() {
    let m = Machine::new(&history_with_failure("a", "b"));
    // The history claims slot A failed, but this system is running from A. A corrupted or
    // stale record must not be able to talk this into overwriting the running system.
    let err = m
        .heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
    assert_eq!(m.spare_bytes(), OLD_SPARE, "nothing may be written");
}

#[test]
fn repair_is_refused_while_a_trial_is_running() {
    let m = Machine::new(&history_with_failure("b", "a"));
    // The machine is inside an unconfirmed trial of slot A (booted A, default B), so no
    // slot may be written: the other slot is the only confirmed system. The failed slot
    // named by the history is B, which is NOT what we are running from — so this gets past
    // the guard above and is refused by the **boot policy** instead, which is the point:
    // two independent reasons exist for saying no, and this exercises the second.
    let mid_trial = Running {
        booted: Slot::A,
        default: Slot::B,
    };
    let err = m
        .heal
        .repair(&mid_trial, Slot::A, &m.running_root, &m.spare_root)
        .unwrap_err();
    assert!(matches!(err, Error::Update(_)), "got {err}");
    assert!(
        err.to_string().contains("trial"),
        "the refusal should say it is about the trial in progress: {err}"
    );
    assert_eq!(m.spare_bytes(), OLD_SPARE);
}

#[test]
fn repair_refuses_when_there_is_nothing_owed() {
    let m = Machine::new(&history_with_failure("b", "a"));
    m.heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap();
    let err = m
        .heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
}

#[test]
fn a_missing_kernel_for_the_running_slot_fails_before_anything_is_written() {
    let m = Machine::new(&history_with_failure("b", "a"));
    std::fs::remove_file(m.heal.esp.join("morpheos/a/vmlinuz")).unwrap();
    let err = m
        .heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "got {err}");
    assert_eq!(
        m.spare_bytes(),
        OLD_SPARE,
        "the payload is staged before the spare is touched"
    );
}

#[test]
fn the_source_image_survives_the_cleanup() {
    // The staged image is a SYMLINK to the running root, so the cleanup at the end removes
    // a link — it must not follow it and delete the device it points at. Nothing else here
    // would catch that: the spare would still hold the right bytes while the machine's own
    // system was gone.
    let m = Machine::new(&history_with_failure("b", "a"));
    m.heal
        .repair(&running_a(), Slot::A, &m.running_root, &m.spare_root)
        .unwrap();
    assert!(
        m.running_root.exists(),
        "the running root must still exist after the repair"
    );
    assert_eq!(std::fs::read(&m.running_root).unwrap(), RUNNING_IMAGE);
}

#[test]
fn the_report_does_not_exist_until_a_repair_has_succeeded() {
    let m = Machine::new(&history_with_failure("b", "a"));
    assert!(!Path::new(&m.heal.report).exists());
    let _ = m.base;
    assert!(m.heal.needed().unwrap().is_some());
}
