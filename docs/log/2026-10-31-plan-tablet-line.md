# 2026-10-31 · DeepSeek · The Slate line opened: a tablet, never an iPad

The ask that started the evening was MorpheOS for old iOS devices, turned into mesh build nodes
that also run Firefox. The device named was an iPad Pro 12.9" (**A12X**), and the hardware
answered in one sitting: checkm8 is A5–A11 only, Asahi's FAQ says iPads cannot run a custom
kernel even with a jailbreak, and Project Sandcastle supports no iPad at all. That device is
also activation-locked with no account, so nothing could be installed on it either way.

Rob then made the call — *"This can be put in the MorpheOS repo as another product line"* — and
that is what this `plan/` pull request is:

- [doc 23](../23-tablet-line.md): the Slate line — the shipped `plain` x86 image on an x86_64
  UEFI tablet, the hardware criteria, the iOS exclusion with its sources, and the mesh role.
- [ADR 0021](../adr/0021-slate-line.md), **Accepted** with Rob's words and the date: one image,
  no second port, first device chosen by criteria, iOS hardware permanently out.
- [doc 20](../20-build-plan.md): **E21**, `proposed`, needing E6 — a Slate rests on the
  real-hardware boot that has not happened yet, so it cannot be `open`.
- [doc 07](../07-scaling-profiles.md): the Slate named as Desk's shape, not a new tier.

**Nothing here is proven on hardware.** No tablet has been booted; the x86 image has never been
booted on real hardware at all (HARDWARE.md). The language throughout is "builds" and "would
prove", and E21's Done when is written as report lines for exactly that reason.

Next, outside this pull request: the `plain` image build from `main` on `pacland-rog`, then the
stick, then E6.
