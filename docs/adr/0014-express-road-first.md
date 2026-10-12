# 0014 · Express road: a general user drives it first, the rest re-sequences behind it

Status: **Accepted** · 2026-10-12

Supersedes: the phase sequencing in [doc 10](../10-roadmap.md). Does **not** change the vision or the wedge
in [doc 00](../00-strategy.md). **Nothing is dropped** — every doc 10 item is re-sequenced, not removed.

## Context

Doc 10 lays out a four-phase, ~30-month plan (foundations → unbreakable → daily driver → everything talks).
Read against reality it is long for the wrong reason: it is ~10 subsystems serialized one after another, and
its first phase — the "never breaks" wedge — is **already built and VM-proven**: ADRs 0001–0013, the Manifest,
the image factory, the transaction / heal / update crates, Strata v0, and the Flow shell's frame, input,
session model and two visible feels. Rob's direction (2026-10-12): *"let's not lose anything, but express road
to a general user driving it."*

The 30 months is a *vision horizon* wearing a *project timeline's* clothes. The product — a machine that
cannot break and that a person can sit at — is much closer than the number suggests, because the hard part is
done.

## Decision

**Two lanes instead of four phases:**

1. **The express road** — ship a machine a general user drives, in this order:
   1. finish the shell so it hosts a real window (Classic windows on `xdg-shell`, ADR 0013 item 3),
   2. a browser — the app that makes it a computer (a package, not code we write),
   3. real hardware on one reference laptop — wifi, keyboard/touchpad, display,
   4. persistence + A/B + heal on that laptop (already built; this is booting it bare-metal, not new code),
   5. nothing breaks — the wedge, already built, running on real hardware.

   **Exit:** a general user boots, gets online, browses, and cannot make the machine unbootable.

2. **The widening backlog** — every remaining doc 10 item, re-sequenced and gated, none dropped:
   capsules (Wine/Android/microVMs), the AI layer (additive — AI proposes, deterministic decides), the mesh,
   Morph Link (one platform first, then four), the Resource Governor / Device Broker / Studio profile,
   Phoenix + hardware monitoring, Strata native (kept "if measurements justify it"), interop protocols, paid
   Fleet (revenue-gated), Flow learning, Any-hardware / GPU / Windows pro apps, and Phase 4's phones, Morph
   kernel and OEM (always "ongoing").

## Consequences

- The wedge stops waiting on the hardest part (it is done); the shell has been landing a slice a day, not the
  ~9 months doc 10's Phase 2 allocated.
- The long tail (capsules, AI, 4-platform Morph Link, native Strata, fleet) becomes releases behind a live
  product rather than a pre-revenue gate — the same "tasks point toward plan milestones" discipline as the
  workspace `BACKLOG.md`.
- **The honest remaining risk is hardware, not code**: wifi and a browser on a real laptop is the one thing
  that must be tried, not estimated.
- Doc 10 is **not edited** (the vision is the target); it stays the full item list, and this ADR records the
  re-sequencing on top of it.
