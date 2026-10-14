<!-- MorpheOS pull request. CI checks most of this; the rest is for the reviewer. -->

**Task:** <!-- the doc 20 task ID this serves (E7, S3, ...), or fix/... or plan/... -->

**What and why:** <!-- what a person or the machine can do now that it could not, in a few lines -->

**The check:** <!-- the test or VM scenario that proves it, and how you saw it fail before the change (AGENTS.md H3) -->

**Checklist:**
- [ ] `bash scripts/check.sh` ends with `all checks passed` (run in WSL2 or Linux, not `--partial`)
- [ ] doc 20: the task's status, and an **Evidence:** line if it is now done
- [ ] a session-log file `docs/log/<date>-<branch>.md` (CI refuses the PR without it)
- [ ] every number in what you wrote is labelled Measured, Simulated or Target
- [ ] nothing here needs Rob's decision (security model, keys, accounts, money, opening a task); if it does, it is asked below and not built

**For Rob:** <!-- decisions or questions, with options and a recommendation; "none" if none -->
