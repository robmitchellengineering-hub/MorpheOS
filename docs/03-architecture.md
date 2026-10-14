# 03 · System Architecture

## Layer overview

```
┌──────────────────────────────────────────────────────────────────────┐
│  EXPERIENCE         Flow shell · Intent bar · Sessions · Compositor  │
├──────────────────────────────────────────────────────────────────────┤
│  INTELLIGENCE       Intent Engine · Configurator · Predictor         │
│                     (AI proposes → Verifier decides)                 │
├──────────────────────────────────────────────────────────────────────┤
│  ENVIRONMENTS       Capsules: native · Linux · Windows · Android ·   │
│                     web · microVM · translated (x86↔ARM)             │
├──────────────────────────────────────────────────────────────────────┤
│  CORE SERVICES      morphd (state & transactions) · Strata storage · │
│                     Resource Governor · Device Broker · Mesh         │
├──────────────────────────────────────────────────────────────────────┤
│  KERNEL             Linux (phase 1–2) → Morph kernel option (phase 4)│
├──────────────────────────────────────────────────────────────────────┤
│  HARDWARE           Pi Zero 2 W … laptop … multi-socket server       │
└──────────────────────────────────────────────────────────────────────┘
```

## Kernel strategy (pragmatic first, radical later)

Writing a new kernel *and* drivers for every chip is the reason most new
operating systems never ship. Our approach:

- **Phase 1–3: the Linux kernel, used as a hardware abstraction layer.** It covers
  everything from ARMv8 SBCs to 400-core servers, brings the drivers, and has
  KVM, cgroups v2, namespaces, io_uring, eBPF, sched_ext and PREEMPT_RT
  (merged upstream). We use a curated config per profile with
  MorpheOS-specific eBPF/sched_ext schedulers and LSM policy. **Nothing GNU or
  traditional-distro is visible above it.**
- **Phase 4+: the Morph kernel (optional research track).** A capability-based
  Rust microkernel, in the spirit of seL4, Redox and Theseus, for high-assurance profiles. It
  would host Linux drivers in isolated driver VMs (the "driver capsule"
  approach), so we never lose hardware support.

Our real advantage is in *everything above the kernel*. The kernel is swappable
because our core services talk to it through a narrow internal API (`morph-hal`).

## Core services

### `morphd`: the state and transaction daemon

> **Superseded in part by [ADR 0017](adr/0017-systemd-is-pid-1.md):** systemd is PID 1 for v1, and
> `morphd` is a service under it. It does not replace init or the service manager; what follows
> about the Manifest and transactions stands.

- Replaces the package manager and settings store, and drives the service manager.
- The whole machine is described by a **Manifest**, a typed, declarative document
  (the schema language is still open: Nickel, KCL, CUE or our own).
- Every state change is a **Transaction**: `plan → validate → snapshot → apply → health-check → commit | rollback`.
- Boot is a dependency graph that runs in parallel and is driven by events and sockets. User-visible
  services start first; everything else is deferred.
- The system image is immutable, with A/B or snapshot-based roots. Mutable state lives in Strata.

### Strata: storage ([04](04-strata-filesystem.md))
A content-addressed, versioned object store with a metadata graph and semantic
index. It is exposed through a native API, a POSIX view for compatibility, and
query-based "virtual folders".

### Resource Governor
- One governor owns CPU, GPU, NPU, memory, I/O, network and power.
- It works from **intent classes** rather than nice values:
  `interactive` › `realtime` (audio/video) › `foreground-batch` › `background` › `opportunistic`.
- It is implemented with cgroups v2, a sched_ext BPF scheduler, PSI pressure signals,
  io_uring priorities, GPU and NPU queue arbitration, and memory compression (zram/zswap).
- Every intent class has a latency budget, and the governor enforces those budgets first.

### Device Broker
- Hardware is assigned to **Sessions**, not apps. The "video call" session gets the
  camera, mic and echo cancellation. The "studio" session gets exclusive audio-interface access.
- It handles hot-plug and suggests or applies drivers and firmware through transactions.
- It sits on PipeWire (audio/video graph), libinput, Wayland, and
  KMS/DRM for display.

### Mesh (Constellation)
- Your MorpheOS devices find each other and form a trusted mesh
  (WireGuard-based, Noise-authenticated device identities).
- The mesh shares storage tiers, compute offload (for example a Pi asks a desktop GPU to run an
  AI model), clipboard, sessions and capsules.

## The Session model (the core UX abstraction)

A **Session** is a live, persistent workspace for one task or goal:

```yaml
session: "Q3 report"
intent: document.authoring
capsules: [docs-editor, browser(profile=work), spreadsheet]
files: strata://query?project=q3-report
devices: { display: primary, audio: default }
resources: { class: interactive }
network: { profile: work-vpn }
history: continuous        # can be rewound as a whole
```

- You can suspend, resume, move a session to another device, share it, or rewind it as a unit.
- Apps are *contributors* to a session, not the top-level concept.
- Most sessions are created implicitly: opening a file or starting a call creates one.

## Experience layer

- **Flow shell:** a Wayland compositor (built on Smithay, in Rust) with a session-centric
  window model, a universal **Intent Bar** (type or speak), and a timeline view.
- **For nerds:** `morph` CLI, Manifest files, a full REST/gRPC/D-Bus-style API,
  and a plain terminal with a POSIX environment available at any time.
- **Headless profiles:** the same API over SSH or HTTPS, with no compositor.

## Internal API sketch

```text
morph.state.plan(diff)          → Plan (with risk score, affected services)
morph.state.apply(plan)         → TxId
morph.state.rollback(txId)      → ok
morph.strata.find(query)        → [ObjectRef]
morph.strata.at(ref, time)      → ObjectRef@version
morph.capsule.run(target, ctx)  → CapsuleHandle
morph.session.create(intent)    → Session
morph.governor.reserve(session, class, budget)
morph.mesh.offload(job, constraints)
```

## Language & tooling

- **Rust** for all core services, the compositor and Strata (memory safety, no GC pauses,
  embedded through to server).
- **Zig or C** only where interfacing with low-level firmware or drivers requires it.
- **eBPF** for scheduler and policy hooks.
- **WASM (WASI)** as the portable plugin and extension format: sandboxed and architecture-neutral.
