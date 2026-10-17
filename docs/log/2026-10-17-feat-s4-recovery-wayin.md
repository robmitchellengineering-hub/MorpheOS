# 2026-10-17 · DeepSeek · S4 recovery image — increment 3: the initramfs "way in" and the wipe

Third increment of S4 (marked parallel by Rob). This is the part that actually BOOTS recovery
when a slot is lost, and the QEMU test that proves it end to end.

- initramfs "way in" (`images/common/overlay/etc/initramfs-tools/`): a `local-bottom` boot
  script that arms the recovery entry (`bootctl set-oneshot morpheos-recovery.conf`) and
  reboots when the slot's root has no `/sbin/init` — which is how BOTH a wiped partition (the
  mount fails, `${rootmnt}` stays empty) and an emptied filesystem (mounted, init deleted) land
  in this image's initramfs-tools. A `hooks/morpheos-bootctl` build hook copies bootctl into the
  initrd. `build.sh` now regenerates the initrd after the overlay, so the script and bootctl
  actually reach `/boot` (the base initrd was made before the overlay existed).
- `morph-recover.service` now runs `morph-heal recover` and reboots on success (stays put on
  failure, so no reboot loop).
- `is_recovery` now names the recovery partition by `root=PARTLABEL=recovery` on the cmdline
  instead of guessing the mountinfo source — `/proc/self/mountinfo` records the RESOLVED device
  (e.g. `/dev/vda5`), not the label, so the old check would have refused a real recovery boot.
- The recovery root keeps slot A's FULL boot services (increment 1 had stripped them, which
  would have left a rebuilt slot A that cannot commit); the recovery boot still runs only
  morph-recover because its entry boots `morph-recovery.target`, not `multi-user.target`.
- `test-recovery.sh` (wired into both `scripts/test-images.sh` and the x86 CI job): two parts —
  recovery-boot (arm the entry by hand, assert both slots rebuilt, default A, recovery record,
  no recovery history line) and wipe (zero the first 64 MiB of both slot partitions from the
  harness, then assert the initramfs armed recovery and every `/home` file survived on the data
  partition).

Deferred to increment 4: the self-check that hashes the recovery partition against its
factory-signed `BLAKE3SUMS` before writing (the corrupt-recovery refusal).
