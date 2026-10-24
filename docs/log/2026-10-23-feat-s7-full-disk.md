# 2026-10-23 · DeepSeek · S7 full disk — PR 1: the space guard

PR 1 of S7: Strata survives a full disk instead of crash-looping on it. Two things, both with
unit tests.

- `crates/strata` gains a space guard (`guard.rs` + `strata guard`): it reads free space on
  the store's filesystem with `statvfs` and, below the Manifest's `storage.min_free_percent`,
  runs the same `prune` a person runs — so it never deletes more than the retention policy
  allows and always keeps the newest version of every file. Object writes now go through a
  single `write_atomic` (temp file, fsync, rename, remove-on-failure), so a write that hits
  ENOSPC publishes nothing and leaves the store readable.
- `crates/morphd` gains the `MinFreePercent` applier, writing
  `/persist/morph/strata-min-free.json` (the same `apply` / `read_back` / `undo` contract as
  retention), and applies + health-checks it alongside hostname and retention.
- `crates/morph-manifest`: `/storage/min_free_percent` leaves `unenforced_settings()` — the
  machine now obeys it (AGENTS.md H5).

Tests: below the floor prunes and reports bytes freed, at/above the floor does nothing, and a
write that cannot complete leaves the store readable. Local strata lib tests: 32 passed. The
full Linux bar (`scripts/check.sh`) runs on the mesh, not the Mac (H9/H16).

Next (PR 2): `flow-notice` and `test-full-disk.sh`, which wire the guard into the image and
prove the whole scenario in a VM.
