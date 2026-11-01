# 07 · Scaling Profiles: Pi Zero → Enterprise

One codebase, one set of concepts, one Manifest schema. **Profiles** are
presets of which components are enabled and how they're tuned. They're not forks.

## Profiles

| | **Seed** | **Desk** | **Studio** | **Fleet** |
|---|---|---|---|---|
| Example hardware | Pi Zero 2 W (4×A53, 512 MB), SBCs, IoT | Laptops, mini PCs, 8–16 GB | Workstations, gaming PCs, 32 GB+, dGPU | Multi-socket servers, clusters |
| UI | Headless or a minimal kiosk compositor | Full Flow shell | Full Flow shell, multi-display, RT audio | Headless with a web console |
| Strata | Log-structured, flash-aware, inline dedup off | Full | Full and serves the mesh as a tier | Distributed, replicated, erasure-coded |
| Capsules | Native/Linux, WASM, box64 (lightweight) | All rungs up to microVM | All rungs, GPU passthrough | Containers/microVMs as the workload platform |
| AI | Deterministic plus mesh offload | Small local model | Medium local model, serves the mesh | Shared inference service |
| Base footprint target | < 64 MB RAM, < 300 MB disk | < 600 MB RAM idle | Same as Desk | Depends on role |

> **Original Pi Zero (ARMv6, single core):** a stretch target, as a **"Node"**
> sub-profile (a headless sensor or mesh-relay agent) rather than a full OS
> experience. The Zero 2 W (ARMv8, 64-bit) is the real minimum.

**The Slate (a tablet)** is the Desk profile in another shape: the same x86 image, the same Flow
shell, a touch screen instead of a pointer and a battery instead of a wall socket. It is a line
of its own — [23](23-tablet-line.md), [ADR 0021](adr/0021-slate-line.md) — because touch, an
on-screen keyboard and suspend are questions the Desk profile has never had to answer; it adds
no tier, no kernel and no second image. It is **not** reachable on iPad or any other iOS
hardware — doc 23 §3 gives the reasons with sources.

## How one design scales down

- Every core service is **modular, with features chosen at compile time and at runtime**
  (Rust features plus Manifest flags).
- On Seed, `morphd`, the Strata core and the Governor run as **one static binary**.
- Services on Seed use no background polling, and everything is event-driven, which saves both battery and SD-card wear.
- The root filesystem is read-only, with writes batched in a log, so pulling the power can't
  corrupt the system.

## How one design scales up

- **Fleet Manifest:** one declarative document describes thousands of machines
  (roles, policies, capsules, update rings).
- **Attestation:** each machine proves, via TPM or measured boot, which Manifest generation it's
  running, so drift is detectable and fixable automatically.
- **Workload capsules:** the same capsule format runs services, so microVMs and containers
  scale across hosts. An orchestrator speaks the same API and could be compatible with a
  Kubernetes subset at the edge.
- **NUMA- and topology-aware Governor:** it pins capsules to sockets and
  CCDs, and pools GPU and NPU across them.
- **Strata, distributed:** erasure-coded across nodes, with dedup across the fleet
  (for example, a thousand identical base images stored once).

## Constellation: your devices as one computer

Every MorpheOS device can join a personal or org **mesh**:

- **Identity:** each device has a keypair, enrolled by tapping, QR code or org policy.
- **Transport:** WireGuard tunnels, with LAN discovery via mDNS and NAT traversal
  through optional relays.
- **What gets shared:**
  - *Storage tiers:* a laptop treats the home server as a cold tier, and a phone or
    Pi pulls files on demand.
  - *Compute:* "Run this AI request, video export or compile job on the fastest idle
    device", within your policy.
  - *Sessions:* start writing on the desktop and continue on the laptop.
  - *Capsule caches and translation caches:* each one is downloaded or computed once per mesh.
- **Example:** a Pi Zero 2 W running a home dashboard asks the desktop GPU for
  natural-language queries. If the desktop is asleep, it falls back to deterministic intents.

## Update model (all profiles)

- Updates are atomic: a new root image, or a snapshot plus a Manifest generation.
- Rollout is staged in **rings** (canary → early → broad), and automatically rolled back
  if health checks fail.
- Deltas are content-addressed, so a device downloads only the chunks it doesn't already have,
  possibly from a mesh peer.
