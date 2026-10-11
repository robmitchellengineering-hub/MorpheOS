# 0009 · The heal ladder: six rungs, what each may do alone, and which of them exist

Status: **Accepted** · 2026-10-11

## Context

The wedge is *"never breaks, and heals itself"* ([ADR 0003](0003-unbreakable-wedge.md)). The
first half now has something real behind it: an A/B trial boot that commits on health and
falls back on its own, an updater that writes and verifies a slot, and a VM suite that
proves a bad update is booted **at most once**. The second half — *heals* — has a ladder in
[doc 15](../15-self-healing.md) and no code.

Two things in the doc set disagree about that ladder, and both are cheap to settle now and
expensive to settle later:

1. **How many rungs it has.** [doc 15](../15-self-healing.md) defines **H0–H5**, with
   **Phoenix restore as H5**. [ADR 0003](0003-unbreakable-wedge.md) — Accepted, and an
   accepted record is superseded, never edited — says *"the heal engine (rungs H0–H4), and
   Phoenix restore"*, which reads as Phoenix being **outside** the ladder.
   [doc 10](../10-roadmap.md) follows the ADR and lists them as two separate roadmap items.

2. **What the headline number is.** [doc 00](../00-strategy.md) says *"Delete the system
   folder live on stage, and it heals in **30 seconds**"*. [doc 15](../15-self-healing.md)
   describes that exact demonstration as **H4** and gives it *"**~2–5 min**"*, and its own
   H3 (*"roll back to the last known-good system generation"*) is the rung that costs
   *"**< 30 s (reboot)**"*.

## Decision

**1. The ladder is H0–H5, and Phoenix restore is rung H5.** [doc 15](../15-self-healing.md)'s
table is the definition, because doc 15 owns healing. ADR 0003's sentence names the same six
capabilities; it is a scoping statement about what phase 1 builds, not a claim that the ladder
stops at H4. **This ADR adds and removes nothing** — it settles that two names refer to one
thing, so a later reader does not conclude a rung is missing or duplicated.

**2. The cheapest rung that can work is the one that runs.** Already stated in
[doc 15](../15-self-healing.md); restated here because it is the property that makes the
ladder safe rather than merely ordered.

**3. H0–H4 lose no user data and run without asking. H5 may lose data and must ask first.**
Every rung from H0 to H4 carries *"Data loss: None"* in doc 15's table, so consent would be
theatre. H5 restores *"back to the last sync point"* — a real, if usually small, loss — so it
asks. **One exception, and it is the point of the rung:** H5 may begin on a device that has no
working system at all, because there is nothing left to lose and the alternative is a dead
machine. It must still report what it is doing and what the sync point was.

**4. What each rung is, in this architecture, today.**

| Rung | What it is | State |
|---|---|---|
| H0 | restart the process or service | **not built** |
| H1 | rebuild a capsule from its recipe | **not built** — capsules are phase 2 |
| H2 | roll back one offending transaction | **not built** — needs the transactional Manifest |
| H3 | roll back to the last known-good system generation | **built, as a boot policy**: the A/B trial boot and `morph-boot confirm`'s fallback. The `unhealthy` and `crash` VM scenarios are its end-to-end evidence. **Nothing repairs after it** — see the first consequence |
| H4 | rebuild in place from the immutable recovery image, keeping Strata | **the mechanism exists, unwired**: `crates/morph-update` writes and verifies a slot. What is missing is the immutable recovery image and the decision to invoke it |
| H5 | Phoenix restore from a mesh peer or a backup | **not built** — needs replication, which is not built |

**5. Every time figure in that table is a target, not a measurement.** Nothing in the ladder
has been measured; the closest existing numbers are the x86 scenarios' three boots in about
25 seconds each. [doc 13](../13-physics-and-math.md) already requires that a figure say
whether it was measured or intended, and none of these do.

## Consequences

- **The 30-second demo claim is not supported by the doc set, and it is the one that gets
  said out loud.** Deleting the system root is H4, whose own authoritative figure is
  *"~2–5 min"*; **30 seconds is H3's number, and H3 deletes nothing** — it reboots into the
  other slot, which is a different demonstration. Either the demo is described with the rung
  that performs it, or H4 needs a measurement before the 30-second figure is used again.
  **This ADR records the discrepancy; it does not edit
  [doc 00](../00-strategy.md)**, which is the strategy authority and Rob's to change.
- **The next rung worth building is H4's repair half, and it is small now.** After an H3
  fallback the machine has one good slot and one **broken spare**, and it stays that way until
  the next update — so the redundancy the whole design rests on is absent exactly when it has
  just been needed. `crates/morph-update` can already write a slot; rebuilding the spare from
  the running system restores redundancy **without needing the recovery image** that H4
  otherwise depends on. That is the next piece, and it is provable in the existing VM suite.
- **H2 is blocked on the Manifest's transactional apply**, and H1 on capsules. Neither should
  be attempted before its dependency exists; a ladder with a missing middle rung is still a
  ladder, and pretending otherwise would put a step in it that cannot hold weight.
- **No rung decides alone.** [doc 06](../06-ai-layer.md) and
  [doc 09](../09-security-privacy.md) already fix that the AI proposes and the system
  verifies; the ladder inherits it. A rung is deterministic policy, and the AI may add
  hypotheses only where the playbooks run out.
