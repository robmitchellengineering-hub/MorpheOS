# 12 · Radical Concepts: Thinking Past the Desktop

Docs 01–11 describe a *better* OS. This doc asks what an OS would look like
if we were inventing computing today, knowing what we know now. Every concept
is tagged with a feasibility rating:

- 🟢 **Grounded**: buildable now from known techniques; the novelty is in the integration
- 🟡 **Stretch**: needs real R&D, but has no known blockers
- 🔴 **Moonshot**: may not work, but would change everything if it did

---

## The big reframe

> **A traditional OS manages programs.
> MorpheOS manages *intent*, over a versioned graph of everything, using
> whatever hardware is around.**

Under that reframe, the whole system reduces to three primitives:

| Primitive | Replaces | Concept |
|---|---|---|
| **The Graph** | Filesystem, registry, settings, app data silos | Everything (files, settings, messages, sessions, devices) is a versioned node with typed relationships |
| **Verbs** | Applications | Capabilities that act on node types (`crop(image)`, `transcribe(audio)`, `sign(pdf)`), supplied by any program |
| **The Conductor** | Scheduler, init, task manager | It turns intents with constraints into scheduled verb execution on whatever hardware is available |

The concepts below follow from those three primitives.

---

## 1. Multiverse Execution: try before you commit 🟢→🟡

Strata is copy-on-write and capsules clone in milliseconds, so **forking the
whole relevant world is cheap**. MorpheOS can therefore run any risky or
uncertain action in a *parallel universe* first:

- "Upgrade to the new version of my DAW": the upgrade happens in a fork. Your
  project is opened there, plugins are checked, and you get a report: *"2 plugins
  broke, here's the fork if you want to look. Merge or discard?"*
- "Rename these 4,000 photos by date": you see the result before it's real.
- The AI's riskier plans **always** run in a fork first. The Verifier compares the
  forked world's health against the current one before merging.
- **Branch your computer like git:** "Let me try Linux-gaming setup B for a week,"
  then keep whichever branch you prefer.

Undo was the 1980s idea. **"Preview the future, then choose it"** is the 2020s version.

## 2. Precomputed Futures: answering before you ask 🟡

CPUs got fast by **branch prediction**: guessing the next instruction and doing it early.
MorpheOS applies the same idea to human actions:

- You finish recording a podcast. While you're still taking off the headphones,
  the OS has already transcribed it, normalised loudness, and rendered a draft
  export *in a fork*. When you ask, the result is already there.
- You open a repo. The OS has already built it in the background, so tests run instantly.
- You save an invoice draft. The PDF has already been rendered.

The rules: precomputation only uses *idle, mains-powered or surplus*
energy, and its results go into forks that are thrown away if unused. A wrong
guess costs electricity, never your time. Perceived latency becomes the **lookup time for
a precomputed answer**, which is close to zero.

## 3. Apps Dissolve into Verbs 🟡

Today you go to an *app* to do a *thing*. In MorpheOS, every program
**declares verbs** (what it can do, to which data types, with what quality and cost),
and the OS composes them:

- Right-click any image, from anywhere, and you get every `image.*` verb from every installed
  capsule: Photoshop's `heal`, GIMP's `curves`, an NPU model's
  `remove-background`.
- **Chains:** `audio → transcribe → summarise → translate(fr) → email draft`, built
  by voice or by drag-and-drop, then saved as a reusable verb. It's Unix pipes for the GUI era.
- **Legacy apps get verbs too.** The OS reads any app's accessibility and
  UI-automation tree, and the AI learns to drive it ("computer use"), wrapping it
  as typed verbs. **Every Windows app ever written becomes scriptable**, which is
  a huge deal for enterprise automation (it replaces brittle RPA).
- When several providers offer a verb, the Conductor picks by quality, speed, energy,
  and the user's preference.

## 4. Malleable Interfaces: UI is a projection, not a prison 🟡

If the OS can see every app's UI tree, the UI becomes *editable by the user*:

- "Hide everything in this app except these three buttons" persists as an
  **overlay**, and works even on a 2009 Windows app.
- **Task-generated UI:** for a one-off job ("compare these 3 quotes"), the OS
  generates a throwaway interface from the data schema and the verbs available.
- **Accessibility as a foundation, not an afterthought:** the same projection makes any app
  usable by voice, a switch device, a screen reader or a big-button layout.

## 5. Your Devices Are One Computer: liquid processes 🟡

Today, a running program is tied to the machine it started on. In MorpheOS it isn't:

- **Live migration of capsules** (proven for VMs in datacentres) is brought to
  personal devices. You start a video export on the laptop, close the lid, and it
  *moves* to the desktop and keeps running. You pick up a phone and the session follows
  you, rendered remotely.
- **Data gravity:** large data stays put and *computation travels to it*. Your 2 TB
  video library lives on the home server, and the laptop edits it through streamed
  compute instead of copying files.
- **Disaggregated hardware:** every device's GPU, NPU, camera, storage and
  speakers become pooled resources in the mesh. A Pi Zero with a borrowed
  GPU is a capable AI terminal.

## 6. The OS With a Memory, and the Right to Forget 🟢

People remember *episodes*, not paths. MorpheOS keeps a **local, encrypted
episodic memory** of activity: which session, which files, which people, which
web pages, and when. Then you can ask:

- *"What was that site I had open next to the mix session on Tuesday night?"*
- *"Bring back exactly what I was doing before the call interrupted me."*

Earlier attempts at this in other operating systems drew criticism for privacy, so the design starts from **forgetting**:

- Memory is opt-in, per-scope, encrypted at rest with a key only the user holds, and
  **never** synced or used for training.
- Memory **decays** by default: detail fades to summaries, then to nothing,
  the way human memory does.
