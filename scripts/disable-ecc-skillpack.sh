#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/disable-ecc-skillpack.sh [all|<comma-separated-skills>]
Examples:
  scripts/disable-ecc-skillpack.sh all
  scripts/disable-ecc-skillpack.sh api-design,verification-loop
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

explicit_selection=0
if [[ "$arg" != "all" ]]; then
  explicit_selection=1
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
source_root="$repo_root/skillpacks/ecc/skills"
target_root="$repo_root/skills"

[[ -d "$source_root" ]] || { echo "[x] Missing $source_root" >&2; exit 1; }
[[ -d "$target_root" ]] || { echo "[ok] nothing to disable"; exit 0; }

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

removed=0
status=0
while IFS= read -r name; do
  [[ -n "$name" ]] || continue
  dst="$target_root/$name"

  if [[ ! -d "$dst" ]]; then
    if [[ "$explicit_selection" -eq 1 ]]; then
      echo "[x] Skill not enabled: $name" >&2
      status=1
    fi
    continue
  fi

  if [[ ! -f "$dst/.ecc-skillpack-marker" ]]; then
    echo "[!] Skip non-ECC skill: $dst"
    continue
  fi

  rm -rf "$dst"
  echo "[ok] disabled $name"
  removed=$((removed + 1))
done < <(resolve_names)

echo "[ok] disabled_count=$removed"
exit "$status"
