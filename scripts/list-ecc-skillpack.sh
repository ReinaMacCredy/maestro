#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
source_root="$repo_root/skillpacks/ecc/skills"
target_root="$repo_root/skills"

[[ -d "$source_root" ]] || { echo "[x] Missing $source_root" >&2; exit 1; }

echo "AVAILABLE ECC SKILLS"
find "$source_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort

echo
echo "ACTIVE ECC SKILLS"
if [[ -d "$target_root" ]]; then
  find "$target_root" -mindepth 1 -maxdepth 1 -type d | while read -r d; do
    if [[ -f "$d/.ecc-skillpack-marker" ]]; then
      basename "$d"
    fi
  done | sort
fi
