#!/usr/bin/env bash
# Memory search wrapper script
# Usage: memory-search.sh [index|search|add|status] [args]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="${SCRIPT_DIR%/*}/lib"

if [ ! -f "${LIB_DIR}/memory_search.py" ]; then
    echo "Error: memory_search.py not found" >&2
    exit 1
fi

python3 "${LIB_DIR}/memory_search.py" "$@"
