# 2026-10-17 · DeepSeek · Three reference-laptop fixes: tailscaled, animation, tap-to-click

Root causes found on the 2012 MacBook Pro's first real boots, fixed in the image:

1. **tailscaled never started** — the enable loop linked `tailscaled.service` at
   `/usr/lib/systemd/system/`, but the tailscale package ships it at `/lib/systemd/system`
   (Debian convention). On a non-usrmerged root the link was broken, so `tailscale up` failed
   with "failed to connect". build.sh now links to whichever of the two paths the package
   actually created, and fails the build loudly if neither exists.

2. **The launch animation still flashed** — `LIBGL_ALWAYS_SOFTWARE=1` alone is not enough:
   Firefox on Wayland loads its own GL. `firefox-user.js` now also sets
   `gfx.webrender.software` and `layers.acceleration.disabled`, which make WebGL render on the
   CPU (llvmpipe) instead of the buggy nouveau GPU.

3. **Tap-to-click was off** — the compositor never called libinput's tap config. `live.rs`
   now calls `config_tap_set_enabled(true)` for pointer devices, beside the acceleration it
   already set.

The Rust/compositor change is Linux-only and CI-compiled (AGENTS.md H16); `check.sh` and the
QEMU scenarios are the proof.
