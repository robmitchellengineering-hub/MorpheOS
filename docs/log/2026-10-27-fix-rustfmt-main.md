# 2026-10-27 · DeepSeek · rustfmt the rollback-notice write, and unblock main's `rust` job

`ci`'s `rust` job has been red on `main` since `ce0bfc4` on a single `cargo fmt --all --check`
diff: `crates/morphd/src/main.rs:159` splits the `std::fs::write(...).map_err(...)` chain over
two lines where the pinned rustfmt (1.99.0, `rust-toolchain.toml`) wants one. Formatting only,
no behaviour change. Verified on the pinned toolchain: `cargo fmt --all --check` clean and
`cargo check --all-targets` exits 0 on this tree.

Found on the way, worth the next person's time:

- **`scripts/verify-pr.sh` requires every pull request to add a file here.** Without one the
  `rust` job fails at "the branch adds no session-log file", which reads like a code failure and
  is not one. This branch's first push did exactly that.
- **`main`'s other required check is red for an unrelated, repeatable reason.** "Try to Break It:
  A/B updates (x86 VM)" fails in `images/x86-qemu/test-full-disk.sh`: every full-disk assertion
  prints `ok`, then the step dies with `BrokenPipeError` inside the QMP screendump loop
  (`q.cmd("screendump")` on a socket QEMU had already closed). Reproduced on two separate runs
  (runs on `8e8d039` and on this PR), so it is not a flake. Whether the notice's warning band
  drew is **unknown** — the loop crashed before it printed a verdict.

Next: the full-disk capture must be taken while the guest is still up rather than after the
`flow-notice: shown` wait, and a closed QMP socket must end the check with a named verdict
instead of a traceback.
