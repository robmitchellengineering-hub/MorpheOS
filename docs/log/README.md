# Session log

One file per piece of work (each pull request adds one), so that two pull requests never edit
the same lines. The newest is the last by name.

**Name:** `<date>-<branch>.md`, with the branch's `/` as `-`: `2026-10-16-feat-e7-installer.md`.

**Content:** a few lines, as plain as the rest of the repository:

```markdown
# 2026-10-16 · DeepSeek · E7, PR 1: morph-install's plan and refusals

What was done, in a sentence or two. What was found that the next person needs (a trap, a
measured number labelled Measured, a fact that turned out wrong). What is next.
```

`scripts/verify-pr.sh` (CI, on every pull request) refuses a pull request that adds no file
here. Facts that stay true belong in HANDOFF.md, AGENTS.md or doc 20, not only here: the log is
what happened, not the plan.

The entries from before this folder existed are in
[2026-10-13-to-14-history.md](2026-10-13-to-14-history.md).
