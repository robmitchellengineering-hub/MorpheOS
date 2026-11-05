//! The layout an install makes, and the reasons it refuses to make one.
//!
//! Doc 20, E7: "an installer that lays out ESP, slot A, slot B and data on the internal disk, and writes the system
//! with `morph_update`'s verified write". This module is the part that can be tested on any machine — the plan and
//! the refusals — so that the destructive part is the only thing that needs a disk.
//!
//! Nothing here touches hardware. Everything is a function from facts to a decision, which is why the refusals can
//! be proven rather than asserted.

/// The smallest disk an install will accept, in bytes.
///
/// Derived from the plan below: 1 MiB (alignment) + 1 GiB ESP + 8 GiB + 8 GiB slots + 16 GiB data = 33 GiB, rounded
/// up to 34 GiB so a disk reported as "32 GB" by its vendor cannot be mistaken for one that fits.
pub const MIN_DISK_BYTES: u64 = 34 * 1024 * 1024 * 1024;

/// Where each partition starts and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partition {
    pub index: u32,
    pub name: &'static str,
    /// The GPT type, as `sfdisk` spells it.
    pub type_guid: &'static str,
    pub start_bytes: u64,
    pub size_bytes: u64,
}

const TYPE_ESP: &str = "uefi";
/// x86-64 root filesystem. The same GUID the stick uses, because the slot images find their partitions by
/// `PARTLABEL` and by type; a different type would make an installed disk look unlike the thing it was installed
/// from.
const TYPE_ROOT: &str = "4f68bce3-e8cd-4db1-96e7-fbcaf984b709";
const TYPE_DATA: &str = "linux";

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// The four partitions an installed disk has, in order.
///
/// The slots are 8 GiB, far larger than the 1600 MiB stick image, because a slot must hold the kernel plus the root
/// filesystem of a *future* release and an A/B update needs room for both. Apps go on `/persist`, not in a slot.
pub fn plan(disk_bytes: u64) -> Vec<Partition> {
    let data = disk_bytes.saturating_sub(MIB + GIB + 8 * GIB + 8 * GIB);
    vec![
        Partition {
            index: 1,
            name: "esp",
            type_guid: TYPE_ESP,
            start_bytes: MIB,
            size_bytes: GIB,
        },
        Partition {
            index: 2,
            name: "root-a",
            type_guid: TYPE_ROOT,
            start_bytes: MIB + GIB,
            size_bytes: 8 * GIB,
        },
        Partition {
            index: 3,
            name: "root-b",
            type_guid: TYPE_ROOT,
            start_bytes: MIB + 9 * GIB,
            size_bytes: 8 * GIB,
        },
        Partition {
            index: 4,
            name: "data",
            type_guid: TYPE_DATA,
            start_bytes: MIB + 17 * GIB,
            size_bytes: data,
        },
    ]
}

/// The device node a partition of `disk` gets.
///
/// The naming rules are not cosmetic: `/dev/vda` → `vda1`, `/dev/nvme0n1` → `nvme0n1p1`, `/dev/mmcblk0` →
/// `mmcblk0p1`. Writing to the wrong one is how an installer erases the disk it booted from, so this is a function
/// with tests rather than a format string in a shell script.
pub fn partition_device(disk: &str, index: u32) -> Option<String> {
    let disk = disk.trim();
    if disk.is_empty() || index == 0 {
        return None;
    }
    let base = disk.rsplit('/').next()?;
    if base.is_empty() {
        return None;
    }
    // nvme0n1, mmcblk0, loop0: the kernel puts a `p` between the disk and the partition number because the disk
    // name already ends in a digit.
    let needs_p = base
        .rfind(|c: char| c.is_ascii_digit())
        .map(|i| i == base.len() - 1)
        .unwrap_or(false);
    Some(if needs_p {
        format!("{disk}p{index}")
    } else {
        format!("{disk}{index}")
    })
}

