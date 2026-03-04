#!/usr/bin/env bash
set -euo pipefail

repo_url="https://github.com/ReinaMacCredy/maestro.git"
codex_home="${CODEX_HOME:-"$HOME/.codex"}"
dest="${codex_home}/skills/maestro"
with_ecc_skillpack=0

while (($#)); do
  case "$1" in
    --with-ecc-skillpack)
      with_ecc_skillpack=1
      shift
      ;;
    --help|-h)
      cat <<'EOF'
Usage: scripts/install-codex.sh [--with-ecc-skillpack]

Options:
  --with-ecc-skillpack   After install/update, enable optional ECC skillpack
                         using scripts/enable-ecc-skillpack.sh all (if available)

Examples:
  scripts/install-codex.sh
  scripts/install-codex.sh --with-ecc-skillpack
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

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

if [[ "$with_ecc_skillpack" -eq 1 ]]; then
  skillpack_script="${dest}/scripts/enable-ecc-skillpack.sh"
  if [[ -f "$skillpack_script" ]]; then
    bash "$skillpack_script" all
    echo "Enabled optional ECC skillpack"
  else
    echo "Optional ECC skillpack requested, but ${skillpack_script} is not available yet." >&2
    echo "You can enable it later with: bash ${dest}/scripts/enable-ecc-skillpack.sh all" >&2
  fi
fi

echo "Restart Codex to pick up new skills."
