#!/usr/bin/env bash
# Build the quantum path commands (D43) — sextant (prove) + quantum (compute).
#
# The build.sh image script installs these into /usr/bin when they exist; this script is the
# "build here" step (the mesh or a local machine), independent of the GitHub CI workflow.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p morph-verify -p morph-quantum
