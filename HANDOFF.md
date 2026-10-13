# HANDOFF

From **2026-10-16** this repository is built by a DeepSeek agent on Rob's Mac. Until then it was
built by Claude Code. This file is how the work continues without anyone who was here before.

**Read in this order:** [AGENTS.md](AGENTS.md) (the rules, and the hazards: mistakes already
made here), this file, [doc 00](docs/00-strategy.md) (what wins arguments), then
[ADR 0014](docs/adr/0014-express-road-first.md) (what we are building next and why).

## The goal, in one line

**A general user boots a laptop, gets online, browses, and cannot make the machine
unbootable** (ADR 0014's exit). Everything below serves that. Anything that doesn't goes in
the widening backlog (ADR 0014, lane 2) and waits.

## Where things stand (2026-10-13)

| Piece | State | Evidence |
|---|---|---|
| A/B boot, one-shot trial, fallback (`morph-boot`) | **VM-proven** | `images` workflow: good / unhealthy / crash scenarios |
| Write + verify a new system into the spare slot (`morph-update`) | **VM-proven** | `update` scenario: slot B carries the payload's marker after boot |
| Rebuild the spare after a failed trial (`morph-heal`, H3/H4 groundwork) | **VM-proven** | ADR 0009; `images` workflow |
| Transactions that roll themselves back (`morph-transaction`, `morph-graph`) | **tested, CLI-proven** | `scripts/verify-transaction-cli.sh`, `verify-apply-cli.sh` |
| Manifest, invariants, risk tiers, signed generations (`.rob`) | **tested** | `morph-manifest` tests; ADRs 0005, 0011, 0012 |
| Strata: versions, rewind, ransomware detection, in the x86 image | **VM-proven** | `test-ransomware.sh`: 24 files back byte-for-byte |
| Flow shell: frame, input, session model, two feels | **software-rendered only** | CI renders `frame.png` and fails if blank. **No Wayland yet** |
| Bootable x86 USB image for a person | **builds, boots in QEMU** | [HARDWARE.md](images/x86-qemu/HARDWARE.md). **Never booted on real hardware** |
| Pi 5 image | **builds and is read back** | `verify-pi-image.mjs`. **Never booted on a Pi** |

"VM-proven" means a CI job boots it and reads the verdict off the disk. It does **not** mean
it has run on real hardware. Say "builds" or "VM-proven", never "works", until a person has
run it on a machine (AGENTS.md, verification bar).

## How to work (every task, every time)

1. `git fetch origin && git switch -c <type>/<slug> origin/main`. Never commit to `main`.
2. Take the **lowest-numbered open task** in [doc 20](docs/20-build-plan.md) whose `needs` are
   done. Put its ID in the branch name (`feat/e2-classic-windows`). One task per branch.
3. **Write the check first.** Every task's "Done when" names something a machine verifies.
   Make that check exist and fail *before* writing the code that makes it pass. If you cannot
   make it fail, it does not test anything (AGENTS.md H3, H9).
4. Implement the smallest change that makes it pass.
5. `rustup update stable && bash scripts/check.sh`. It must end with `all checks passed`. The
   update matters: CI uses the newest stable, and an older local clippy passes code CI
   rejects (until doc 20's S9 pins the version).
6. Push, open a pull request, and read **every** check by name, including the `images`
   workflow (AGENTS.md H6, H10). If a VM job times out, download the serial-log artifact and
   read it before forming any theory (H11).
7. In the same pull request: set the task's status in doc 20, add its **Evidence:** line (CI run or artifact), and
   add one line to the **Session log** at the bottom.
8. Rob merges. Do not merge your own pull requests.

**Never:** weaken, skip or delete a test to get green; claim something works without
evidence; rename a required CI job without saying so in the PR (H10); edit an accepted ADR
(write a new one that supersedes it).

**A new idea** goes into doc 20 as a `proposed` task in a `plan/` pull request. Don't build it
until Rob opens it.

**When stuck for more than two attempts:** stop. Write what you tried and what you saw under
**Open questions** below, push it, and ask Rob. A clear question beats a third guess.

## Decisions that are Rob's, not yours

Write the question up (options and a recommendation) and wait:
- merging any pull request;
- anything that changes the security model: signing, the `.rob` mark (ADRs 0011, 0012), keys,
  what runs as root;
- copying code from Morpheus (licence, decided file by file: [doc 17](docs/17-morpheus-family.md));
- which laptop is the reference machine, and anything that costs money;
- turning a `proposed` task into `open`, dropping a task, or reordering milestones;
- changing a settled-architecture decision (A1–A9 in doc 20).

## What to build: docs/20-build-plan.md

**The task list is [doc 20](docs/20-build-plan.md), and only doc 20.** It holds:
- the north star (N1–N5: what "finished" means);
- the settled architecture (A1–A9: decisions you do not reopen without an ADR Rob approves);
- every task, M1 to M5, each with a **Done when:** check.

`scripts/verify-plan.mjs` enforces its format in CI, and refuses a pull request whose branch
names no open task. Use `fix/…` for bugs in built things and `plan/…` for plan changes.

**Current milestone: M1, the express road.** Take the lowest-numbered open task whose
`needs` are done.

## Open questions

Write here when stuck: the date, what you tried, what you saw, what you need.

- *(none yet)*

## Session log

One line per piece of work: date, who, what, what's next.

- 2026-10-13 · Claude Code · Wrote this file and `scripts/check.sh` (the whole local bar in one
  command). Next: E1.
- 2026-10-13 · Claude Code · Doc 20 (the build plan: north star, settled architecture, M1–M5
  tasks) and `verify-plan.mjs` (format and branch guard in CI). Next: E2.
- 2026-10-13 · Claude Code · E1 done (#38): `flow-compositor` hosts real Wayland clients,
  headless, proven by pixels. Doc 20 merged (#39). Next: E2, then E3.
