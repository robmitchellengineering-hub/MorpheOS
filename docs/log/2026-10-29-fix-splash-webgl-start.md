# 2026-10-29 · DeepSeek · the full splash starts on its own, and only where it can draw

## The other half: the full splash (tier 2) existed and was born frozen

`flow-browser.service` already opens `login.html` — the real three.js animation with bloom and the
audio clock — as the first thing on the shell. It never played, because the page waited for a
**tap** before starting either the picture or the music:

```js
window.addEventListener('pointerdown', begin);
window.addEventListener('keydown', begin);
```

At boot nobody taps. So the machine showed a frozen first frame for eight seconds, then the meta
refresh took it to the start page. Three changes:

1. **It starts without a gesture.** `begin()` runs immediately in both branches (the `fetch()`
   branch and the `file://` `<audio>` branch, which is the one a real boot uses), and the gesture
   listeners stay as the fallback for a browser that refuses audio without one — where a tap now
   *resumes* a suspended context instead of doing nothing.
2. **The profile allows autoplay** (`firefox-user.js`): `media.autoplay.default=0`,
   `blocking_policy=0`, `block-webaudio=false`. A booted machine has nobody to tap for it.
3. **"Can it handle it" is now asked.** `new THREE.WebGLRenderer` throws where WebGL is absent —
   the old code left a blank screen until the handoff. It is wrapped: no WebGL → straight to
   `start.html`, because the software splash has already played at boot and a blank screen reads as
   a hung machine.

That is the tiering in practice: **software splash at boot on every machine** (CPU, no browser),
**full WebGL splash on the shell where WebGL works**, and `start.html` where it does not.
