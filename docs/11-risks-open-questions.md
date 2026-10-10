# 11 · Risks & Open Questions

## Biggest risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| **Scope** | This is many projects (an OS, a filesystem, a compatibility layer, an AI and a mesh) | Ship the userland MVP on Linux first, and reuse the best existing components ruthlessly |
| **Drivers** | They're the reason most new operating systems fail | Use the Linux kernel. A new kernel is a research track only |
| **Windows compatibility gaps** | Anti-cheat, DRM, kernel drivers and enterprise agents | The microVM rung, honest compatibility ratings, and partnerships |
| **AI reliability** | One bad automatic change destroys trust | Typed config space, Verifier, risk tiers, atomic rollback, and the OS works fully with AI off |
| **AI on tiny hardware** | A Pi Zero can't run a useful LLM | Deterministic intents plus mesh offload, with a clear capability matrix |
| **Performance overhead of new layers** | FUSE, capsules and indexing could make things slower | Benchmark harness from day 0, budgets in CI, and native paths when needed |
| **Licensing** | Running macOS outside Apple hardware, Windows VM licences, Google Play | Respect licences: user-supplied licences, and open stores only |
| **Wedge dilution** | Building everything at once, so nothing is great | Phase 1 is *only* "unbreakable". Everything else waits for the Try to Break It suite to pass |
| **"Everything talks" needs devices we don't control** | iOS, locked Android and work PCs can't run MorpheOS | Built-in interop protocols plus Morph Link ([16](16-interop-and-link.md)) |
| **Self-healing overpromises** | One unrecovered failure damages the brand | Published honest limits, a measured mean time to recovery, and the public challenge as the acceptance test |
| **Licence entanglement with Morpheus** | Code copied from proprietary Morpheus becomes Apache-2.0 here | Copying is allowed (same owner), but decide file by file whether it belongs on the open side ([17](17-morpheus-family.md)) |
| **Ecosystem / adoption** | Users need apps and a reason to switch | Run *existing* software from day one. The pitch is "everything you have, with less friction" |

## Open design questions

1. **Manifest language:** Nickel, CUE, KCL, Nix-like, or our own typed DSL?
2. **Strata v1:** a new on-disk format, or extend bcachefs?
3. **Session model:** how explicit should sessions be? (They're implicit by default; power users get full control.)
4. **Licence:** decided. Apache-2.0 for the userland ([00](00-strategy.md))
5. **Distribution:** a full OS image only, or also a "MorpheOS layer" on existing Linux to get early adopters?
6. **Business model:** open core decided. The exact free/paid boundary is drafted in [00](00-strategy.md)
7. **Identity:** local accounts, mesh identity, or optional org SSO, and how do they relate?
8. **Accessibility** from day one: screen reader, voice control and high-contrast need to be designed into Flow shell, not added later.

## Things to validate early with prototypes

- [ ] Cold vs warm Wine capsule launch time using CoW snapshot clones
- [ ] Strata-on-FUSE overhead vs native btrfs for typical desktop workloads
- [ ] A boot-to-session time under 3 s on the reference laptop with I/O replay
- [ ] Intent Engine accuracy on the top 20 tasks, with deterministic matchers only and with a 3B model
- [ ] Studio audio round-trip latency with the PipeWire RT profile on a Pi 5 and on a laptop
- [ ] Pi Zero 2 W Seed image idle RAM under 64 MB
