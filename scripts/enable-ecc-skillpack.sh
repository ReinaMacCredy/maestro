#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/enable-ecc-skillpack.sh [all|<comma-separated-skills>]
Examples:
  scripts/enable-ecc-skillpack.sh all
  scripts/enable-ecc-skillpack.sh api-design,verification-loop
USAGE
}

arg="all"
if [[ $# -gt 1 ]]; then
  usage >&2
  exit 1
fi

if [[ $# -eq 1 ]]; then
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --*)
      echo "[x] Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      arg="$1"
      ;;
  esac
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
source_root="$repo_root/skillpacks/ecc/skills"
target_root="$repo_root/skills"

[[ -d "$source_root" ]] || { echo "[x] Missing $source_root" >&2; exit 1; }
mkdir -p "$target_root"

resolve_names() {
  if [[ "$arg" == "all" ]]; then
    find "$source_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \;
    return
  fi

  IFS=',' read -r -a raw <<< "$arg"
  for item in "${raw[@]}"; do
    trimmed="$(printf '%s' "$item" | xargs)"
    [[ -n "$trimmed" ]] || continue
    if [[ "$trimmed" == maestro:* ]]; then
      echo "$trimmed"
    else
      echo "maestro:$trimmed"
    fi
  done
}

installed=0
status=0
while IFS= read -r name; do
  [[ -n "$name" ]] || continue
  src="$source_root/$name"
  dst="$target_root/$name"

  if [[ ! -d "$src" ]]; then
    echo "[x] Skill not found: $name" >&2
    status=1
    continue
  fi

  if [[ -e "$dst" && ! -f "$dst/.ecc-skillpack-marker" ]]; then
    echo "[!] Skipping $name (target exists and is not ECC-managed): $dst"
    continue
  fi

  rm -rf "$dst"
  cp -R "$src" "$dst"
  {
    echo "source=$src"
    echo "enabled_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } > "$dst/.ecc-skillpack-marker"
  echo "[ok] enabled $name"
  installed=$((installed + 1))
done < <(resolve_names)

echo "[ok] enabled_count=$installed"
exit "$status"
