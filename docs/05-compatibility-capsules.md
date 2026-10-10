# 05 · Compatibility & Capsules

> Double-click a Windows `.exe` on a Raspberry Pi and it should *run*.
> Slower, maybe, but it runs, and the user never configured anything.

## What is a capsule?

A **capsule** is a lightweight, isolated execution environment that MorpheOS
builds for one program (or a group of them), automatically and on demand. It
contains:

- a **runtime** (native, Wine, Android, microVM, browser engine…)
- a **layered, deduplicated filesystem image** (shared base plus app layer plus
  user-data layer, all in Strata)
- a **capability set** (which files, devices and network it may use)
- a **resource class** (from the Resource Governor)

Capsules are *cheap*. They share layers, start from prewarmed snapshots, and
are disposable. Your data is never trapped in one, because each capsule's user
layer is a view into Strata.

## The compatibility ladder

When asked to run something, the **Capsule Planner** picks the *cheapest
correct* rung, records the result, and moves up a rung if that one fails:

| Rung | Target | Technology | Overhead |
|---|---|---|---|
| 0 | MorpheOS-native, WASM apps | Direct | ~0 |
| 1 | Linux apps (ELF, Flatpak, AppImage, deb/rpm) | Namespaces and cgroups container | ~0 |
| 2 | Web apps and PWAs | Shared browser engine, per-site capsule | Low |
| 3 | Windows apps and games | **Wine / Proton / DXVK / VKD3D** | Low–medium |
| 4 | Android apps | Containerised AOSP (similar to Waydroid) | Low–medium |
| 5 | Foreign CPU architecture | **FEX-Emu / box64 / box86** (x86 on ARM), with QEMU-user as a fallback | Medium |
| 6 | Anything else (kernel drivers, anti-cheat-free legacy, DOS, old Windows) | **KVM microVM** (Firecracker or Cloud Hypervisor class), with GPU paravirtualisation (virtio-gpu, Venus) | Medium–high |
| 7 | Full foreign OS | Full VM (user-supplied Windows licence) | High |

The planner learns from experience. A shared, opt-in **compatibility database** (similar to
ProtonDB, but machine-readable) records which rung and tweaks worked for each
program hash. The second person to run an app gets the working recipe instantly.

## The "just double-click it" flow (Windows `.exe` example)

1. The user opens `setup.exe`.
2. Strata identifies it as a PE32+ installer with a known hash in the compatibility DB, and finds the recipe
   `wine-9.x + vcrun2022 + dotnet48`.
3. The planner clones a **prewarmed Wine base snapshot** (CoW, so it starts in milliseconds),
   applies the recipe layers, and runs the installer in a capsule.
4. Shortcuts, file associations and the app's UI appear like those of any native app
   (rootless Wayland windows, no "VM window").
5. If the app crashes or fails the health checks, the planner retries on the next rung, up to a microVM,
   and records the outcome.
6. The capsule is listed under Apps and also attached to the current Session.

## Platform-by-platform notes

- **Windows:** Wine and Proton cover a very large share of desktop software and
  games. Known gaps include kernel-level anti-cheat, some DRM, drivers and some
  enterprise agents. Those go to rung 6 or 7, or are flagged as unsupported.
- **Linux:** a native-speed, first-class target.
- **Android:** containerised AOSP with GPU passthrough. App store access depends
  on licensing (F-Droid, Aurora, or a user-provided store).
- **macOS apps:** be honest. Apple's licence only permits virtualising macOS
  on Apple hardware. On Macs we can offer a macOS VM capsule through
  Virtualization.framework-class hosting. Elsewhere, we support macOS *file formats*,
  not macOS binaries.
- **Web:** treated as a first-class platform, with one engine instance per capsule for isolation.
- **Audio plugins (VST2/VST3/CLAP):** Windows plugins are bridged through a Wine
  host (as yabridge does) into the native audio graph. Latency targets are in [08](08-hardware-flows.md).

## Making foreign software feel native

- **Unified windowing:** every rung renders to Wayland surfaces, so there are no nested desktops.
- **Unified files:** every capsule sees the same Strata objects, mapped to its native
  conventions (`C:\Users\me\Documents` ↔ the Strata documents view).
- **Unified clipboard, drag-and-drop, notifications, printing and audio.**
- **Unified updates:** capsule bases are updated transactionally, and each app can
  be pinned to a known-good version.

## Performance tricks

- **Prewarmed snapshots:** each common base (Wine, Android, the browser) is booted once,
  frozen (CRIU for containers, VM snapshots for microVMs), and cloned on demand.
- **Shared, deduplicated layers:** ten Wine apps cost one Wine install.
- **Translation caches:** FEX and box64 JIT output is cached in Strata per binary
  hash and shared across the mesh.
- **Predictive launch:** the Predictor starts warming the capsules you're likely to use
  (for example, every weekday morning you open the CRM in a Windows capsule).

## Open questions
- What governance and privacy model should the shared compatibility DB use?
- Should we ship our own Wine/Proton fork, or follow upstream with recipes only?
- Should GPU sharing for microVMs (virtio-gpu Venus/native-context) be required for rung 6 games?