- "Forget the last hour" and "Never remember this app" are one tap away, and
  forgetting is **cryptographic**: the per-period keys are destroyed.
- Data has **lifetimes**: downloads, temp files and caches *expire* unless
  they're touched or pinned. Digital clutter stops building up.

## 7. Outcome Contracts: tell it the goal and the deadline 🟡

People think in deadlines and trade-offs, not in process priorities:

> *"Have this album master exported in all formats by 6 pm.
> Keep the battery above 30%. Don't make the fans loud while I'm on calls."*

The Conductor treats this as an **optimisation problem**, using model predictive
control (the maths behind self-driving and power grids). It weighs energy, thermals,
deadlines, the user's calendar and mesh resources, then schedules the work, possibly
moving it to the desktop overnight. The user deals only in **outcomes and
constraints**.

**Energy as a budget:** "Endurance mode: 11 hours, this flight" turns the
battery into a planned budget. Every task is priced in joules, and the OS solves
for the best experience within that budget.

## 8. The Self-Tuning OS: evolution within guardrails 🟡

Every machine is different, so static tuning is always wrong somewhere.

- MorpheOS runs **continuous, safe experiments** on itself: scheduler variants,
  prefetch policies and I/O parameters. It treats this as a multi-armed bandit problem,
  where each configuration choice is tried, scored on a measured metric such as launch
  time or energy per task, and the winners are kept.
- Experiments only ever draw from the **known-good configuration space** (see doc 06).
  Anything that hurts a latency budget is rolled back immediately.
- **Fleet learning:** devices share *anonymised configuration-to-outcome results*
  (federated learning, without user data), so every MorpheOS laptop of a given model
  learns from all the others.

## 9. Proof-Carrying Changes: the "can't brick" theorem 🟡→🔴

Most system failures are a *broken invariant*: no boot path, no network, no
display, no admin access. MorpheOS defines these as **formal invariants** over the
Manifest:

- `∀ generation: ∃ bootable path ∧ ∃ input device ∧ ∃ display ∧ ∃ admin recovery`
- Every proposed change (whether from the user, the AI or an update) is checked by an
  **SMT solver** (Z3-class) against the invariants *before* it's applied.
- The aim is a provable statement: **no sequence of configuration changes can
  leave the machine without a recovery path.** (Hardware failure is out of scope.)

## 10. Deterministic Replay: bugs that can't hide 🟡

When something goes wrong, the system can *replay it* exactly:

- Capsules can be recorded at the level of system calls and nondeterminism
  (as the record-and-replay debugger `rr` does), always-on in a cheap ring buffer.
- "Send this moment to support" sends an exact replay instead of a vague
  description. A developer sees exactly what happened, privacy-scrubbed, and only with consent.
- Combined with multiverse forks, the OS can **bisect its own failures**: replay
  with configuration A vs B and find exactly which change caused the problem.

## 11. Zero-Install Software 🟢

"Installing" is a 1990s idea from the days of copying floppy disks. In MorpheOS:

- Software is a **content-addressed recipe** (a hash) that is *mounted*, not
  installed. The first launch streams only the chunks that are actually used, as
  CernVM-FS and game-streaming launchers do. That's often under 10% of the package.
- Every version of everything is addressable, so rollback, side-by-side versions and
  reproducibility come built in.
- **Share an environment, not a file:** send a colleague a link to a capsule hash and they
  get *your exact working setup*, with the right DAW version, plugins and project.

## 12. Multiplayer by Default 🟡

The Graph is built on **CRDTs**, data structures that merge concurrent edits without
conflicts, for the object types that suit them:

- Any document, folder, playlist or session can be **shared live** without a cloud
  intermediary. Edits sync peer-to-peer through the mesh, and they still work offline.
- Collaboration becomes a built-in OS property, not a per-app SaaS feature.

## 13. A Personal Constitution for the AI 🟢

Instead of an endless list of permission toggles, the user writes, or picks from templates, a short
**constitution**, which compiles into Verifier policy:

> - Never send anything off this machine without asking.
> - Never touch `/Projects/Clients` without showing me a diff.
> - During studio sessions, never interrupt me and never run updates.
> - Prefer free and open-source tools when quality is equal.

The rules are deterministic, so the AI can't argue its way around them.

## 14. Intent-Native Hardware 🔴

If MorpheOS succeeds, it will create demand for hardware built for these flows:

- **Snapshot-native storage controllers** that do CoW and dedup in the
  SSD firmware.
- **Hardware capsule boundaries** (CHERI-style capability pointers) that give
  isolation without virtualisation overhead. CHERI exists today in Arm's Morello
  prototype boards.
- **Always-on micro-cores** that run the Predictor and Conductor at milliwatt
  levels, like a phone's sensor hub but for intent.

Not needed for v1, but the architecture should keep the door open to it.

---

## How the concepts fit together

```
            Intent ("master the album by 6pm, keep battery > 30%")
                                   │
                         Outcome Contract (§7)
                                   │
            ┌───────── Conductor (optimiser + scheduler) ─────────┐
            │                      │                               │
   Precomputed Futures (§2)   Verb composition (§3)    Liquid processes (§5)
            │                      │                               │
            └──────► Multiverse fork (§1) ◄──── Proof check (§9) ──┘
                                   │
                        merge  ⇄  discard / replay (§10)
                                   │
                    The Graph: versioned, CRDT, forgetful (§6, §12)
```

## Which concepts to build first

1. **Multiverse Execution (§1):** it builds directly on Strata CoW and capsules,
   and makes a powerful demo.
2. **Zero-Install (§11):** it falls out of content-addressed storage.
3. **Verbs (§3):** start with 20 verbs over images, audio, PDF and text, and add
   accessibility-tree wrapping for one Windows app.
4. **AI constitution (§13):** cheap to build, and it earns trust early.
