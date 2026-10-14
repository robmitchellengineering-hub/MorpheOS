# MorpheOS

**The operating system that gets out of the way.**

MorpheOS is an AI-native operating system that runs on everything from a
Raspberry Pi Zero 2 W up to a many-core enterprise server. It treats the computer
as a tool for getting a job done. You say what you want (or just start doing it),
and the system puts together the environment, apps, files and resources the job
needs, then gets out of the way.

> You shouldn't have to know which app opens a file, which OS a program was
> written for, where you saved something, or which setting broke your Wi-Fi.
> MorpheOS should know, or work it out, or safely try until it does.

## The five pillars

| Pillar | One-liner | Doc |
|---|---|---|
| **Intent, not apps** | You start tasks, not programs. The OS puts together whatever the task needs. | [Vision & Principles](docs/01-vision-and-principles.md) |
| **Strata storage** | Files are found by meaning and history, not by folder path. Every change can be undone. | [Strata Filesystem](docs/04-strata-filesystem.md) |
| **Capsules** | Any program from any platform runs in an instant, isolated environment built for it. | [Compatibility](docs/05-compatibility-capsules.md) |
| **Known-good states** | The AI configures the system, but only into states that are validated, atomic and reversible. | [AI Layer](docs/06-ai-layer.md) |
| **One OS, every scale** | One **codebase**, one set of concepts and one Manifest schema from a Pi Zero to a server fleet — **not one image**, which is impossible across architectures and profiles. | [Scaling Profiles](docs/07-scaling-profiles.md) |

## Documents

0. [Strategy & Decisions](docs/00-strategy.md): **start here**. The wedge, product family, open core and honest limits
1. [Vision & Principles](docs/01-vision-and-principles.md): what we're building and the rules we won't break
2. [Use-Case Analysis](docs/02-use-case-analysis.md): working backwards from real workflows and the friction in them
3. [System Architecture](docs/03-architecture.md): kernel, core services, layers
4. [Strata Filesystem](docs/04-strata-filesystem.md): a redesigned storage model
5. [Compatibility & Capsules](docs/05-compatibility-capsules.md): running Windows, Linux, Android and web software
6. [AI Layer](docs/06-ai-layer.md): the intent engine, the configurator and the stability guarantees
7. [Scaling Profiles](docs/07-scaling-profiles.md): Seed → Desk → Studio → Fleet
8. [Hardware Flow Optimisation](docs/08-hardware-flows.md): designing around real laptop components
9. [Security & Privacy](docs/09-security-privacy.md)
10. [Roadmap](docs/10-roadmap.md): phased plan, MVP, proposed repo layout and tech stack
11. [Risks & Open Questions](docs/11-risks-open-questions.md)
12. [Radical Concepts](docs/12-radical-concepts.md): multiverse execution, precomputed futures, apps that dissolve into verbs, liquid processes, proof-carrying changes and more
13. [Physics, Maths & the Real Speed Limits](docs/13-physics-and-math.md): where the actual limits are and where the wins come from
14. [A Day in the Life](docs/14-day-in-the-life.md): scenarios that become demos
15. [Self-Healing](docs/15-self-healing.md): **the flagship**. The healing ladder, Phoenix restore, and the "Try to Break It" demo
16. [Interop & Morph Link](docs/16-interop-and-link.md): speaking every existing protocol, and the agent for Windows, macOS, Android and iOS
17. [The Morpheus Family](docs/17-morpheus-family.md): how MorpheOS and Morpheus fit together
18. [Flow Learning](docs/18-flow-learning.md): learning how you work, so the next step is ready before you ask
19. [Any Hardware](docs/19-any-hardware.md): adapting existing drivers or building new ones, without risking the machine
20. [Build Plan](docs/20-build-plan.md): **the only task list.** What "finished" means, the settled architecture, and every task to get there
21. [The Intent Engine](docs/21-intent-engine.md): words to a safe action; the catalogue, the tiers, what may run alone, and the evaluation that decides which tiers earn their place (tasks I1–I10)

## The big reframe

> A traditional OS manages **programs**. MorpheOS manages **intent**, over a
> versioned **graph** of everything, using whatever **hardware** is around.

