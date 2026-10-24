# 2026-10-24 · DeepSeek · S7: the space guard recovers a full disk instead of dying on its own index write

The `feat/s7-full-disk-image` full-disk run booted `degraded` with
`space-guard: strata guard failed: strata: version index: disk I/O error`, and the store was never
pruned, so free space stayed at 1% (below the 10% floor).

**The failing operation.** `guard` → `Strata::prune` ran, in order: `all_versions` (a read),
`index.delete` — a `DELETE … FROM versions` committed to the SQLite WAL, the one step that **writes**
the index — then `referenced_contents` (a read) and `collect_garbage` (the object deletes that
actually free space). On a full disk the WAL append has no room, so the `index.delete` write failed
with `disk I/O error` *before* anything was freed. The guard's whole job is to recover a full disk,
but the recovery itself needed free space to record itself.

**The fix** — delete-first, then record:
- `Strata::prune_plan` lists the doomed version ids from a read (nothing written).
- `Strata::collect` deletes the objects only the doomed versions refer to, using a new
  `Index::file_contents` read instead of a post-delete read — so it frees space without writing the
  index.
- `Strata::record_prune` removes the doomed rows (the only index write), after the deletes.
- `prune` keeps its signature and now runs plan → collect → record.
- `guard` calls the three steps separately and, if the record write is still refused after the
  deletes, reports the recovered space (objects/bytes freed) instead of failing the whole boot.

**Tests** (all on a normal filesystem — no real ENOSPC):
- `guard::frees_space_even_when_the_index_refuses_the_record` — a `BEFORE DELETE` trigger stands in
  for a full medium; the guard still recovers the space, keeps the newest version, and returns `Ok`
  with `versions_removed == 0`.
- `guard::prune_frees_the_objects_before_it_records` — the same refusal through `prune` proves the
  CLI path also deletes the objects before the index write.

Verified on macOS (partial — H16): `cargo test -p strata` is 34 passed / 0 failed / 3 ignored (the
three watcher tests, off Linux), and `cargo fmt --check` and `cargo clippy -p strata --all-targets`
are clean. The full Linux bar (`scripts/check.sh`) is CI's, and the QEMU full-disk scenario itself
remains to be proven by the `images` workflow on the `feat/s7-full-disk-image` branch — not here.
