# 0013 · The shell's feel is a property of the session; a Desktop session is first-class

Status: **Accepted** · 2026-10-12

Supersedes: nothing. Extends [0010](0010-flow-shell-first.md) (which decided the shell is our own and named
the first slice) and [0011](0011-the-humans-mark.md). The "what is deliberately absent" list in 0010 stays
true for the *frame* slice and gains this as the next rung.

## Context

The wedge works and the first shell slice exists (a frame, then input — PR #29). Before the shell grows a
surface for real work, Rob set the direction on the *feel* (2026-10-12): **it should offer both a
traditional desktop way to drive it and the native MorpheOS way, and the feel must align with the ethos and
with how people actually use computers.**

The docs already constrain the answer:

- [Doc 01](../01-vision-and-principles.md) P1 — *the task is the unit, not the app* — is the principle a
  desktop most easily breaks.
- [Doc 14](../14-day-in-the-life.md) — the native feel is Margaret's: *"She never sees an 'app'. She sees
  'Photos of the grandkids', 'Call Jenny' and 'that letter'."*
- [Doc 02](../02-use-case-analysis.md) — *users think in tasks, projects, people and time, not paths*; and
  most friction is *bridging between worlds* (apps ↔ files, platform ↔ platform).
- [Doc 18](../18-flow-learning.md) — the OS anticipates: *having things ready before you ask*.
- [Doc 01](../01-vision-and-principles.md) "for nerds" — *everything is a declarative, inspectable,
  scriptable state; the AI is a fast pair of hands, never a black box*.
- [ADR 0010](0010-flow-shell-first.md) warns: *a stock desktop that is shipped becomes the product by
  default*.

**The trap this record exists to avoid:** the obvious shape is a global toggle — *Desktop mode* vs *Native
mode*. That is the one option the ethos rules out. Two modes are two products; the native one dies because
nobody is forced to learn it; and a desktop *mode* makes the app the unit the moment it is on, breaking P1.
It is exactly the "stock desktop becomes the product" failure 0010 was written against.

## Decision

1. **The feel is a property of the Session, not of the OS.** A person's "Photos" session is a gallery, their
   "Dev" session is a tiling desktop, and an everyday machine can be one Flow session. One shell, one
   session model, many feels.
2. **There are three first-class feels.** Flow, Session + shelf, and Classic desktop (defined below).
   The **Session + shelf** is the default for a new session.
3. **A "Desktop" is a session type, not a mode.** The task *"run these legacy/desktop apps in windows"* is
   itself a thing you are doing, so it is a session like any other — it can be pinned, full-screened, and
   given a dock — and it is reached the same way as any other session, not by a separate switch.
4. **The session model carries the feel.** The feel is data in the session model crate (which, per 0010,
   does not depend on Smithay), so the compositor renders a feel it did not invent. This is what keeps the
   three feels from becoming three shells.
5. **Accessibility (doc 11 item 8) and transparency (P9) are non-negotiable across all three.** A Classic
   desktop that is inaccessible, or a Flow view that cannot be explained or scripted, is refused.
6. **Each feel scales as far as it can run reliably** (Rob, 2026-10-12). A feel is offered where the hardware
   can deliver it and reduced or off where it cannot: Flow and Session + shelf degrade down toward the Seed
   profile; Classic desktop (real windowing) runs only where the compositor can move a window without dropping
   a frame. This is doc 01 P7 — capability turns a feature on or off, it never forks the design — so the feels
   are one codebase, not three.
7. **The feel is inferred, not asked** (Rob, 2026-10-12). The OS chooses a session's feel from what the session
   is opening — the same way the rest of MorpheOS makes its mind up rather than showing a wizard. There is no
   "which feel?" dialog. The choice is a deterministic rule over the session's contents and intent: a single
   legacy app opens into Classic, a task with a few tools into Session + shelf, nothing app-shaped into Flow.
   It is visible and overridable (P9 transparency — the human can change it), but the default is the system
   deciding, never the user being asked.

## The three feels

| feel | who | what you see | the ethos it leans on |
|---|---|---|---|
| **Flow** (intent-first) | everyday (Margaret) | no app grid; surfaces are tasks, people and time; an intent bar; large type; voice | P1 task-is-unit, P9 transparency |
| **Session + shelf** (default) | professional (Ava) | the current task up front; its tools prewarmed around it (doc 18 R0–R2); a thin shelf of the session's capsules, not an app grid | P1, P5 latency budgets, flow-learning |
| **Classic desktop** (explicit) | nerd, legacy apps (Sam) | windows, overlapping, a dock, a terminal, a file manager; everything inspectable and scriptable | P6 compatibility, "for nerds" |

## How "how people actually use computers" maps

The native feel bets on four real habits, taken from doc 02's friction inventory and observation:

1. **Search-first, not icon-first.** People search, they do not file. The intent bar is search wearing the
   OS's clothes; the app grid is the old world's answer to a question the intent bar answers directly.
2. **A handful of live things, not a hundred windows.** Most people live in roughly four ongoing things and
   switch fast. Sessions match that; a window grid does not.
3. **Muscle-memory switching.** Whatever it is called, the switcher behaves like alt-tab / cmd-tab: instant,
   two-key, predictable. This is not optional in any feel.
4. **One thing at a time.** Focus, not clutter. The desktop's real virtue is predictability; we keep that and
   drop the clutter.

## Consequences

- **The session model gains a field** (a `feel`/presentation, with per-feel layout), which is exactly why the
  session model is its own crate — this lands there, not in the compositor.
- **The "desktop becomes the product by default" risk is retired by construction:** a Classic desktop is one
  surface among others, not the shell.
- **Honest sequencing:** the Classic desktop is the expensive feel. Real overlapping windows need Wayland
  `xdg-shell`/`xdg-decoration` on the Smithay side, not the hand-rasterised rectangles the frame and input
  slices use. Flow and the Session + shelf feel can be built on the software-rasterised path first; Classic
  desktop is gated on the real protocol work.
- **CI proof (per H17, a check that never ran is not a pass):** each feel renders a frame and an assertion
  that something was drawn, with a negative control — a blank or wrong-feel frame must fail.

## First buildable slice per feel

1. **Session + shelf:** the session's surfaces plus a shelf component, one focus indicator, and the intent
   bar as a search field. Proved by a frame where focus moves and a query returns a result.
2. **Flow:** session surfaces as tiles (people/time/task), large-type rendering, no app grid. Proved by a
   frame where a tile opens a session.
3. **Classic desktop:** one real `xdg-shell` window on screen, moved and resized, then two. Proved by a
   capture where a window moved and the compositor reported the new geometry.

## Open questions

None open.

*Resolved 2026-10-12 by Rob — "scales as far as it can reliably" (Decision 6), and "the feel makes its mind up
based on the session it's opening" (Decision 7, which also answers the single-legacy-app question: a lone
legacy app opens into Classic because that is what the session is opening).*
