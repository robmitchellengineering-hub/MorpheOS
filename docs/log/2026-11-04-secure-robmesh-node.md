# 2026-11-04 · DeepSeek · MorpheOS becomes a robmesh node, on real hardware

Rob's framing, which set the target: *"the most important thing is getting this working to a point where it can be
used as a build node on the robmesh and so you can send it jobs to build itself from the inside … we can then have
a native install stick for MorpheOS that can install it as a working Robmesh build node."*

## The decision, written down before the code

**Transport: git (`scripts/node.sh`)** — not the board's HTTP transport. The agent needs *one* secret either way;
git's is a deploy key generated **on the machine**, whose public half an operator adds to the repo. The board would
have made every MorpheOS node depend on one process being up on one Mac — and it was `HTTP 000` when the decision
was taken. Every other node in the estate (win, win-2, pacland-rog, studio-linux) is a git node.

## What the image now carries (the OS work)

| file | why |
|---|---|
| `morpheos-mesh-node.service` | the loop, `Environment=HOME=/persist/mesh/home`, `NODE_REPOS_DIR=/persist/mesh/code`, `TimeoutStartSec=900`, `Restart=on-failure` |
| `tailscaled.service.d/persist.conf` | clears the stock `ExecStart` **and** `CacheDirectory`, keeps `--state` on `/persist` |
| `mesh-node-setup` | idempotent: dirs, key, `ssh_config`, clone, identity marker; `--print-key` prints the one credential line |
| `etc/systemd/user/{pipewire,pipewire-pulse,wireplumber}.service.d/root-session.conf` | clears `ConditionUser=!root` — see below |
| `nodejs` + assertion | the agent parses jobs with `node -e` |

## Four failures the first real boot found, none of which a plan would have

1. **`tailscaled` never started.** The stock unit declares `CacheDirectory=tailscale`, so systemd builds
   `/var/cache/tailscale` *before* any `ExecStartPre` — `239/CACHE_DIRECTORY` on a read-only root.
2. **The clone was killed at 90 s.** systemd's default start timeout SIGTERMed a full fetch mid-stream, which reads
   exactly like a bad credential. `TimeoutStartSec=900`.
3. **`known_hosts` written to `/root`** — read-only. Now `/persist/mesh/known_hosts`.
4. **No Node, so no job ever parsed.** The node pulled its queue and answered `no command and no task` for every
   job. A machine that looks like a node and can never run anything.

And one product bug found by Rob rather than by a test: **audio never worked**, because `pipewire`,
`pipewire-pulse` and `wireplumber` all carry `ConditionUser=!root` and MorpheOS autologins **as root**. The codec
(`CS4206`) had been bound correctly the whole time. Clearing the condition made sound appear — Rob heard it.

## The proof

```
[node morpheos-2012] running 2026-11-04T0631Z-…-first-job-through-the-agent  (mode=shell, timeout=600s)
[node morpheos-2012] 2026-11-04T0631Z-…-first-job-through-the-agent: pass (exit 0, 0s)

runs/morpheos-2012/2026-11-04T0631Z-…/stdout.txt:
  morpheos-a
  8
  PRETTY_NAME="MorpheOS 1.0"
  agent clone present
  Wed Nov  4 06:50:36 UTC 2026
```

That is a job dispatched from the Mac, pulled by the machine's own loop, executed on the 2012 MacBook, and returned
to the mesh repo — not an ssh command posted by hand.

## What is not done

- **The reboot test.** The whole point is that a reboot does not lose the node, and it has not been run: the fixes
  for the four failures are in the image, not yet on the stick, and the running machine has them only in `/run`
  and `/persist`.
- **`main` is contended.** The node's pushes lose races against other sessions and retry; results commit locally and
  publish on a later successful push. Worth a look; not a correctness problem.
- **A non-root session** is the real fix for the audio condition, and for a whole class of read-only-root friction.
  That is a login change, and it belongs in a plan, not smuggled into an audio commit.

## Two more failures, found by watching the node for half an hour

Neither would have shown up in a plan, and both look like "the mesh is broken" from outside:

1. **The node polled a lagging mirror.** It cloned `github.com`, but the orchestrator pushes to the **hub**
   (`git://100.90.120.32:9418/mesh-hub.git`) and GitHub only catches up when the hub is mirrored. So a job
   written to the bus took half an hour to be seen — or was never seen — while the machine reported itself
   healthy and idle. The clone now targets the bus (`MESH_BUS` overrides it).
