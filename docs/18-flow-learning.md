# 18 · Flow Learning: Having Things Ready Before You Ask

> *"It should be able to learn your flow of operations so it can gain
> efficiencies in speed, having things ready to go."* (Rob, 2026-10-10)

This doc turns the Predictor (doc 06 §3) and Precomputed Futures (doc 12 §2)
into a concrete design. The idea: MorpheOS watches *how you work* (locally,
privately), learns the sequences you repeat, and spends spare capacity getting
the next step ready. Then, when you ask, the answer is already sitting in the
fastest place it can be.

## 1. What a "flow" is

A **flow** is a sequence of operations you repeat, with a context:

```
context: weekday 08:5x, at home, laptop on charger, studio interface plugged in
flow:    open session "Album" → DAW capsule → load project.morph
         → plugin set {amp-sim, reverb} → audio graph @ 64 frames
         → (2–40 min later) export stems → upload to client folder
```

The OS already sees each step as a typed event, because every action passes
through `morphd`, Strata, the capsule planner or the intent engine. So flows can
be learned from **structured events, not screen recording**.

| Event source | Examples |
|---|---|
| Sessions | session opened/closed/switched, which session was active when |
| Capsules | capsule launched, which recipe and version, time to ready |
| Strata | objects opened, saved, exported, with their types and projects |
| Devices | audio interface, display, network and power state changes |
| Intents | "export stems", "join call", whether the user accepted or corrected the result |
| Time and place | time of day, weekday, network identity (no GPS needed) |

## 2. How it learns (local, small, explainable)

The approach goes in stages: start simple, and only add complexity where measurements justify it.

1. **Sequence counting (v1, every profile, Pi included).** A variable-order
   Markov model over events: "after A then B in context C, the next step was D
   in 82% of 40 cases". It's cheap, it's explainable ("I got D ready because you
   usually do D after B on weekday mornings"), and it learns from a handful of examples.
2. **Time-to-next estimates.** For each likely next step, a distribution of
   *when* it happens, so warming starts just early enough and not hours early.
3. **Small sequence model (v2, Desk and Studio profiles).** A tiny on-device
   transformer or GRU over event tokens, running on the NPU, for longer and
   fuzzier patterns. It's only used where it beats the counting model on
   held-out history.
4. **Calibration.** Each prediction carries a probability, and that probability
   is checked against reality continuously. A badly calibrated predictor is
   automatically trusted less.

There's no cloud training and no data leaves the device. Fleet learning, where
it exists (doc 12 §8), shares only *configuration → outcome* results, never
events.

## 3. What "ready to go" means: the action ladder

Actions are ranked by cost. The Predictor picks the deepest action whose
expected benefit beats its cost (doc 13 §8):

`p × value_of_time_saved  >  energy_cost(now) + contention_cost`

| Level | Action | Cost if wrong | Example |
|---|---|---|---|
| R0 | **Prefetch**: pull the files and libraries the next step reads into RAM or the fast tier | A few MB of cache | Project files and samples in the page cache before you open the DAW |
| R1 | **Prewarm**: restore a capsule's frozen snapshot so it's resident but idle | RAM, which is evicted first under pressure | The DAW capsule is already "launched" when you click it |
| R2 | **Pre-connect**: bring up the VPN, mount the share, open the TLS session | Negligible | The work file server is already mounted when you open the client folder |
| R3 | **Pre-configure**: stage (not apply) the session's device and power profile | Nothing until applied | Studio audio profile staged and applied the instant the DAW opens |
| R4 | **Precompute**: do the work in a throwaway fork and keep the result (doc 12 §1–2) | CPU and energy, only when idle and on mains power | Stems rendered in a fork while you're still listening back |

The hard rules:

- **Idle only.** Predictive work runs in the `opportunistic` scheduling class
  and is cut off the moment foreground work needs the resources (doc 03, doc 13 §5).
- **Energy-aware.** On battery, only R0–R2. R4 needs mains power or a
  surplus in the energy budget (doc 12 §7).
- **Never visible when wrong.** Mistakes cost cache space or idle CPU, never
  the user's time or attention. Precomputed results live in forks and are
  thrown away if they aren't used.
- **Never acts for you.** "Ready to go" is preparation. Anything that changes
  state the user would notice (sending, deleting, publishing) is still only done
  when asked. Precomputation stops at the fork.

## 4. Where it lives in the architecture

```
 morphd / Strata / capsules / intents ──events──► Flow Recorder (ring buffer, local)
                                                         │
                                                  Flow Model (per user)
                                                         │ predictions + probabilities
                                                         ▼
                                Readiness Planner (decides R0–R4 within budgets)
                                                         │ opportunistic-class jobs
                                    ┌────────────────────┼─────────────────────┐
                                 Strata tiers      Capsule snapshots     Fork executor
```

- The **Flow Recorder** stores events encrypted, under the user's key, with the
  same decay and forgetting rules as the episodic memory (doc 12 §6).
- The **Readiness Planner** is deterministic. It takes predictions and
  budgets and outputs jobs. The model never schedules work directly.
- Every readiness action is logged with its reason, so the Explainer can answer
  *"why is the DAW already open?"*

## 5. Measuring whether it works

| Metric | Target |
|---|---|
| Hit rate: predicted steps that actually happened | > 60% for R1 and above |
| Time saved per day (measured, launch and open latency avoided) | Reported to the user in plain terms |
| Wasted energy from wrong predictions | < 2% of daily energy use |
| Foreground slowdowns caused by predictive work | 0 (enforced by the Governor) |

The benchmark harness (doc 02 "Top 20") gains a **"second time" run**: the same
task on day 5 of a scripted routine should be measurably faster than on day 1.

## 6. Privacy controls

- It's on by default only for R0–R1 (cache and prewarm), which are invisible and harmless.
- There's a clear view of "what MorpheOS has learned about how you work", where any
  pattern can be deleted, with "forget the last hour" (doc 12 §6) applying here too.
- Per-session opt-out, for example "never learn from my banking session".
- Enterprise policy can disable it, or limit it to R0 only.

## 7. Roadmap placement

- **Phase 2:** Flow Recorder plus the counting model, with R0 prefetch and R1 capsule prewarm.
  These come together with capsules and Strata v1, and need nothing else.
- **Phase 3:** R2–R3, the NPU sequence model, and readiness metrics in the shell.
- **Phase 4:** R4 precompute in forks, once multiverse execution exists.

## Open questions
- Should one user's flows on several devices merge (through the mesh) or stay per-device?
- How should prediction be weighted after a routine changes (a new project, holidays)? Decay rate.
- R4 and correctness: a precomputed export must be invalidated when any of its inputs change.
  Strata content hashes give this for free, but this needs a design pass.
