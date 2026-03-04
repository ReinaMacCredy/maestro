#!/bin/bash
# Compatibility no-op hook for UserPromptSubmit.
# Keeps legacy hook references valid without mutating prompt behavior.
set -euo pipefail
cat >/dev/null || true
exit 0
