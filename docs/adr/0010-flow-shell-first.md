# 0010 · The Flow shell is built first: a Smithay compositor, and nothing stands in for it

Status: **Accepted** · 2026-10-11

## Context

The wedge works. An A/B trial boot commits on health and falls back on its own, an updater writes and
verifies a slot, and a VM suite proves a bad update is booted at most once. Rob's next question was the
obvious one — *"if its not actual user tests to use it as a computer we will probably wait till thats
ready"* — and the answer today is that **nothing above the kernel is user-visible at all**: the image
boots to a root shell. There is no session, no window, no application.

[Doc 03](../03-architecture.md) is explicit about what should be there: *"**Flow shell:** a Wayland
compositor (built on **Smithay**, in Rust) with a session-centric…"*. [Doc 00](../00-strategy.md) puts it in
Phase 2, "Daily driver", beside capsules. [Doc 10](../10-roadmap.md) lists *"Flow shell v1: Intent Bar,
Sessions"*.

The choice put to Rob was: ship a **stock desktop** (a wlroots compositor plus a panel) as a named
placeholder and use it as a computer sooner; or **build the shell** and see nothing on a screen for weeks;
or both at once. He chose to build the shell.

This record exists because that choice is expensive in a direction that is easy to reverse by accident:
the first person who wants a browser on a laptop will reach for GNOME, and a stock desktop that is
*shipped* becomes the product by default. The design says MorpheOS has its own shell, and an accepted
record is superseded, never edited.

## Decision

**The shell is MorpheOS's own, built on Smithay, and no stock desktop is shipped in its place.**

1. **A Rust compositor on Smithay**, per [doc 03](../03-architecture.md) and [ADR 0002](0002-rust-core.md).
   Not wlroots (C), not GNOME/KDE, not an X11 session. The session model is the shell's own data, which is
   why the shell comes *before* the session model rather than after it.
2. **The first slice is a frame, not a framework.** A compositor that starts on the real display device,
   shows one session's first surface (a terminal), takes keyboard and pointer input, and can be captured.
   The Intent Bar, multi-display, capsules and the Device Broker are deliberately absent from it.
3. **CI proves a frame, not a unit.** A systemd unit reporting `active` is not a computer somebody can use
   ([H17](../../AGENTS.md) in the sibling repo: *a check that never ran is not a pass*). The compositor's
   **headless backend plus a screenshot** is the proof: the check asserts that something was actually drawn.
4. **Nothing ships as "the desktop" until it is the shell.** A diagnostic console is fine and already
   exists; a stock desktop image is not built at any point, so it cannot be mistaken for progress.
5. **Accessibility is designed in now.** [Doc 11](../11-risks-open-questions.md) item 8 says screen reader,
   voice control and high contrast must not be retrofitted. A compositor that has never had a place for
   them is the wrong foundation, so the first slice carries the seam even though it implements nothing.

## Consequences

- **Weeks before anything is on a screen, and that is accepted.** The alternative buys a usable machine
  sooner and moves the product away from its own design.
- **CI gets slower.** Building a Rust Wayland compositor and booting it to a captured frame adds minutes to
  every image job. That cost is taken deliberately, and the frame check is the thing that pays for it.
- **A dependency risk is now load-bearing.** Smithay's API moves; pinning it and keeping MorpheOS's own
  session model in a crate that does not depend on Smithay is the mitigation, and it is why the session
  model does not live inside the compositor crate.
- **The hardware questions don't go away, they move.** GPU, touchpad and especially **wifi** still have to
  work on the real laptop; a compositor that draws nothing because there is no working driver is the same
  failure with more code in front of it.
- **What this does not decide:** the Intent Bar's behaviour, how capsules get surfaces, what a session
  looks like across displays, and when the stock-font and theme questions are answered. Those are later
  records.

## The first slice, in the order it lands

1. A crate that starts, opens the display (DRM/KMS on hardware, headless under CI) and exits cleanly.
   Proven by a test that runs the headless path and asserts a frame was rendered.
2. One client surface on screen — a terminal — with keyboard and pointer input routed to it.
3. A screenshot in CI, asserted non-blank, so "the session came up" is a picture rather than a claim.
4. The session model, in its own crate, with the shell as one consumer of it.
