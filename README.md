# MorpheOS — boot/login animation and brand mark

The animated mark that opens the shell (doc 20, E19): a green **geomesh** that draws itself into
completion — phasing through cyberpunk colours and landing on matrix green — with the **gyro data
rings** (built like the harness-mesh cables, each carrying a travelling data bulge) around it,
over a **twinkling starfield**, with the **"MorpheOS"** wordmark. It plays once (one 4/4 bar at
42 BPM), fires a green settle ripple, then fades to black.

**What the mark means** — the poetic explanation: [LOGO.md](LOGO.md).

- **`login.html`** — the animation. Self-contained; the Three.js library is vendored in `vendor/`
  (MIT, see `vendor/LICENSE`). Open it in a browser — no network needed.
- **`logo-*.svg`** — still-logo options: the geomesh mark, the lockup, the wordmark, the hex badge.
- **Still frames:** append `?t=<seconds>` to the URL (e.g. `login.html?t=6.0`) to freeze the
  animation at that instant — used for screenshots.

## Audio — auto-synced

`theme.mp3` (next to `login.html`) is the current **placeholder** — an F# major chord that builds
over the draw and rings out at the settle — while Rob records the real track. The animation waits
for a tap/keypress, then starts the music and the picture together.

Over HTTP the track is decoded and played sample-accurately, with Web Audio's clock as the master,
so picture and music cannot drift. On a `file://` page — how the booted image opens `login.html` —
the browser blocks `fetch()`, so the loader falls back to an `<audio>` element and a
`performance.now()` clock; the two still start on the same tap, within the element's start-up
latency. With no `theme.mp3` present, the animation just auto-plays on a timer.

## Hero still — `still-logo.png`

The chosen still, composed in `compose.html`: green geomesh, three data rings at —

| ring | X    | Y    | Z |
|------|------|------|---|
| pink | 6°   | -5°  | 0° |
| cyan | 106° | -18° | 0° |
| blue | 81°  | 30°  | 0° |

Recreate it: open `compose.html`, set each ring's X/Y/Z sliders to those values (the geomesh is
already green), then Download PNG.

## Brand colours

**Primary** — the voice.

| colour | hex | role |
|---|---|---|
| Matrix green | `#00ff41` | the living mark, healing, success, the accent |
| Black | `#05070d` | the depth the OS lives in — backgrounds |
| White | `#ffffff` | the wordmark, text, honesty |

**Secondary** — the data rings, for support and life.

| colour | hex | ring |
|---|---|---|
| Neon pink | `#ff2fd6` | ring 1 — the working cable |
| Cyan | `#06b6d4` | ring 2 |
| DeepSeek blue | `#4d6bfe` | ring 3 |

Green stays the voice; the secondary colours are the light show around it — for the rings, cables
and live status, never the wordmark or the primary surface.

The chosen still becomes the logo (SVG) for the shell and boot.
