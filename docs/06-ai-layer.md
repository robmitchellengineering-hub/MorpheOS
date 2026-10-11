# 06 · The AI Layer: Intent, Configuration, Prediction

## The core idea: AI moves you between known-good states

"AI reconfigures the OS" is terrifying if the AI can execute arbitrary
commands as root. MorpheOS makes it safe with one rule:

> **The AI never changes the system directly. It writes a proposed diff to
> the typed Manifest. A deterministic Verifier checks it, `morphd` applies it
> as an atomic transaction, health checks confirm it, and a single command
> rolls it back.**

```
 user intent ──► Intent Engine ──► Plan (Manifest diff + steps)
                                         │
                                         ▼
                              Verifier (deterministic)
                        schema ✓ policy ✓ capability ✓ risk score
                                         │
                       low risk ─────────┼───────── high risk
                          │                              │
                  auto-apply as Tx                show plan, ask user
                          │                              │
                          ▼                              ▼
              snapshot → apply → health-check → commit / auto-rollback
```

The configuration space is a **typed schema**: every setting, service,
capsule, network profile and device policy has a type, constraints and
dependencies. The AI can only produce values *inside* that space. That's
what "AI-reconfigurable to stable possibilities" means in practice.

## Components

### 1. Intent Engine
- Turns natural language, voice and context ("the user just plugged in a
  projector") into **intents** such as `display.present`, `call.join(link)`,
  `file.find(desc)` or `system.fix(wifi)`.
- Uses a **tiered model cascade**:
  1. **Deterministic matchers** for common intents (grammar plus embedding lookup).
     These are instant and run on a Pi Zero.
  2. **Small local model** (≈1–4B parameters, quantised; llama.cpp/ONNX on
     CPU/GPU/NPU) for laptops and desktops.
  3. **Larger local or mesh model** on a Studio/Fleet node.
  4. **Optional cloud model**, only with explicit consent and per-request data scoping.
- Every intent resolves to **tool calls against the `morph` API**, never to
  free-form shell commands. A shell tool exists, but it runs sandboxed and always needs
  confirmation.

### 2. Configurator (the "AI setup" engine)
- **First boot:** it detects the hardware, asks 0–3 questions ("Mostly work, creative,
  gaming?"), and generates a Manifest. Optionally, it imports settings, files and apps from an
  old Windows or macOS machine.
- **Ongoing:** it notices friction (repeated failed actions, error loops, slow
  paths) and *suggests* fixes as Manifest diffs ("Your audio crackles. Switch
  this session to the Studio profile?").
- **Self-healing:** it diagnoses failures (Wi-Fi, printing, display, audio) by
  walking a library of diagnostic playbooks, then proposes a fix transaction.

### 3. Predictor
- A small, local, on-device model of your usage patterns (time, location, network,
  connected devices, active session).
- Prewarms capsules, prefetches files to fast tiers, and preloads models.
- The ground rule: prediction only ever spends **idle** resources, so a wrong guess costs
  nothing the user would notice.

### 4. Explainer
- Every AI action has a plain-language explanation and a pointer to its
  transaction: *"I switched your audio output to the USB headset because you
  joined a call and it's the device you used last time. Undo?"*

## Risk tiers for automatic actions

| Tier | Examples | Behaviour |
|---|---|---|
| 0: Cosmetic | Theme, layout, notification grouping | Auto-apply, silent |
| 1: Reversible local | Audio device, power profile, prewarm, capsule recipe | Auto-apply, show a toast with Undo |
| 2: Persistent config | Install an app, change a network profile, firewall rule | Ask, with a one-tap approve |
| 3: Data-affecting | Delete or move files, change retention, share externally | Always ask, show a diff |
| 4: Security-critical | Disable encryption, grant admin, change trust | Requires strong auth; the AI can't trigger it alone |

In enterprise deployments, policy can tighten or relax these tiers per fleet.

**Where a tier does something today.** `morph-manifest apply` is the write path for a Manifest
generation, and it enforces three things: an invalid generation is refused for *every* caller,
approved or not; a change above tier 1 is refused unless a person is named (`--approved-by`), so
tier 4's *"the AI can't trigger it alone"* is a property of the code rather than a promise about
behaviour; and the write is atomic, so an interrupted apply leaves the previous generation intact.
Tiers 0–1 auto-apply with no approver, which is what the table above says they should.

**What that does not yet cover:** these tiers are enforced on the `apply` path only. The applier
that owns a *store* of generations — keeping the previous ones, and rolling back to one — is
`morphd`, and it is not built. A caller that writes a Manifest by hand bypasses all of it.

## Hardware-aware AI placement

| Profile | Default AI capability |
|---|---|
| Seed (Pi Zero 2 W, 512 MB) | Deterministic intents only; offloads to a mesh peer if one is available |
| Desk (8–16 GB laptop) | Small local model on CPU, iGPU or NPU, with embeddings indexed when idle |
| Studio (32 GB+ and dGPU) | Medium local model, serves the mesh |
| Fleet (server) | Shared inference service for the org, with policy-controlled models |

## Guarantees we will publish

1. With AI turned off entirely, the OS is still fully usable and configurable.
2. Every AI-initiated change can be inspected and rolled back.
3. No user data leaves the device without an explicit, per-scope grant.
4. AI tool access is capability-scoped. It can't touch anything the user couldn't.
5. Prompt-injection defence: content from files, web pages or messages is
   *data*, never instructions. Any tool call that comes from untrusted content
   needs user confirmation.

## Open questions
- Which local model family should we bundle, and how do we update models (as Strata capsules)?
- Should there be a typed DSL for diagnostic playbooks, written by humans and extended by AI?
- How do we measure "friction" signals without becoming surveillance? (Everything stays local by default.)
