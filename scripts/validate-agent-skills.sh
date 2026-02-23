#!/usr/bin/env bash
set -euo pipefail

# Validate SKILL.md files against core Agent Skills requirements.
# Checks:
# - YAML frontmatter exists
# - required fields: name, description
# - name format: lowercase alphanumeric + hyphens, <= 64 chars
# - directory name equals frontmatter name
# - description is non-empty and <= 1024 chars

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [ ! -d "skills" ]; then
  echo "[x] missing canonical skills directory: skills/"
  exit 1
fi

errors=0
count=0

while IFS= read -r -d '' file; do
  count=$((count + 1))
  skill_dir="$(basename "$(dirname "$file")")"

  frontmatter="$(sed -n '/^---$/,/^---$/p' "$file")"
  if [ -z "$frontmatter" ]; then
    echo "[x] $file: missing YAML frontmatter"
    errors=$((errors + 1))
    continue
  fi

  name="$(printf '%s\n' "$frontmatter" | sed -n 's/^name:[[:space:]]*//p' | head -n1)"
  description="$(printf '%s\n' "$frontmatter" | sed -n 's/^description:[[:space:]]*//p' | head -n1)"

  if [ -z "$name" ]; then
    echo "[x] $file: missing required frontmatter field 'name'"
    errors=$((errors + 1))
  fi

  if [ -z "$description" ]; then
    echo "[x] $file: missing required frontmatter field 'description'"
    errors=$((errors + 1))
  fi

  if [ -n "$name" ]; then
    if ! printf '%s' "$name" | rg -q '^[a-z0-9-]{1,64}$'; then
      echo "[x] $file: invalid name '$name' (must match ^[a-z0-9-]{1,64}$)"
      errors=$((errors + 1))
    fi

    if [ "$name" != "$skill_dir" ]; then
      echo "[x] $file: name '$name' must match parent directory '$skill_dir'"
      errors=$((errors + 1))
    fi
  fi

  if [ -n "$description" ]; then
    desc_len="$(printf '%s' "$description" | wc -c | tr -d ' ')"
    if [ "$desc_len" -gt 1024 ]; then
      echo "[x] $file: description exceeds 1024 chars ($desc_len)"
      errors=$((errors + 1))
    fi
  fi
done < <(find -L skills -mindepth 2 -maxdepth 2 -name SKILL.md -print0)

if [ "$count" -eq 0 ]; then
  echo "[x] no SKILL.md files found under skills/"
  exit 1
fi

if [ "$errors" -gt 0 ]; then
  echo "[x] validation failed: $errors issue(s) across $count skill(s)"
  exit 1
fi

echo "[ok] validated $count skill(s); all checks passed"
