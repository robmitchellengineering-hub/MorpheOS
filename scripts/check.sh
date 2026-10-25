#!/usr/bin/env bash
# The whole local bar, in the order CI's `rust` job runs it. Run this before every
# pull request:
#
#     bash scripts/check.sh
#
# WHY ONE SCRIPT. AGENTS.md lists the bar as separate commands, and a list is easy to
# run half of. This runs every step CI runs, stops at the first failure, and says which
# step failed. It covers the `rust` job only: the `images` workflow boots VMs and builds
# the Pi image, which needs root and minutes. Run that on your branch through CI.
#
# Keep it in step with .github/workflows/ci.yml. If you add a step there, add it here.
set -euo pipefail
cd "$(dirname "$0")/.."

# ⚠️ OFF LINUX THIS IS NOT THE BAR (AGENTS.md H16). Every flow-compositor test is
# `#![cfg(target_os = "linux")]`, so on macOS or Windows it compiles to nothing and `cargo
# test` still passes; most of M1 is compositor work. Refuse, unless asked for a partial run,
# and then say so at the end instead of "all checks passed".
PARTIAL=0
[ "${1:-}" = --partial ] && PARTIAL=1
if [ "$(uname -s)" != Linux ] && [ "$PARTIAL" = 0 ]; then
    cat >&2 <<EOF
check.sh: this is $(uname -s), and here the bar skips every flow-compositor test without
saying so (AGENTS.md H16). Run it on Linux: on Windows, inside WSL2 Ubuntu 24.04, after
'bash scripts/setup-linux.sh' (HANDOFF.md). To run what this machine can anyway, and be
told it was partial:   bash scripts/check.sh --partial
EOF
    exit 2
fi

step() { printf '\n== %s\n' "$*"; }

# The docs and plan checks are Node. Ubuntu 24.04's apt gives 18; nothing here needs newer.
node_major=$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)
if [ "$node_major" -lt 18 ]; then
    echo "check.sh: Node 18 or newer is needed (found: $(node --version 2>/dev/null || echo none)); run bash scripts/setup-linux.sh" >&2
    exit 2
fi
trap 'printf "\nFAILED at: %s\n" "${BASH_COMMAND}" >&2' ERR

step "format";   cargo fmt --all --check
step "clippy";   cargo clippy --all-targets -- -D warnings
step "tests";    cargo test --all
step "example manifests"
for f in manifest/examples/*.toml; do cargo run -q -p morph-manifest -- validate "$f"; done
step "shell frame is not blank"
cargo run -q -p flow-shell -- --out "${TMPDIR:-/tmp}/frame.png" --sessions 3
step "a real client's window through the compositor"
[ "$(uname -s)" != Linux ] && echo "skipped: the compositor is Linux only (a PARTIAL run)" || (
    export XDG_RUNTIME_DIR=$(mktemp -d)
    cargo build -q -p flow-compositor
    target/debug/flow-compositor --socket check --out "$XDG_RUNTIME_DIR/frame.png" --timeout 30 &
    until [ -S "$XDG_RUNTIME_DIR/check" ]; do sleep 0.1; done
    WAYLAND_DISPLAY=check target/debug/solid-window --colour 00a0ff --size 320x200 || true
    wait %1
)
step "docs"
node scripts/verify-docs.mjs --self-test
node scripts/verify-docs.mjs
step "plan"
node scripts/verify-plan.mjs --self-test
# Judged against main as it is now, not as this clone last saw it.
git fetch -q origin main 2>/dev/null || echo "(could not fetch origin/main: judging against the last fetched copy)"
branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" = HEAD ]; then
    echo "detached HEAD: switch to your branch (git switch <branch>) so the branch checks can run" >&2
    exit 1
fi
if [ "$branch" = main ]; then
    # Nothing to judge a branch by: the plan's own rules only, as CI does on main. (A fresh
    # clone is on main, and HANDOFF.md's first step is this script, so it must pass here.)
    node scripts/verify-plan.mjs
else
    git show origin/main:docs/20-build-plan.md > "${TMPDIR:-/tmp}/base-plan.md"
    node scripts/verify-plan.mjs --branch "$branch" --base "${TMPDIR:-/tmp}/base-plan.md"
fi
step "pull request carries its log; scenarios are wired"
bash scripts/verify-pr.sh --self-test
if [ "$branch" = main ]; then
    bash scripts/verify-pr.sh
else
    bash scripts/verify-pr.sh --branch "$branch" --base origin/main
fi
step "task cards";       node scripts/task.mjs --self-test
step "console text"
node scripts/verify-console-text.mjs --self-test
node scripts/verify-console-text.mjs
step "hardware report names missing firmware";  bash scripts/verify-hwreport.sh
step "the VM tests' embedded Python compiles"
bash scripts/verify-embedded-python.sh --self-test
bash scripts/verify-embedded-python.sh
step "verifiers can fail"
node scripts/verify-pi-image.mjs --self-test
node scripts/verify-motd.mjs --self-test
step "apply CLI";        bash scripts/verify-apply-cli.sh
step "transaction CLI";  bash scripts/verify-transaction-cli.sh
step "the x86 image runs morphd boot"
bash scripts/verify-morphd-boot.sh --self-test
bash scripts/verify-morphd-boot.sh
if [ "$PARTIAL" = 1 ] && [ "$(uname -s)" != Linux ]; then
    printf '\nPARTIAL: everything this %s machine can run passed, but that is NOT the bar:\n' "$(uname -s)"
    printf 'the flow-compositor tests did not run. Run scripts/check.sh on Linux (WSL2).\n'
    exit 3
fi
printf '\nall checks passed\n'
