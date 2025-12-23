#!/usr/bin/env bash
set -euo pipefail

repo_url="https://github.com/ReinaMacCredy/maestro.git"
codex_home="${CODEX_HOME:-"$HOME/.codex"}"
dest="${codex_home}/skills/maestro"

if ! command -v git >/dev/null 2>&1; then
  echo "Error: git is required." >&2
  exit 1
fi

if [ -d "${dest}/.git" ]; then
  git -C "${dest}" pull --ff-only
  echo "Updated Maestro in ${dest}"
elif [ -e "${dest}" ]; then
  echo "Error: ${dest} exists but is not a git repo." >&2
  echo "Remove it, or move it aside, then rerun." >&2
  exit 1
else
  mkdir -p "$(dirname "${dest}")"
  git clone "${repo_url}" "${dest}"
  echo "Installed Maestro to ${dest}"
fi

echo "Restart Codex to pick up new skills."
