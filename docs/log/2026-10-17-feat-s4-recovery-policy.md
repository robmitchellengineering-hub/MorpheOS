# 2026-10-17 · DeepSeek · S4 recovery image — increment 2: the recovery policy and morph-heal recover

Second increment of S4 (marked parallel by Rob). This adds the one policy question the
recovery system must answer before it writes, and the command that rebuilds both slots.

- `morph-boot`: `is_recovery(cmdline, mountinfo)` — a boot is the recovery system only when
  `morph.recovery=1` is on the command line **and** `/` is mounted from the recovery
  partition (from `/proc/self/mountinfo`). Either alone is forgeable; the pair stops a slot
  that resembles recovery from overwriting the slot it runs from. Unit-tested.
- `morph-update`: `write_slot(payload, layout, slot)` — the mechanical write-and-verify of a
  named slot, extracted from `install` with every policy decision removed. `install` keeps
  `update_target` + release-age + incomplete-marker checks, then calls it. No `--force` flag
  was added (H7): the "never write the running system" invariant stays in `install`.
- `morph-heal`: `recover` — refuses unless `is_recovery`; refuses if a target is the recovery
  partition; stages the recovery partition as `root.img` (referenced, not copied), seals and
  signs with a per-run key, writes A then B via `write_slot`, sets the default to A, and
  writes an ESP recovery record naming both slots.

Deferred to increment 4 (per the plan's PR split): the self-check that hashes the recovery
partition against its factory-signed `BLAKE3SUMS` before writing (the corrupt-recovery
refusal). The initramfs "way in" and `test-recovery.sh` are increment 3.
