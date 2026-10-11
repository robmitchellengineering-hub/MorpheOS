//! Real installs against real files: no root, no loop devices, no mocks of the thing
//! under test.
//!
//! The root "device" is an ordinary file here and a block device in production — the
//! code path is the same `write` + `read-back-and-hash`, which is the point of taking
//! paths rather than partitions.
//!
//! Every refusal below is asserted as hard as a success, and each one also asserts what
//! it was protecting: **a refused update must leave the inactive slot exactly as it
//! found it**, because a half-written slot is worse than no update at all.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use morph_boot::{BootControl, Slot};
use morph_update::{Error, Layout, Payload, install};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("morph-update-{}-{name}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A payload whose files are real and whose manifest matches them.
fn payload_dir(base: &Path, root: &[u8]) -> PathBuf {
    let dir = base.join("payload");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("root.img"), root).unwrap();
    std::fs::write(dir.join("vmlinuz"), b"a kernel for slot b").unwrap();
    std::fs::write(dir.join("initrd.img"), b"an initramfs for slot b").unwrap();
    write_manifest(&dir);
    dir
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn write_manifest(dir: &Path) {
    let mut text = String::new();
    for name in ["root.img", "vmlinuz", "initrd.img"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        text.push_str(&format!("{}  {name}\n", digest(&bytes)));
    }
    std::fs::write(dir.join("BLAKE3SUMS"), text).unwrap();
}

/// The device starts full of a pattern, so "was it touched?" is a byte comparison.
const PATTERN: &[u8] = &[0xAA; 512];

struct Fixture {
    payload_dir: PathBuf,
    layout: Layout,
}

impl Fixture {
    fn new(root: &[u8]) -> Fixture {
        let base = scratch("case");
        let payload_dir = payload_dir(&base, root);
        let esp = base.join("esp");
        std::fs::create_dir_all(&esp).unwrap();
        let device = base.join("root-b.img");
        std::fs::write(&device, PATTERN).unwrap();
        Fixture {
            payload_dir,
            layout: Layout {
                esp,
                root_device: device,
            },
        }
    }

    fn open(&self) -> Payload {
        Payload::open(&self.payload_dir).unwrap()
    }

    fn device_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.layout.root_device).unwrap()
    }
}

/// An A/B machine where slot A is running and committed: slot B is the update target.
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

const NEW_ROOT: &[u8] = b"the new slot b root filesystem, longer than the pattern it replaces";

#[test]
fn installs_into_the_inactive_slot_and_verifies_what_landed() {
    let f = Fixture::new(NEW_ROOT);
    let mut payload = f.open();
    let out = install(&running_a(), &mut payload, &f.layout, None).unwrap();

    assert_eq!(out.slot, Slot::B, "the inactive slot is the one written");
    assert_eq!(out.root_bytes, NEW_ROOT.len() as u64);
    assert_eq!(out.root_digest, digest(NEW_ROOT));

    // The device really holds the new image, not the pattern it started with.
    assert_eq!(f.device_bytes(), NEW_ROOT);

    // Both boot files are beside the slot, where the loader entry points.
    let boot = f.layout.boot_dir(Slot::B);
    assert_eq!(
        std::fs::read(boot.join("vmlinuz")).unwrap(),
        b"a kernel for slot b"
    );
    assert_eq!(
        std::fs::read(boot.join("initrd.img")).unwrap(),
        b"an initramfs for slot b"
    );

    // And the slot is not left looking unfinished.
    assert!(
        !f.layout.marker(Slot::B).exists(),
        "the incomplete marker must be cleared"
    );
}

#[test]
fn a_payload_that_does_not_match_its_manifest_never_touches_the_device() {
    let f = Fixture::new(NEW_ROOT);
    // Flip one byte of the root image AFTER the manifest was written, which is exactly
    // what a truncated download or a bad disk looks like.
    let mut bytes = NEW_ROOT.to_vec();
    bytes[0] ^= 0xFF;
    std::fs::write(f.payload_dir.join("root.img"), &bytes).unwrap();

    let mut payload = f.open();
    let err = install(&running_a(), &mut payload, &f.layout, None).unwrap_err();
    assert!(matches!(err, Error::BadPayload(_)), "got {err}");

    assert_eq!(
        f.device_bytes(),
        PATTERN,
        "a payload that fails its own manifest must be refused BEFORE the slot is written"
    );
    assert!(
        !f.layout.marker(Slot::B).exists(),
        "nothing should have been marked"
    );
}

#[test]
fn a_payload_missing_a_file_is_refused() {
    let f = Fixture::new(NEW_ROOT);
    std::fs::remove_file(f.payload_dir.join("initrd.img")).unwrap();
    let mut payload = f.open();
    let err = install(&running_a(), &mut payload, &f.layout, None).unwrap_err();
    assert!(matches!(err, Error::BadPayload(_)), "got {err}");
    assert_eq!(f.device_bytes(), PATTERN);
}

