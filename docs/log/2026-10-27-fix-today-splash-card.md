# 2026-10-27 · DeepSeek · the launch screen becomes a live, full-screenable card

Rob, in the session: *"put that in a card that can full screen on the MorpheOS today page instead
of the screen shots"*.

The generated "MorpheOS — today" page no longer shows three inlined screenshots of the launch
screen. It embeds the launch screen itself — `<iframe src="login.html?nohandoff">`, the same
`design/login.html` the boot image runs — with its music, and a "Full screen" button that puts
the card on the whole screen (and "Exit full screen" back).

- **Why `?nohandoff`:** without it the splash hands over to `start.html` after 8 s
  (`login.html`, the `setTimeout` at the end), which would walk a visitor out of the card
  mid-animation. The splash already documents that flag as the one "the card and the video
  render use".
- **Audio:** the splash starts its music on a tap inside the frame, so the tap is the user's own
  gesture and no autoplay permission is needed. `allow="autoplay; fullscreen"` is set anyway.
- **Full screen on iOS:** Safari has no element fullscreen for a `<div>`, so a refused request
  falls back to opening `login.html` itself — never a dead button.
- **The page is 84× lighter:** 12,615 bytes against 1,064,884, because the three ~250 KB stills
  were base64-inlined into every view of the page, on phones too.

Verified locally (Chromium, 1280×900 and 390×844) against a served copy of the site:

- The card renders and the iframe loads `login.html?nohandoff`.
- Tapping the frame starts it: `document.body.classList.contains('go')` is true, `theme.mp3` is
  fetched, and the `AudioContext` state is `running`.
- "Full screen" gives `document.fullscreenElement.id === 'splash'` with the card at 1280×900 and
  the frame at 1280×842; the button reads "Exit full screen" and the page URL does not change
  (no fallback was needed). Pressing it again exits and the label returns.
- "Replay" re-points the frame with a cache-buster (`login.html?nohandoff&r=<ms>`) — confirmed
  the `src` changed.
- On a 390×844 phone the frame is 340×453 (3:4, so the mark is not cropped the way the 16:9
  landing page crops it) and the caption and both buttons sit on one row.

Not verified: iOS Safari's fallback path (no device here) and Firefox. Both are the plain
`requestFullscreen` / `location.href` paths, and the Safari branch is the documented one.

Next: this needs `main`'s "A/B updates (x86 VM)" check to be green before it can merge — see
`2026-10-27-fix-rustfmt-main.md`; that job is red for a QMP `BrokenPipeError`, unrelated to this.
