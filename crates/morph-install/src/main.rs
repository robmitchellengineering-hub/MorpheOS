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
        eprintln!("  prints the layout it would make and every reason it would refuse; it writes nothing (E7)");
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
    println!("\nNOTE: nothing has been written. The install steps are E7's next half.");
    ExitCode::SUCCESS
}

fn disk_size(disk: &str) -> u64 {
    // sysfs reports 512-byte sectors for block devices; a disk given as /dev/vda2 still needs its parent's size.
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

/// A mountinfo line is: `id parent major:minor root mount_point options [optional...] - fstype source superopts`.
///
/// The SOURCE is not field 3 — that is `major:minor`. It sits after the `-` separator, which is why an installer
/// that reads the wrong field "sees" no mounted partition and erases one that is in use. Returns the mount point
/// and the source.
fn mountinfo_parts(line: &str) -> Option<(&str, &str)> {
    let mut it = line.split_whitespace();
    let _id = it.next()?;
    let _parent = it.next()?;
    let _majmin = it.next()?;
    let _root = it.next()?;
    let point = it.next()?;
    let rest: Vec<&str> = it.collect();
    let sep = rest.iter().position(|f| *f == "-")?;
    let fstype = *rest.get(sep + 1)?;
    let source = *rest.get(sep + 2)?;
    let _ = fstype;
    Some((point, source))
}

fn mounted_partitions(disk: &str) -> Vec<String> {
    let base = disk.rsplit('/').next().unwrap_or("");
    let mut out = Vec::new();
    if let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") {
        for line in mountinfo.lines() {
            if let Some((_point, src)) = mountinfo_parts(line) {
                if src.starts_with("/dev/") && src.contains(base) {
                    out.push(src.to_string());
                }
            }
        }
    }
    out
}

/// The parent disks of `/` and `/efi`, so the installer can refuse the disk it booted from.
fn running_disks() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") {
        for line in mountinfo.lines() {
            let Some((point, src)) = mountinfo_parts(line) else { continue };
            if point != "/" && point != "/efi" {
                continue;
            }
            let Some(part) = src.rsplit('/').next() else { continue };
            if !part.starts_with('v') && !part.starts_with('s') && !part.starts_with("nvme") && !part.starts_with("mmcblk") {
                continue;
            }
            // vda1 -> vda, nvme0n1p2 -> nvme0n1, mmcblk0p3 -> mmcblk0
            let disk = if let Some(rest) = part.split_once('p') {
                if part.starts_with("nvme") || part.starts_with("mmcblk") { rest.0 } else { part.trim_end_matches(|c: char| c.is_ascii_digit()) }
            } else {
                part.trim_end_matches(|c: char| c.is_ascii_digit())
            };
            if !disk.is_empty() {
                out.push(format!("/dev/{disk}"));
            }
        }
    }
    out
}
