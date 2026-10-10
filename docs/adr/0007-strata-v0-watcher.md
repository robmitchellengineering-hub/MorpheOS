# 0007 · Strata v0: a watcher and a content-addressed store, not FUSE

**Status:** Accepted, 2026-10-10. Supersedes the "v0" line of doc 04's implementation path.

## Context
Doc 04 planned Strata v0 as a FUSE layer over btrfs or bcachefs. Phase 1
(ADR 0003) needs one thing from Strata first: **you never lose a file**, whether
by overwriting, deleting or ransomware. It needs that on the Pi's SD card, the
x86 image's ext4, and a user's existing disks.

## Decision
- **Watch, don't intercept.** An inotify watcher records a version whenever a
  file is closed after writing, moved or deleted. No filesystem sits in the I/O
  path, so there's no FUSE overhead, and it works on any Linux filesystem.
- **Own the history.** Versions go into a content-addressed store (FastCDC
  chunks, BLAKE3 names, zstd) plus a SQLite index. That is the object model of
  doc 04, so v1 can keep the store and replace only how changes are captured.
- **Rewind never destroys.** The current state is snapshotted before anything is
  changed, and files that did not exist at the rewind point go to quarantine.
  A rewind can be undone by rewinding to the moment before it.
- **The store is private** (mode 0700, outside every watched folder). Software that
  can write your files must not be able to rewrite their history.
- **Ransomware detection** by entropy bursts, with the rewind point in the alert.

## Consequences
- Known v0 limits, all documented in `crates/strata`:
  - A file written and deleted faster than it can be read is recorded only as deleted.
  - Writes through a memory map or a long-open file are captured at close.
  - The watcher sees what changed, not who changed it, so v0 can **detect and
    advise** but cannot freeze the attacker. Freezing needs fanotify (which reports the
    writer's PID) and lands with the heal engine.
  - Ransomware that gives files random names is caught only through in-place rewrites.
- FUSE or a native layer (doc 04 v1) is still the route to versioning *every*
  write, and to views. That waits until measurements show the watcher isn't enough.
- Proven by `crates/strata/tests/break_it.rs`: edits, deletes and a simulated
  ransomware attack (in place and replace patterns) are detected and rewound byte
  for byte, including permissions. The rewind is then undone and redone.
