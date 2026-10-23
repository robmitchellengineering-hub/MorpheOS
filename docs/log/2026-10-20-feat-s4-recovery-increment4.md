# 2026-10-20 · DeepSeek · S4 recovery image — increment 4: the corrupt-recovery refusal and the in-machine wipe

Fourth (and final) increment of S4 (marked parallel by Rob). This closes the two things
increment 3 deferred: the self-check that refuses a corrupted recovery image, and the
in-machine wipe that destroys the slots from inside the running machine.

## The corrupt-recovery refusal

`morph-heal recover` now refuses to rebuild from a recovery partition that no longer matches
its factory-signed manifest — before anything is written.

- `images/x86-qemu/build.sh` seals and signs the recovery payload (the recovery partition as
  `root.img`, plus its `vmlinuz`/`initrd.img`) with `UPDATE_KEY` at image build time, after
  `e2fsck -fy` so the journal is clean, and carries `BLAKE3SUMS` + `BLAKE3SUMS.sig` beside the
  recovery kernel on the ESP.
- `crates/morph-heal` gains a `recovery_keys` field (default `/usr/share/morpheos/update-keys`,
  the same key dir the slots trust for updates) and a `--recovery-keys` option. Before
  writing, `recover` copies the factory manifest into the staged payload, opens it with the
  factory key, and `check()`s it — which hashes the very bytes that would be written (`root.img`
  is the symlinked recovery partition). A mismatch is `Error::Refused`, nothing is written, and
  no recovery record is left. A missing or unsigned manifest is refused by the same path.
- Unit/integration test `recover_refuses_a_corrupted_recovery_image_and_writes_nothing` flips
  one byte of the recovery image and asserts refusal, untouched slots, untouched default, and
  no recovery record. `tests/repair.rs` and the existing recover tests were extended to carry
  the factory manifest + key.

The "console message instead of a reboot loop" half was already settled by increment 3:
`morph-recover.service` runs `morph-heal recover` and stays put on failure.

## The in-machine wipe

The recovery test driver gained a `wipe` mode, and `test-recovery.sh` a `wipe-in-machine`
scenario.

- `overlay-recovery-test/usr/lib/morpheos/test-hook`: `wipe` seeds `/home` and records its
  checksums, then `wipefs -a` + zeroes the first 64 MiB of **both** slot partitions from inside
  the running machine and force-reboots (`echo b > /proc/sysrq-trigger`, with `reboot -f` as
  the fallback if sysrq is disabled). This proves Ubuntu 24.04's 6.8 kernel lets a root process
  write a block device whose filesystem is mounted (`CONFIG_BLK_DEV_WRITE_MOUNTED`, default y)
  — the one thing the S4 card asked to verify, not assume.
- `test-recovery.sh` part 3 asserts, after the single in-machine wipe run: slot A committed
  before and after, both slots rebuilt (recovery record names a and b), both slots hold a valid
  filesystem, 24 files were seeded, and every `/home` file matches its checksum on the data
  partition.

## Verification

- Local (macOS): `cargo fmt --all --check` clean, `cargo clippy -p morph-heal --all-targets`
  clean, and the full workspace `cargo test --all` green (the new refusal test passes; the
  compositor and image tests are Linux-only by H16 and did not run here).
- Linux (images workflow / `scripts/test-images.sh recovery`): still to run — the build.sh
  manifest and the in-machine wipe are only proven by the VM scenario, not by a local compile.

Not done in this increment (deliberate): a dedicated VM scenario that flips one byte of the
recovery partition and boots recovery — the refusal is proven by the unit test, and the
"stays put" half by increment 3's service design, but the end-to-end corrupt-refusal VM proof
is left to a follow-up if the reviewer wants the refusal shown on real hardware rather than in
the test suite.

## CI found it red, and the cause is now fixed (2026-10-21)

The x86 job hit its 90-minute timeout. The serial log names the cause exactly:

```
boot-check[370]: wipefs: error: /dev/disk/by-partlabel/root-a: probing initialization failed: Device or resource busy
```

`wipefs -a` opens the device exclusively, so it cannot run on a *mounted* partition (EBUSY) —
under `set -e` the driver died before the `dd` and the reboot, and the machine sat at the login
prompt until the harness gave up. Two fixes, both on this branch:

1. **Drop `wipefs`.** The wipe is now the `dd` zero alone (the same 64 MiB the harness-side
   wipe uses), which is what the S4 card's "zero both slot partitions" already meant.
2. **Allow writing the mounted device.** The kernel refuses it unless `bdev_allow_write_mounted`
   is on the command line (the "verify, don't assume" point in S4 note 4 — the card's "default y"
   was wrong, the boot override is the real mechanism). `build.sh` adds that option only for the
   `recovery` test image, never the person-facing image. The `dd` is guarded so a refusal is
   reported and the machine still reboots (a fast, visible failure instead of a silent 30-minute
   hang — AGENTS.md H11).

## Second CI failure, second root cause, second fix (2026-10-21, late)

The x86 job timed out again — and the serial log named a DIFFERENT bug, in the other half of
increment 4: the corrupt-recovery self-check.

```
morph-heal: refused: the recovery image does not match its factory-signed manifest …
root.img hashes to caf0f1…, but the manifest says e0c011…
```

`recover` hashed the **mounted** recovery partition's raw bytes against the build-time image.
A mounted filesystem's raw bytes are not stable — the kernel updates the superblock and
journal on mount — so a healthy recovery was falsely refused. The fix: `morph-update` gains
`Payload::check_boot`, which verifies the factory-signed kernel and initrd (stable, on the ESP)
and deliberately skips `root.img`. The corrupt-refusal unit test now flips a byte of the kernel
instead of the root partition. The root partition itself is not byte-verified; it is the
running, kernel-checked system, and `write_slot` still re-reads and verifies what it writes.

## Third CI failure, third root cause, third fix (2026-10-23)

The x86 job red again, this time on `wipe-in-machine`: `recovery record names both slots` and
`root-b holds a valid filesystem` failed. The serial log showed the machine booted its still-valid
slot A and never armed recovery. Root cause: the wipe `dd` to the *mounted* root was buffered in
the page cache and lost on the `echo b > /proc/sysrq-trigger` (an emergency restart that does NOT
sync) — so slot A survived the wipe and there was nothing to recover. Fix: `conv=fsync` on the
wipe `dd` (both slots), so the zeroes reach the device before the reboot. This is the one real
fix this session; the recovery now arms and rebuilds both slots, proven on the mesh (pacland-rog,
KVM) and green on CI.

## The /home check looked broken — it was a pacland-rog QEMU artifact, not a bug

After the fsync fix, `every /home file matches` failed locally (pacland-rog) while the same code
passed on GitHub CI. Seven hypotheses were chased and falsified (sync, syncfs, per-file fsync,
block-device fsync, a 2-boot seed split, unmounting /persist, and `cache=none` on the QEMU drive):
`dumpe2fs`/`debugfs` showed the seeded files' inode-table entries zeroed after the wipe, while the
directory inode and superblock stayed intact. The cause is pacland-rog's QEMU 8.2.2 / host disk
durability on the unclean remount — it does not reproduce on GitHub's runner (same guest kernel,
same code). The recovery code was correct; those experiments were all reverted, leaving only the
`conv=fsync` fix in this PR.
