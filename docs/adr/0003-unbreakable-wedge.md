# 0003 · "Never breaks, self-heals" is the wedge

**Status:** Accepted, 2026-10-10

## Context
A vision this broad can't ship all at once. People switch operating systems for one thing that is
ten times better.

## Decision
Phase 1 builds only what makes a machine unbreakable: immutable A/B system,
transactional Manifest, versioned storage, the heal engine (rungs H0–H4), and Phoenix
restore. Phase 1 is done when the public "Try to Break It" suite (doc 15) passes on a
Pi and an x86 laptop.

## Consequences
- Compatibility capsules, the AI shell and the mesh UX wait for phase 2.
- Every Manifest generation must satisfy the unbreakable invariants (ADR 0005).
