# 2026-10-20 · DeepSeek · S11 morphd — increment 2: retention, Strata's policy file, and the unenforced_settings change

Second increment of S11. This makes `/storage/retention` obeyed: `morphd` writes the
generation's retention policy, and `strata prune` (with no flags) prunes by it — so the
Manifest's `storage.retention` is the one place the policy is decided.

- `crates/morphd` gains the `Retention` value and the `RetentionPolicy` applier (the same
  `apply` / `read_back` / `undo` contract as the hostname): it writes
  `/persist/morph/strata-retention.json`, and refuses before writing when the path cannot be
  made. Three unit tests: apply-then-read-back, undo restores, and a policy whose parent is a
  file is refused.
- `crates/strata`: `prune` with no flags reads that file (falling back to 24/30/365 when
  `morphd` has never written one), and a new `strata policy` command prints the policy in
  force. Flags still override the file.
- `crates/morph-manifest`: `/storage/retention` and `/machine/name` leave
  `unenforced_settings()` — the two settings the machine now obeys. The schema-walk test and
  the "one grouped W-UNENFORCED warning" test are unchanged and still pass (they iterate the
  list, so removing the two entries shrinks the warning rather than breaking it).

Verified locally: `cargo test -p morphd -p strata -p morph-manifest` (all green),
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean.

Next: increment 3 — the `morphd` service, the seeded generation 1, and `test-manifest.sh`.
