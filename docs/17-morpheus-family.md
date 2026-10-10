# 17 · The Morpheus Family

MorpheOS is part of the same family as **Morpheus** (morpheus.nz, the
`morpheus-self-hosted` repo): the chat-driven AI app builder that writes, compiles and
deploys software to every target. Morpheus is already a working
system, so this doc maps what MorpheOS can *reuse and connect to*
rather than reinvent.

> **Scope warning.** The Jarvis ↔ Morpheus assistant vision is, in Morpheus's own
> docs, *unstarted and unscoped, and must not be implemented opportunistically*.
> Everything below about Jarvis is a direction to scope later, not a plan of record.

## What Morpheus already proves that MorpheOS needs

| Morpheus today | MorpheOS equivalent | Reuse |
|---|---|---|
| **The self-correcting build loop:** compile → run → an AI reads logs → fix → recompile, escalating only when stuck | **The Heal Engine loop:** detect → diagnose → repair → verify ([15](15-self-healing.md)) | Same pattern and the same discipline: real logs, root-cause before patching, escalate with an actionable step |
| **Restricted self-dev modes:** allow-listed paths *enforced server-side, not merely prompted for* | **Verifier and risk tiers** ([06](06-ai-layer.md)): AI proposes, deterministic policy decides | The same principle at two scales. Share the policy design |
| **Bootable OS images:** Raspberry Pi and PC/server Linux images, customised at build time, network-flashed over SSH | **MorpheOS image factory** for the Seed/Desk/Studio profiles | Phase 0–1: build MorpheOS images with Morpheus-style GitHub Actions pipelines, and flash Pis over the network |
| **One chat, every target** (web, EXE, APK, iOS, Pi, Arduino) | **Capsules and verbs** | Add **"MorpheOS capsule / verb"** as a Morpheus compile target, so users can build their own verbs and apps by chat |
| **Marketplace** for templates and widgets | Compatibility recipes, verbs, Flow shell widgets | A shared marketplace for MorpheOS verbs and widgets (paid, open-core boundary) |
| **Command Deck widgets** synthesising data across connections | **Flow shell widgets and sessions** | The Deck widget model could become the MorpheOS widget model |
| **"No dead ends":** every error links to a real next step | Heal Engine escalation and Explainer | Adopt as a MorpheOS UX rule, word for word |
| **Cost transparency:** every AI action shows its cost up front | The AI layer and premium cloud tier | Show the cost and *where it runs* (local, mesh or cloud) for every AI action |

## How they fit together

```
 you ──chat──► Morpheus ──builds──► capsule / verb / widget / image
                                              │
                                              ▼
                                   MorpheOS device(s) on your mesh
                                              │
            Jarvis (future, to be scoped) ◄───┘  the assistant/intent front end,
                                                 aware of both Morpheus's build abilities
                                                 and MorpheOS's system verbs
```

- **Near term (phases 0–1):** Morpheus's image pipeline builds and flashes MorpheOS
  dev images. This is the most concrete and lowest-risk link, and it saves us building
  a release pipeline from scratch.
- **Mid term (phase 2):** "MorpheOS capsule" as a Morpheus compile target. Any app
  you build in Morpheus installs straight into your MorpheOS mesh.
- **Long term (to be scoped):** Jarvis as a candidate for the MorpheOS assistant and
  intent front end, with the Intent Engine exposing system verbs to it. When the
  OS needs a missing capability, Morpheus could *build* it as a verb, under
  the same restricted-mode, server-enforced scope rules.

## Licensing boundary

- **Morpheus** is proprietary and source-available (its LICENSE grants no reuse).
- **The MorpheOS core** is open source (planned Apache-2.0, see [00](00-strategy.md)).
- So **no Morpheus source code may be copied into MorpheOS.** We reuse *patterns
  and ideas*, and integrate through *APIs and build targets*. The Morpheus-powered
  features sit on the paid side of the open-core line.
- Trademark: "Morpheus" and "MorpheOS" should be registered and checked for conflicts together.

## Open questions
- Should the MorpheOS image factory live in Morpheus (as a compile target) or in this
  repo (as open tooling)? The leaning is the open tooling in this repo, with Morpheus calling it.
- Shared identity: does a morpheus.nz account become the MorpheOS mesh identity
  provider for the paid hosted relay and backup?
