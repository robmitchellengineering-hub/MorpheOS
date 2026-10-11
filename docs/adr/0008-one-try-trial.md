# 0008 · A trial boot gets exactly one try

**Status:** Accepted, 2026-10-10. Corrects doc 15's "3 failed boots"; refines ADR 0006.

## Context
Doc 15 promised *"boot counting: 3 failed boots trigger an automatic fallback"*,
and the Manifest carried `system.boot.max_boot_attempts = 3` to match. Nothing
ever read that field (AGENTS.md, H5). The boot model that was actually built
(ADR 0006, `crates/morph-boot`) falls back after **one** failed boot, because both
firmware primitives it uses are one-shot by construction:

- **Raspberry Pi:** `tryboot` boots the trial partition once. The firmware clears
  the flag on that boot, so the next boot reads `autoboot.txt`'s default again.
- **systemd-boot:** `LoaderEntryOneShot` is removed by the boot loader the moment
  it is used.

So the docs and the code disagreed. Either the docs were wrong or the code was
missing a counter.

## Decision
- **An update's trial gets one boot.** If the trial crashes, hangs (the watchdog
  resets it) or fails its health check, the next boot is the proven default
  slot. There is no retry.
- **No boot counter is built.** We don't add one to the Pi, where the firmware has
  none, or to x86, where systemd-boot's `+N-M` entry counting exists but would
  make the two platforms behave differently.
- **`system.boot.max_boot_attempts` is removed from the Manifest.** A field
  whose only honest value is 1, which nothing reads, is a promise the system
  does not keep. `INV-BOOT` keeps its real half: A/B slots must stay enabled.

### Why one try is right, not just what the firmware allows
- **The costs are lopsided.** Rejecting a good update costs a retried
  download. Retrying a bad update costs the person two more broken boots,
  and on a Pi without a screen that is minutes of looking at a dead device.
- **A trial that fails once has already told us something.** A transient fault
  (a power cut during the trial, a slow USB disk) is the only case a retry
  would help, and in that case the update is offered again later, from a
  system that works.
- **One rule on every platform.** The x86 VM test proves the same behaviour the
  Pi relies on. A counter on one platform only would split that.

## Consequences
- Doc 15's firmware row and "Try to Break It" step 2 now say a failed **trial**
  falls back on its next boot.
- **Gap, stated rather than hidden:** a **committed** slot that later stops
  booting (disk corruption, a hardware change) is not covered by the trial. Both
  slots hold proven images, but nothing switches between them without a working
  system. That is the job of the immutable recovery image (heal rung H4), which is
  a separate phase-1 item, not of a boot counter.
- A Manifest that still sets `max_boot_attempts` is now rejected, as an unknown
  field. There are no deployed Manifests, so this costs nothing today. After the
  first release, removing a field needs a `manifest_version` bump.
