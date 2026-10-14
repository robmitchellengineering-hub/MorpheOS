# 2026-10-14 · Claude Code · Guardrails for the next builder

Every mistake made on 2026-10-14 now has a check, so the builder from 2026-10-16 meets it in
seconds, not 25 minutes into a VM run:

- The session log is one file per pull request in `docs/log/` (the old entries are in
  `2026-10-13-to-14-history.md`). Every merge that day conflicted in HANDOFF.md's single log.
- `scripts/verify-pr.sh` (CI and `check.sh`): a pull request adds its own log file, and every
  `images/x86-qemu/test-*.sh` is run by both the images workflow and `scripts/test-images.sh`.
- `crates/flow-wifi/tests/colours.rs`: every colour drawn on the screen is distinct within the
  tests' tolerance. Writing it found three clashes (the shell's background and the picker's
  password field among them); the picker's background and field and the shell's intent bar moved.
- A pull-request template with the checklist; AGENTS.md H20 (colours) and H21 (embedded Python).

Next: a handoff rehearsal, a fresh agent given only the repository, and fix what it stumbles on.
