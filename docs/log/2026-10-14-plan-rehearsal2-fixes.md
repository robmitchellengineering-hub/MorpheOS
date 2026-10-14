# 2026-10-14 · Claude Code · What the second handoff rehearsal stumbled on, fixed

A fresh agent was given only a clone of `main` at #71 and told to onboard as written. It found
21 friction points. Each is fixed here, except the flaky Strata test, which is #73 (`fix/`).

**The blocker.** On a fresh clone (on `main`), `check.sh` failed at its plan step: it asked
`verify-plan` to judge the branch `main`, which names no task. It now runs the plan's own rules
there, as CI does.

**Waiting on Rob is now a field, not prose.**
- `task.mjs` used to flag a task by a regex over its text ("Rob's decision"). That flag could
  never clear, so E6 could never legally start. HANDOFF also claimed `verify-plan` refused
  such a branch, and it did not.
- Now doc 20 has three fields: `**Waiting on Rob:**` (blocks a `feat/` branch),
  `**Ask Rob to open:**` and `**Rob decides before <step>:**`.
- `node scripts/task.mjs --rob` prints them all. It replaces the two hand-kept lists, in
  HANDOFF and in doc 20's M1, which had drifted. Two items in them were already decided.
- E6, E7 and P1 wait on Rob. S3, S4, E6 and E8 have decisions before a step. S5, S11, E13,
  I1 and F1 are worth asking him to open; I1 and F1 also need `parallel`, which nothing said
  before.

**A pull request cannot do Rob's part.** `verify-plan` now refuses, on any branch, any of
these without a line `Rob on <ID>: ...` in the branch's own log:
- a task leaving `proposed`;
- a task newly marked `parallel`;
- a new task that is not `proposed`;
- a removed **Waiting on Rob:** line.

The rehearsal had opened E13 and F1 on a `plan/` branch and every check passed.

**Branches.**
- A branch must be `feat/`, `fix/` or `plan/`. `wip/e6-x` used to pass.
- A `feat/` branch names exactly one real task. `feat/e6-and-e7` used to pass.
- A `plan/` branch changes nothing under `crates/` or `images/` (`verify-pr.sh`).

**One place says what to do while waiting:** HANDOFF.md, "While you wait". It covers how to
ask Rob (in the session, or under **For Rob:** in a PR), how to record that you asked, how
to record his answer, and what work is allowed meanwhile. `task.mjs --next` and doc 20 point
to it. Before this there were three versions that disagreed.

**Smaller fixes.**
- `verify-docs.mjs`, `verify-plan.mjs` and `task.mjs` refuse arguments they do not know.
  `verify-docs --self-test` used to print success. `task.mjs --help` works.
- The log file must start `# <date> · <who> · <what>` and say something after it. An
  uncommitted one is pointed out.
- `check.sh` must run every `scripts/` check that `ci.yml` runs. That was a prose rule only.
- `check.sh` needs Node 18 or newer.
- `setup-linux.sh` installs `gh`.
- HANDOFF's first two steps are swapped: the task is picked before the branch is named.
- A test that fails once is a bug: rerun once, log it, fix it on `fix/`.
- E6's step 1 no longer mentions a "console stick". Since E4 there is one stick, with a
  console fallback.
- H18 and E6 cite doc 20's machine list, not the private `MACHINES.md`.
- The ADR index explains ADR 0014's `BACKLOG.md`.
- The re-anchor count starts at #71.
- AGENTS.md gives the same reading order as HANDOFF.

**Next:** `verify-docs.mjs --self-test` is the first `plan/` item named in "While you wait".
