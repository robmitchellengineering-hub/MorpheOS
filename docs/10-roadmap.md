# 10 · Roadmap

We ship the experience early on top of a Linux kernel, prove that the concepts
remove friction, and go deeper (native Strata, the Morph kernel) only where
measurements show it's needed.

## Phase 0: Foundations (months 0–3)
- [ ] Freeze v0 of this design, and write ADRs (architecture decision records) for the key choices
- [ ] Draft the Manifest schema: system, services, capsules, sessions
- [ ] Reference hardware: **Pi Zero 2 W**, **Pi 5**, one **x86 laptop** (Intel or AMD with an NPU) and one **ARM laptop**
- [ ] Benchmark harness: automate the "Top 20 tasks" from [02](02-use-case-analysis.md) and measure them on Windows 11, macOS, Ubuntu and Fedora for baselines
- [ ] Build system: reproducible images (mkosi or a custom tool), UKI, A/B roots

## Phase 1: "Morph userland" MVP (months 3–9)
Goal: boot into MorpheOS on the reference hardware and show the core ideas.
- [ ] `morphd`: PID 1 with the transaction engine, Manifest apply and rollback, and parallel boot
- [ ] Strata v0: a Rust FUSE layer over btrfs or bcachefs, with versions, time travel, a metadata graph and full-text search
- [ ] Capsules v0: rung 1 (Linux containers) and rung 3 (Wine), with prewarmed snapshots
- [ ] Flow shell v0: a Smithay compositor with the Intent Bar and Sessions
- [ ] Intent Engine v0: deterministic matchers plus a local small model through llama.cpp
- [ ] `morph` CLI
- **Demo:** double-click a Windows installer on an ARM laptop and it runs. Ask "Find the PDF Sam sent last week" and it's found. Break the Wi-Fi config and one click recovers it.

## Phase 2: Daily-driver alpha (months 9–18)
- [ ] Resource Governor with a sched_ext scheduler and intent classes
- [ ] Device Broker: session-scoped devices and PipeWire integration, including the Studio audio profile
- [ ] Capsules: Android, the FEX/box64 translation rung, and microVMs (Cloud Hypervisor)
- [ ] The compatibility DB service
- [ ] Configurator: first-boot AI setup, migration from Windows and macOS, diagnostic playbooks
- [ ] Constellation mesh v0: device pairing, file sharing, compute offload
- [ ] Seed profile: headless Pi Zero 2 W image under 64 MB RAM

## Phase 3: Beta + Fleet (months 18–30)
- [ ] Strata v1 native on-disk format (if the v0 measurements justify it)
- [ ] Semantic index and the Predictor (NPU acceleration)
- [ ] Fleet Manifests, attestation, update rings, a web console
- [ ] GPU-accelerated microVMs for rung 6
- [ ] Security audit, and a public spec for the Manifest and capsule formats

## Phase 4: Research tracks (ongoing)
- Morph kernel (a capability microkernel with Linux driver capsules)
- Learned I/O prefetch and an energy-aware scheduler with power modelling
- Cross-capsule content-addressed memory sharing

## Proposed repository layout

```
morpheos/
├── README.md
├── docs/                     # this design set + ADRs
│   └── adr/
├── manifest/                 # Manifest schema + examples
├── crates/
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

## First concrete steps (this week)
1. Create the `morpheos` repo and copy this folder in as the root.
2. Add `docs/adr/0001-linux-kernel-first.md` and `0002-rust-core.md`.
3. Write the first draft of the Manifest schema with 3 example machines (Seed Pi, Desk laptop, Studio).
4. Prototype `strata-fuse`: CoW versions plus `morph strata at <time> <path>`.
5. Prototype the capsule planner on Wine: a prewarmed base with CoW clones, and measure cold vs warm launch.
