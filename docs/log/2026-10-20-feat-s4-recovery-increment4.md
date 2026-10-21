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
