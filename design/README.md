# MorpheOS — boot/login animation and brand mark

The animated mark that opens the shell (doc 20, E19): a green **geomesh** that draws itself into
completion — phasing through cyberpunk colours and landing on matrix green — with the **gyro data
rings** (built like the harness-mesh cables, each carrying a travelling data bulge) around it,
over a **twinkling starfield**, with the **"MorpheOS"** wordmark. It plays once (one 4/4 bar at
42 BPM), fires a green settle ripple, then fades to black.

- **`login.html`** — the animation. Self-contained; the Three.js library is vendored in `vendor/`
  (MIT, see `vendor/LICENSE`). Open it in a browser — no network needed.
- **`logo-*.svg`** — still-logo options: the geomesh mark, the lockup, the wordmark, the hex badge.
- **Still frames:** append `?t=<seconds>` to the URL (e.g. `login.html?t=6.0`) to freeze the
  animation at that instant — used for screenshots.

## Audio — auto-synced

Rob is adding the music. When the track is ready, drop it next to `login.html` as **`theme.mp3`**:
the animation then waits for a tap/keypress, starts the music and the picture on the **same audio
sample**, and runs the whole animation off the **audio's own clock** (Web Audio is the master) — so
the two cannot drift, and the reverb/delay tail plays out to the file's natural end while the
scene fades to black. With no `theme.mp3` present, the animation just auto-plays on a timer.

## Hero still — `still-logo.png`

The chosen still, composed in `compose.html`: green geomesh, three data rings at —

| ring | X    | Y    | Z |
|------|------|------|---|
| pink | 6°   | -5°  | 0° |
| cyan | 106° | -18° | 0° |
| blue | 81°  | 30°  | 0° |

Recreate it: open `compose.html`, set each ring's X/Y/Z sliders to those values (the geomesh is
already green), then Download PNG.

The chosen still becomes the logo (SVG) for the shell and boot.