/// Why an install will not run. Each variant is a refusal with its own test, because each one protects something
/// that cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The disk holds the running system's root or `/efi`.
    HoldsRunningSystem,
    /// A partition on it is mounted — someone is using it.
    MountedPartition(String),
    /// Too small for the plan.
    TooSmall { bytes: u64, need: u64 },
    /// Removable media, and the caller did not name it explicitly.
    RemovableNeedsConfirmation,
}

impl Refusal {
    /// The sentence a person sees. A refusal that does not say why is a wall.
    pub fn reason(&self) -> String {
        match self {
            Refusal::HoldsRunningSystem => {
                "that disk holds the system this machine booted from; install onto a different disk"
                    .to_string()
            }
            Refusal::MountedPartition(p) => {
                format!("{p} is mounted; unmount it, or choose another disk")
            }
            Refusal::TooSmall { bytes, need } => format!(
                "{} GiB is too small: an install needs {} GiB (1 ESP, 8+8 slots, 16 data)",
                bytes / GIB,
                need / GIB
            ),
            Refusal::RemovableNeedsConfirmation => {
                "that is removable media; name it with --yes /dev/disk/by-id/... if you mean it"
                    .to_string()
            }
        }
    }
}

/// What an install is being asked to do, as facts rather than as a device.
#[derive(Debug, Clone, Default)]
pub struct Target {
    pub disk: String,
    pub size_bytes: u64,
    pub removable: bool,
    /// Mount sources of this disk's partitions, from `/proc/self/mountinfo`.
    pub mounted_sources: Vec<String>,
    /// The parent disks of `/` and `/efi`.
    pub running_disks: Vec<String>,
    /// True when the caller named this disk by its `/dev/disk/by-id` path.
    pub named_explicitly: bool,
}

/// The parent disk of a path, so `/dev/vda2` and `/dev/vda` compare equal. Not cosmetic: the refusal for "holds the
/// running system" is worthless if a partition path slips past it.
fn parent_disk(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or("");
    // vda1 -> vda, sdb4 -> sdb
    let trimmed = base.trim_end_matches(|c: char| c.is_ascii_digit());
    // nvme0n1p2 -> nvme0n1, mmcblk0p3 -> mmcblk0 (the `p` before the partition number is the kernel's)
    let disk = if (base.starts_with("nvme") || base.starts_with("mmcblk")) && trimmed.ends_with('p')
    {
        trimmed.trim_end_matches('p')
    } else {
        trimmed
    };
    format!("/dev/{disk}")
}

