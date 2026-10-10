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
| **One OS, every scale** | The same image and the same concepts from a Pi Zero to a server fleet. | [Scaling Profiles](docs/07-scaling-profiles.md) |

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
| `manifest/` | Example machines and how to validate them ([README](manifest/README.md)) |

```bash
cargo test
cargo run -q -p morph-manifest -- validate manifest/examples/studio-pc.toml
```

Licensed under [Apache-2.0](LICENSE).

## Status

Design phase. Strategic decisions are recorded in [00](docs/00-strategy.md):

- **The OS first**, with Morph Link for existing devices after it
- **First famous for:** never breaking, and healing itself
- **Open core**
- **Part of the Morpheus family**

Every doc ends with open questions for that area.