2. **`git commit` failed silently for want of an identity.** `mesh-node-setup` had written no `~/.gitconfig`, so
   every result stayed *staged*: the job never published, and because it never published the node re-ran it on
   every poll. `mesh-node-setup` writes the identity and the unit sets `GIT_AUTHOR_*`/`GIT_COMMITTER_*` as well.

With both fixed, a job dispatched from the Mac ran unattended and its result reached the hub:

```
[node morpheos-2012] running 2026-11-04T0717Z-…-second-job-unattended  (mode=shell, timeout=600s)
[node morpheos-2012] 2026-11-04T0717Z-…-second-job-unattended: pass (exit 0, 1s)

runs/morpheos-2012/2026-11-04T0717Z-…/stdout.txt  (on the hub):
  agent identity: morpheos-a
  uptime: 10737s
  v22.11.0
  active
```

## The boot test was telling the truth, and it took three more fixes to hear it

`test-boot.sh` counts the failure colour in the boot status line, and it went red three times. Each red was a
real defect, and the machine's own `/efi/morph-boot/history.jsonl` named the culprit:

```
{"state":"degraded","status":{"booted":"a",...,"phase":"committed"},"decision":{"action":"none"}}
boot-check: failed unit: tailscaled.service
```

1. **`RequiresMountsFor=/persist/tailscale`** — a mount requirement on a path that `ExecStartPre` is supposed to
   create. The unit failed before anything ran. Removed; `After=persist.mount` is the honest dependency.
2. **`CacheDirectoryMode=` with an empty value** — systemd logs `Failed to parse mode value, ignoring:` and then
   **rejects the whole drop-in**, so the stock unit ran on a read-only root again and tailscaled failed exactly as
   before. Only the empty-valued keys that genuinely reset a setting are set now.
3. **`Environment=GIT_SSH_COMMAND=ssh -F /persist/…`, unquoted** — systemd splits it on spaces, so the variable
   became `ssh` and `-F`/the path were rejected as invalid assignments. The agent used plain ssh, wrote
   `/root/.ssh/known_hosts` on a read-only root, and none of its ssh config applied. Quoted.

Found by running `systemd-analyze verify` against the **running machine** rather than waiting for a job to fail:
the tool systemd ships for exactly this had not been used once in this project.

Also fixed, in the test itself: `history=$(mtype … )` ran under `set -e`, so a failing `mtype` killed the script
with **no message** — the run ended after the console check, exit 1, and the failing assertion was never named.
That is how two of the three reds were reported as "the image is broken" with nothing to act on. It now reads
tolerantly and prints `FAIL the ESP has no /morph-boot/history.jsonl` instead.

## The last two, and why they hid

1. **The boot history defaulted to a tmpfs.** `boot-check` reads `HISTORY=${MORPH_BOOT_HISTORY:-/var/lib/morph-boot/history.jsonl}`
   and on this image `/var` is volatile, so the record of every boot went into space that empties — while the
   file's own comment promises "on storage that survives reboots". The path was arriving from whichever overlay
   happened to set it (the Pi5 sets it; the reference laptop had it; the CI test image's ESP had no `/morph-boot`
   at all). The image now sets `MORPH_BOOT_HISTORY=/efi/morph-boot/history.jsonl` itself.
2. **`systemctl is-system-running --wait` with a 300 s default, inside a boot unit.** The CI boot test kills the VM
   at 300 s, so a boot that had not settled produced *no record at all* and read as "the image recorded no boot
   history". The hardware only escaped because its boot was `degraded`, which returns immediately. Capped at 120 s:
   an unsettled boot is `timeout`, which the script already knows how to write down.

Both were invisible until the boot test was changed to **print what the ESP holds and every `boot-check` line**
instead of only saying "missing". A failure that names an absence sends the next person guessing; a failure that
names the contents ends the guessing in one run.

## Where this stands

Eleven defects, all found by making the machine do the job rather than by reading the design. The node runs
unattended for hours, takes jobs from the bus, commits its results and publishes them. What is **not** proven is the
one thing the whole exercise is for: that a **reboot** brings it back with nobody logged in. That needs the stick
flashed and one reboot at the machine.
