# MorpheOS — boot/login animation and brand mark

The animated mark that opens the shell (doc 20, E19): a green **geomesh** that draws itself into
completion — phasing through cyberpunk colours and landing on matrix green — with the **gyro data
rings** (built like the harness-mesh cables, each carrying a travelling data bulge) around it,
over a **twinkling starfield**, with the **"MorpheOS"** wordmark. It plays once (one 4/4 bar at
42 BPM), fires a green settle ripple, then fades to black.

- **`login.html`** — the animation. Self-contained; the Three.js library is vendored in `vendor/`
  (MIT, see `vendor/LICENSE`). Open it in a browser — no network needed.
- **`logo-*.svg`** — still-logo options: the geomesh mark, the lockup, the wordmark, the hex badge.
- **Music:** drop a `theme.mp3` next to `login.html` and it waits for a tap, then starts the music
  and the picture on the same sample (the Web Audio clock is the master). Without it, it auto-plays.
- **Still frames:** append `?t=<seconds>` to the URL (e.g. `login.html?t=6.0`) to freeze the
  animation at that instant — used for screenshots.

The chosen still becomes the logo (SVG) for the shell and boot.