There are three primitives: **the Graph** (versioned, typed, everything),
**Verbs** (capabilities that replace apps), and **the Conductor** (turns intents
and constraints into execution across devices). See [doc 12](docs/12-radical-concepts.md).

## What's in the repo

| Path | Contents |
|---|---|
| `docs/` | The design set (start with [00](docs/00-strategy.md)) and [ADRs](docs/adr/README.md) |
| `crates/morph-manifest` | The Manifest: typed machine description, unbreakable invariants, risk-tiered change plans |
| `crates/morph-boot` | A/B boot control: trial-boot an update once, commit on health, fall back automatically |
| `crates/morph-update` | Write a new system into the inactive slot and verify what was written, before anything trials it |
| `crates/strata` | Strata v0: a version of every saved file, restore, folder rewind, ransomware detection ([ADR 0007](docs/adr/0007-strata-v0-watcher.md)) |
| `crates/morph-heal` | Rebuild the spare slot from the running system after a failed trial, so a fallback does not leave the machine unredundant ([ADR 0009](docs/adr/0009-heal-ladder.md)) |
| `crates/morph-transaction` | A state change that health-checks itself: arm a trial, apply, and commit or roll back without anyone having to notice |
| `crates/morph-graph` | The service graph: order a Manifest's services (start/stop/update), refuse cycles, missing dependencies and invariant breaks, and apply a change to them transactionally |
| `crates/flow-session` | The session model: identity, what it is opening, and the inferred feel (Flow / Session + shelf / Classic) — data only, no Smithay ([ADR 0010](docs/adr/0010-flow-shell-first.md), [ADR 0013](docs/adr/0013-flow-shell-feels.md)) |
| `crates/flow-shell` | The Flow shell's own frame: a session laid out and painted, with a check that asserts something was actually drawn rather than that a unit started; the Session + shelf feel draws a shelf of the session's openables and an intent bar whose query is a deterministic match ([ADR 0010](docs/adr/0010-flow-shell-first.md), [ADR 0013](docs/adr/0013-flow-shell-feels.md)) |
| `crates/flow-compositor` | The Flow shell as a real Wayland compositor (Smithay): headless with software rendering for now, proven by real clients' windows appearing in its frame (HANDOFF.md E1) |
| `crates/flow-wifi` | The Wi-Fi picker: a window on the shell that lists networks, takes a password and joins over NetworkManager's D-Bus API; its model unit-tested, its window driven on the compositor, its join proven in a VM against a simulated WPA2 network (doc 20, E5) |
| `manifest/` | Example machines and how to validate them ([README](manifest/README.md)) |
| `images/x86-qemu` | A/B test disk for a UEFI VM, with Strata watching `/home`, and the "Try to Break It" update and ransomware tests ([README](images/README.md)) |
| `images/pi5` | Raspberry Pi 5 immutable A/B image built with rpi-image-gen ([README](images/pi5/README.md)) |

```bash
cargo test
cargo run -q -p morph-manifest -- validate manifest/examples/studio-pc.toml
cargo run -q -p morph-manifest -- plan manifest/examples/desk-laptop.toml manifest/examples/desk-laptop-proposed.toml
```

`apply` is the write path, and a risk tier does something there: a change above tier 1 is
refused — writing nothing — unless a person is named, and an invalid generation is refused for
every caller. Run it against the examples with `--to` a scratch file to see both answers.

```bash
cargo run -q -p morph-manifest -- apply manifest/examples/desk-laptop.toml \
  manifest/examples/desk-laptop-proposed.toml --to /tmp/out.json
```

`--to` writes one generation where you say. `--store <dir>` **keeps them**: every generation is
retained, `current` names the newest, and `rollback` moves that pointer back — or forward again —
without deleting anything. Use it when the change should be undoable rather than only refused.

Licensed under [Apache-2.0](LICENSE).

## Status

Design phase. Strategic decisions are recorded in [00](docs/00-strategy.md):

- **The OS first**, with Morph Link for existing devices after it
- **First famous for:** never breaking, and healing itself
- **Open core**
- **Part of the Morpheus family**

Open questions live at the end of the docs that have them, and
[doc 11](docs/11-risks-open-questions.md) collects the cross-cutting ones.
