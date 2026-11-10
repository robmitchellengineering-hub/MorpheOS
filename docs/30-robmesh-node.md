# 26 · MorpheOS as a robmesh node

**What this is.** How a MorpheOS machine becomes a node on the robmesh — the mesh of Rob's own machines that share
work through one git repository — and what that machine can then do: take a job from the mesh, run it in its own
environment, and publish the result. It is the groundwork for an install stick that turns any x86 machine into a
build node in one command.

**Status, 2026-11-04.** A node works on real hardware: the reference machine (`morpheos-a`, a 2012 MacBook Pro)
takes jobs through its own loop, runs them, and publishes results to the bus. What is **not** yet proven is that it
survives a bare reboot with nobody logged in — the last item, and the one that turns this into an install stick.

## 1 · The one command

```
morph-node status     # what this machine is, and what it is missing
morph-node claim      # become a node: marker, key, clone, identity; prints the key line
morph-node key        # print the deploy-key line an operator adds to the repository
morph-node start      # start the loop now
morph-node stop       # stop it
```

`status` is honest by design — it reports what is absent rather than what is expected:

```
machine:    morpheos-a
os:         MorpheOS 1.0
node id:    morpheos-2012
mesh:       bed3d736c node(morpheos-2012): … - pass
agent:      active (enabled)
tailnet:    100.108.12.111
builder:    morph-build (no vendored crates)
```

## 2 · What claiming does, and why each part exists

| part | where | why it is not optional |
|---|---|---|
| identity marker | `/persist/mesh/host-id` | decides **whose queue this machine owns**. Without it the unit refuses to start (a machine must not adopt another's work), and without the marker check an unclaimed machine spent its boot timeout trying to clone with no credential |
| deploy key | `/persist/mesh/keys/deploy` | generated **on the machine**; the public half is added to the repo by an operator. No secret ships in the image |
| clone | `/persist/mesh/harness-mesh` | from the **bus** (`git://100.90.120.32:9418/mesh-hub.git`), not GitHub — GitHub is a mirror and lags, so a node fetching it misses work |
| git identity | `/persist/mesh/home/.gitconfig` | without it `git commit` fails, results stay **staged**, and the node re-runs the same job forever |
| the loop | `morpheos-mesh-node.service` | starts at boot, `Restart=on-failure`, `HOME` and `NODE_REPOS_DIR` on `/persist` |

Everything writable is on `/persist` because the root is read-only. Nothing needs `/root`, `/var` or `/etc` at
runtime.

## 3 · The failures this cost, as a troubleshooting table

Each of these was measured on the reference machine, and each looked like "the mesh is broken" from outside.

| symptom | cause | fix |
|---|---|---|
| every job says `no command and no task` | the agent parses jobs with `node -e`; the image had **no Node** | `nodejs` in the image, asserted at build |
| the node looks idle while jobs sit on the bus | the clone fetched **GitHub**, an hour behind the hub | clone the bus (`MESH_BUS`) |
| results never appear | no git identity → `git commit` fails, result stays staged → job re-runs forever | `~/.gitconfig` + `GIT_AUTHOR_*` in the unit |
| `tailscaled` never starts | the stock unit asks for `CacheDirectory` on a **read-only root** (`239/CACHE_DIRECTORY`), and an empty `CacheDirectoryMode=` makes systemd **reject the whole drop-in** | clear both; state on `/persist` |
| no sound at all | `pipewire`/`wireplumber` carry `ConditionUser=!root` and the session **is root** | user-unit drop-ins clearing the condition |
| the boot screen shows a failure | `apt-daily*` fail on an immutable root, so the system is always `degraded` | the timers are masked; `morph-autoupdate` is how this system updates |
| no boot history | `boot-check`'s default history path is under `/var` (a tmpfs), and an unbounded `is-system-running --wait` never records | `MORPH_BOOT_HISTORY` on the ESP; the wait is bounded to 240 s |

## 4 · What a node can do once it is one

- **Build its own OS.** `morph-build [crate]` compiles the workspace from vendored crates with **no network**, or
  online if the machine has it. Measured on the reference machine: 3 m 56 s for the whole workspace, `BUILD-EXIT=0`.
- **Take work from the mesh.** `dispatch.py --node morpheos-2012 --command '…'` from the orchestrator; the result
  lands in `runs/morpheos-2012/<id>/` on the bus.

## 5 · Not done yet

- **The reboot proof.** The node works today; that it comes back after a reboot with nobody logged in is the last
  unmeasured claim, and it needs the flashed image.
- **A non-root session.** The desktop autologins as root, which is why the audio units needed condition-clearing and
  why a class of read-only-root friction exists. It is a login change, not an audio one — a plan, not a patch.
- **The install stick itself.** Everything above is the mechanism; presenting it as "flash this, tap once, it is a
  node" is the product.

## 6 · The stick that claims the machine it boots

The operator steps belong in **preparing the stick**, not in installing the OS. Two files on the ESP make that
work:

```
/efi/morpheos-node.conf
  host=morpheos-2012
  bus=git://100.90.120.32:9418/mesh-hub.git
  tailnet_authkey_file=/efi/ts-key        # optional, see below
```

- `scripts/claim-stick.sh /dev/disk2 morpheos-2012` writes it.
- `morpheos-claim.service` (oneshot, conditional on the file) claims the machine at first boot, **before** the
  agent starts. A machine already claimed as a different node is **refused**, not adopted: a result attributed to
  the wrong machine is worse than no result.
- `morph-node-tailnet` runs next and does nothing unless the file names a key.

**The tailnet trade, stated plainly.** A login is a grant, so the default is one tap by hand. For a stick prepared
for a fleet, that tap is the only thing between "boot" and "it is a node" — so the claim file *may* name a
**single-use, expiring** Tailscale auth key. That key is then a secret on the stick, which is a real risk; the script
mitigates it by deleting the key file after joining and removing the line from the config, and `morph-node status`
records that the machine joined unattended. If the key file cannot be removed, it says so and tells the operator to
revoke the key.

Verified on the reference machine: no key line → "needs one login tap by hand" and no change; `--check` reports
without acting; a named-but-missing key file is refused.

## 7 · Measured, on the reference machine

| what | result |
|---|---|
| whole workspace, offline (vendored crates) | `BUILD-EXIT=0` in 3 m 56 s |
| whole workspace, online | `BUILD-EXIT=0` in 6 m 33 s, `flow-compositor` 3,908,208 B, `morphd` 1,804,368 B |
| jobs through the node's own loop | 5, all `pass`, results in `runs/morpheos-2012/` on the bus |
| uptime at the fifth job | 19,335 s with the agent active throughout |
