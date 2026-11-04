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
