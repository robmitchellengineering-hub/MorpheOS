# 2026-10-20 · DeepSeek · S11 morphd — increment 3 (part 1): the binary and the service

Third increment of S11. This is the part that runs: the `morphd` binary and its systemd unit.

- `crates/morphd/src/main.rs` (the binary):
  - `morphd boot` — first resolves an interrupted transaction (the marker outlives the
    process, so the next boot takes the machine back), then applies the current generation's
    `machine.name` and `storage.retention` through the appliers.
  - `morphd apply <new.toml> [--approved-by WHO]` — resolves any interrupted transaction,
    parses + validates the new Manifest, arms the trial (`Transaction::begin`), applies the
    settings, health-checks (`read_back` equals the generation **and** `strata.service` is
    active), then commits or rolls back — undoing the machine state on a refused apply or a
    failed health check.
- `images/common/overlay/etc/systemd/system/morphd.service` — `Type=oneshot`,
  `RemainAfterExit=yes`, `RequiresMountsFor=/persist`, `Before=strata.service`, runs
  `morphd boot`.

Verified: `cargo build/test -p morphd` (7 pass), `cargo fmt`, `cargo clippy -D warnings`.

Still to land in this PR (next commit): the seeded generation 1 in `build.sh`
(`images/x86-qemu/manifest/machine.toml`), and `test-manifest.sh` + the workflow wiring.
