# 2026-11-04 · DeepSeek · K1: the arm's code on main, and the image next

The `secure` arm (62 commits, 12 behind) lands on `main` as its own thing, on Rob's instruction:
*"put it on main as its own thing separate from the everyday use morpheos"*, then *"do the work and
flash on vms"*. The plan that makes it legal is #180 (doc 26, ADRs 0020/0022, a gate that reads the
arm's task list); this branch is the code.

## What the merge needed

Three conflicts, all of the "both sides added something here" kind, none of them a disagreement:

- `images/x86-qemu/build.sh`, two hunks. **Both kept**: the DEV_PKGS toolchain check (main) and the
  `morph-attest` / `morph-session` installs (arm) are additive — the image a person boots should
  carry both the compiler and the commands that can state and seal what it is. The second hunk was
  main-only (the phone-home and the sshd key path), so it was kept and the empty side dropped.
- `images/x86-qemu/run-vm.sh`: the arm's `VM_SSH` explanation, kept — the flag existed on another
  branch first and that is exactly the surprise worth a comment.
- `docs/ATTESTATION.md` (add/add): main's version, which points at doc 26 and ADR 0022 instead of
  at a planning file that is not in this repository.

## A defect the merge found, which is the point of landing an arm

`verify-docs.mjs` was **red on the merge**: `crates/morph-attest` and `crates/morph-capsule` existed
but the README's crate table did not name them. The arm had not run its checks since 2026-11-01, and
nothing would have noticed until someone looked. The table now names both, with what each proves —
and the attest crate's honest limit (signed claim, not proof of integrity) is in its row, not only in
its module doc.

## What is true after this lands

- the kernel recipe (`secure/kernel/`), its three-arch gate, the attack suite (`security/attacks/`),
  and the seal / attestation / measured-boot records are on `main` as the arm's own tree;
- `morph-attest`, `morph-capsule`, `morph-session` and the seal's mandatory path are in the
  workspace, so `cargo test` covers them;
- **the image still ships Ubuntu's kernel.** K1's open half is that boot: an image whose slots carry
  our kernel and modules, booted by `test-shell.sh`, `test-wifi.sh` and `test-audio.sh` in a VM, with
  the drivers intact. That is the next piece of work, and it is VM-only until `morpheos-a-1` is free.

## Not claimed

The hardware falsifier is untouched: nothing here boots our kernel on real hardware, so Phase 0 is
not passed. `morpheos-a-1` takes that boot when Rob says it is free.

- DSH on K1: opened as `open · parallel` — its build is done and gated on the arm; the image running
  it is the remaining work, 2026-11-04.
