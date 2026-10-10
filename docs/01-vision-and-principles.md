# 01 · Vision & Principles

## The problem

Today's operating systems were designed around the hardware and habits of the
1970s–90s: a hierarchical filesystem, one program per window, the user as system
administrator, and a separate world for each platform. Since then, computers
have grown a thousand times faster, but much of the *perceived* speed went into:

- **Waiting**: boot, updates, app launch, indexing, a spinning wheel during "Preparing…"
- **Administering**: drivers, settings, permissions, storage cleanup, backups
- **Translating**: "which app opens this?", "this only runs on Windows", "where did I save it?"
- **Recovering**: broken updates, corrupted configs, lost files, "it worked yesterday"

None of this is the work the user sat down to do. MorpheOS treats all of it as a bug.

## Vision

> **A computer you never have to manage, which runs anything, finds anything,
> undoes anything, and scales from a $15 board to a datacentre.**

- **For everyday users:** it just works. No setup wizards, no driver hunts, no
  "which version of Office". Tell it the goal ("join this call", "make my
  photos into a slideshow", "print this").
- **For nerds:** everything is a declarative, inspectable, scriptable state.
  The AI is a fast pair of hands, never a black box. You can see, diff and edit
  every change it makes.
- **For enterprise:** fleet-wide declarative policy, reproducible machines,
  instant rollback, a full audit trail, and the same OS on edge devices and servers.

## Design principles

These are the rules we won't break. When a design choice goes against one of them, the design loses.

1. **The task is the unit, not the app.**
   The OS models *what you are doing* (a "Session": writing a report, mixing a
   track, running a service) and gathers apps, files, network and compute around it.

2. **Every change can be undone.**
   Every file write, setting change, install and AI action is a versioned,
   reversible step. "Undo" is a system-wide verb that covers time, not just text.

3. **AI proposes; deterministic systems decide.**
   AI never sits on the critical path of correctness. It produces *plans* and
   *config diffs*. A deterministic engine validates them against schemas and
   policies, applies them atomically, and keeps the previous state available to
   roll back to. The AI can only move between **known-good states**.

4. **Fully offline-capable, local-first.**
   Core functions must work with no network and no cloud model. Cloud and peer
   compute are optional accelerators, used only with consent.

5. **Latency budgets are product requirements.**
   Every user-visible interaction has a budget, such as input-to-photon < 10 ms,
   app/capsule ready < 300 ms warm and < 2 s cold, resume < 1 s, search < 50 ms.
   Background work must never break a foreground budget.

6. **Compatibility is non-negotiable.**
   If it ran on a computer before, it should run on MorpheOS. We pick the
   *cheapest correct environment* for it, whether native, translated, containerised or
   virtualised, without the user ever having to choose.

7. **One system, every scale.**
   There's one codebase and one set of concepts. Hardware capability turns features on or off.
   It never forks the design.

8. **Use the best existing work; invent only where it pays.**
   The Linux kernel and its drivers, Wine/Proton, FEX/box64, KVM, Wayland,
   zstd, llama.cpp and other proven tools are building blocks. We put new engineering into the
   *integration and the model*, where nobody else has done it.

9. **Transparency over magic.**
   The system can always explain what it did, why, and how to reverse it, in
   plain language and in machine-readable form.

10. **No physics violations, but zero tolerance for waste.**
    We won't beat the speed of light or the limits of NAND flash. We will remove the
    vast majority of slowness, which is software waste: serial boot chains,
    cold caches, redundant copies, polling, bloat and bad scheduling.

## What MorpheOS is *not*

- **Not a Linux distro with a chatbot.** It replaces everything above the
  kernel: init, storage semantics, the package and app model, the shell, the
  compositor and configuration.
- **Not cloud-dependent.** The AI layer degrades gracefully down to a pure
  deterministic system on a Pi Zero.
- **Not a walled garden.** Open formats, open specs, exportable state.

## Success metrics (north stars)

| Metric | Target |
|---|---|
| Time from power-on to doing the task | < 3 s (laptop), < 10 s (Pi Zero 2 W) |
| "I need to fix my computer" incidents per user per year | → 0 |
| Share of everyday Windows apps that run without user intervention | > 90% (non-anti-cheat) |
| Time to find any file, given a vague description | < 2 s |
| Unrecoverable data loss events caused by the OS or the user's mistakes | 0 |
| Fresh machine to "my setup" | < 5 min, unattended |
