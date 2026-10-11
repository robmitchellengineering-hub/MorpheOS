# 10 · Roadmap

We ship the experience early on top of a Linux kernel, prove that the concepts
remove friction, and go deeper (native Strata, the Morph kernel) only where
measurements show it's needed.

## Phase 0: Foundations (months 0–3)
- [ ] Freeze v0 of this design, and write ADRs (architecture decision records) for the key choices
- [ ] Draft the Manifest schema: system, services, capsules, sessions, **invariants**
- [ ] Reference hardware: **Pi Zero 2 W**, **Pi 5**, one **x86 laptop**, and later one **ARM laptop**
- [ ] Image factory: reproducible images (mkosi or custom), UKI, A/B roots, built on CI. Reuse Morpheus's image and flash pipeline patterns ([17](17-morpheus-family.md))
- [ ] Benchmark harness: the "Top 20 tasks" ([02](02-use-case-analysis.md)) plus the **"Try to Break It" suite** ([15](15-self-healing.md)), with baselines on Windows 11, macOS and Ubuntu

## Phase 1: "Unbreakable" (months 3–9): the wedge
Goal: a bootable MorpheOS that **cannot be broken**, on a Pi and an x86 laptop.
- [ ] `morphd`: PID 1, the transaction engine, Manifest generations, invariant checks, parallel boot
- [ ] Immutable, verified system image, A/B boot with a one-shot trial ([ADR 0008](adr/0008-one-try-trial.md)), an immutable recovery image
- [x] Strata v0: versions on every save, file restore, folder rewind (undoable), ransomware burst detection ([ADR 0007](adr/0007-strata-v0-watcher.md))
- [ ] Strata: run the watcher as a service in the images, metadata, full-text search
- [ ] Heal Engine v0: health probes, deterministic playbooks (boot, network, display, audio, disk), healing ladder H0–H4
- [ ] Phoenix v0: encrypted replication to a second MorpheOS device or a USB disk, and bare-metal restore
- [ ] Hardware health monitoring: NVMe/SMART, battery, thermals, SD wear
- [ ] Minimal shell: the `morph` CLI, plus a simple compositor with a resilience status and a timeline/undo UI
- **Exit criterion:** passes the full "Try to Break It" suite on both reference devices, publicly.

## Phase 2: "Daily driver" (months 9–18)
- [ ] Capsules: Linux containers, Wine/Proton with prewarmed snapshots, Android, FEX/box64 translation, microVMs
- [ ] Flow shell v1: Intent Bar, Sessions
- [ ] Intent Engine plus Configurator: AI first-boot setup, AI diagnosis added to the Heal Engine playbooks, Windows and macOS migration import
- [ ] Resource Governor (sched_ext, intent classes), Device Broker, Studio audio profile
- [ ] Mesh v1 between MorpheOS devices: sharing, handoff, compute offload, mesh-based Phoenix
- [ ] Heal recipe learning (opt-in, no user data)
- [ ] Flow learning v1: Flow Recorder, counting model, R0 prefetch and R1 capsule prewarm ([18](18-flow-learning.md))
- [ ] Any-hardware v1: D1 driver recipes, D3 userspace device SDK (USB/HID/serial), Device Doctor ([19](19-any-hardware.md))
- [ ] Managed GPU stack (AMD, Intel, NVIDIA) with kernel and driver updated as one A/B trial, per-capsule driver pinning, Proton gaming capsules, shared shader caches ([19 §4](19-any-hardware.md))
- [ ] "MorpheOS capsule" as a Morpheus build target

