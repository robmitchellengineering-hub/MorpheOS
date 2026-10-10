# 13 · Physics, Maths & the Real Speed Limits

"Push the boundaries of physics" is a good instinct, but we can only beat
waste we can measure. This doc sets out the real limits, so the engineering effort
goes where the gains actually are.

## 1. The two clocks: human and hardware

**Human perception thresholds** (the budgets that matter):

| Threshold | Value | Implication |
|---|---|---|
| Pen and touch drag (perceptible lag) | ~1–10 ms | Input-to-photon path must bypass queues |
| Musician monitoring latency (noticeable) | ~5–10 ms | Studio profile round trip under 5 ms |
| Audio/video sync (noticeable) | ~±40 ms | A/V clock unification in the Device Broker |
| "Instant" response | < 100 ms | Every click must show feedback within 100 ms |
| "Flow" maintained | < 1 s | Anything slower needs progress or must be precomputed |
| Attention lost | ~10 s | Anything slower must be backgrounded or contracted (doc 12 §7) |

**Hardware latency ladder** (approximate orders of magnitude):

| Tier | Latency | Relative to L1 cache |
|---|---|---|
| CPU L1 cache | ~1 ns | 1× |
| DRAM | ~100 ns | 100× |
| NVMe random read | ~20–100 µs | ~50,000× |
| LAN round trip (mesh peer) | ~0.2–2 ms | ~1,000,000× |
| Same-city internet | ~5–20 ms | ~10,000,000× |
| Cross-continent | ~70–150 ms | ~100,000,000× |
| SD card random write (Pi) | ~1–20 ms (and highly variable) | ~10,000,000× |

**The core design law:**
> Perceived speed comes from *which tier the answer is in when the human asks*.
> Prediction, prewarming and precomputation (doc 12 §2) don't make hardware
> faster. They move answers **up the ladder** before they're needed.

## 2. Light-speed is a hard floor for the mesh and cloud

Light in fibre covers roughly **200 km per millisecond**. A cloud AI call to a
datacentre 2,000 km away pays at least ~20 ms round trip *before any compute*.
In practice it's 50–300 ms or more.

Implications:
- **Interactive intents must resolve locally or on the LAN.** Cloud is only for
  work that isn't time-critical.
- Mesh offload from a Pi Zero to a desktop costs about 1–3 ms over Wi-Fi, which is fine for
  intents but not for audio buffers.
- The Conductor carries a **latency model per resource**, and only offloads when
  `remote_compute + 2×network < local_compute`.

## 3. Moving data costs more than computing

Moving data costs far more energy than computing on it. Approximate
figures for 45 nm silicon (Horowitz, ISSCC 2014):

| Operation | Energy |
|---|---|
| 32-bit integer add | ~0.1 pJ |
| 32-bit float multiply | ~3.7 pJ |
| 32-bit SRAM cache read (small) | ~5 pJ |
| 32-bit DRAM read | ~640 pJ |

A DRAM read costs **thousands of times** more than an add. Network and
storage I/O cost far more still. Therefore:
- **Compute moves to the data** (data gravity, doc 12 §5).
- **Deduplication and compression save energy as well as space.** Every byte
  not moved is energy saved.
- Strata's content addressing means identical data is **never transferred
  twice** anywhere in the mesh.

## 4. Thermodynamics: the limit we're not near

Landauer's principle sets the minimum energy to erase one bit at
`kT·ln 2 ≈ 2.9 × 10⁻²¹ J` at room temperature. Today's chips spend roughly
**a billion times more** than that per operation. Physics is not what limits
our efficiency; architecture is. That's encouraging, because every gain we need
comes from *not doing* unnecessary work and *not moving* unnecessary bits.

## 5. Queueing theory: why the system feels slow at 80% load

For a simple queue (M/M/1), average wait grows as `W = 1 / (μ − λ)`: as
utilisation `ρ = λ/μ` approaches 1, latency **goes to infinity**.

| Utilisation | Relative wait |
|---|---|
| 50% | 2× |
| 80% | 5× |
| 90% | 10× |
| 99% | 100× |

**The Governor's rule:** interactive resources (P-cores, GPU queue, I/O queue)
keep **reserved headroom**, so they never run above about 70% for foreground classes.
Background work fills the rest and is preempted instantly. That's how you keep
everything feeling instant on a busy machine.

**Little's Law** (`L = λ·W`) lets the Governor turn measured queue depth into
latency predictions in real time, without needing a model of every workload.

## 6. Amdahl's Law for user flows

`Speedup = 1 / ((1 − p) + p/s)`: making part of a flow faster helps only in
proportion to that part's share of the total time.

Applied to "open an attachment":
- If 70% of the time goes to finding or launching the right app and 30% to rendering, a
  10× faster renderer gives only ~1.4×. Removing the launch step (zero-install,
  prewarmed capsules) gives ~3×.

**Rule:** profile *whole flows* (the top 20 tasks), then attack the biggest
serial portion. That's usually orchestration, not computation.

## 7. Information theory: the limits of compression and dedup

Shannon's source-coding theorem: data can't be losslessly compressed below its
entropy. Therefore:
- **Already-compressed media** (JPEG, MP3/AAC, H.264/5, ZIP) gets essentially nothing from
  more compression. Strata detects it and skips compression, which saves CPU.
- **Dedup gains are structural, not magic.** They come from repetition *across*
  objects: versions, VM and capsule bases, the same attachment saved in 5 places,
  and identical OS images across a fleet. There, savings of 5–50× are realistic.

## 8. The economics of prediction

A speculative precompute (doc 12 §2) is worth doing when:

```
p × V(latency_saved)  >  E(compute) × c(energy_now)  +  C(contention)
```

- `p` = predicted probability that the user actually wants it
- `V` = value of the time saved (larger during interactive work)
- `c(energy_now)` ≈ 0 on mains power with idle cores, high on battery
- `C(contention)` must be 0, so precomputation never competes with the foreground

The Predictor is calibrated against this inequality, not against accuracy alone.

## 9. Where the "impossible" wins actually come from

| Perceived as "faster than possible" | Actual mechanism |
|---|---|
| Boot in under 3 s | Resume from snapshot plus parallel I/O replay. Most of the work isn't redone |
| Instant app launch | CoW clone of a prewarmed snapshot |
| Instant export or render | Done earlier, speculatively (§8) |
| A Pi Zero answering AI queries | Offload to a mesh GPU (§2) |
| A 1 TB library on a 256 GB laptop | Tiered and streamed, with data gravity |
| "It never breaks" | Atomic transactions, proof-checked invariants, multiverse forks |

None of these break physics. All of them avoid waste.
