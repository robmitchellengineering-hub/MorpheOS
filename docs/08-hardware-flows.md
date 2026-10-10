# 08 · Hardware Flow Optimisation

Rather than optimising components one at a time, MorpheOS optimises **flows**:
the path a user action takes through the hardware. Below, each part of a typical
laptop is mapped to the flows it takes part in, and to what we change.

## Component map of a typical modern laptop

| Component | Typical reality | Where time and energy are wasted today | MorpheOS approach |
|---|---|---|---|
| **CPU** (hybrid P/E cores) | 8–16 cores, mixed performance and efficiency cores | Background work on P-cores, foreground on E-cores, frequency ramp delays | Intent-class scheduling (sched_ext): interactive → P-cores with a fast ramp, background → E-cores capped |
| **GPU** (iGPU/dGPU) | Shared memory or discrete VRAM | dGPU woken for trivial work, compositor overdraw | Compositor using direct scanout and overlay planes; dGPU powered only for sessions that need it |
| **NPU** | 10–50 TOPS, mostly idle today | Unused | Default target for the AI layer: embeddings, Predictor, background transcription |
| **RAM** | 8–32 GB | Duplicated libraries across apps and VMs, cold-start churn | Shared deduped capsule layers, KSM for VMs, zram/zswap, memory budgeted by session priority |
| **NVMe SSD** | 3–7 GB/s, very low latency | Serial I/O at boot, small random writes, fsync storms | io_uring everywhere, parallel prefetch from a recorded boot and launch profile, log-structured writes in Strata |
| **Battery & PMIC** | 50–100 Wh | Polling, wakeups, background syncs | Event-driven services, batched timers, race-to-idle, mesh-aware sync windows |
| **Display** | 60–165 Hz, often VRR | Rendering at full rate when nothing changes | Panel self-refresh, VRR down to 1–10 Hz when static, damage-only rendering |
| **Wi-Fi / BT** | Wi-Fi 6/7, BT LE | Scan storms, captive portal confusion, slow reconnect | Predictive reconnect, profile cache per location, an automatic captive-portal capsule |
| **Audio** | Onboard codec, USB interfaces | Large buffers, no RT guarantees, sample-rate thrashing | PipeWire graph with Studio RT profile: isolated cores, threaded IRQs, fixed quantum, no resampling |
| **Camera / mic** | Webcam, array mics | Per-app permissions, duplicated processing | One processing pipeline per session (noise suppression and framing run once, on the NPU) |
| **Input** | Keyboard, touchpad, touch, pen | Input latency hidden behind compositor queues | Input→photon budget < 10 ms: high-priority input thread, latest-frame-wins presentation |
| **Sensors** | Lid, ambient light, accelerometer, presence | Ignored | Inputs to context for the Predictor (lid closing → snapshot the session, presence → wake early) |
| **TPM / secure enclave** | TPM 2.0 | Only used for BitLocker-style encryption | Measured boot, Manifest attestation, device mesh identity, key storage |

## Flow: power-on → working

Target: under 3 seconds to the last session on an NVMe laptop.

1. Firmware → **UKI** (unified kernel image) loaded directly. No bootloader menu delay.
2. The kernel starts `morphd` immediately, with a minimal initramfs or none at all.
3. `morphd` **replays a recorded boot I/O profile** through parallel io_uring reads,
   so the SSD is fully used rather than read serially.
4. The compositor and the last session's capsules restore from **snapshot images**
   (the CRIU/VM-snapshot approach). They resume rather than start from scratch.
5. Non-visible services (indexing, sync, updates) are deferred until after first
   input, and run in the `background` class.

Optional **"instant on"**: suspend-to-RAM by default, with a hybrid hibernate image
written in the background, so there's no data loss if the battery dies.

## Flow: app or capsule launch

1. The Predictor has already prewarmed likely capsules and file chunks in idle time.
2. On launch, the capsule CoW-clones a frozen snapshot, which takes milliseconds.
3. Pages are mapped lazily from Strata, with hot pages already in the page cache.
4. The Governor gives the new foreground session a short burst of P-cores at boosted
   frequency (a "launch boost").

## Flow: video call

1. The user clicks a meeting link → the Intent Engine creates a `call.join` session.
2. The Device Broker attaches the preferred camera, mic and output (learned per context).
3. One NPU pipeline runs noise suppression, echo cancellation and background blur,
   once for every app.
4. The Governor reserves the `realtime` class for the A/V threads and limits background
   work, so there are no dropped frames when the indexer wakes.

## Flow: low-latency audio (the Studio profile)

Target: a stable round trip under 5 ms at 48 kHz on supported USB interfaces.

- Choose the PREEMPT_RT kernel variant, or tune threaded IRQs on a standard kernel.
- Reserve 1–2 cores (`isolcpus`/cgroup cpuset) for the audio graph.
- Run PipeWire with a fixed quantum (64–128 frames) and no resampling when the hardware rate matches.
- Hold CPU frequency and C-states at a floor while the session is active, and release them at once afterwards.
- Run Windows VST plugins through a Wine-bridged plugin host (as yabridge does), with per-plugin
  capsules so that one plugin crashing can't take the DAW down.
- When a buffer underrun happens, the Governor records the cause (which IRQ, which process)
  and the Configurator proposes a fix.

## Flow: shutdown / lid close

- Close the lid → snapshot the session state → suspend. Strata already has every write on
  disk, so there's nothing to "save".
- Real shutdown is rare, and is just "checkpoint and power off" in under a second.

## Where we push past standard practice

These are the research tracks:

- **Learned I/O prefetch:** a per-device model that predicts chunk access
  sequences for boot, launch and session restore.
- **Cross-capsule memory dedup:** content-addressed page sharing that works with Strata
  chunk hashes (in the spirit of KSM, but cooperative instead of scanning).
- **Energy-aware scheduling with a real power model:** measure joules per task on
  each core type and schedule to minimise energy within the latency budget.
- **Speculative session restore:** start restoring the most likely session when
  the lid begins to open (using the hinge sensor and presence sensor).
