# 2026-10-25 · DeepSeek · fix S8 rollback: morphd boot never ran (service not enabled)

Root cause of the S8 config-rollback failure is **not** in the Rust. The digest IS recorded at
`Store::apply` and the digest-based rollback already works — `crates/morphd/tests/heal.rs`
(`a_manifest_with_the_wrong_digest_rolls_back`) passes, and both of the suspected causes were
ruled out by reading the code and running the tests:

- "digest never recorded" — false: `Store::apply` records `digest: Some(…)` in the meta, and
  both `morphd apply` and `morph-manifest apply --store` go through it.
- "converge swallows the error" — false: `converge` propagates every failure with `?`.

The actual bug: `images/x86-qemu/build.sh` never **enabled** `morphd.service` (only
`morph-boot-check`, `strata`, `strata-guard` were symlinked into `multi-user.target.wants/`), so
`morphd boot` — which calls `converge` — never ran at boot. That is why there was no rollback, no
heal.jsonl, no "rolled back" line and no failed unit, and the hostname was the image default
(`morpheos-a`, generation 1's name).

Fix: add `morphd` to the enablement loop in `images/x86-qemu/build.sh`, with a comment saying
why it must stay. Guard: `scripts/verify-morphd-boot.sh` (wired into `check.sh` and `ci.yml`)
asserts the service runs `morphd boot`, is `WantedBy=multi-user.target`, and is enabled by the
build — with a `--self-test` that proves each rule can fail (including the H3 control: a comment
naming `morphd` must not count).

Not proven yet: the QEMU scenario (`images/x86-qemu/test-config-rollback.sh`) still has to be
run on Linux to confirm the boot now rolls back. The `current`-symlink-missing detail from the
studio-linux run is not explained by the Rust code (nothing there deletes `current`) and needs
that run to diagnose; it is consistent with "morphd boot never ran", since no converge path
touches the store at all.
