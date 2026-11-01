# 2026-11-01 · DeepSeek · the x86 image had booted on real hardware, and the file said it never had

`images/x86-qemu/HARDWARE.md` opened with **"It has never been booted on real hardware."** That was
false, and it had been false for a day before anyone noticed — the file was older than the machine.

**Rob, correcting it:** *"The x86 booted on real hard ware you can see it on the robmesh MorpheOS."*

**The record that proves him right**, and that the file did not consult: `nodes/morpheos-2012.json`
in the harness-mesh repository — written by a probe that ran as root **inside** the booted machine —
shows a 2012 MacBook Pro (`MacBookPro9,1`) running this image from the USB stick: read-only root,
hostname `morpheos-a`, `/persist` and `/persist/work` mounted, root ssh answering. Its run record is
`runs/morpheos-2012/2026-10-31T0254Z-morpheos-2012-ssh-dispatch/result.json`.

**What the file says now**, narrowed to what is true:

- one machine booted it, from a stick laid out at **1800 MiB** slots — `main` builds **2400**;
- **no tablet** has been booted, so touch, tablet Wi-Fi and tablet firmware remain unproven;
- the warning that QEMU is not your machine stays, because it is still true.

**Why this is a `fix/` and not a `plan/`:** the change is inside `images/`, and `verify-pr` refuses a
`plan/` branch that touches it — correctly. The Slate line (doc 23, ADR 0021, E21) landed separately
in #175; only this correction moved here.

**The lesson, filed as a violation:** V-32 in the harness mesh's `SOP-VIOLATIONS.md` — a claim
published without a live reference to the thing it described. A stale document is not the machine.
