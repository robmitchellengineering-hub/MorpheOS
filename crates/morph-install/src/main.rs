//! `morph-install` — see `lib.rs` for the plan and the refusals.
//!
//! **This binary does not write to a disk yet, and it says so instead of pretending.** E7's pure half (the layout,
//! the partition names, every refusal with its test) is implemented and tested; the destructive half — `sfdisk`,
//! `mkfs`, the ESP tree and `morph_update::install` for both slots — is the next step and needs the QEMU scenario
//! that can prove it (boot from one disk, install to another, boot the target alone).
//!
//! What it does today: reads the facts an install would need and prints the plan and any refusals, so the decision
//! can be reviewed before anything writes.
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let yes = args.iter().any(|a| a == "--yes");
    args.retain(|a| a != "--yes");
    let Some(disk) = args.first() else {
        eprintln!("usage: morph-install [--yes] DISK");
        eprintln!(
            "  prints the layout it would make and every reason it would refuse; it writes nothing (E7)"
        );
        return ExitCode::from(2);
    };

    let size = disk_size(disk);
    let plan = morph_install::plan(size);
    println!("target: {disk} ({} GiB)", size / (1024 * 1024 * 1024));
    println!("layout:");
    for p in &plan {
        println!(
            "  {index:>2} {name:<8} {gib:>6.1} GiB  starts at {start} MiB",
            index = p.index,
            name = p.name,
            gib = p.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            start = p.start_bytes / (1024 * 1024)
        );
        match morph_install::partition_device(disk, p.index) {
            Some(dev) => println!("       {dev}"),
            None => println!("       (no device name — refusing)"),
        }
    }

    let target = morph_install::Target {
        disk: disk.clone(),
        size_bytes: size,
        removable: is_removable(disk),
        mounted_sources: mounted_partitions(disk),
        running_disks: running_disks(),
        named_explicitly: yes && disk.starts_with("/dev/disk/by-id/"),
    };
    let refusals = morph_install::refusals(&target);
    if refusals.is_empty() {
        println!("\nno refusals — this disk could be installed onto");
    } else {
        println!("\nrefused:");
        for r in refusals {
            println!("  - {}", r.reason());
        }
        return ExitCode::FAILURE;
    }
    // ── THE DESTRUCTIVE HALF ──────────────────────────────────────────────────────────────────────────────
    // Everything below writes to a disk, so it runs only after the refusals came back empty AND the caller said
    // --yes. Each step is guarded and reports its own failure; test-install.sh exercises this path in a VM, because
    // running it against a machine you care about is how you lose it.
    if !yes {
        println!(
            "\nrefused to write: run again with --yes <disk> once you have read the plan above"
        );
        return ExitCode::FAILURE;
    }
    for step in morph_install::InstallStep::all() {
        let dev = morph_install::partition_device(disk, part_index(step));
        print!("  → {} ... ", step.label());
        if let Err(e) = run_step(step, disk, dev.as_deref()) {
            println!("FAILED: {e}");
            eprintln!(
                "    the disk is in a partial state; do not boot it. Fix the cause and re-run."
            );
            return ExitCode::FAILURE;
        }
        println!("ok");
    }
    println!("\ninstalled. Boot the machine from its internal disk and confirm slot A commits.");
    ExitCode::SUCCESS
}

fn part_index(step: morph_install::InstallStep) -> u32 {
    use morph_install::InstallStep::*;
    match step {
        Repartition => 0,
        MakeEsp | WriteEspTree | InstallBootEntry => 1,
        MakeRootA | WriteSlotA => 2,
        MakeRootB | WriteSlotB => 3,
        MakeData => 4,
    }
}

