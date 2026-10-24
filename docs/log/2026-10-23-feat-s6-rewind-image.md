# 2026-10-23 · DeepSeek · S6 rewind — PR 2: the VM scenario

The image half of S6 (doc 20): the `rewind` scenario and its test, so the timeline window is
proven against the real Strata store, not only the fake in the compositor test.

- `overlay-shell/flow-timeline.service` runs the window on the shell's compositor, and
  `flow-timeline.path` (`PathModified=/var/lib/strata/alerts.jsonl`, `RequiresMountsFor=` so
  the inotify watch resolves through the `/persist/strata` bind mount) opens it by itself when
  Strata records an alert. The `.service` is not wanted at boot.
- `build.sh` gains the `rewind` scenario (`SHELL_UI=1`, the ransomware hook with
  `MORPH_TEST_REWIND=no` via a drop-in on `morph-boot-check.service`) and installs the
  `flow-timeline` binary on every shell image.
- `ransomware-test` honours `MORPH_TEST_REWIND=no`: attack, wait for the alert, then wait for
  the timeline's rewind (no `.locked` left), leave a moment for the harness, and power off.
- `test-rewind.sh` boots it, waits for the window's "ready" line and the parent folder (so the
  re-issued alert's later folders are in), drives the confirm on the VM keyboard, screendumps
  the list and the green result band, then reads the shared verdict off the disk.
- `verify-ransomware.sh` now holds the document checks both `test-ransomware.sh` and
  `test-rewind.sh` source, so the two verdicts cannot drift.

Found and fixed while wiring it: the timeline read alerts once, so a window that opened on the
first alert (clients only) before the burst reached the parent folder would offer only that
folder. The worker now re-reads alerts and updates the list while it waits (doc 20, S6
"several folders"). Not yet proven on hardware: the VM scenario itself needs the images
workflow or the mesh QEMU, neither of which ran here.

Next: mark S6 done once the images workflow proves the scenario.
