# 2026-10-23 · DeepSeek · S11 — re-land the morphd service after an orphaned merge

PR #128 (feat/s11-morphd-service) was marked merged but its merge commit did not reach main —
the `--admin` merge produced an orphaned commit, so `crates/morphd` and `test-manifest.sh` were
missing from main while the PR showed green. This branch re-applies #128's commits on top of
main (which already has #127, the retention applier) and marks S11 done in doc 20, so the code
actually lands.
