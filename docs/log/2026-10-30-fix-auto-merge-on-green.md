# 2026-10-30 · DeepSeek · arm auto-merge on green, estate-wide

Adds `.github/workflows/auto-merge-on-green.yml`, which arms squash auto-merge on every non-draft pull request so a
green PR lands itself. Rob, 2026-10-30: *"everything edits and builds on branches or atleast it should and then when
we get a green it should push to main cause i forget all the time"*.

The mechanism is already proven on `morpheus-self-hosted`: PR #732 merged itself with all three required checks green
and nobody clicking merge.

## What the next person needs

- MorpheOS enforces two protocol gates on a PR, and both of them refused this change before it was fixed:
  the branch must be named for the protocol (`fix/<slug>` here — `auto-merge-on-green` was refused), and the PR
  **must carry a session-log file** (this one). A CI-only change is not exempt from either.
- Renaming a branch does not change its commits, so the old red runs still attach to the same SHA. A push or a
  branch update is needed before the new name is judged.
- On `airmatrix` the same workflow could not even start: hosted runners fail with *"recent account payments have
  failed or your spending limit needs to be increased"*, visible only in a check annotation after a zero-step
  failure. Its arming job therefore runs on the repo's self-hosted label.
- `allow_auto_merge` is now on for all 100 repositories in the estate, and `allow_update_branch` for the seven that
  matter, because required checks with `strict: true` make every PR go BEHIND whenever main moves.