/// Every reason not to install onto this target, in a stable order. Empty means it may proceed.
pub fn refusals(t: &Target) -> Vec<Refusal> {
    let mut out = Vec::new();
    let this = parent_disk(&t.disk);
    if t.running_disks.iter().any(|d| parent_disk(d) == this) {
        out.push(Refusal::HoldsRunningSystem);
    }
    for m in &t.mounted_sources {
        out.push(Refusal::MountedPartition(m.clone()));
    }
    if t.size_bytes < MIN_DISK_BYTES {
        out.push(Refusal::TooSmall {
            bytes: t.size_bytes,
            need: MIN_DISK_BYTES,
        });
    }
    if t.removable && !t.named_explicitly {
        out.push(Refusal::RemovableNeedsConfirmation);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_covers_the_disk_without_gaps() {
        let disk = 64 * GIB;
        let p = plan(disk);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].start_bytes, MIB);
        // each partition begins exactly where the previous one ends
        for w in p.windows(2) {
            assert_eq!(
                w[0].start_bytes + w[0].size_bytes,
                w[1].start_bytes,
                "{:?}",
                w
            );
        }
        let last = p.last().unwrap();
        assert_eq!(last.start_bytes + last.size_bytes, disk);
        // and data keeps at least the 16 GiB the plan promises on a minimum disk
        assert!(plan(MIN_DISK_BYTES).last().unwrap().size_bytes >= 16 * GIB);
    }

    #[test]
    fn partition_names_follow_the_kernels_rules() {
        assert_eq!(partition_device("/dev/vda", 1).unwrap(), "/dev/vda1");
        assert_eq!(partition_device("/dev/sdb", 4).unwrap(), "/dev/sdb4");
        assert_eq!(
            partition_device("/dev/nvme0n1", 2).unwrap(),
            "/dev/nvme0n1p2"
        );
        assert_eq!(
            partition_device("/dev/mmcblk0", 3).unwrap(),
            "/dev/mmcblk0p3"
        );
        assert_eq!(partition_device("/dev/loop0", 1).unwrap(), "/dev/loop0p1");
        assert_eq!(partition_device("", 1), None);
        assert_eq!(partition_device("/dev/vda", 0), None);
    }

    #[test]
    fn refuses_the_disk_it_booted_from() {
        let t = Target {
            disk: "/dev/vda".into(),
            size_bytes: 64 * GIB,
            running_disks: vec!["/dev/vda".into()],
            ..Default::default()
        };
        assert_eq!(refusals(&t), vec![Refusal::HoldsRunningSystem]);
        // a partition path for the same disk is the same refusal, not a different one
        let t2 = Target {
            disk: "/dev/vda2".into(),
            ..t.clone()
        };
        assert!(refusals(&t2).contains(&Refusal::HoldsRunningSystem));
    }

    #[test]
    fn refuses_a_disk_with_a_mounted_partition() {
        let t = Target {
            disk: "/dev/vdb".into(),
            size_bytes: 64 * GIB,
            mounted_sources: vec!["/dev/vdb1".into()],
            ..Default::default()
        };
        assert_eq!(
            refusals(&t),
            vec![Refusal::MountedPartition("/dev/vdb1".into())]
        );
    }

    #[test]
    fn refuses_a_disk_that_is_too_small() {
        let t = Target {
            disk: "/dev/vdb".into(),
            size_bytes: 20 * GIB,
            ..Default::default()
        };
        let r = refusals(&t);
        assert!(matches!(r.as_slice(), [Refusal::TooSmall { .. }]));
        assert!(r[0].reason().contains("too small"), "{}", r[0].reason());
    }

    #[test]
    fn refuses_removable_media_unless_it_is_named() {
        let t = Target {
            disk: "/dev/sdc".into(),
            size_bytes: 64 * GIB,
            removable: true,
            ..Default::default()
        };
        assert_eq!(refusals(&t), vec![Refusal::RemovableNeedsConfirmation]);
        let named = Target {
            named_explicitly: true,
            ..t
        };
        assert!(refusals(&named).is_empty());
    }

    #[test]
    fn a_good_target_is_not_refused() {
        let t = Target {
            disk: "/dev/vdb".into(),
            size_bytes: 64 * GIB,
            ..Default::default()
        };
        assert!(refusals(&t).is_empty());
    }

    #[test]
    fn every_refusal_says_what_to_do() {
        for r in [
            Refusal::HoldsRunningSystem,
            Refusal::MountedPartition("/dev/vdb1".into()),
            Refusal::TooSmall {
                bytes: 1,
                need: MIN_DISK_BYTES,
            },
            Refusal::RemovableNeedsConfirmation,
        ] {
            let s = r.reason();
            assert!(!s.is_empty());
            // a refusal is a sentence, not a code
            assert!(s.len() > 20, "{s}");
        }
    }
}

/// The sfdisk input that lays the plan out, one line per partition, in the order sfdisk expects.
///
/// Pure so it can be tested without a disk: the moment this touches a device it is a command, not a function.
/// `label: gpt` first, then `start=…, size=…, type=…, name=…` for each partition. Sizes are in sectors (512 bytes),
/// which is what sfdisk reads — producing MiB/GiB here would be a silent off-by-a-factor.
pub fn sfdisk_script(plan: &[Partition]) -> String {
    let mut out = String::from("label: gpt\n");
    for p in plan {
        let start_s = p.start_bytes / 512;
        let size_s = p.size_bytes / 512;
        out.push_str(&format!(
            "start={start_s}, size={size_s}, type={}, name={}\n",
            p.type_guid, p.name
        ));
    }
    out
}

