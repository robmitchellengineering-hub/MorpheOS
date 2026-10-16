# 2026-10-16 · DeepSeek · the splash music placeholder, and it plays at boot

Rob approved a placeholder track while he records the real one. Two things landed together:

- **`design/theme.mp3`** (and the copy in `images/x86-qemu/overlay-browser/…/morpheos/`) — an
  F# major chord that builds over the draw and rings out at the settle (one 4/4 bar at 42 BPM,
  synthesized in the session). Rob replaces this with the real recording.
- **The loader now works at boot.** `login.html` fetched `theme.mp3` with `fetch()`, which the
  browser blocks on a `file://` page (Firefox gives each file a unique origin) — exactly how the
  booted image opens it, so the music would never have played. The `.catch()` now falls back to a
  plain `<audio>` element (media elements load `file://` without CORS) with a `performance.now()`
  clock, started on the same tap. Over HTTP the sample-accurate Web Audio path is unchanged.

`design/README.md`'s audio section now says the track is the placeholder and documents both
protocols.
