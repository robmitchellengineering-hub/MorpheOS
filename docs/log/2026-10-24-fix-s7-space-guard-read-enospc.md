# 2026-10-24 · DeepSeek · S7: the space guard reads the index on a full disk

PR #143 made the guard delete-first, but the full-disk run still died with
`space-guard: strata guard failed: strata: version index: disk I/O error` — one layer deeper than
the last fix. The write that records a prune was no longer first; the **read** was, and the read
failed.

**The exact failing call.** `Index::open` (`crates/strata/src/index.rs`) opened the index with
rusqlite `Connection::open` and then `PRAGMA journal_mode=WAL`. A WAL-mode database needs its
`-wal`/`-shm` sidecars even for a *read*: they have to be created, and — after an unclean stop
(the watcher is SIGTERMed at reboot, leaving a stale wal-index) — recovered. Both are writes, and
on a full disk that write fails before any SELECT returns. So `prune_plan` → `all_versions()` (and
`collect` → `file_contents()`) could never run, and the guard could not work out what to prune.
Reproduced on a real full medium (an 8 MiB HFS+ image filled to 0 bytes free): `strata guard`
failed with `version index: unable to open database file` / `disk I/O error` depending on whether
the sidecars were absent (clean close) or stale (unclean stop); a read-only connection failed the
same way, because it too needs the sidecars to exist.

**The fix.** The index now uses a rollback journal (`PRAGMA journal_mode=DELETE`), not WAL. A
rollback journal is only touched by a *write*, so the guard's planning reads need nothing next to
`index.sqlite` and work on a full disk. The object deletes that free space (`collect`) still run
first, and `record_prune` stays best-effort — if the index write is refused the guard still
reports the space it recovered instead of failing the boot (#143's shape is unchanged). The
watcher's "safe to kill at any time" note is still true: a rollback journal recovers a
half-written transaction on open, exactly as WAL did. The 10 s busy timeout stays, so a concurrent
`strata watch` write makes a `Files` read wait briefly instead of failing (doc 20, S5, updated).

**Tests** (two new; the four existing space-guard tests pass unchanged in intent):
- `index::reads_survive_a_read_only_directory` — a read-only directory stands in for the full
  medium (SQLite's own "read-only medium" case): `Index::open`, `all_versions` and `file_contents`
  must all succeed with nothing writable next to `index.sqlite`. Under WAL this test fails; with
  the rollback journal it passes.
- `guard::frees_space_when_the_index_directory_cannot_be_written` — the store root is made
  read-only (the object folders stay writable), and `guard` must still recover space: the objects
  of the pruned versions are gone, the newest version still restores, and the refused index write
  reports `versions_removed == 0` without failing the guard.

**Verified.** `ROBMESH_ALLOW_MAC=1 cargo test -p strata`: 36 passed / 0 failed / 3 ignored (the
three watcher tests, off Linux). `cargo fmt --check` and `cargo clippy -p strata --all-targets`
clean. The fix was also proven end-to-end on a real full disk: `strata guard` and `strata stats`
succeed with 0 bytes free, where before the fix they died with the index error. The QEMU
full-disk scenario itself remains to be proven by the `images` workflow (PR 2 of S7).
