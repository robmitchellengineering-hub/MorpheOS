# 29 · The install stick: flash, boot, it is a build node

**What this is.** The product shape the robmesh work has been building toward: one stick that installs MorpheOS onto
a machine **and** makes that machine a node on the robmesh, so the first thing it does is build and test MorpheOS on
itself. Rob, 2026-11-04: *"a native install stick for MorpheOS that can install it as a working Robmesh build node
and allow us to do the real native direct on hardware work on many different devices."*

**Status: the pieces exist separately, the flow does not.** Each half is built and measured; nothing yet joins them
into one boot.

## What is already true, measured

| piece | where | evidence |
|---|---|---|
| the OS builds itself | `morph-build` | `BUILD-EXIT=0` in 3 m 56 s offline (vendored crates) and 6 m 33 s online, on the reference laptop |
| a machine becomes a node | `morph-node claim` | a job dispatched from the mesh ran on the hardware and its result published to the bus; eight jobs so far |
| a stick can claim the machine it boots | `/efi/morpheos-node.conf` + `morpheos-claim.service` | claimed, then refused to adopt a different node's identity |
| the install layout and its refusals | `crates/morph-install` | 8 tests green on a Linux mesh node: the plan, partition naming, and every refusal |
| the boot report explains itself | `hwreport` | report 003 names the read-only filesystem behind `apt-daily`, which `-p err` could not show |

## The flow, and what each step still needs

1. **Prepare the stick (a person, once).** Flash the image; write the claim file with `scripts/claim-stick.sh
   <device> <node-id>`; optionally name a single-use tailnet key so the join needs no tap.
   *Needs:* nothing new.
2. **Boot the stick on the target machine.** It comes up as MorpheOS from the stick.
   *Needs:* nothing new.
3. **Install to the internal disk.** `morph-install /dev/sda` — lay out ESP, slot A, slot B and data, write both
   slots, install the firmware entry.
   *Needs:* **E7's destructive half**, which is the remaining work: `sfdisk`, the ESP tree, `morph_update::install`
   for both slots, and `test-install.sh` (boot from one disk, install to the other, boot the target alone).
4. **First boot from the internal disk.** The claim file is on the *stick's* ESP, so it has to be carried across by
   the install: the installer copies `/efi/morpheos-node.conf` to the target ESP, and the machine claims itself on
   first boot with nobody logged in.
   *Needs:* one line in the installer, and a test that the target's first boot is a claimed node — not just a
   committed slot.
5. **It is a build node.** `morph-node status` shows the id, the tailnet address and whether the toolchain and
   crates are there; `morph-build` compiles the OS; the mesh dispatches work to it.

## What must be true before this is called an install stick

- **The install refuses to touch the disk it booted from.** Implemented as a value with a test; it must also hold
  through the real command, which is what `test-install.sh` proves.
- **Both slots are written.** A slot left empty with `.morph-incomplete` blocks the machine's first update forever —
  the A/B guarantee dies on the day it is needed.
- **The claim survives the install.** A machine that installs and then needs a person at a keyboard to become a node
  is not the product; the claim file moves to the target ESP and first boot does the rest.
- **The machine can build after a reboot with no network**, or says plainly that it cannot: the toolchain lives on
  `/persist`, and a fresh install has none. `morph-node status` reports this rather than implying it.

## The honest order

E7's destructive half, then the claim-carrying line in it, then `test-install.sh` as the gate. Everything else on
this page is already measured and in the image.
