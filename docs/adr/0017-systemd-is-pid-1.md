# 0017 · systemd is PID 1 for v1; `morphd` is a service under it

Status: **Accepted** · 2026-10-14 (decided by Rob as A1 in [doc 20](../20-build-plan.md); recorded here, task S1)

Supersedes: [doc 03](../03-architecture.md)'s "`morphd`: the state and transaction daemon (PID 1)",
and its line that `morphd` "replaces init, service manager, package manager and settings store".
Does **not** change what `morphd` is for (the Manifest, transactions, the service graph), only where
it sits.

(ADR numbers 0015 and 0016 are reserved for the records that close the intent-engine and diagnosis
work, doc 21's I10 and doc 22's F10. This one takes the next free number.)

## Context

Doc 03 put `morphd` at PID 1: it would replace init and the service manager, and boot would be its own
dependency graph. That was written before anything ran. Since then, everything that exists runs under
**systemd**, and leans on it:

- **The wedge itself.** The A/B health check (ADR 0003, 0006, 0008) is "systemd reaches `running` with
  no failed unit". `morph-boot-check` waits on `systemctl is-system-running --wait`, the hardware
  watchdog is systemd's (`RuntimeWatchdogSec`), and a broken update is caught because its unit
  fails (the `unhealthy` scenario in `images/x86-qemu/test-fallback.sh`).
- **The shell.** It is a systemd unit, and E11 (pull request #55) makes it `Type=notify`, ready only
  with a frame on the display, so that a shell that never draws fails a trial instead of being
  committed.
- **Everything else in the images:** seatd, NetworkManager and its D-Bus API, systemd-resolved,
  tmpfiles.d, the read-only root's tmpfs mounts, the Strata watcher, the hardware report.
- **Both platforms.** Raspberry Pi OS (rpi-image-gen) and Ubuntu noble (mmdebstrap) boot systemd.

Replacing init is years of work (a service manager, cgroups, sockets, journald's role, logind's seat
and session handling, udev integration, shutdown ordering) and serves nothing in the express road
(ADR 0014): a general user boots, gets online, browses, and cannot make the machine unbootable. All
of that works today, on systemd.

## Decision

1. **systemd is PID 1 for v1**, on every MorpheOS image.
2. **`morphd` is a service under it**, not init. Its parts already exist as crates and stay as they
   are: `morph-transaction` (plan → validate → snapshot → apply → health-check → commit | rollback),
   `morph-manifest`, and `morph-graph` (the service graph). When `morphd` is assembled it is a
   systemd unit, and it drives systemd (units, targets, transactions over D-Bus) rather than
   replacing it.
3. **Health stays expressed in systemd's terms**: a failed unit fails a trial, and a service that
   matters says it is ready only when it is doing its job (`Type=notify`, as E11 does for the shell).
4. Revisiting this needs a new ADR, and a reason in the north star that systemd blocks; "we could
   own init" is not one.

## Consequences

- Doc 03 links here: its PID 1 claim is superseded, and the rest of it stands.
- Nothing in the code changes: nothing was ever built at PID 1.
- `morphd`'s boot-graph ambitions (event-driven, user-visible services first) are expressed as unit
  ordering and socket activation, which systemd already does.
- The cost, stated plainly: MorpheOS carries systemd's size and its ways, and a person reading a
  MorpheOS journal reads systemd's. That is accepted for v1.
