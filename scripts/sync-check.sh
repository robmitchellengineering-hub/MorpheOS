#!/usr/bin/env bash
# sync-check.sh - how far each branch is behind main, and where it lives.
#
# WHY: Rob, 2026-10-24 - "i need these things to merge and stay synced". A branch that sits 100+
# commits behind does not produce a big conflict because of a bad merge tool: it produces one
# because every file both sides touched since then has to be reconciled in one go. Keeping branches
# close to main is the only thing that makes merges small.
#
# Run it before starting work in a worktree, and before opening a pull request.
#
# exit: 0 every branch within the window · 1 at least one branch is further behind
set -uo pipefail
BASE="${1:-origin/main}"
WINDOW="${2:-10}"
cd "$(git rev-parse --show-toplevel)" || exit 2
git fetch -q origin 2>/dev/null || true

behind_total=0
printf '%-40s %-8s %s\n' "branch" "behind" "worktree"
git worktree list --porcelain | awk '
  /^worktree /{w=$2}
  /^branch /{sub("refs/heads/","",$2); print $2 "\t" w}
' | while IFS=$'\t' read -r branch path; do
  [ -n "$branch" ] || continue
  n=$(git rev-list --count "$branch..$BASE" 2>/dev/null || echo "?")
  flag=""
  case "$n" in
    ''|*[!0-9]*) flag="?" ;;
    *) [ "$n" -gt "$WINDOW" ] && flag="  <- rebase: git -C $path rebase $BASE" ;;
  esac
  printf '%-40s %-8s %s%s\n' "$branch" "$n" "$path" "$flag"
done

# Branches with no worktree are checked too: those are the ones a person forgets.
git for-each-ref --format='%(refname:short)' refs/heads | while read -r branch; do
  path=$(git worktree list --porcelain | awk -v b="$branch" '
    /^worktree /{w=$2} /^branch /{sub("refs/heads/","",$2); if ($2==b) {print w; exit}}')
  [ -n "$path" ] && continue
  n=$(git rev-list --count "$branch..$BASE" 2>/dev/null || echo "?")
  [ "$n" = "?" ] && continue
  if [ "$n" -gt "$WINDOW" ]; then
    printf '%-40s %-8s %s\n' "$branch" "$n" "(no worktree) <- rebase or delete"
    behind_total=$((behind_total + 1))
  fi
done

echo ""
echo "window: a branch more than ${WINDOW} commits behind ${BASE} is flagged; rebase before working."
