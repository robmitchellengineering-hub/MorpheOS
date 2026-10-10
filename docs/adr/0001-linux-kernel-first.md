# 0001 · Use the Linux kernel; innovate above it

**Status:** Accepted, 2026-10-10

## Context
New operating systems usually die on hardware support. MorpheOS has to run on everything
from a Pi Zero 2 W to multi-socket servers, on day one.

## Decision
Phases 1–3 use the upstream Linux kernel as a hardware abstraction layer, with
curated configs per profile. All MorpheOS services talk to it through a narrow internal
API (`morph-hal`). A new kernel (the Morph kernel) is a phase-4 research track only.

## Consequences
- We get drivers, KVM, cgroups v2, io_uring, eBPF, sched_ext and PREEMPT_RT for free.
- The kernel is GPL-2.0. MorpheOS userland stays separately licensed (ADR 0004).
- Nothing GNU or traditional-distro is exposed as the user experience.
