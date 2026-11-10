> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# PLAN — The MorpheOS machine: AI-first OS × AI-first hardware (the custom computer)

Status:      D36 recorded — buyers chosen (secure field + broadcast lane); proof path is USB MIDI → desk brain
Decisions:   D4 (Morpheus family), D12 (one spine, no fifth product), D15 (physics framing / crank filter),
             D17 (two editions), D19 (own the kernel), D20/D22 (Provenance Kernel → Morph), D26 (secure arm),
             **D36 (first buyers + the MIDI proof path)**
Verified:    live GitHub state read 2026-11-07 (below), not recalled
Owner:       Rob (what the product is, who buys it) / agent (the ladder, the falsifiers, the hardware map)

---

## 0. Catch-up — where the "custom kernel, secure" work actually stands (verified today)

Two tracks, and they are different machines with different claims. Conflating them is the first trap.

**Track A — `MorpheOS` (everyday OS, own Linux kernel).** Verified this session: `main` @ `e2c093b`; open PRs
#161 (secrets SOP), #165 (geomesh), #166 (SSH guard), #178 (fleet revenue), #183 (robmesh node), #186
(verification discipline). The own-kernel work (D19 Phase 0) is **done in the VM**: the image boots
`6.8.0-morpheos`, `uname -s` = `MorpheOS`, `test-shell` and `test-wifi` pass — the evidence is
[run 41849919635](https://github.com/robmitchellengineering-hub/MorpheOS/actions/runs/41849919635) and
[REVIEWS/2026-11-04-morpheos-kernel-boot.md](log/2026-11-04-morpheos-kernel-boot.md). **Not yet on real
hardware** — the boot runbook ([MORPHEOS-HARDWARE-BOOT-RUNBOOK.md](log/hardware-boot-runbook.md)) is
the open step.

**Track B — `MorpheOS-Secure` (the bare-metal Core).** This is the one Rob means by "custom kernel".
Verified this session: `main` pushed 2026-11-05; open PRs #42 (virtio-net ARP), #46 (ICMP), #48 (UDP), #56
(blk-read), #86 (send_rtp = program's own RTP). It is a **from-scratch Rust `no_std` Core on ARM64 (Pi 5)** —
never called a kernel by rule — that loads a compiled program, exposes a 5-call ABI (`report`,
`apply_certified`, `yield`, `send_udp`, `send_rtp`), and carries the GICv3 (GIC-600) interrupt path and an
AMT-shaped RTP envelope. The point of it is the **AirMatrix engine running bare-metal**: RTP/AES67/PTP audio
transport with no OS in the loop. Handoff: [MORPHEOS-HANDOFF.md](log/handoff.md).

So the estate already holds **both halves of Rob's sentence**: an AI-first OS (A) and a bare-metal,
real-time, FPGA-adjacent compute Core (B). The new question is whether they become **one machine to sell**.

---

## 1. The vision, translated from intent into precise claims

Rob's sentence, broken into the claims it makes, and whether each is defensible today:

| Rob's intent | precise claim | verdict |
|---|---|---|
| "not just a new OS, a new type of computer" | a machine where the OS and the silicon are **one design**, optimised from both sides | **the real, defensible idea** — Apple's moat, applied to an AI-first, provable machine |
| "embedded native … machine-code level" | critical paths (audio, transport, proof core, inference kernels) run **bare-metal**, no general OS underneath | **real and already started** (Track B) |
| "bare metal algebra AI" | tensor algebra runs on the accelerator through a minimal, **formally verified** runtime | **defensible as *verified kernels*, not as *FPGA makes AI fast*** (§2) |
| "fgma chips" (read: FPGA) | reconfigurable silicon for the deterministic front-end | **right tool for a real job, wrong tool for LLM inference** (§2) |
| "bare metal signal processing and decoding" | audio/RTP/AES67/radio decoding on FPGA + bare-metal Core | **the strongest, most concrete claim** — it is AirMatrix's own domain, already proven at 1.96–6.42× real time |
| "bare metal pacland proofs of all fundamentals, able to derive anything" | a native proof/derivation engine (not an LLM) that can formally verify and re-derive foundations | **at-risk.** "able to derive anything" trips the D15 crank filter. Defensible version: *verifies its own critical kernels + a growing tower of proved theorems* |
| "highly optimised lightweight LLM … frontier-level intelligence at speed within 32 GB RAM" | a model that is frontier *for the machine's own work* fits in 32–64 GB | **not true as stated, true as re-scoped.** Frontier-general models are 100s of GB. Reachable: frontier-at-a-task, or a distilled/quantised/MoE model with **verified** compression |
| "base 64 GB → tiers to 1 TB … more intelligence or separate models" | RAM = the lever: bigger resident model, more resident models, near-unlimited context | **true and falsifiable** — this is the cleanest part of the whole vision |
| "yes no engine pacland" | pacland runs **natively as a proof engine**, not as a prompt through an LLM — this engine is named **SEXTANT** (the yes/no engine on a simulated quantum computer, "Phantom") | **correct, and it is the differentiator** — the deterministic spine (D12) is exactly "no engine", the generative AI is separate |

The one-line honest summary: **"AI-first OS × AI-first hardware" is a real, defensible product; "derive
anything" and "frontier intelligence in 32 GB" are the two phrases that would get us filtered as cranks
before anyone reads a proof.** Everything else is an engineering ladder with a buyer at the end.

---

## 2. The honest physics of the hardware — what each ingredient actually buys

**FPGA.** Ideal for deterministic, streaming, fixed-latency, fixed-precision work: audio/RTP/AES67 transport,
radio/decoding front-end, packet/line-rate networking, cryptography, and small low-precision matmul. **Not**
for LLM inference: attention and the KV cache are memory-bound, irregular gather/scatter, where a GPU/NPU's
massive parallel matmul + HBM wins by an order of magnitude. Consequence, stated plainly: **the FPGA owns the
front-end; the AI compute is an NPU/GPU; the OS co-schedules both.** A pitch that says "FPGA makes the AI
fast" is wrong and will be found out.

**"Bare metal".** Not faster by magic — faster where it *removes work*: no initramfs, no systemd, no scheduler
between the code and the silicon, RT scheduling (D19's own table). The claim is measured latency and
determinism, not "machine code = speed". The AirMatrix Core is the proof-of-concept.

**"Bare-metal algebra AI".** The defensible reading: tensor kernels compiled to the accelerator with a
**formal layer that proves each kernel's correctness** (pacland/Lean). That is novel and it is the spine:
*claim → falsifier → evidence* applied to the lowest level of the stack. It is not "a new kind of ALU".

**The memory ladder — the cleanest claim.** Quantised and MoE models have a real size/quality curve today:

| RAM tier | what it runs resident |
|---|---|
| 64 GB (base) | a 30–70 B quantised model, or a MoE with large *active* params, with a very large context |
| 128–256 GB | a 100–200 B model, or several specialised models resident at once, millions of tokens of context |
| 512 GB–1 TB | a frontier-scale MoE (400–671 B) fully resident, or a *fleet* of specialised bare-metal models, near-unlimited KV cache |

"More RAM → more intelligence or more separate models" is **literally true** and is the honest marketing
sentence. This is the tier story to sell.

**LLM compression + pacland.** The genuinely novel hook: pacland already names **compression** as one of its
deferred engines ("does the structure compress", D17-2) and has the formal spine. The move is **verified
model compression** — compress a model and *prove the compression preserves behaviour within a bound*, rather
than trusting a perplexity number. Nobody sells that today. That is where the estate's four instruments
converge on one machine: MorpheOS (the OS), pacland (**SEXTANT** — the proof the compression is safe), VVS (the certificate
a buyer can check), AirMatrix (the physical-world proof in the audio/transport domain).

---

## 3. The ladder — rungs that each ship something and each carry a falsifier

Consistent with D12/D20/D22: no rung is "write a new silicon". Each rung is independently valuable and
independently falsifiable.

| rung | what ships | rough effort | falsifier |
|---|---|---|---|
| **M0 — the software machine** | MorpheOS (own kernel) + Secure edition + the compressed-Model runtime, running on commodity x86/ARM64 + an existing GPU/NPU. No custom silicon. | weeks–months (D19 Phase 0→2, D20 P1) | a real buyer's workload runs on our OS with a verified compressed model; `uname -s` = MorpheOS on metal |
| **M1 — the FPGA front-end card** | an FPGA accelerator (AMD/Xilinx-class board) doing the **deterministic** work: audio/RTP/AES67 transport, decoding, crypto — driven by the AirMatrix Core (Track B) | months | a measured round-trip latency beat, with the number, against the CPU path — the AirMatrix claim on the card, not in a VM |
| **M2 — the first "custom computer" (assemble, don't tape out)** | a bespoke motherboard + a commodity AI SoC/GPU + the M1 FPGA + our OS, sold as one SKU. Base 64 GB, tiers to 1 TB | months–1 year | a named buyer uses it for a week and reports the *specific* thing it does that a Mac/Linux box cannot — proven, not claimed |
| **M3 — verified compression as the product** | the compressed, formally-verified model stack; the certificate a buyer checks offline (VVS) | overlapping M1–M2 | tamper with the certificate or the model hash — verification fails; a perplexity-only claim is refused (D24) |
| **M4 — custom silicon** | tape-out or chiplets, only after M2 has revenue and a proven winning kernel | years, $10⁷+ | only starts when the number justifies it — never as the first rung |

**The trap the ladder exists to avoid:** starting at M4. "Custom silicon" is a capital event, not a first
step, and the FPGA card (M1) + assembled machine (M2) deliver 90% of the vision for 1% of the money and get a
buyer to *pay for the proof of concept*.

---

## 4. What this must NOT be (the discipline, carried from D12/D15/D17)

1. **Not "the most secure computer in the world"** — D17 already forbids it. The claim is the set of attacks
   on file that failed.
2. **Not "derive anything" / "theory of everything"** — D15 forbids leading with it; it triggers the crank
   filter before a proof is read. The defensible form: *a machine that can prove its own kernels and
   re-derive a growing, formalised foundation.*
3. **Not "frontier intelligence in 32 GB"** without the qualifier. Frontier *general* models are 100s of GB;
   claiming otherwise is a lie a reviewer will find in one afternoon (rule 45). The honest form:
   *frontier-at-a-task, with verified compression.*
4. **Not a fifth product.** D12 names four instruments. The custom computer is **MorpheOS's hardware form**
   (D4 family), with pacland/VVS/AirMatrix as its proof spine — not a new instrument, and it must be recorded
   as such or D12 is broken.
5. **No hype before measurement.** Every "superfast", "powerful", "frontier" becomes a number with a
   falsifier before it appears in copy (D12's spine).

---

## 5. Decisions this needs (the "let's talk about it" part)

1. **Is the machine MorpheOS's hardware form (inside the spine), or a fifth product?** My read: inside —
   but it needs Rob's call, because D12 says no fifth product without a recorded decision.
2. **Which buyer first?** ✅ **RESOLVED (D36):** the **secure field buyer** *and* the **broadcast lane**
   together — the machine as the **brain of a mixing desk**, proven first through **USB MIDI controllers**
   standing in for the desk's control surface. The AI-workstation buyer is not first. See §7 for the concrete
   proof path.
3. **What "fgma" meant** — FPGA (reconfigurable logic), or FMA (fused multiply-add, the tensor ALU primitive)?
   They lead to different hardware. My working read is FPGA.
4. **The honest intelligence bar.** Agree the target is "frontier-at-a-task with verified compression, at
   64 GB base", not "frontier-general in 32 GB" — so the copy never overclaims (D15).
5. **Does M0 already start?** M0 (commodity hardware + our OS + the compressed model) needs no decision beyond
   what's already recorded (D19/D20/D26). The question is whether to green-light it *as the hardware product's
   first rung* while the FPGA and the buyer are discussed.

---

## 6. Falsifiers for this plan

- **Wrong if** any rung ships copy with a superlative ("most", "derive anything", "frontier") that has no
  number and no falsifier behind it — that is D12/D15/D17's forfeit.
- **Wrong if** the FPGA card (M1) is sold as the AI accelerator — the FPGA owns the front-end, and if it ends
  up doing LLM matmul we have built the wrong machine.
- **Wrong if** M2 ships before a named buyer has used M0 for a week and said, in their own words, what it did
  that a commodity box could not.
- **Wrong if** the RAM tiers are priced on "intelligence" rather than the honest curve in §2 — more RAM runs
  bigger/more models; it does not grant intelligence by itself.

---

## 7. The chosen lane — the mixing-desk brain, proven through USB MIDI

D36 makes the broadcast lane concrete: the machine is the **brain of a mixing desk**. What that brain *is*,
technically, is exactly the AirMatrix stack: input channels → DSP (EQ/dynamics/FX) → routing matrix → summing
→ outputs, plus scene recall and networking (AES67/Dante for stage boxes and FOH). The **control surface**
(faders, knobs, buttons) is what a USB MIDI controller stands in for, at ~1% of a desk's cost.

**Why MIDI first is the right proof.** A USB class-compliant MIDI controller needs no driver work, no custom
silicon, and no FPGA — it plugs into the MorpheOS machine and the class driver is already there. It makes the
control path *real hardware* while the DSP is still on the CPU/GPU. It is the cheapest possible falsifier of
"the brain of a mixing desk": a fader moves, audio changes, and we attach a number to how long that took.

**The desk ladder (replaces M0–M2's abstract "buyer" with a concrete one):**

| rung | what it is | hardware | falsifier |
|---|---|---|---|
| **D0 — the MIDI brain** | a USB MIDI controller drives parameters on a running MorpheOS machine; audio passes through a real interface | commodity PC + MIDI controller + audio interface | a fader move produces a measured audio change, with a round-trip latency number, on our OS |
| **D1 — the deterministic brain** | the DSP/routing core moves onto the bare-metal Core (Track B) + FPGA front-end, still driven by the same MIDI surface | + FPGA card, Pi 5 / ARM Core | the same fader move beats D0's measured latency and holds it under load (no jitter beyond a bound) |
| **D2 — the network desk** | AES67/RTP transport between the brain and stage boxes, PTP-synced | + network I/O | a second machine joins the stream and the clock stays locked — the AirMatrix claim on metal |
| **D3 — the whole desk** | a real control surface (motorised faders) + the brain + the FPGA, sold as the desk's engine | custom board (M2) | a named engineer runs a real session on it and reports the measured thing it does that a commodity desk brain cannot |

**The secure field buyer rides the same machine.** The desk brain and the field computer share the chassis:
the attestation + encrypted `/persist` + default-deny controls (D17) are what make the *broadcast* version
compliant and tamper-evident, and the *field* version a machine a journalist or clinic can carry. One machine,
two loads — the Secure edition's one-way flow (D17) is exactly this.

**Next step to start D0** — the only things not already decided are the physicals: which MIDI controller, which
audio interface, and which machine is the bench. Everything software-side (MorpheOS image, the MIDI class
driver, the AirMatrix DSP crates) already exists or is in flight.

---

## 8. The ultimate goal — our AI designs, pacland verifies, a cheap shuttle fabricates

Rob, 2026-11-07: *"Ultimate goal, might not be that expensive to build research flux ai for hardware design."*
(Reading "research flux ai" as the research-synthesis AI — the Ask Alice engine pointed at chip design.)

This **reframes M4** (§3). Custom silicon is only a $50M+ scare number if it starts at the leading edge. The
expensive part of chip design is not the fab — it is **verification** (proving the design before you pay to
make it), which is exactly this estate's moat. So the ultimate goal is: a **hardware-design research AI**
proposes the RTL and floorplan, **pacland/Lean + the open EDA flow verifies it** (D22 principle 3: AI proposes,
deterministic decides), and a **shared MPW shuttle fabricates it**.

| step | cost reality |
|---|---|
| leading-edge tape-out (5 nm) | $50M+ — the scare number, not the first move |
| shared MPW shuttle (130 nm) | **~$100–300** (Tiny Tapeout tile) up to **~$10k–50k** (a few mm², SkyWater/Efabless/MOSIS) |
| open-source EDA (OpenROAD / Yosys / KLayout + SKY130 PDK) | **$0** — removes the $100k+/yr Synopsys/Cadence licence |
| the AI itself | software on the self-hosted Morpheus path + open models |
| verification | **our existing strength** (pacland, VVS) |

Precedent is real: Google's RL floorplanning (Nature 2021), NVIDIA's ChipNeMo, and a measurable benchmark
(**VerilogEval**) so the AI's hardware-design ability is *proven*, not claimed.

**What the first custom chip is:** a **130 nm chip** carrying a RISC-V core + our DSP/control kernels +
MIDI/PTP timestamping + the secure root-of-trust — the desk brain's front-end and the field computer's
attestation anchor. **What it is not:** a frontier AI accelerator. 130 nm cannot run a frontier LLM, and
nothing about "cheap" changes that — the AI accelerator stays a *bought* NPU/GPU, co-scheduled by MorpheOS.

**The guardrail (binds when ratified):** **AI proposes, never AI-fabricates-unverified.** Every AI-generated
RTL passes the open EDA flow (lint / synthesis / timing / DRC) *and* the critical kernels carry a pacland
proof **before** any shuttle money is spent. That is the difference between "we built a chip with AI" and "we
built a chip we can prove."

**Not a fifth product:** this is pacland/VVS's method applied to a new discipline — the falsifier's source of
truth becomes simulation / timing closure / DRC (D16's adaptation axis) — in service of the D36 machine. The
Zynq/Versal board (S2) still comes first: it de-risks the RTL on bought silicon before we pay for a wafer.
