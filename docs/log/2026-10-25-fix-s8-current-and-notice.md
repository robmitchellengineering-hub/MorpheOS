# 2026-10-25 · DeepSeek · fix S8's last two failures: the `current` read and the notice race

After the morphd-enable fix (#151) the config-rollback rollback itself works — `heal.jsonl` is
written (`{rung:H2,bad:2,…}`) and the hostname reverts to generation 1. Two checks still
failed, and each has a different root cause; neither is in the rollback logic.

## Failure 1 — `the store's current names generation 1 (got: )`

**Root cause: a harness assertion against an assumed spelling, not a missing symlink.**

`Store::rollback` → `point_current_at` *does* persist `current` at `/persist/morph/current`
(on-disk `/morph/current`) as a fast symlink to `generations/0001.json`, and it runs before
`append_heal` writes `heal.jsonl` — so if `heal.jsonl` is on the disk, the symlink is too.
Confirmed by reading `crates/morph-manifest/src/store.rs` and by the passing
`morphd` heal tests (a digest mismatch rolls back through exactly this path).

The test read the symlink with `debugfs -R "stat /morph/current"` and grepped for
`Fast_link_dest: generations/0001.json`. e2fsprogs 1.47.0 (Ubuntu 24.04, the images job's OS)
prints that field with a **space and quotes** — `Fast link dest: "generations/0001.json"` — so
the underscore spelling never matched, the `got:` capture came back empty, and the check failed
while the symlink sat right there. (Measured on the mesh's `studio-linux`, `debugfs 1.47.0`;
`readlink` is not a debugfs command in this version, so the fix is a corrected `stat` parse,
not a different command.)

**Fix:** `images/x86-qemu/test-config-rollback.sh` now asserts the real output —
`Fast link dest: .*generations/0001.json` — and shows the real line in the `got:` message. The
assertion is not weakened: it still fails on a missing symlink, on a symlink pointing at
generation 2, and on a non-symlink `current`.

## Failure 2 — `the notice never showed` / `the notice names generation 2`

**Root cause: a ~100 ms boot race, not enablement or staging.**

All three things the failure report suspected are correct: `flow-rollback-notice.service` *is*
enabled (the `for unit in seatd flow-shell flow-notice flow-rollback-notice …` loop), its
`ConditionPathExists=/run/morpheos/rollback-notice` matches the path
`stage_rollback_notice` writes, and `stage_rollback_notice` *does* run (the rollback ran, so
the branch that stages the notice ran). None of those is the bug.

The bug is the same one the S7 space notice shares, and the serial log shows it: on KVM the
boot reaches the notice unit in ~2.7 s, `is-system-running --wait` returns "running" the moment
the `Type=simple` notice unit has *spawned* `flow-notice`, and the boot-time test driver
(`morph-boot-check` → `test-hook`) powers the machine off ~100 ms later — before `flow-notice`
finishes its Wayland handshake and prints `flow-notice: shown`. The notice never gets its first
frame to the screen, so the harness sees no `shown` line.

**Fix:** `flow-notice` now sends `sd_notify(READY=1)` after its first frame is on the screen
(the same `notify_ready` the compositor uses), and both `flow-notice.service` and
`flow-rollback-notice.service` are `Type=notify`. READY now means *shown*, not *spawned* (H19),
so `is-system-running` cannot report "running" — and the boot-time driver cannot power off —
until the notice is actually on the screen.

## Verified

- `bash -n` on `images/x86-qemu/test-config-rollback.sh`.
- The morph-manifest and morphd test suites are green (10 morphd unit tests, 4 heal tests,
  including the digest-mismatch rollback that exercises `point_current_at`).
- `flow-notice` parses and checks cleanly on this machine; its Linux-only `client` code (where
  `notify_ready` lives) is type-checked by the Linux CI job, and the added `notify_ready` is a
  verbatim copy of the compositor's proven one.

**Not proven yet:** the QEMU scenario (`images/x86-qemu/test-config-rollback.sh`) still has to
run on Linux to confirm the boot now shows the notice and the corrected `current` assertion
passes. That is the images workflow's job, not a local compile.
