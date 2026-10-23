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

## The seeded generation and test-manifest.sh (landed)

- `images/x86-qemu/manifest/machine.toml`: generation 1 (admin user with a recovery key,
  `accept_single_copy`, retention 24/30/365) — validates clean.
- `build.sh` installs `morphd` and seeds generation 1 into the data partition
  (`morph-manifest apply machine.toml machine.toml --store $DATADIR/morph`), and adds the
  `manifest` scenario's driver overlay.
- `test-manifest.sh`: two boots. `apply` (hostname → `studio`, retention → 7/14/90) and
  `fail` (a 65-byte hostname the kernel refuses — the `validate` accepts it, the hostname
  applier refuses it, the whole change rolls back). Verdicts read off the ESP: the kernel
  hostname and `strata policy`, the machine's own reports — never `morphd`'s log.
- Wired into `test-images.sh` and the x86 `images` job (with `morph-manifest` + `morphd` in
  the cargo build lines).

This closes S11's code: PR 1 (hostname applier), PR 2 (retention + Strata policy +
`unenforced_settings`), PR 3 (the binary, the service, the seed and the VM proof).

## CI red, four fixes, green (2026-10-23)

The `manifest` scenario went red in CI. Four fixes, each found from the serial log:

1. `test-manifest.sh` was `100644` (not executable) → `sudo: …/test-manifest.sh: command not
   found`. `chmod +x`.
2. `overlay-manifest-test/usr/lib/morpheos/test-hook` was `100644` too — `boot-check` guards it
   with `[ -x … ]`, so it silently skipped and the manifest VM hung at login for the full
   timeout. `chmod +x`.
3. `test-manifest.sh`'s `esp_write` overwrote `mode` on the second boot and `mcopy` refused to
   overwrite the existing FAT entry (the apply/fail run died right after the apply checks).
   `mdel` the destination before `mcopy`.
4. The rollback assertion was wrong, not `morphd`. The refused 65-byte hostname correctly rolls
   back to the *previous* committed generation (`studio` / 7 hours), but the test asserted the
   *seeded* generation (`morpheos-a` / 24 hours). Fixed the assertion to expect `studio` / 7h.

All three checks are green on PR #128: `rust`, the Pi image, and "Try to Break It: A/B updates
(x86 VM)".