/// Every destructive step an install performs, in order, so the main binary can say *which* step failed rather
/// than "it failed". A silent "install failed" is how a person spends an evening guessing whether the disk was
/// partitioned, formatted, or half-written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Repartition,
    MakeEsp,
    MakeData,
    MakeRootA,
    MakeRootB,
    WriteEspTree,
    InstallBootEntry,
    WriteSlotA,
    WriteSlotB,
}

impl InstallStep {
    pub fn label(self) -> &'static str {
        match self {
            InstallStep::Repartition => "sfdisk (partition the disk)",
            InstallStep::MakeEsp => "mkfs.vfat on the ESP",
            InstallStep::MakeData => "mkfs.ext4 on data",
            InstallStep::MakeRootA => "mkfs.ext4 on root-a",
            InstallStep::MakeRootB => "mkfs.ext4 on root-b",
            InstallStep::WriteEspTree => "copy the boot tree to the ESP",
            InstallStep::InstallBootEntry => "bootctl install",
            InstallStep::WriteSlotA => "write slot A",
            InstallStep::WriteSlotB => "write slot B",
        }
    }

    /// The order, as a value, so the tests and the runner share one list rather than two that can drift.
    pub fn all() -> [InstallStep; 9] {
        [
            InstallStep::Repartition,
            InstallStep::MakeEsp,
            InstallStep::MakeData,
            InstallStep::MakeRootA,
            InstallStep::MakeRootB,
            InstallStep::WriteEspTree,
            InstallStep::InstallBootEntry,
            InstallStep::WriteSlotA,
            InstallStep::WriteSlotB,
        ]
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;

    #[test]
    fn the_sfdisk_script_is_in_sectors_and_covers_the_disk() {
        let p = plan(64 * GIB);
        let script = sfdisk_script(&p);
        // first line labels the table GPT
        assert!(script.starts_with("label: gpt\n"), "{script}");
        // every partition line is size=… and the sizes sum back to the disk (in sectors)
        let mut sectors: u64 = 0;
        for (i, part) in p.iter().enumerate() {
            let line = script.lines().nth(i + 1).unwrap();
            assert!(line.contains(&format!("name={}", part.name)), "{line}");
            let size_s = line
                .split(", ")
                .find_map(|kv| kv.strip_prefix("size=").and_then(|v| v.parse::<u64>().ok()))
                .unwrap();
            sectors += size_s;
        }
        // The partitions start at 1 MiB, the GPT alignment gap, so they cover from there to the END of the disk:
        // the sum of their sizes is `disk - 1 MiB`, not `disk`. Asserting `== 64 GiB` was the bug the first version
        // of this test carried, and the 1-MiB shortfall it reported was the script being RIGHT.
        assert_eq!(sectors * 512, 64 * GIB - MIB);
        assert_eq!(p[0].start_bytes, MIB);
    }

    #[test]
    fn the_steps_name_every_failure_point() {
        let all = InstallStep::all();
        assert_eq!(all.len(), 9);
        // no two labels the same, and every label says what it is
        let labels: std::collections::HashSet<_> = all.iter().map(|s| s.label()).collect();
        assert_eq!(labels.len(), all.len());
        for s in all {
            assert!(s.label().len() > 10, "{}", s.label());
        }
    }

    #[test]
    fn the_first_and_last_steps_are_the_irreversible_edges() {
        let all = InstallStep::all();
        assert_eq!(all[0], InstallStep::Repartition);
        assert_eq!(all[all.len() - 1], InstallStep::WriteSlotB);
    }
}
