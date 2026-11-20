#!/usr/bin/env bash
# Runs every check from CLAUDE.md in order. Usage: ./scripts/check-all.sh
set -euo pipefail
export LC_ALL="${LC_ALL:-C.UTF-8}"
cd "$(dirname "$0")/.."
step() { printf '\n=== %s ===\n' "$1"; }
step "web: npm ci";     (cd web && npm ci)
step "web: typecheck";  (cd web && npm run typecheck)
step "web: test";       (cd web && npm test)
step "web: build";      (cd web && npm run build)
step "cargo fmt";       cargo fmt --all --check
step "cargo clippy";    cargo clippy --workspace --all-targets -- -D warnings
step "cargo test";      cargo test --workspace
step "check-repo";      node scripts/check-repo.mjs
step "version check";   node scripts/version.mjs check
printf '\nAll checks passed.\n'