#[test]
fn a_manifest_that_is_not_exactly_the_three_files_is_refused() {
    let f = Fixture::new(NEW_ROOT);

    // An extra entry: a payload with a file nobody described is a payload whose
    // signature covers something other than what gets installed.
    let mut text = std::fs::read_to_string(f.payload_dir.join("BLAKE3SUMS")).unwrap();
    text.push_str(&format!("{}  something-else\n", digest(b"x")));
    std::fs::write(f.payload_dir.join("BLAKE3SUMS"), &text).unwrap();
    assert!(matches!(
        Payload::open(&f.payload_dir),
        Err(Error::BadPayload(_))
    ));

    // A missing one.
    let text: String = std::fs::read_to_string(f.payload_dir.join("BLAKE3SUMS"))
        .unwrap()
        .lines()
        .filter(|l| !l.ends_with("vmlinuz"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(f.payload_dir.join("BLAKE3SUMS"), &text).unwrap();
    assert!(matches!(
        Payload::open(&f.payload_dir),
        Err(Error::BadPayload(_))
    ));
}

#[test]
fn writing_the_running_slot_is_refused() {
    let f = Fixture::new(NEW_ROOT);
    let mut payload = f.open();
    // The caller asserts slot A. A is running, so the policy says B — and the policy
    // wins. This is the one mistake that cannot be recovered from.
    let err = install(&running_a(), &mut payload, &f.layout, Some(Slot::A)).unwrap_err();
    assert!(matches!(err, Error::Refused(_)), "got {err}");
    assert_eq!(f.device_bytes(), PATTERN);
}

#[test]
fn an_update_is_refused_while_a_trial_is_running() {
    let f = Fixture::new(NEW_ROOT);
    let mut payload = f.open();
    // Booted B, default A: B is an unconfirmed trial, so A is the only known-good
    // system and neither slot may be written.
    let trial = Running {
        booted: Slot::B,
        default: Slot::A,
    };
    let err = install(&trial, &mut payload, &f.layout, None).unwrap_err();
    assert!(matches!(err, Error::Policy(_)), "got {err}");
    assert_eq!(f.device_bytes(), PATTERN);
}

#[test]
fn a_slot_left_incomplete_by_an_earlier_update_is_refused() {
    let f = Fixture::new(NEW_ROOT);
    let boot = f.layout.boot_dir(Slot::B);
    std::fs::create_dir_all(&boot).unwrap();
    std::fs::write(f.layout.marker(Slot::B), b"did not finish\n").unwrap();

    let mut payload = f.open();
    let err = install(&running_a(), &mut payload, &f.layout, None).unwrap_err();
    assert!(matches!(err, Error::Incomplete(_)), "got {err}");
    assert_eq!(f.device_bytes(), PATTERN);
}

#[test]
fn a_write_that_cannot_happen_leaves_the_slot_marked_incomplete() {
    let f = Fixture::new(NEW_ROOT);
    // A directory where the device should be: the write fails, after the marker exists.
    // This is the shape of a power cut mid-update, and the slot must NOT look ready.
    let layout = Layout {
        esp: f.layout.esp.clone(),
        root_device: f.layout.esp.clone(),
    };
    let mut payload = f.open();
    let err = install(&running_a(), &mut payload, &layout, None).unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "got {err}");
    assert!(
        f.layout.marker(Slot::B).exists(),
        "an interrupted update must leave the slot marked, so it cannot be trialled by accident"
    );
}

#[test]
fn a_payload_file_that_is_a_device_is_read_not_called_missing() {
    // `/dev/null` stands in for a block device: `Path::is_file()` is false for both, and
    // that is precisely what made a repair report *"root.img is missing"* for a file that
    // was present and perfectly readable. A portable stand-in, so the property is covered
    // on every machine rather than only where a spare partition exists.
    let base = scratch("device");
    let dir = base.join("payload");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink("/dev/null", dir.join("root.img")).unwrap();
    std::fs::write(dir.join("vmlinuz"), b"k").unwrap();
    std::fs::write(dir.join("initrd.img"), b"i").unwrap();

    morph_update::seal(&dir).expect("a device is a readable payload file, not a missing one");
    let mut payload = Payload::open(&dir).unwrap();
    let sizes = payload.check().expect("and it still checks");
    assert_eq!(
        sizes["root.img"], 0,
        "/dev/null reads as empty, which is the point: it was read at all"
    );
}

#[test]
fn check_reports_the_files_without_writing_anything() {
    let f = Fixture::new(NEW_ROOT);
    let mut payload = f.open();
    let sizes = payload.check().unwrap().clone();
    assert_eq!(sizes["root.img"], NEW_ROOT.len() as u64);
    assert_eq!(sizes.len(), 3);
    assert_eq!(f.device_bytes(), PATTERN, "check must not write");
}
