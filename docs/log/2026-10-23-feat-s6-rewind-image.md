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

## 2026-10-24 · fix: the rewind never ran — the harness clicked the wrong window

The images workflow failed the `rewind` scenario: the disk still had the `.locked` files and
the ransom note, so "every document matches" and "nothing of the attacker's is left" both
failed. The serial log showed the timeline window open (`points …`, `ready`) but no
`checking`/`would rewind`/`rewound` line — the confirm never happened. The `strata rewind
… --at …` lines in that log are `strata alerts`' advice output, not a rewind running, and
the alert's rewind point (`rewind_to = first_ts − 1s`, before the attack) was already correct
(and already proven by `break_it.rs::a_folder_survives_ransomware_and_the_rewind_can_be_undone`
and the passing `ransomware` scenario).

The real cause: `test-rewind.sh` clicked the title bar at `(40, 30)`, assuming the timeline
was the first window at `(16, 16)` as in the compositor test. In the VM the demo window is
the first window, so the timeline opens below it, and `(40, 30)` hit the demo window — the
Enter key reached the demo, not the timeline. The fix locates the window from its selected
row's amber band and clicks the title bar above it (the band's left edge is the window's,
the title bar is `TITLE_H` = 40px above). Verified: shell + embedded Python parse, and the
strata + flow-timeline unit tests (32 + 9 passed; the Linux-only watcher/compositor tests are
ignored off Linux, as expected). The VM scenario itself is still unverified — it needs the
images workflow.
