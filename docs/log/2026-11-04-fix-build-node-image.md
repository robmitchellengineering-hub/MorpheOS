# 2026-11-04 · DeepSeek · MorpheOS as a robmesh node, and twelve failures it took

Making a MorpheOS machine a node on the robmesh — a machine that takes work from the mesh, runs it in its own
environment and publishes the result — and, on the way, the twelve defects that a real machine found.

**What is real now.** `morph-node status|claim|key|start|stop|build|logs` turns a machine into a node in one verb.
The node takes jobs through its own loop and publishes results to the bus. It builds its own OS on demand:
**Measured** on the reference laptop, `cargo build --release --workspace` → `BUILD-EXIT=0` in 3 m 56 s offline
(vendored crates) and 6 m 33 s online. Five jobs ran through the loop, the fifth after 5 h 22 m of uptime.
A stick can claim the machine it boots (`/efi/morpheos-node.conf` + `morpheos-claim.service`), with an optional
single-use tailnet key that is deleted from the stick after joining.

**The twelve, and each one's evidence**, in the order they bit:

1. `tailscaled` asks systemd for `CacheDirectory` on a read-only root → `239/CACHE_DIRECTORY`, never starts.
2. A full clone exceeded systemd's 90 s start timeout and was SIGTERMed — which reads exactly like a bad credential.
3. `known_hosts` written to read-only `/root`.
4. An **empty** `CacheDirectoryMode=` is a parse error, and systemd rejects the *whole* drop-in — silently undoing #1's fix.
5. `Environment=GIT_SSH_COMMAND=ssh -F …` unquoted: systemd splits on spaces, so the variable became `ssh`.
6. **No Node in the image**, and the agent parses every job with `node -e` — every job read as `no command and no task`.
7. The clone fetched **GitHub**, a mirror that lags the **hub** the orchestrator writes to; the node polled a stale tree.
8. **No git identity**, so `git commit` failed, results stayed staged, and jobs re-ran every poll forever.
9. `pipewire`/`pipewire-pulse`/`wireplumber` carry `ConditionUser=!root` and the session **is root** — no application
   could make a sound on a machine whose CS4206 codec was bound correctly all along.
10. `apt-daily*` cannot work on an immutable root, so `is-system-running` said `degraded` on **every** boot.
11. `boot-check`'s history defaulted to `/var/lib` — a **tmpfs** — so every boot's record was written into space that empties.
12. `systemctl is-system-running --wait` with a 300 s default, against a test that kills the VM at 300 s: the record was
    never written. Capped at 240 s, chosen from the two windows (300 s kill / 900 s fallback allow) rather than by taste.

The last four were only visible because the boot test was made to **print what the ESP holds** and every
`boot-check` line, instead of just "missing". A failure that names an absence sends the next person guessing.

**Also picked up from the build plan while waiting:** D2's mechanism (`morph-volume`, measured through
`100 → down 20 → 80 → up 10 → 90 → mute → set 100`), D3's battery (`morph-power`: `56% (Charging, plugged) · to full
0.1 h · health 57% of design` — the 56% is a **worn** battery, not a flat one), P4's facts (`morph-health`), and
E6's first hardware report (`docs/hardware/mac-2012.md`). Each records what is done **and what is not**.

**Rob decides:** E11's account model — may one ordinary user `morph` (uid 1000, no password, autologin on VT1) exist
as a placeholder until S5? The session running as root is the root cause behind #9, `/root` being unwritable and
`/var` being a tmpfs. E11's recorder and enforcement check are built and waiting on that answer.

**What is not proven, and is not claimed:** the **reboot**. Today's fixes live in the image, and on the running
machine in `/run` and `/persist`, which a reboot clears. That a bare reboot brings the node back with nobody logged
in needs the flashed image and one reboot at the machine.

## Added while waiting for the reboot: the report now says *why* a boot failed

`journalctl -b -p err` was the report's only error evidence, and on this machine it was silent about the thing that
mattered: `apt-daily*` fails at **info**, not error, so a boot that said `degraded` had no explanation in the
report. That is the same trap AGENTS.md records for sshd — *"`journalctl -b -p err` is not a log of what happened"* —
and it cost a hand-reconstruction today.

`hwreport` now writes `failed-units.txt` (each failed unit, plus its last 20 lines **at any priority**) and the
summary names the units under `FAILED UNITS THIS BOOT` instead of only counting error lines. Measured on the
reference machine, report `003`: it answers in one line —

```
apt.systemd.daily[27630]: cannot create /var/lib/apt//daily_lock: Read-only file system
apt-daily-upgrade.service: Failed with result 'exit-code'
```

— which is the diagnosis that took a journal read by hand this morning. `scripts/verify-hwreport.sh` still passes
(it names missing firmware, and says `none` when there is none).
