# 0021 · The Slate line: MorpheOS runs on an x86 UEFI tablet, and an iPad is never a target

Status: **Accepted**, 2026-10-31. Rob's words, in the planning room: *"This can be put in the
MorpheOS repo as another product line"* — after the iPad was ruled out on evidence rather than
preference.

## Context

The ask that started this was a version of MorpheOS for "old iOS devices", turning them into
build nodes that also run Firefox for personal computing. Three facts closed that door in one
evening, all of them about the hardware and none of them about effort:

1. **checkm8 covers A5–A11.** The device in hand is an iPad Pro 12.9" (A12X), so there is no
   bootrom path at all.
2. **Asahi's FAQ** states that iPads "do not support running custom OS kernels", and that even
   arbitrary code execution in userspace "would not help with running a Linux kernel".
3. **Project Sandcastle** supports no iPad, and on the one phone it does support the GPU, audio,
   camera and cellular are all absent.

A jailbreak is not a workaround: the A12+ jailbreaks install from an app, which needs a home
screen, which an activation-locked device does not have.

What was left was the thing the ask actually wanted — a tablet a person can hold, running
MorpheOS itself, and usable for ordinary computing — which is a hardware **shape**, not a kernel
port. The repository already builds the software for it: the `plain` x86 image is the shell,
Firefox, the Wi-Fi picker and the A/B pair, and it has never been booted on real hardware
([HARDWARE.md](../../images/x86-qemu/HARDWARE.md)).

## Decision

1. **There is a Slate line** ([doc 23](../23-tablet-line.md)): the existing `plain` x86 image on
   an x86_64 UEFI tablet, run from a stick (or from internal storage once E7 exists).
2. **One image, one port.** A Slate gets no tablet-specific build. If a slate needs something the
   laptop image does not have, that is a bug in the image or a new task, not a fork.
3. **The first device is chosen by criteria, not by model:** x86_64 + UEFI, Intel Wi-Fi, touch on
   mainline HID, 4 GB+ RAM, Secure Boot that can be turned off, USB boot. §2 of doc 23 is that
   list.
4. **iOS and iPadOS hardware are excluded permanently.** An iPad participates as a client through
   Morph Link (task W5), never as a Slate.

## Consequences

- **E21** is opened as a `proposed` task in [doc 20](../20-build-plan.md), needing E6 — a Slate
  rests on a real-hardware boot that has not happened yet.
- **The touch path is a gap, not a detail:** touch input, an on-screen keyboard, rotation and
  suspend are unexercised by every test that exists, and the on-screen keyboard may be large
  enough to need its own task (doc 23, open question 3).
- **No new kernel work, and no new driver work** unless the chosen device forces it; a device
  that forces it was the wrong device to pick first.
- **The wedge is unchanged.** A Slate inherits the A/B trial, the fallback and Strata; it adds no
  new promise, and ADR [0003](0003-unbreakable-wedge.md) still wins any argument about ordering.
