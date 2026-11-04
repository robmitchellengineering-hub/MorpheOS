# 2026-11-04 · DeepSeek · the account model decided, the secure line's queue, and the kernel track opened

Rob asked to start the security division and the custom kernel. Both were sitting as proposals
waiting on him; this branch is the plan change that unblocks them, and it builds nothing.

What changed:

1. **[ADR 0020](../adr/0020-account-model.md) is Accepted** with his three answers, and S5 in
   [doc 20](../20-build-plan.md) is open, marked `parallel` so it runs beside the express road.
   The console credential is the first control: today Ctrl+Alt+F2 is root with no password
   (red team finding F1), and it is written down in a public repository's HARDWARE.md.
2. **The secure line gets a queue** as proposed tasks S12–S17, mapped from the
   secure-edition plan's controls: encrypted `/persist` (F2), default-deny network, per-machine
   identity (F3), measured boot, disposable browsing, anti-rollback. They are `proposed`:
   nothing jumps the queue.
3. **The kernel track is opened** as [ADR 0022](../adr/0022-own-the-kernel-build.md) plus M6's
   K1–K3. K1 (own the build) is `open · parallel`; K2 and K3 wait on it.

What the next person needs to know:

- **The tasks' IDs collide with the secure-edition plan's S1–S8.** Doc 20 already uses S1–S11
  for "Safe to hand to someone", so the plan's S2 (encrypt `/persist`) is doc 20's **S12**, S5
  (default-deny) is **S13**, S4 (SSH CA) is **S14**, S7 is **S15**, S8 is **S16**, and S6's
  missing half is **S17**. When editing either file, say which numbering you mean.
- **`UTS_SYSNAME` is not assumed.** K1 says to verify it in the 6.8 tree and patch
  `include/linux/uts.h` if it is not there. A name set by a wrapper is not the check, because
  the check is the syscall.
- **The 2012 laptop has no TPM 2.0**, so S12's key cannot be sealed to one and S15 cannot run
  on it. Saying that now is cheaper than discovering it in the middle of the work.
- **Both lanes are marked `parallel` on Rob's instruction, not by the agent's judgement**: the
  express road (E6/E7/E8) still holds the milestone, and these run beside it.

Next: dispatch S5 (the account, the locked root, `test-accounts.sh`) and K1 (the kernel build
on a Linux node), in that order.

- Rob on S5: model **2** (one owner, password at first boot, on `/persist`, root locked);
  *"VT2 on test sticks only + recovery resets by physical presence"*, in answer to ADR 0020's
  questions, 2026-11-04.
- Rob on K1: *"get started on the security devision and the kustum kernal"*, 2026-11-04 —
  the kernel is ours to build (ADR 0022), not written from scratch.
