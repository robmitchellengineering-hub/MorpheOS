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

## Status

Design phase. Nothing in here is final. Every doc ends with open questions
for that area.
