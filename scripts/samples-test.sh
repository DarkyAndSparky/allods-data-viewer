#!/usr/bin/env bash
# Runs the tests on real packs. Usage: ./scripts/samples-test.sh /path/to/Packs
set -euo pipefail
export LC_ALL="${LC_ALL:-C.UTF-8}"
cd "$(dirname "$0")/.."
step() { printf '\n=== %s ===\n' "$1"; }
export ADV_SAMPLES="${1:?usage: samples-test.sh /path/to/Packs}"
step "tests on real files"; cargo test -p adv-core --test samples -- --nocapture
