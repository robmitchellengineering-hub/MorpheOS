# 0006 · Image factory and A/B boot model

**Status:** Accepted, 2026-10-10

## Context
Phase 1 (ADR 0003) needs bootable images that a bad update can't break, on a
Raspberry Pi and on x86 PCs. Morpheus already builds Pi images with `pi-gen`,
but those are classic single-root, mutable systems.

## Decision
- **One boot model for every platform:** a persistent *default slot* that has
  always passed a health check, plus a firmware *one-shot trial* that's
  cleared the moment it's used. Implemented once in `crates/morph-boot`;
  platforms only supply four primitives.
- **Raspberry Pi 4/5:** the firmware's `autoboot.txt` and `tryboot`, exactly as in the
  official Raspberry Pi documentation, on Raspberry Pi's own **rpi-image-gen**
  `image-rota` layout (immutable system slots plus a persistent partition), pinned to
  a specific commit. MorpheOS adds layers on top; it doesn't fork the tool.
- **x86 UEFI:** systemd-boot. The default is `loader.conf`'s `default` line, and the
  trial is `bootctl set-oneshot` (`LoaderEntryOneShot`). Built from Ubuntu
  noble with `mmdebstrap`, with no loop devices or mounts needed.
- **Health:** in phase 1, a trial is healthy only if systemd reaches `running`
  with no failed units within the timeout. This is deliberately strict.
- **Hangs:** a hardware watchdog (`RuntimeWatchdogSec`) plus `panic=5` turn hangs
  and panics into resets, and a reset ends a trial.
- **Morpheus reuse:** pinned build-tool refs, settings checked before the build,
  and no `|| true` on steps that must succeed. The Pi network-flash script is to follow.

## Consequences
- The x86 VM test (`images/x86-qemu/test-fallback.sh`) is the first automated
  "Try to Break It" check, and it runs in CI.
- The Pi image is built in CI. Real-hardware verification needs a person with a Pi 5.
- The strict health rule may reject updates for unrelated failed units. The phase-2
  heal engine replaces it with per-service health targets.
