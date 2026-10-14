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
#      It starts "# <YYYY-MM-DD> · <who> · <what>" and says something after that heading:
#      an empty file, or one with no heading, passes no one's reading;
#   2. every VM scenario script (images/x86-qemu/test-*.sh) is run by the images workflow AND
#      by scripts/test-images.sh, which the builder runs locally: a scenario only one of them
#      runs is a scenario that silently stops being run somewhere;
#   3. a plan/ branch changes no file under crates/ or images/: code goes on fix/ or feat/,
#      where it is reviewed as code (the second handoff rehearsal put feature code on a
#      plan/ branch and every check passed);
#   4. scripts/check.sh runs every scripts/ check the CI workflow runs: "keep it in step with
#      ci.yml" was a rule in prose only, and the local bar can drift from CI's unnoticed.
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

# Rule 1's content, as logic: $1 is the log file. Prints the problem and fails.
log_content_rule() {
    local file=$1
    if ! head -n 1 "$file" | grep -Eq '^# [0-9]{4}-[0-9]{2}-[0-9]{2} · .+ · .+'; then
        echo "$file does not start with '# <YYYY-MM-DD> · <who> · <what>' (docs/log/README.md)"
        return 1
    fi
    if [ -z "$(tail -n +2 "$file" | tr -d '[:space:]')" ]; then
        echo "$file has a heading and nothing else: say what changed and why"
        return 1
    fi
}

# Rule 3, as logic: $1 is the branch, the rest are the files it changes. Prints each file a
# plan/ branch may not change, and fails if there is one.
plan_scope_rule() {
    local branch=$1 bad=0 f
    shift
    case "$branch" in plan/*) ;; *) return 0 ;; esac
    for f in "$@"; do
        case "$f" in crates/*|images/*) echo "$f: a plan/ branch changes no code under crates/ or images/; use fix/ or feat/"; bad=1 ;; esac
    done
    return $bad
}

# Rule 4, as logic: $1 is the CI workflow, $2 the local bar. Prints every scripts/ check
# the workflow runs and the bar does not, and fails if there is one. The two setup scripts
# install things; they are not checks.
bar_rule() {
    local ci=$1 bar=$2 gaps=0 s
    # Comments name scripts too; only what the workflow runs counts.
    for s in $(grep -vE '^[[:space:]]*#' "$ci" | grep -oE 'scripts/[a-z0-9-]+\.(sh|mjs)' | sort -u); do
        case "$s" in scripts/setup-linux.sh|scripts/rust-toolchain.sh) continue ;; esac
        grep -q "$s" "$bar" || { echo "$s is run by $(basename "$ci") but not by $(basename "$bar")"; gaps=1; }
    done
    return $gaps
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
    printf '# 2026-10-16 · DSH · A change\n\nWhat and why.\n' > "$tmp/good.md"
    log_content_rule "$tmp/good.md" > /dev/null || fail "a good log file was refused"
    : > "$tmp/empty.md"
    ! log_content_rule "$tmp/empty.md" > /dev/null || fail "an empty log file was accepted"
    printf '# 2026-10-16 · DSH · A change\n\n' > "$tmp/bare.md"
    ! log_content_rule "$tmp/bare.md" > /dev/null || fail "a log file with only a heading was accepted"
    printf 'notes\nwhat\n' > "$tmp/nohead.md"
    ! log_content_rule "$tmp/nohead.md" > /dev/null || fail "a log file without its heading was accepted"
    plan_scope_rule plan/x docs/20-build-plan.md scripts/x.mjs > /dev/null || fail "a plan/ branch's docs were refused"
    ! plan_scope_rule plan/x crates/strata/src/lib.rs > /dev/null || fail "code on a plan/ branch was accepted"
    plan_scope_rule fix/x crates/strata/src/lib.rs > /dev/null || fail "code on a fix/ branch was refused"
    printf 'run: node scripts/a.mjs\nrun: bash scripts/setup-linux.sh\n' > "$tmp/ci"
    printf 'node scripts/a.mjs\n' > "$tmp/bar"
    bar_rule "$tmp/ci" "$tmp/bar" > /dev/null || fail "a bar that runs every check was refused"
    printf '  # scripts/only-named.sh in a comment\n' >> "$tmp/ci"
    bar_rule "$tmp/ci" "$tmp/bar" > /dev/null || fail "a script named only in a comment was counted"
    printf 'run: bash scripts/b.sh\n' >> "$tmp/ci"
    ! bar_rule "$tmp/ci" "$tmp/bar" > /dev/null || fail "a check missing from the bar was accepted"
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

if out=$(bar_rule .github/workflows/ci.yml scripts/check.sh); then
    echo "  ok    scripts/check.sh runs every check the CI workflow runs"
else
    printf '  FAIL  %s\n' "$out"
    status=1
fi

if [ -n "$branch" ] && [ -n "$base" ]; then
    # Added since the base, committed or not (a log file written but not yet committed
    # counts, so check.sh can be run before the commit), plus files git does not track yet.
    mb=$(git merge-base "$base" HEAD)
    mapfile -t added < <(
        git diff --name-only --diff-filter=A "$mb"
        git ls-files --others --exclude-standard
    )
    if found=$(log_rule "$branch" "${added[@]}"); then
        if why=$(log_content_rule "$found"); then
            echo "  ok    the session log has this branch's entry ($found)"
            if git ls-files --others --exclude-standard | grep -qx "$found"; then
                echo "        (not committed yet: git add it, or CI will not see it)"
            fi
        else
            echo "  FAIL  $why"
            status=1
        fi
    else
        echo "  FAIL  $found"
        status=1
    fi
    mapfile -t changed < <(
        git diff --name-only "$mb"
        git ls-files --others --exclude-standard
    )
    if why=$(plan_scope_rule "$branch" "${changed[@]}"); then
        case "$branch" in plan/*) echo "  ok    this plan/ branch changes no code" ;; esac
    else
        printf '  FAIL  %s\n' "$why"
        status=1
    fi
fi
exit $status
