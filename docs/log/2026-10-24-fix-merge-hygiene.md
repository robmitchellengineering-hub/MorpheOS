# 2026-10-24 · DeepSeek · merge hygiene: generated artifacts are not text, staleness is visible

Added `.gitattributes` and `scripts/sync-check.sh`. Nothing committed changed; only how two
branches reconcile when they touch the same file.

**Measured, on this machine:** fourteen branches sat **64–118 commits behind main** (`fix/node-tools`
118, `fix/e20-mark-done` 114, `fix/firefox-offline` 110, `fix/tailscale-splash` 104 …). That is why a
merge from that far back is enormous — every file either side touched over two months reconciles in
one go. Separately, **45 branches already differ from main by zero files** (squash-merged remnants), so
they can be deleted without losing anything; **11 carry real changes** and need reviewing or rebasing.

**The trap this fixes:** `.argb` rasters are produced by `scripts/rasterize-*.py` from the SVGs, and
`Cargo.lock` by cargo. Both change wholly on regeneration, so any two branches conflicted in files no
human edits, and the reported conflict was the whole file. `-merge` (keep ours, regenerate) is the
correct policy for generated output; `binary` does the same for real binaries.

**Next:** the 14 stale branches need rebasing or retiring one at a time — Rob decides which are dead.
`bash scripts/sync-check.sh` prints each branch, how far behind, and the exact rebase command.
