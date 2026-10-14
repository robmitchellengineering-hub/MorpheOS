# 2026-10-14 · Claude Code · CI's base fetch is no longer shallow

#76's `rust` job failed in verify-pr: "the branch adds no session-log file", for a branch
that had one. The cause was not the branch, and not verify-pr's logic:

1. The plan step in `ci.yml` ran `git fetch --depth=1 origin main`.
2. #74 merged while #76's job was running, so that fetch brought a *new* main commit, and
   brought it shallow: a commit with no parents.
3. The next step's `git merge-base origin/main HEAD` then had no common ancestor. `git diff ''`
   failed, and verify-pr counted no added files.

It only happens when main moves during a run, which is why it was never seen before. The
fix: the fetch is a full one (`checkout` already has `fetch-depth: 0`). verify-pr now fails
with the real reason if there is ever no common ancestor again.

Reproduced locally: a `--depth=1` clone with a branch carrying its log file, and a new
commit on main fetched shallow. `merge-base` fails, the old script exits 1 with no reason,
and the new one says the clone is shallow.