## Phase 3: "Everything talks" (months 18–30)
- [ ] Interop protocols built into the OS ([16](16-interop-and-link.md)): SMB, Quick Share, casting, IPP, Matter, cloud drives, enterprise directory join
- [ ] **Morph Link** on Windows, macOS, Android and iOS: mesh, sync, Phoenix key, and Windows healing
- [ ] **Paid:** Fleet console (policy, attestation, rings), hosted relay, hosted encrypted backup, premium AI
- [ ] Strata v1 native format (if measurements justify it), semantic index, Predictor
- [ ] Security audit; publish the Manifest, capsule and heal-recipe specs
- [ ] Flow learning v2 (pre-connect, pre-configure, NPU model) and AI-generated D3 drivers through the Morpheus loop
- [ ] Windows pro apps (SolidWorks, Premiere, AutoCAD) in VMs with GPU passthrough, SR-IOV or single-GPU handover

## Phase 4: "Everywhere" (ongoing)
- MorpheOS on unlocked phones, with convergence (the phone docks to become the desktop)
- Morph kernel research (a capability microkernel with Linux driver capsules)
- Doc 12 research tracks: precomputed futures, proof-carrying changes, liquid processes
- Hardware and OEM partners, and a DRM certification path
- R4 precompute-in-fork (doc 18) and D4/D5 driver VMs and promotion (doc 19)

## Proposed repository layout

```
morpheos/
├── README.md
├── docs/                     # this design set + ADRs
│   └── adr/
├── manifest/                 # Manifest schema + examples
├── crates/
│   ├── heal/                 # heal engine, playbooks, phoenix
│   ├── morphd/               # PID 1, transactions, service graph
│   ├── morph-hal/            # kernel abstraction
│   ├── strata/               # object store, graph, index, FUSE view
│   ├── capsule/              # planner, runtimes, snapshots
│   ├── governor/             # resource governor (+ bpf/ sched_ext)
│   ├── broker/               # device broker
│   ├── mesh/                 # constellation
│   ├── intent/               # intent engine, model cascade, verifier
│   ├── flow-shell/           # compositor + UI
│   └── morph-cli/
├── images/                   # build recipes per profile (seed/desk/studio/fleet)
├── compat-db/                # recipe format + seed recipes
├── bench/                    # top-20 task benchmark harness
└── tools/
```

## Tech stack summary

| Concern | Choice | Why |
|---|---|---|
| Kernel | Linux (curated configs, PREEMPT_RT option) | Drivers, scale, KVM, eBPF |
| Core language | Rust | Safety, performance, embedded through to server |
| Extensions | WASM / WASI | Sandboxed and portable |
| Compositor | Smithay (Wayland) | Rust, flexible |
| Audio/video | PipeWire | De facto standard, low latency |
| Windows compat | Wine, Proton, DXVK, VKD3D-Proton | The most mature option |
| x86-on-ARM | FEX-Emu, box64 | Fast, actively maintained |
| VMs | KVM + Cloud Hypervisor/Firecracker | Lightweight and fast to start |
| Containers | Our own runtime on namespaces/cgroups v2 (OCI-compatible) | Integration with Strata layers |
| Snapshots | CRIU, VM snapshots | Instant capsule start |
| Local AI | llama.cpp / ONNX Runtime (NPU backends) | Portable, quantised |
| Hashing / compression | BLAKE3, zstd, lz4 | Fast, proven |
| Mesh | WireGuard + Noise | Simple, secure |
| Image build | mkosi or custom, UKI, systemd-boot-style loader | Reproducible, atomic |

## First concrete steps
1. ✅ ADRs 0001–0005 ([docs/adr](adr/README.md)).
2. ✅ Manifest v0 with invariants and risk-tiered plans (`crates/morph-manifest`), with example machines in `manifest/examples`.
3. ✅ Image factory spike: `morph-boot` plus an A/B x86 VM image (tested: good, unhealthy and crashing updates) and a Pi 5 A/B image built on CI with rpi-image-gen ([ADR 0006](adr/0006-image-factory.md)).
4. Prototype `strata-fuse`: CoW versions plus `morph strata at <time> <path>`.
5. Prototype the Heal Engine loop with two playbooks (broken network config, failed boot) and healing rungs H2 and H3.
