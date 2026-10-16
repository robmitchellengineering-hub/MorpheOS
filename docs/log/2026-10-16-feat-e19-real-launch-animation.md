# 2026-10-16 · DeepSeek · the launch screen is the real Three.js animation

Rob saw the Rust splash on hardware and said it was not the animation he approved. He is
right: the approved animation is the Three.js `design/login.html` — which already carries the
"Never breaks · Heals itself" tagline and the `theme.mp3` audio clock, and has the bloom and
cable rings the software port dropped.

So the boot now runs the real thing: Firefox starts fullscreen on `login.html`, plays the
animation, and `login.html` redirects to `start.html` once the mesh has drawn and settled. The
Rust port is disabled at boot (`splash::shown_at_boot() -> false`) and kept as tested reference
code.
