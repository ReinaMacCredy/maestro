#!/usr/bin/env bash
set -uo pipefail
CHECKOUT=${1:?checkout path}
ROOT=$(mktemp -d /tmp/maestro-verify.XXXX)
mkdir -p "$ROOT/home/.local/bin" "$ROOT/repo"
BUNDIR=$(dirname "$(command -v bun)")
SESSION=(MAESTRO_SESSION_ID=verify-journey MAESTRO_SESSION_PID=$$)
run() { env -i HOME="$ROOT/home" PATH="$ROOT/home/.local/bin:$BUNDIR:/usr/bin:/bin" TMPDIR=/tmp "${SESSION[@]}" "$@" 2>&1; echo "[exit $?]"; }
add() { env -i HOME="$ROOT/home" PATH="$ROOT/home/.local/bin:$BUNDIR:/usr/bin:/bin" "${SESSION[@]}" maestro work add "$@" --json | sed -n 's/.*"id":"\([^"]*\)".*/\1/p'; }
cd "$ROOT/repo" && git init -q -b main && git -c user.name=t -c user.email=t@t.invalid commit -q --allow-empty -m init

echo "## launch: install from $CHECKOUT @ $(git -C "$CHECKOUT" rev-parse --short HEAD)"
run bun "$CHECKOUT/bin/maestro.ts" install | tail -4

echo "## doctor"
run maestro version
run maestro doctor | head -12

echo "## drive: recipe catalog"
run maestro recipe list | grep -E 'verification-skill|exit'
run maestro recipe show verification-skill | head -3

echo "## drive: policy-proof and the check on a bug"
run maestro plugin enable policy-proof
ID=$(add "journey probe" --kind bug --start)
echo "id=$ID"
run maestro work list
run maestro work done "$ID"
run maestro work done "$ID" --check "true"
run maestro work show "$ID" | grep -E '^(proof kind|check):'
EID=$(add "evidence probe" --start)
run maestro work done "$EID" --evidence "probe drove add --start and done (journey)"
run maestro work show "$EID" | grep -E '^proof kind:'
run maestro work list

echo "## drive: decision add"
run maestro decision add "journey decision" --rationale "probe" --work "$ID"
run maestro decision list

echo "## drive: playbooks, principles and skills"
run maestro recipe list | grep -cE '^(investigation|bug-fix|feature|refactoring|prototype|perf-issue|hillclimb|forensics|eval|authoring-a-skill|visual-parity|multi-phase-plan|opening-a-pr|babysit|shipping|figure-it-out|no-comments|review)\b'
run maestro recipe show bug-fix | grep -E '^[0-9]+\. ' | head -3
run maestro recipe show work | tail -1
grep -c '^## ' "$ROOT/home/maestro/PRINCIPLES.md"
grep -c 'Inside a playbook the main chat never writes code' "$ROOT/home/maestro/WORKFLOW.md"
ls "$ROOT/home/maestro/skills"
ls "$ROOT/home/.claude/agents" "$ROOT/home/.codex/agents" 2>/dev/null | grep -c '^maestro-'
grep -c PreToolUse .claude/settings.json

echo "## drive: SessionStart brief and the delivery grant"
run maestro hook record --event SessionStart | grep -E '^maestro:|delivery grant|exit'
run maestro plugin enable policy-auto-pr
run maestro hook record --event SessionStart | grep -E 'delivery grant|exit'
run maestro plugin disable policy-auto-pr

echo "## drive: bundle check and auto-archive"
BID=$(add "bundle probe" --start)
run maestro bundle open probe --work "$BID" | tail -1
run maestro bundle check probe | tail -2
run maestro bundle check probe --final | tail -2
run maestro work done "$BID" --check "true"
run maestro bundle list --all

echo "## drive: reinstall keeps the enable"
run bun "$CHECKOUT/bin/maestro.ts" install >/dev/null
cat .maestro/config; echo

echo "## clean up: $ROOT"
rm -r "$ROOT"
