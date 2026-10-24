# 2026-10-23 · DeepSeek · S7 full disk — PR 2: the notice window and the VM test

PR 2 of S7. PR 1 (the space guard + ENOSPC survival) is merged; this wires the guard into the
image, gives the shell the warning, and proves the whole scenario in a VM.

- `crates/flow-notice` (new): a one-message, one-colour notice window in the `flow-wifi` shape.
  The model (`notice.rs`) has no Wayland in it and is unit-tested anywhere; the window
  (`main.rs`) is a plain Wayland client that answers pings and frame callbacks until a key or a
  click dismisses it. S8 reuses it. Its `WARN` / `OK` / `FAIL` colours are listed in
  `crates/flow-wifi/tests/colours.rs` (H20), so a screendump reads the notice by colour.
- `crates/strata` guard reports free space *after* pruning (`free_percent_after`) and says
  "now N% free, back above / still below the floor" — the "why not" the notice shows when the
  policy cannot free enough (S7 note 5: only warn, never delete beyond it).
- `images/x86-qemu/overlay`: `strata-guard.service` + `space-guard` run `strata guard` at boot,
  *before* morphd, so on a full disk the guard frees space before morphd writes the
  retention / min-free files; it stages the notice under `/run`.
- `images/x86-qemu/overlay-shell`: `flow-notice.service` + `show-space-notice` show that notice
  once the compositor is up.
- `images/x86-qemu/build.sh`: a `full-disk` scenario (SHELL_UI=1, no demo window, its own
  driver) and the `flow-notice` binary.
- `images/x86-qemu/run-vm.sh`: a `VM_RTC=` variable (`-rtc base=…`, S7 step 3).
- `images/x86-qemu/test-full-disk.sh` (new): run 1 stamps five versions of three files 30 days
  ago; run 2 fills `/persist` as root until ENOSPC and reboots. Verdicts read off the disk
  (H8): boot history committed + healthy with `strata.service` not failed; the copied store
  read with the host's own `strata --store` (old versions gone, newest kept); `dumpe2fs` free
  space above `storage.min_free_percent` or the notice saying why not; and a screendump showing
  the notice.
- Wired into `scripts/test-images.sh` and `.github/workflows/images.yml`.

Verified here: `flow-notice` unit tests, the strata guard tests, and the colour-uniqueness test
all pass on the Mac (non-Wayland). `bash -n` and the embedded-Python check pass. The VM
scenario itself is only proven by the images workflow (or mesh QEMU) — that is the slow,
unverified half. Two things the VM run will confirm or expose: that `strata guard`'s prune can
write its SQLite WAL on a truly full partition, and that morphd's writes land once the guard
has freed room (the reason `strata-guard.service` is `Before=morphd.service`).
