# 2026-10-20 · DeepSeek · S11 morphd — increment 1: the hostname applier

First increment of S11 (the Manifest is obeyed: `morphd` v0). Rob's direction this session:
the daily-driver strand has been retarding development, so the builder moves to the deep OS
work. S11 is the highest-value next piece — it makes the Manifest actually drive the machine,
and it unblocks S7 and S8.

Rob on S11: "continue at will" — open S11 and mark it parallel, and proceed.
DSH on S11: opened (proposed → open) and marked parallel, per Rob's "continue at will".

This increment builds `crates/morphd` with the first settings applier: the machine hostname.

- `Applier` (lib.rs): the contract every setting obeys — `apply`, `read_back`, `undo`. A
  change is a pair of values; an applier puts the new one in force, reads the value that is
  actually in force (so a health check compares real state, not intent), and puts the previous
  one back when the change is refused or rolled back.
- `Hostname` (hostname.rs): applies the Manifest's `machine.name` to the **kernel** hostname
  (`/proc/sys/kernel/hostname`), never `/etc/hostname` — that file lives in the read-only root
  and the image writes `morpheos-<slot>` there (S11 note 3). It refuses a name longer than the
  kernel's 64-byte limit before writing, which is the "real reason" failure a later test uses
  for a rollback.
- Unit tests drive a fake hostname system (an in-memory name, injectable failure) so they never
  touch `/proc`: apply-then-read-back, undo restores the previous name, a name over the limit
  is refused, and a failed apply is undone.

Next increments: the retention applier + Strata's policy file (PR 2); the `morphd` service,
the seeded generation 1 and `test-manifest.sh` (PR 3).
