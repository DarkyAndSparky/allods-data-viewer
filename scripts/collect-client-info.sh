#!/usr/bin/env bash
# Client passport: file list and tiny version files. Usage:
#   ./scripts/collect-client-info.sh "/path/to/client" current
set -euo pipefail
export LC_ALL="${LC_ALL:-C.UTF-8}"
cd "$(dirname "$0")/.."
step() { printf '\n=== %s ===\n' "$1"; }
client="${1:?usage: collect-client-info.sh <client dir> <name>}"
name="${2:?usage: collect-client-info.sh <client dir> <name>}"
out="$HOME/Downloads/BUILDS/client-$name"
mkdir -p "$out/meta"
( cd "$client" && find . -type f ! -path '*/Cache/*' ! -path '*/Mods/*' ! -path '*/Logs/*' -printf '%P\t%s\n' | sort ) > "$out/files.txt"
( cd "$client" && find . -type f -size -64k \( -name '*.rev' -o -name '*.version' -o -name '*.ver' \) ! -path '*/Cache/*' ! -path '*/Mods/*' -exec cp {} "$out/meta/" \; )
echo "Done: $out ($(wc -l < "$out/files.txt") files listed)"
