# 2026-11-04 · DeepSeek · the Secure arm: its own build, its own kernel, its own queue

Rob asked to start the security division and the custom kernel, then corrected where they live:
*"not the main morpheos … a separate arm in the morphos repo"*. This branch is that arm's
charter. **It builds nothing**: the build, the kernel and the image tests all run on a node, not
on the Mac.

What changed:

1. **[doc 26](../26-secure-arm.md)** — the Secure arm: a separate build of the same core, its own
   image target, its own attack suite in CI, its own release line. Nine tasks in the order they
   are built: X3 console credential → X2 encrypted `/persist` → X5 default-deny → X4 per-machine
   identity → X6 anti-rollback → K1 the kernel → X7 attestation → X8 disposable browsing → X1
   verified boot.
2. **[ADR 0020](../adr/0020-account-model.md) Accepted** with Rob's three answers: model 2, VT2 kept
   on the hardware test sticks only with a banner and gone from `plain`, a forgotten password
   reset by the recovery system after a confirmation typed on the machine. It is built here as
   X3, not on the everyday queue.
3. **[ADR 0022](../adr/0022-own-the-kernel-build.md) Accepted**: the custom kernel is the Linux
   kernel built and patched by us, and it is built **in this arm first** — K1 own the build, K2 the
   failure path, K3 the policy. ADR 0001 stands; a kernel written from scratch is still not the plan.

What the next person needs to know:

- **Doc 20 is untouched.** The everyday build's task list gains nothing from this arm, including S5,
  which stays `proposed`. **E11** (the shell as an ordinary user) is blocked on that account, and the
  account is now X3 — so E11's dependency points at this arm, and re-parenting it is Rob's call.
- **The old numbering collides.** The secure-edition plan's controls were S1–S8; doc 20 already uses
  S1–S11, and the Slate line's ADR is 0021, so the controls are **X1–X8** here and the kernel is
  **K1–K3**. Do not reintroduce S-numbers for them.
- **Doc 26, not 27.** PR #178 (`docs/26-revenue-fleet`) is open and red, and `verify-docs.mjs`
  refuses a gap, so the first doc to land takes 26. That PR becomes 27 when it rebases; a comment on
  it says so rather than letting its author discover the collision in CI.
- **The 2012 laptop cannot host M3.** No TPM 2.0 and no UEFI Secure Boot to enrol keys into, so X7,
  X1 and firmware measurement are impossible there. `win-2`'s fTPM is the machine in reach.
- **A kernel build is not a Mac job.** K1 and every image scenario are mesh or CI work on a Linux
  node; the arm adds no local build step, and `scripts/check.sh` still refuses on macOS.

Next: X3 first (the account, the locked root, `attack-console.sh` green against a booted Secure
image), with the image target `images/x86-secure/` as its first piece of work.

- Rob on X3: model **2** — one owner, password at first boot, on `/persist`, root locked;
  *"VT2 on test sticks only + recovery resets by physical presence"*, in answer to ADR 0020's
  questions, 2026-11-04.
- Rob on K1: *"get started on the security devision and the kustum kernal"*, 2026-11-04, then
  *"not the main morpheos … a separate arm in the morphos repo"* — the kernel is this arm's.
- DSH on K2: opened as `in progress`, not proposed — its flight recorder half already landed on the
  `secure` arm (a real `sysrq` panic, readable after reboot), and the kexec switch is the open half,
  2026-11-04.
