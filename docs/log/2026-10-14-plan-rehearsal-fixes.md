# 2026-10-14 · Claude Code · A handoff rehearsal, and what it found

A fresh agent was given only a clone of the repository and "You are the builder now. Start."
It oriented in about 22 files and 2,800 lines, chose correctly (ask Rob one question; its
recommended next step, S3's release numbers, is what #68 does), and listed 29 points of
friction. Fixed here:

- **Picking work.** HANDOFF's "lowest-numbered open task" contradicted doc 20's order of work;
  "work on something else" contradicted "do not pick work for yourself". Now one rule: ask once,
  and while waiting only `fix/` and `plan/` work. `verify-plan.mjs` now refuses a branch whose
  task may not start (needs not done, or a later milestone not marked parallel) and says why;
  `task.mjs --next` uses the same rule. E11 is `blocked` (on S5), the status that existed unused.
- **HANDOFF's open-PR table** was stale within hours: replaced by "look at GitHub", and "merge
  or base on an open `plan/` PR that changes the rules".
- **check.sh** fetches main before judging, refuses a detached HEAD with the fix, counts an
  uncommitted log file, and runs CI's compositor-frame step it lacked.
- **AGENTS.md:** what HANDOFF holds, the reading order, ADR 0014's order beside ADR 0003's
  wedge, `check.sh` as the bar, the allowed branch types, H4 and H5 noted as since fixed.
- **Doc 20:** M1's order of work says steps 1 to 3 are done; E4's cache path; E6 says
  `verify-docs.mjs` has no self-test yet and how to find a CI run; where a re-anchor goes.
- CI's `rust` job pinned to `ubuntu-24.04`; verify-docs' stale header.

Not fixed (left as notes): harness-mesh's MACHINES.md is private (HANDOFF now says so).
