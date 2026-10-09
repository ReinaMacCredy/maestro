#!/usr/bin/env bash
set -uo pipefail
CHECKOUT=${1:?checkout path}
ROOT=$(mktemp -d /tmp/maestro-verify.XXXX)
mkdir -p "$ROOT/home/.local/bin" "$ROOT/repo"
BUNDIR=$(dirname "$(command -v bun)")
SESSION=(MAESTRO_SESSION_ID=verify-journey MAESTRO_SESSION_PID=$$)
run() { env -i HOME="$ROOT/home" PATH="$ROOT/home/.local/bin:$BUNDIR:/usr/bin:/bin" TMPDIR=/tmp "${SESSION[@]}" "$@" 2>&1; echo "[exit $?]"; }
cd "$ROOT/repo" && git init -q -b main && git -c user.name=t -c user.email=t@t.invalid commit -q --allow-empty -m init

echo "## launch: install from $CHECKOUT @ $(git -C "$CHECKOUT" rev-parse --short HEAD)"
run bun "$CHECKOUT/bin/maestro.ts" install | tail -4

echo "## doctor"
run maestro version
run maestro doctor | head -12

echo "## drive: recipe catalog"
run maestro recipe list | grep -E 'verify-skill|exit'
run maestro recipe show verify-skill | head -3

echo "## drive: policy-layer on a bug"
run maestro plugin enable policy-layer
ID=$(env -i HOME="$ROOT/home" PATH="$ROOT/home/.local/bin:$BUNDIR:/usr/bin:/bin" "${SESSION[@]}" maestro work add "journey probe" --kind bug --atomic-reason probe --json | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
echo "id=$ID"
run maestro work start "$ID" | tail -1
run maestro work done "$ID" --evidence "tests pass"
run maestro work done "$ID" --evidence "suite green (source)"
run maestro work done "$ID" --evidence "probe drove add/start/done (journey)"

echo "## drive: reinstall keeps the enable"
run bun "$CHECKOUT/bin/maestro.ts" install >/dev/null
cat .maestro/config; echo

echo "## clean up: $ROOT"
rm -r "$ROOT"
