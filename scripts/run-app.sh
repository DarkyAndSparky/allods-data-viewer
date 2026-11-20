#!/usr/bin/env bash
# Builds the web part and starts the desktop window (needs webkit2gtk-4.1 dev packages).
set -euo pipefail
export LC_ALL="${LC_ALL:-C.UTF-8}"
cd "$(dirname "$0")/.."
step() { printf '\n=== %s ===\n' "$1"; }
step "web: npm ci";  (cd web && npm ci)
step "web: build";   (cd web && npm run build)
step "run window";   cargo run --release -p adv-desktop
