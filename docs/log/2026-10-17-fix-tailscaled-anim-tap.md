# 2026-10-17 · DeepSeek · Three reference-laptop fixes: tailscaled, GPU, tap-to-click

Root causes found on the 2012 MacBook Pro's first real boots, fixed in the image:

1. **tailscaled never started** — the enable loop linked `tailscaled.service` at
   `/usr/lib/systemd/system/`, but the tailscale package ships it at `/lib/systemd/system`
   (Debian convention). On a non-usrmerged root the link was broken, so `tailscale up` failed
   with "failed to connect". build.sh now links to whichever path the package actually
   created, and fails the build loudly if neither exists.

2. **The launch animation flashed garbage** — the hybrid Mac's NVIDIA chip was being driven by
   the open-source `nouveau` driver, which renders WebGL nonsense; the Intel i915 driver is
   mature and drives the panel exactly as macOS uses the Intel GPU. A modprobe blacklist
   keeps `nouveau` from claiming the display, so Firefox renders on i915. This changes only
   WHICH GPU draws — the animation itself renders exactly as designed (Rob: the GitHub link
   is the reference, and it must look the same). No rendering/WebGL prefs are touched.

3. **Tap-to-click was off** — the compositor never called libinput's tap config. `live.rs`
   now calls `config_tap_set_enabled(true)` for pointer devices.

The Rust/compositor change is Linux-only and CI-compiled (AGENTS.md H16); `check.sh` and the
QEMU scenarios are the proof. The GPU fix is validated on the real hardware (CI's QEMU uses
virtio-gpu, not nouveau).
