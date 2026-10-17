# 2026-10-17 · DeepSeek · S4 recovery image — increment 1: the recovery system boots

First increment of S4 (marked parallel by Rob). This builds the recovery partition and boot
entry, and proves the recovery target boots instead of the shell — before anything writes.

- build.sh: partition 5 `recovery` (same size as a slot, factory root = slot A minus the
  boot services plus `morph-recover`), its kernel/initrd under `esp/morpheos/recovery/`, and
  `loader/entries/morpheos-recovery.conf` (`root=PARTLABEL=recovery morph.recovery=1
  systemd.unit=morph-recovery.target`).
- overlay-recovery: `morph-recovery.target` + `morph-recover.service`. The service, for now,
  only prints "morpheos: recovery booted" and powers off — the rebuild (morph-heal recover)
  and the initramfs "way in" are the next increments.
- The recovery root is assembled **before** the ESP, so its kernel/initrd actually reach
  `esp/morpheos/recovery/` (first cut assembled it after the ESP was written, so the entry's
  `cp` found nothing and the build "passed" with no recovery kernel; now asserted).
- The recovery root gets its **own fstab** (ESP + tmpfs only): it must not mount `/persist`
  or its `/home` and Strata bind mounts — that is the shared data recovery leaves untouched.
  The swap is asserted so a recovery that mounts data cannot ship silently.
- The recovery partition is built only for the person-facing images (`HARDWARE=1`): the
  self-test scenarios don't use it and each disk carries a whole extra slot (S4 note 6).

Next increments: the morph-boot recovery policy, `morph-heal recover` (hash + rebuild both
slots), the initramfs hook that arms the recovery entry when a slot is lost, and
`test-recovery.sh`.
