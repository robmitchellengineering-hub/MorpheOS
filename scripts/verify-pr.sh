#!/bin/bash
# What every pull request must carry, and what the repository must keep wired together.
#
# WHY. From 2026-10-16 the builder is an agent that was not here when these habits formed. A
# habit in prose is skipped on a busy day; a check is not. Each rule below is a mistake that
# was made, or a step that was forgotten, on 2026-10-14:
#
#   1. a pull request adds its own session-log file, docs/log/<date>-<branch>.md (one file per
#      pull request, so two never edit the same lines; HANDOFF.md's single log made every
#      merge conflict);
#   2. every VM scenario script (images/x86-qemu/test-*.sh) is run by the images workflow AND
#      by scripts/test-images.sh, which the builder runs locally: a scenario only one of them
#      runs is a scenario that silently stops being run somewhere.
#
#   bash scripts/verify-pr.sh [--branch NAME --base REF]   # rule 1 needs both
#   bash scripts/verify-pr.sh --self-test                  # proves each rule can fail
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)

# Rule 1, as logic: $1 is the branch, the rest are the files the branch ADDS. Prints the
# problem and fails, or prints the log file it found.
log_rule() {
    local branch=$1 slug want file
    shift
    slug=$(printf '%s' "$branch" | tr '/' '-')
    want="docs/log/[0-9]{4}-[0-9]{2}-[0-9]{2}-$slug\.md"
    for file in "$@"; do
        if printf '%s\n' "$file" | grep -Eqx "$want"; then
            echo "$file"
            return 0
        fi
    done
    echo "the branch adds no session-log file: expected docs/log/<YYYY-MM-DD>-$slug.md (see docs/log/README.md)"
    return 1
}

# Rule 2, as logic: $1 is a list of scenario scripts, $2 and $3 the two files that must name
# each one. Prints every gap and fails if there is one.
wiring_rule() {
    local scripts=$1 workflow=$2 local_runner=$3 gaps=0 s
    for s in $scripts; do
        grep -q "$s" "$workflow" || { echo "$s is not run by $(basename "$workflow")"; gaps=1; }
        grep -q "$s" "$local_runner" || { echo "$s is not run by $(basename "$local_runner")"; gaps=1; }
    done
    return $gaps
}

if [ "${1:-}" = --self-test ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    fail() { echo "verify-pr: self-test: $*" >&2; exit 1; }
    log_rule feat/e7-installer docs/log/2026-10-16-feat-e7-installer.md > /dev/null \
        || fail "a correct log file was refused"
    ! log_rule feat/e7-installer crates/x.rs > /dev/null || fail "no log file was accepted"
    ! log_rule feat/e7-installer docs/log/2026-10-16-feat-e7.md > /dev/null \
        || fail "another branch's log file was accepted"
    ! log_rule feat/e7-installer docs/log/README.md > /dev/null || fail "the README was accepted"
    printf 'run: test-a.sh test-b.sh\n' > "$tmp/wf"
    printf 'test-a.sh\n' > "$tmp/local"
    wiring_rule "test-a.sh" "$tmp/wf" "$tmp/local" > /dev/null || fail "complete wiring was refused"
    ! wiring_rule "test-a.sh test-b.sh" "$tmp/wf" "$tmp/local" > /dev/null \
        || fail "a scenario missing from the local runner was accepted"
    echo "verify-pr: every rule can fail"
    exit 0
fi

branch="" base=""
while [ $# -gt 0 ]; do
    case "$1" in
        --branch) branch=$2; shift 2 ;;
        --base) base=$2; shift 2 ;;
        *) echo "usage: verify-pr.sh [--branch NAME --base REF] | --self-test" >&2; exit 2 ;;
    esac
done

status=0
cd "$REPO"
scripts=$(cd images/x86-qemu && ls test-*.sh)
if wiring_rule "$scripts" .github/workflows/images.yml scripts/test-images.sh; then
    echo "  ok    every VM scenario is run by the images workflow and by test-images.sh"
else
    status=1
fi

if [ -n "$branch" ] && [ -n "$base" ]; then
    mapfile -t added < <(git diff --name-only --diff-filter=A "$base"...HEAD)
    if found=$(log_rule "$branch" "${added[@]}"); then
        echo "  ok    the session log has this branch's entry ($found)"
    else
        echo "  FAIL  $found"
        status=1
    fi
fi
exit $status
