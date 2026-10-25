# 2026-10-23 · DeepSeek · S8 config rollback — PR 2: the VM scenario and the notice

PR 2 of S8. PR 1 (generation digests + `morphd`'s corrupt-check rollback, in
`crates/morph-manifest` + `crates/morphd`) is merged on `origin/main`; this wires the rollback
into the image, gives the shell the notice naming the rolled-back generation, and proves the
whole scenario in a VM. It reuses `crates/flow-notice` (S7) unchanged.

- `crates/morphd/src/main.rs`: `morphd boot`, when it rolls a corrupt generation back, now also
  stages a notice under `/run/morpheos/rollback-notice` — two lines, message then colour, the
  same shape the space guard stages (S7). Staging is best-effort: the rollback is the recovery,
  and a notice that cannot be written must not turn a successful rollback into a failed boot
  (S11's trap). The message names the generation rolled back and the reason.
- `images/x86-qemu/overlay-shell`: `flow-rollback-notice.service` + `show-rollback-notice` show
  that notice once the compositor is up — the S7 `flow-notice.service` pattern, but conditioned
  on the rollback notice, not the space notice.
- `images/x86-qemu/overlay-config-rollback` (new): the test driver. Run 1 applies generation 2
  (a new hostname) through `morphd apply`, then corrupts generation 2's stored **file** by hand
  — `readlink current` first, then `sed` the file it names, never the `current` symlink — and
  powers off. Run 2 records the kernel hostname and powers off.
- `images/x86-qemu/build.sh`: a `config-rollback` scenario (SHELL_UI=1, no demo window) and the
  `flow-rollback-notice` unit.
- `images/x86-qemu/test-config-rollback.sh` (new): the scenario. Verdicts read off the disk
  (H8): the store's `current` names generation 1 and generation 2 is still in the store;
  `heal.jsonl` names `bad:2`, the reason ("does not match its digest"), and `now:1`; generation
  2's own record (meta + `.rob` mark) names it and its approver; the kernel hostname on the ESP
  is generation 1's again; and a screendump shows the notice naming generation 2.
- Wired into `scripts/test-images.sh` and `.github/workflows/images.yml` (flow-notice is already
  built by S7's build lines — not duplicated).

The corruption changes the hostname string inside generation 2's stored file, so the file still
parses and validates and the **digest** recorded at apply is what refuses it — PR 1's check, not
a parse failure.

Verified here: `bash -n` on the new and modified scripts; the embedded Python compiled; and the
`morphd` crate type-checks with its test suite green (10 unit tests, 4 heal tests). The Wayland
path (flow-notice, flow-compositor) does not compile on macOS and was not checked. The VM
scenario itself is only proven by the images workflow (or mesh QEMU) — that is the slow,
unverified half.