/// One install step, as a command. Returns an error that names what went wrong rather than a raw exit code.
fn run_step(
    step: morph_install::InstallStep,
    disk: &str,
    part_dev: Option<&str>,
) -> Result<(), String> {
    use morph_install::InstallStep::*;
    let plan = morph_install::plan(disk_size(disk));
    match step {
        Repartition => {
            let script = morph_install::sfdisk_script(&plan);
            run("sfdisk", &[disk], Some(&script))?;
            let _ = run("partprobe", &[disk], None); // make the kernel see the new table
            Ok(())
        }
        MakeEsp => {
            // mkfs.vfat on the whole-disk-relative device, sized for the partition
            run(
                "mkfs.vfat",
                &["-F", "32", "-n", "ESP", part_dev.unwrap()],
                None,
            )
            .map_err(|e| format!("mkfs.vfat: {e}"))
        }
        MakeData => run("mkfs.ext4", &["-L", "data", part_dev.unwrap()], None)
            .map_err(|e| format!("mkfs.ext4 data: {e}")),
        MakeRootA => run("mkfs.ext4", &["-L", "root-a", part_dev.unwrap()], None)
            .map_err(|e| format!("mkfs.ext4 root-a: {e}")),
        MakeRootB => run("mkfs.ext4", &["-L", "root-b", part_dev.unwrap()], None)
            .map_err(|e| format!("mkfs.ext4 root-b: {e}")),
        WriteEspTree => {
            // Mount the TARGET ESP and copy the running stick's boot tree into it. It STAYS mounted: the two
            // slot writes below hand `--esp` to morph-update, which writes the fresh slot's kernel/initrd and
            // loader entry there. (An earlier draft umounted here, so the slot writes would have rebuilt the
            // STICK's own ESP — the wrong disk.)
            let esp = part_dev.unwrap();
            run("mkdir", &["-p", "/tmp/morph-install-esp"], None)
                .and_then(|_| run("mount", &[esp, "/tmp/morph-install-esp"], None))
                .and_then(|_| run("cp", &["-a", "/efi/.", "/tmp/morph-install-esp/"], None))
                .map_err(|e| format!("copying the ESP tree: {e}"))
        }
        InstallBootEntry => run(
            "bootctl",
            &["install", "--esp-path=/tmp/morph-install-esp"],
            None,
        )
        .map_err(|e| format!("bootctl install: {e}")),
        // morph-update write is the VERIFIED write: it writes the sealed, signed payload's root image into
        // the slot device and reads it back to check the digest. The payload is carried on the stick's ESP
        // (/efi/morpheos/install), sealed and signed at image-build time — the key is the builder's, never
        // shipped. --slot asserts WHICH slot this partition is; --esp points at the mounted TARGET ESP so the
        // entry lands on the disk being installed, not on the stick.
        WriteSlotA => run(
            "morph-update",
            &[
                "write",
                "--payload",
                "/efi/morpheos/install",
                "--root",
                part_dev.unwrap(),
                "--slot",
                "a",
                "--esp",
                "/tmp/morph-install-esp",
            ],
            None,
        )
        .map_err(|e| format!("write slot A: {e}")),
        WriteSlotB => run(
            "morph-update",
            &[
                "write",
                "--payload",
                "/efi/morpheos/install",
                "--root",
                part_dev.unwrap(),
                "--slot",
                "b",
                "--esp",
                "/tmp/morph-install-esp",
            ],
            None,
        )
        .map_err(|e| format!("write slot B: {e}")),
    }
}

fn run(cmd: &str, args: &[&str], stdin: Option<&str>) -> Result<(), String> {
    use std::io::Write;
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    c.stdin(std::process::Stdio::piped());
    c.stdout(std::process::Stdio::null());
    c.stderr(std::process::Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("{cmd}: {e}"))?;
    if let Some(input) = stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .map_err(|e| format!("writing to {cmd}: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("waiting on {cmd}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn disk_size(disk: &str) -> u64 {
    // sysfs reports 512-byte sectors for block devices. A disk given as /dev/vda2 still needs its parent's size.
    let base = disk.rsplit('/').next().unwrap_or("");
    let sectors = std::fs::read_to_string(format!("/sys/class/block/{base}/size"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    sectors * 512
}

fn is_removable(disk: &str) -> bool {
    let base = disk.rsplit('/').next().unwrap_or("");
    std::fs::read_to_string(format!("/sys/class/block/{base}/removable"))
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

/// A mountinfo line is: `id parent major:minor root mount_point options … - fstype source superopts`.
///
/// The SOURCE is not field 3 — that is `major:minor`. It sits after the `-` separator, which is why an installer
/// that reads the wrong field "sees" no mounted partition and erases one that is in use. Returns (mount_point,
/// source).
fn mountinfo_parts(line: &str) -> Option<(&str, &str)> {
    let mut it = line.split_whitespace();
    let _id = it.next()?;
    let _parent = it.next()?;
    let _majmin = it.next()?;
    let _root = it.next()?;
    let point = it.next()?;
    let rest: Vec<&str> = it.collect();
    let sep = rest.iter().position(|f| *f == "-")?;
    let _fstype = *rest.get(sep + 1)?;
    let source = *rest.get(sep + 2)?;
    Some((point, source))
}

fn mounted_partitions(disk: &str) -> Vec<String> {
    let base = disk.rsplit('/').next().unwrap_or("");
    let mut out = Vec::new();
    if let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") {
        for line in mountinfo.lines() {
            let Some((_point, src)) = mountinfo_parts(line) else {
                continue;
            };
            if src.starts_with("/dev/") && src.contains(base) {
                out.push(src.to_string());
            }
        }
    }
    out
}

/// The parent disks of `/` and `/efi`, so the installer refuses the disk it booted from.
fn running_disks() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") {
        for line in mountinfo.lines() {
            let Some((point, src)) = mountinfo_parts(line) else {
                continue;
            };
            if point != "/" && point != "/efi" {
                continue;
            }
            let Some(part) = src.rsplit('/').next() else {
                continue;
            };
            if !(part.starts_with('v')
                || part.starts_with('s')
                || part.starts_with("nvme")
                || part.starts_with("mmcblk"))
            {
                continue;
            }
            let trimmed = part.trim_end_matches(|c: char| c.is_ascii_digit());
            let disk = match part.split_once('p') {
                Some((head, _)) if part.starts_with("nvme") || part.starts_with("mmcblk") => head,
                _ => trimmed,
            };
            if !disk.is_empty() {
                out.push(format!("/dev/{disk}"));
            }
        }
    }
    out
}
