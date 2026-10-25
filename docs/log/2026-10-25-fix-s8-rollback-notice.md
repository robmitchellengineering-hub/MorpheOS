# 2026-10-25 · DeepSeek · fix S8: the rollback notice never shows (ordering gap)

After #151 (morphd enabled) and #153 (the notice is `Type=notify`, READY after its first
frame), the config-rollback scenario has exactly one failing check left: the harness never sees
`flow-notice: shown`, so `the notice never showed` and `the notice names generation 2` both
FAIL. The rollback itself is green — `heal.jsonl` (`{rung:H2,bad:2,…}`) is written, the hostname
reverts to generation 1, and `current` reads `Fast link dest: "generations/0001.json"`.

## Root cause — the notice file is not there when the condition is checked

Three suspected causes were read and ruled out or confirmed, in order:

1. **Not enabled.** False. `images/x86-qemu/build.sh`'s shell loop is
   `for unit in seatd flow-shell flow-notice flow-rollback-notice $apps` — `flow-rollback-notice`
   is enabled, exactly as `flow-notice` (the S7 space notice, which works) is.

2. **Staging / ordering.** Confirmed. `morphd boot` stages the file
   (`stage_rollback_notice` writes `/run/morpheos/rollback-notice`, and the branch that calls it
   runs — `rolled_back_from` is `Some(2)` because the heal record is written). The path matches
   `ConditionPathExists=/run/morpheos/rollback-notice`, and `morphd.service` has **no** sandboxing
   (`PrivateTmp`/`ProtectSystem`). The gap is **ordering**: `morphd.service` is
   `Before=strata.service` and the notice unit is `After=flow-shell.service` — the two are
   unrelated, so systemd gives no guarantee that morphd has written the file before the notice
   unit's condition is evaluated. On the run that failed, the shell came up before morphd
   finished, the condition found no file, and the unit was **skipped** — a skipped unit has no
   failed state and no console line, so the serial shows nothing (exactly what the log shows:
   the shell is ready, and there is no `flow-rollback-notice.service` line at all).

3. **The notice client freezing before "shown".** False. `crates/flow-notice/src/main.rs` draws
   and announces on its **first** `xdg_surface.configure` — it does **not** request or wait for a
   `wl_surface.frame` callback (H17 does not apply). The compositor sends that configure at
   `new_toplevel`, and the S7 space notice proves the whole client+compositor path reaches
   `flow-notice: shown`.

A skipped notice unit also defeats #153's fix: `is-system-running --wait` only waits for a unit
that is *starting*. A skipped unit is not starting, so `is-system-running` reports "running"
immediately and the boot-time test driver (`morph-boot-check` → `test-hook`) powers the machine
off before any notice can appear.

## Fix

`images/x86-qemu/overlay-shell/usr/lib/systemd/system/flow-rollback-notice.service` now orders
itself after the service that stages its file:

```ini
After=flow-shell.service morphd.service
```

`morphd.service` is `Type=oneshot`, so `After=morphd.service` means the notice unit (and its
`ConditionPathExists`) is only reached once `morphd boot` has exited — after
`stage_rollback_notice` has written `/run/morpheos/rollback-notice`. The notice can no longer be
skipped because the file is not there yet.

## Verified

- `cargo test -p morphd` — the morphd and heal suites are green (including
  `a_manifest_with_the_wrong_digest_rolls_back`, the exact run-2 path).
- `cargo check -p flow-notice` — parses and type-checks cleanly (on macOS the Linux-only client
  is gated; it is the unchanged code #153 already type-checked in the Linux CI job).
- The `.service` edit is a systemd-unit change only (`After=` list + a comment); no shell script
  changed, so no `bash -n` is needed.

**Not proven yet:** the QEMU scenario (`images/x86-qemu/test-config-rollback.sh`) still has to run
on Linux to confirm the boot now shows the notice. That is the `images` workflow's job. S8 is not
marked done here.
