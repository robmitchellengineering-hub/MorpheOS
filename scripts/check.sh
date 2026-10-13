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

step() { printf '\n== %s\n' "$*"; }
trap 'printf "\nFAILED at: %s\n" "${BASH_COMMAND}" >&2' ERR

step "format";   cargo fmt --all --check
step "clippy";   cargo clippy --all-targets -- -D warnings
step "tests";    cargo test --all
step "example manifests"
for f in manifest/examples/*.toml; do cargo run -q -p morph-manifest -- validate "$f"; done
step "shell frame is not blank"
cargo run -q -p flow-shell -- --out "${TMPDIR:-/tmp}/frame.png" --sessions 3
step "docs";     node scripts/verify-docs.mjs
step "plan"
node scripts/verify-plan.mjs --self-test
git show origin/main:docs/20-build-plan.md > "${TMPDIR:-/tmp}/base-plan.md"
node scripts/verify-plan.mjs --branch "$(git rev-parse --abbrev-ref HEAD)" --base "${TMPDIR:-/tmp}/base-plan.md"
step "console text"
node scripts/verify-console-text.mjs --self-test
node scripts/verify-console-text.mjs
step "verifiers can fail"
node scripts/verify-pi-image.mjs --self-test
node scripts/verify-motd.mjs --self-test
step "apply CLI";        bash scripts/verify-apply-cli.sh
step "transaction CLI";  bash scripts/verify-transaction-cli.sh
printf '\nall checks passed\n'
