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

**Correction, same session — it *has* booted on real hardware.** Rob: *"The x86 booted on real
hardware you can see it on the robmesh MorpheOS."* He is right, and the first draft of this PR was
wrong: `nodes/morpheos-2012.json` is a probe run **inside** a booted MorpheOS on a 2012 MacBook Pro
(`MacBookPro9,1`) — read-only root, hostname `morpheos-a`, `/persist` and `/persist/work` mounted,
root ssh answering — and `runs/morpheos-2012/2026-10-31T0254Z-…` is the run record. HARDWARE.md,
doc 23 and ADR 0021 all said "never booted"; all three now say what is true: **one machine, one
older layout (1800 MiB slots; `main` builds 2400), and no tablet yet.** The lesson is the mesh's own
A10: a stale document is not the machine.

**Nothing here is proven on a tablet.** No touch screen, no tablet Wi-Fi chip, no tablet firmware. The language throughout is "builds" and "would
prove", and E21's Done when is written as report lines for exactly that reason.

Next, outside this pull request: the `plain` image build from `main` on `pacland-rog`, then the
stick, then E6.
