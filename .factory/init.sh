#!/bin/bash
set -euo pipefail

echo "== Maestro Mission Control init =="

if ! command -v bun >/dev/null 2>&1; then
  echo "[!] Bun is required: https://bun.sh"
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  echo "[!] Git is required for Maestro CLI development"
  exit 1
fi

echo "[ok] bun $(bun --version)"
echo "[ok] $(git --version)"

if [ ! -d "node_modules" ]; then
  echo "[...] Installing dependencies"
  bun install
else
  echo "[ok] node_modules already present"
fi

echo "[...] Verifying local toolchain"
bun run typecheck >/dev/null
echo "[ok] TypeScript typecheck passes at init time"

echo "== Init complete =="
