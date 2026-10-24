# 2026-10-23 · DeepSeek · S3 auto-update — PR 3: the autoupdate image and test-autoupdate.sh good

S3 steps 3.2 (decision) and 3.3 (fetch) already landed in PR #131; this is the third of its four
PRs — the image and the `good` scenario only (PR 4 adds `bad` and `tampered`).

- `crates/morph-autoupdate` (new binary): the unattended-update entry point the timer runs. It
  reads the boot phase, the running release and the armed-trial record, calls
  `morph-update`'s `decide`/`fetch_with_ca`/`install`, records `{release, slot, state: "armed"}`
  in `/persist/morph-update/state.json` **before** arming the trial, then arms it and (only in
  the test image) reboots. The recorded state is what makes the timer's next fire — before the
  machine has rebooted into the trial — skip instead of rewriting slot B and re-arming in a loop
  (doc 20, S3 note 5). The state derivation and the boot-history fallback reader are pure
  functions with a test table (AGENTS.md H4).
- `crates/morph-update/src/fetch.rs`: `latest(base, ca)` (read `stable/LATEST`) and
  `fetch_with_ca(…)`, so the test image's throwaway CA can be added to the rustls root store on
  top of the bundled `webpki-roots` — no `ca-certificates`, no system store (doc 20, S3 note 5).
- `images/x86-qemu/build-payload.sh`: a `dir` kind that leaves the signed payload as a directory
  (`root.img`/`vmlinuz`/`initrd.img`/`BLAKE3SUMS`/`BLAKE3SUMS.sig`) for a server, instead of an
  ext4 medium.
- `images/x86-qemu/build.sh`: an `autoupdate` scenario — a `network-manager` base (the base has
  no network; curl + ca-certificates were already in it), `DATA_MB` raised to 1024 for this
  scenario alone (the shrunk payload root, ~450 MiB, cannot fit the default 256 MiB), the timer
  and service enabled, and the per-run test CA baked in at
  `/usr/share/morpheos/update-test-ca.pem` when `UPDATE_TEST_CA` names it.
- `images/x86-qemu/overlay-autoupdate/`: `morph-update-check.{service,timer}` (ordered after
  `NetworkManager-wait-online`, `OnBootSec` past the health window), test-only drop-ins with the
  short intervals, the test CA path and "reboot now", and a `test-hook` that walks the boots
  without installing or arming anything (note 2).
- `images/x86-qemu/test-autoupdate.sh`: throws away a CA + a server cert for `IP:10.0.2.2`,
  serves the payload dir over HTTPS (`http.server` wrapped in `ssl`, logged), boots with
  `VM_NET=1`, and asserts `good` off the disk and the server log: history
  `a/committed/none b/trial/committed b/committed/none`, slot B carries the marker, and
  `root.img` was downloaded exactly once.
- Wired into `scripts/test-images.sh` (scenarios group) and `.github/workflows/images.yml` (a new
  step), and `-p morph-autoupdate` added to the Rust-build lines in both.

**Verified:** the crate tests for `morph-autoupdate` and `morph-update` pass on macOS (21 install
+ 2 fetch + the new state/decision tests); `bash -n` on the touched shell scripts and
`scripts/verify-embedded-python.sh` on the new test. **Not verified here:** the VM scenario
itself — it needs the `images` workflow (QEMU on Linux), which I did not run.

**Known gap (not this PR's fix):** `build-payload.sh` seals the payload with `release 2` but the
copied slot-A root inside it still says `/usr/lib/morpheos/release` = 1. `good` does not read
slot B's release file, so it does not affect this scenario's assertions, but a real payload must
carry the release its manifest claims (doc 20, S3 note 3.1). PR 4 or a follow-up should write it.

**Debug (2026-10-24):** once the CA/streaming/`DATA_MB=2048` fixes were in, the CI `autoupdate
good` run reached the verdict and failed with `FAIL: expected boots: a/committed/none
b/trial/committed b/committed/none` — even though `serial-autoupdate.log` showed all three boots
correct (`a` committed/none → `b` trial/committed → `b` committed/none, then poweroff). The flow
was right; the *assertion* was not. `test-autoupdate.sh` set `want` as a space-joined literal
while `recorded()` (like `test-fallback.sh`'s `expected`) emits newline-joined items, and the
`echo` in the display/FAIL lines collapses whitespace — so the two looked identical while
`[ "$got" = "$want" ]` could never match a multi-boot history. Fixed to
`want=$(printf '%s\n' a/committed/none b/trial/committed b/committed/none)`. Verified `bash -n`
and `ROBMESH_ALLOW_MAC=1 cargo test -p morph-autoupdate -p morph-update` (4 + 26 pass); the VM
scenario itself is left to the `images` workflow.
