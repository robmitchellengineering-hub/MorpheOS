# 2026-10-17 · DeepSeek · The native boot splash, enabled

Rob's clarification: the load screen must be drawn by the OS itself, not Firefox. The splash
was already fully wired in the compositor (`splash.rs` renders the icosahedron geomesh,
starfield, rings and wordmark through the same `render` path the shell uses) but was disabled
(`shown_at_boot()` false) and missing the tagline.

- `shown_at_boot()` now returns true: the compositor draws the geomesh as the machine boots.
- Added the "Never breaks · Heals itself" tagline: `design/logo-tagline.svg` rasterized by
  `scripts/rasterize-tagline.py` to `logo-tagline.argb`, drawn below the wordmark (bottom 8%,
  rising at 1.5 s, matching `login.html`'s `.tag`).

The mesh is a proper icosahedron (12 vertices / 30 equal edges, unit-tested) — the shape
matches `design/login.html`'s `IcosahedronGeometry(R, 1)`. Verified: `cargo test -p
flow-compositor --lib` 40/40 on macOS; the QEMU boot shows it live.
