# 2026-10-17 · DeepSeek · Intel GPU (drivers) + cable-tube splash rings

Two changes for the reference laptop's brand quality:

1. **Drivers (the real fix).** Re-adds the `blacklist-nouveau.conf` so the hybrid
   MacBookPro9,1 renders on the mature Intel i915 instead of the buggy nouveau driver —
   the same GPU macOS uses. This is what makes WebGL render correctly rather than garbage.
   (Only WHICH GPU draws changes; nothing about how the animation renders.)

2. **Cable-tube rings.** The native splash's three data rings were thin line circles; they
   now render as tubes with the login.html shader's `flow` (a wave moving along the tube)
   and `pulse` (a bright white bulge travelling around it), so the splash matches the
   Three.js scene rather than being a simplified stand-in.

Verified: `cargo test -p flow-compositor --lib` 40/40 on macOS. The QEMU boot shows it live.
