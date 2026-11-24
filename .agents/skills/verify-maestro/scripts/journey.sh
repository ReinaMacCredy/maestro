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
run maestro recipe list | grep -E 'verification-skill|exit'
run maestro recipe show verification-skill | head -3

echo "## drive: policy-proof on a bug"
run maestro plugin enable policy-proof
ID=$(env -i HOME="$ROOT/home" PATH="$ROOT/home/.local/bin:$BUNDIR:/usr/bin:/bin" "${SESSION[@]}" maestro work add "journey probe" --kind bug --atomic-reason probe --json | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
echo "id=$ID"
run maestro work start "$ID" | tail -1
run maestro work done "$ID"
run maestro work done "$ID" --evidence "probe drove add/start/done (journey)"

echo "## drive: playbooks, principles and skills"
run maestro recipe list | grep -cE '^(investigation|bug-fix|feature|refactoring|prototype|perf-issue|hillclimb|forensics|eval|authoring-a-skill|visual-parity|multi-phase-plan|opening-a-pr|babysit|shipping|figure-it-out|no-comments|review)\b'
run maestro recipe show bug-fix | grep -E '^[0-9]+\. ' | head -3
run maestro recipe show work | tail -1
grep -c '^## ' "$ROOT/home/maestro/PRINCIPLES.md"
grep -c 'Inside a playbook the main chat never writes code' "$ROOT/home/maestro/WORKFLOW.md"
ls "$ROOT/home/maestro/skills"
ls "$ROOT/home/.claude/agents" | grep -cE 'maestro-(delegate|comment-sicko|explorer|explainer|investigator|arena-runner|arena-judge|fan-out-worker|reviewer-structure|reviewer-correctness-codex)\.md'
grep -c PreToolUse .claude/settings.json

echo "## drive: SessionStart brief and the delivery grant"
run maestro hook record --event SessionStart | grep -E '^method:|delivery grant|exit'
run maestro plugin enable policy-auto-pr
run maestro hook record --event SessionStart | grep -E 'delivery grant|exit'
run maestro plugin disable policy-auto-pr

echo "## drive: bundle check"
run maestro bundle open probe | tail -1
run maestro bundle check probe | grep -oE 'U1 is missing [^:]+:|exit [0-9]+' | sort -u

echo "## drive: reinstall keeps the enable"
run bun "$CHECKOUT/bin/maestro.ts" install >/dev/null
cat .maestro/config; echo

echo "## clean up: $ROOT"
rm -r "$ROOT"
