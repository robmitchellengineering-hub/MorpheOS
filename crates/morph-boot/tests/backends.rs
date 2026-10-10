//! The real backends against real files: a fake Pi device tree, a fake ESP,
//! and a stand-in `bootctl` that records how it was called.

use std::fs;
use std::path::{Path, PathBuf};

use morph_boot::pi::{Autoboot, Pi};
use morph_boot::sdboot::SdBoot;
use morph_boot::{Decision, Phase, Slot, confirm, start_trial, status, update_target};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("morph-boot-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_dt(dir: &Path, partition: u32, tryboot: u32) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("partition"), partition.to_be_bytes()).unwrap();
    fs::write(dir.join("tryboot"), tryboot.to_be_bytes()).unwrap();
}

#[test]
fn pi_update_cycle_follows_the_documented_flow() {
    let root = scratch("pi");
    let autoboot = root.join("autoboot.txt");
    let dt = root.join("dt");
    fs::write(&autoboot, Autoboot { default: Slot::A }.render()).unwrap();
    write_dt(&dt, 2, 0);
    let mut pi = Pi::new(&autoboot, &dt);

    // Running slot A (partition 2), committed: the update goes to B.
    assert_eq!(status(&pi).unwrap().phase, Phase::Committed);
    assert_eq!(update_target(&pi).unwrap(), Slot::B);
    let reboot = start_trial(&mut pi, Slot::B).unwrap();
    assert_eq!(reboot, ["systemctl", "reboot", "0 tryboot"]);
    assert!(
        start_trial(&mut pi, Slot::A).is_err(),
        "the running slot is never trialled"
    );

    // Firmware boots partition 3 because the tryboot flag was set.
    write_dt(&dt, 3, 1);
    assert!(pi.tryboot_flag().unwrap());
    assert_eq!(status(&pi).unwrap().phase, Phase::Trial);
    assert!(update_target(&pi).is_err(), "no updates during a trial");

    // Health check passes: autoboot.txt swaps to the committed form from the docs.
    assert_eq!(
        confirm(&mut pi, true).unwrap(),
        Decision::Committed { slot: Slot::B }
    );
    let text = fs::read_to_string(&autoboot).unwrap();
    assert!(text.contains("[all]\ntryboot_a_b=1\nboot_partition=3\n[tryboot]\nboot_partition=2\n"));
    write_dt(&dt, 3, 0);
    assert_eq!(status(&pi).unwrap().phase, Phase::Committed);
}

#[test]
fn pi_failed_trial_leaves_autoboot_untouched() {
    let root = scratch("pi-fail");
    let autoboot = root.join("autoboot.txt");
    let dt = root.join("dt");
    let original = Autoboot { default: Slot::A }.render();
    fs::write(&autoboot, &original).unwrap();
    write_dt(&dt, 3, 1);
    let mut pi = Pi::new(&autoboot, &dt);

    let d = confirm(&mut pi, false).unwrap();
    assert!(matches!(d, Decision::Fallback { to: Slot::A, .. }), "{d:?}");
    assert_eq!(fs::read_to_string(&autoboot).unwrap(), original);
}

#[test]
fn pi_rejects_a_hand_broken_autoboot() {
    let root = scratch("pi-broken");
    let autoboot = root.join("autoboot.txt");
    let dt = root.join("dt");
    fs::write(&autoboot, "[all]\nboot_partition=2\n").unwrap();
    write_dt(&dt, 2, 0);
    let pi = Pi::new(&autoboot, &dt);
    assert!(status(&pi).is_err());
}

fn fake_esp(root: &Path) -> (PathBuf, PathBuf) {
    let esp = root.join("esp");
    fs::create_dir_all(esp.join("loader/entries")).unwrap();
    fs::write(
        esp.join("loader/loader.conf"),
        "timeout 0\ndefault morpheos-a.conf\n",
    )
    .unwrap();
    for s in ["a", "b"] {
        fs::write(
            esp.join(format!("loader/entries/morpheos-{s}.conf")),
            format!("title MorpheOS ({s})\noptions morph.slot={s}\n"),
        )
        .unwrap();
    }
    let log = root.join("bootctl.log");
    let bootctl = root.join("bootctl");
    fs::write(
        &bootctl,
        format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bootctl, fs::Permissions::from_mode(0o755)).unwrap();
    }
    (esp, bootctl)
}

#[cfg(unix)]
#[test]
fn sdboot_update_cycle() {
    let root = scratch("sdboot");
    let (esp, bootctl) = fake_esp(&root);
    let cmdline = root.join("cmdline");
    fs::write(&cmdline, "root=PARTLABEL=root-a ro morph.slot=a\n").unwrap();
    let mut sd = SdBoot::new(&esp);
    sd.cmdline = cmdline.clone();
    sd.bootctl = bootctl;

    assert_eq!(update_target(&sd).unwrap(), Slot::B);
    assert_eq!(
        start_trial(&mut sd, Slot::B).unwrap(),
        ["systemctl", "reboot"]
    );
    let log = fs::read_to_string(root.join("bootctl.log")).unwrap();
    assert_eq!(
        log.trim(),
        format!("--esp-path={} set-oneshot morpheos-b.conf", esp.display())
    );

    fs::write(&cmdline, "root=PARTLABEL=root-b ro morph.slot=b\n").unwrap();
    assert_eq!(status(&sd).unwrap().phase, Phase::Trial);
    assert_eq!(
        confirm(&mut sd, true).unwrap(),
        Decision::Committed { slot: Slot::B }
    );
    assert_eq!(
        fs::read_to_string(esp.join("loader/loader.conf")).unwrap(),
        "timeout 0\ndefault morpheos-b.conf\n"
    );
}

#[cfg(unix)]
#[test]
fn sdboot_refuses_a_trial_without_an_entry() {
    let root = scratch("sdboot-noentry");
    let (esp, bootctl) = fake_esp(&root);
    fs::remove_file(esp.join("loader/entries/morpheos-b.conf")).unwrap();
    let cmdline = root.join("cmdline");
    fs::write(&cmdline, "morph.slot=a").unwrap();
    let mut sd = SdBoot::new(&esp);
    sd.cmdline = cmdline;
    sd.bootctl = bootctl;
    assert!(start_trial(&mut sd, Slot::B).is_err());
    assert!(
        !root.join("bootctl.log").exists(),
        "bootctl must not be called"
    );
}
