# 2026-10-15 · DeepSeek · E16 opened: resize a window by its corner

Rob asked for a window he can resize — "i have to be able to resize a window" — and said "go"
to building the usability gaps in parallel with the E6 reports. This `plan/` PR adds E16
(resize a window by its corner) as an open task: the missing Classic-feel primitive, since
move, fullscreen and switch already exist.

- Rob on E16: "yeah go", in the session, 2026-10-15.

Next: `feat/e16-resize` — the check first (wm.rs unit tests for the resize decisions), then the
compositor wiring (corner vs edge, telling the client the new size), then the headless test.
