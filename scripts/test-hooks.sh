#!/bin/bash
# test-hooks.sh - Smoke tests for all Maestro hook scripts
# Runs 15 tests with temp directory isolation and simulated stdin JSON input

set -euo pipefail

SCRIPTS_DIR="$(cd "$(dirname "$0")/../.claude/scripts" && pwd)"
PASS=0
FAIL=0
TOTAL=15

red() { printf '\033[0;31m%s\033[0m\n' "$1"; }
green() { printf '\033[0;32m%s\033[0m\n' "$1"; }
bold() { printf '\033[1m%s\033[0m\n' "$1"; }

pass() {
  PASS=$((PASS + 1))
  green "  PASS: $1"
}

fail() {
  FAIL=$((FAIL + 1))
  red "  FAIL: $1 — $2"
}

# Create isolated temp project directory
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

setup_project() {
  rm -rf "$TMPDIR"/.* "$TMPDIR"/* 2>/dev/null || true
  mkdir -p "$TMPDIR/.claude/skills/test-skill"
  mkdir -p "$TMPDIR/.maestro/plans"
  mkdir -p "$TMPDIR/.maestro/wisdom"
}

bold "=== Maestro Hook Smoke Tests ==="
echo ""

# -------------------------------------------------------
# Test 1: session-start.sh outputs valid JSON with context
# -------------------------------------------------------
bold "Test 1: session-start.sh outputs context when skills/plans/wisdom exist"
setup_project
cat > "$TMPDIR/.claude/skills/test-skill/SKILL.md" <<'SKILL'
---
name: test-skill
description: A test skill
---
# Test Skill
SKILL
cat > "$TMPDIR/.maestro/plans/my-plan.md" <<'PLAN'
# My Plan
## Objective
Do things
PLAN
cat > "$TMPDIR/.maestro/wisdom/lesson.md" <<'WISDOM'
# Lesson Learned
Always test
WISDOM

output=$(CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/session-start.sh" < /dev/null 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"test-skill"* && "$context" == *"my-plan"* && "$context" == *"lesson"* ]]; then
    pass "session-start.sh outputs valid JSON with skills, plans, and wisdom"
  else
    fail "session-start.sh context content" "Missing expected content in: $context"
  fi
else
  fail "session-start.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 2: session-start.sh exits silently when .maestro/ is empty
# -------------------------------------------------------
bold "Test 2: session-start.sh exits silently when empty"
rm -rf "$TMPDIR"/.* "$TMPDIR"/* 2>/dev/null || true
mkdir -p "$TMPDIR/.maestro"

output=$(CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/session-start.sh" < /dev/null 2>&1) || true
if [[ -z "$output" ]]; then
  pass "session-start.sh exits silently with no content"
else
  fail "session-start.sh silent exit" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 3: subagent-context.sh outputs context for kraken
# -------------------------------------------------------
bold "Test 3: subagent-context.sh outputs context for kraken agent"
setup_project
cat > "$TMPDIR/.maestro/plans/work.md" <<'PLAN'
# Work Plan
## Tasks
- [ ] Do something
- [x] Done thing
PLAN

output=$(echo '{"agent_type":"kraken"}' | CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/subagent-context.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"Work Plan"* ]]; then
    pass "subagent-context.sh outputs plan context for kraken"
  else
    fail "subagent-context.sh context" "Missing plan content in: $context"
  fi
else
  fail "subagent-context.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 4: subagent-context.sh exits silently for unknown agent types
# -------------------------------------------------------
bold "Test 4: subagent-context.sh exits silently for unknown agent"
output=$(echo '{"agent_type":"unknown-agent"}' | CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/subagent-context.sh" 2>&1) || true
if [[ -z "$output" ]]; then
  pass "subagent-context.sh exits silently for unknown agent type"
else
  fail "subagent-context.sh unknown agent" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 5: orchestrator-guard.sh denies Write for orchestrator
# -------------------------------------------------------
bold "Test 5: orchestrator-guard.sh denies Write for orchestrator"
output=$(echo '{"tool_name":"Write","agent_name":"orchestrator"}' | bash "$SCRIPTS_DIR/orchestrator-guard.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.permissionDecision' > /dev/null 2>&1; then
  decision=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')
  if [[ "$decision" == "deny" ]]; then
    pass "orchestrator-guard.sh denies Write for orchestrator"
  else
    fail "orchestrator-guard.sh decision" "Expected deny, got: $decision"
  fi
else
  fail "orchestrator-guard.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 6: orchestrator-guard.sh allows Write for non-orchestrator
# -------------------------------------------------------
bold "Test 6: orchestrator-guard.sh allows Write for non-orchestrator"
output=$(echo '{"tool_name":"Write","agent_name":"kraken"}' | bash "$SCRIPTS_DIR/orchestrator-guard.sh" 2>&1) || true
if [[ -z "$output" ]]; then
  pass "orchestrator-guard.sh allows Write for kraken (no output, exit 0)"
else
  fail "orchestrator-guard.sh allow" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 7: plan-protection.sh denies kraken editing .maestro/plans/
# -------------------------------------------------------
bold "Test 7: plan-protection.sh denies kraken editing plans"
output=$(echo '{"tool_name":"Write","agent_name":"kraken","tool_input":{"file_path":".maestro/plans/plan.md"}}' | bash "$SCRIPTS_DIR/plan-protection.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.permissionDecision' > /dev/null 2>&1; then
  decision=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')
  if [[ "$decision" == "deny" ]]; then
    pass "plan-protection.sh denies kraken editing plans"
  else
    fail "plan-protection.sh decision" "Expected deny, got: $decision"
  fi
else
  fail "plan-protection.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 8: plan-protection.sh allows edits outside .maestro/plans/
# -------------------------------------------------------
bold "Test 8: plan-protection.sh allows edits outside plans"
output=$(echo '{"tool_name":"Write","agent_name":"kraken","tool_input":{"file_path":"src/index.ts"}}' | bash "$SCRIPTS_DIR/plan-protection.sh" 2>&1) || true
if [[ -z "$output" ]]; then
  pass "plan-protection.sh allows edits to files outside .maestro/plans/"
else
  fail "plan-protection.sh allow" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 9: plan-validator.sh warns on plan missing ## Objective
# -------------------------------------------------------
bold "Test 9: plan-validator.sh warns on missing sections"
setup_project
# Create a plan missing required sections
cat > "$TMPDIR/.maestro/plans/bad-plan.md" <<'PLAN'
# Bad Plan
Just some text without sections
PLAN

output=$(echo "{\"tool_input\":{\"file_path\":\"$TMPDIR/.maestro/plans/bad-plan.md\"}}" | bash "$SCRIPTS_DIR/plan-validator.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"## Objective"* ]]; then
    pass "plan-validator.sh warns about missing ## Objective"
  else
    fail "plan-validator.sh warning content" "Missing Objective mention in: $context"
  fi
else
  fail "plan-validator.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 10: plan-validator.sh exits silently for non-plan writes
# -------------------------------------------------------
bold "Test 10: plan-validator.sh silent for non-plan files"
output=$(echo '{"tool_input":{"file_path":"src/index.ts"}}' | bash "$SCRIPTS_DIR/plan-validator.sh" 2>&1) || true
if [[ -z "$output" ]]; then
  pass "plan-validator.sh exits silently for non-plan writes"
else
  fail "plan-validator.sh non-plan" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 11: wisdom-injector.sh lists wisdom when reading a plan
# -------------------------------------------------------
bold "Test 11: wisdom-injector.sh lists wisdom for plan reads"
setup_project
cat > "$TMPDIR/.maestro/wisdom/architecture.md" <<'WISDOM'
# Architecture Decisions
Important stuff
WISDOM

output=$(echo "{\"tool_input\":{\"file_path\":\"$TMPDIR/.maestro/plans/my-plan.md\"}}" | CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/wisdom-injector.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"architecture"* ]]; then
    pass "wisdom-injector.sh lists wisdom files when reading a plan"
  else
    fail "wisdom-injector.sh content" "Missing wisdom file reference in: $context"
  fi
else
  fail "wisdom-injector.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 12: verification-injector.sh outputs reminder JSON
# -------------------------------------------------------
bold "Test 12: verification-injector.sh outputs reminder"
output=$(echo '{}' | bash "$SCRIPTS_DIR/verification-injector.sh" 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"VERIFY"* ]]; then
    pass "verification-injector.sh outputs verification reminder"
  else
    fail "verification-injector.sh content" "Missing VERIFY in: $context"
  fi
else
  fail "verification-injector.sh JSON structure" "Output: $output"
fi

# -------------------------------------------------------
# Test 13: plan-context-injector.sh outputs for executing status
# -------------------------------------------------------
bold "Test 13: plan-context-injector.sh outputs active plan context for executing"
setup_project
mkdir -p "$TMPDIR/.maestro/handoff"
cat > "$TMPDIR/.maestro/handoff/test-handoff.json" <<'HANDOFF'
{"status":"executing","topic":"Add auth system","plan_destination":".maestro/plans/add-auth-system.md"}
HANDOFF

output=$(CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/plan-context-injector.sh" 2>&1) || true
if [[ "$output" == *"EXECUTING plan: Add auth system"* ]]; then
  pass "plan-context-injector.sh outputs EXECUTING plan context"
else
  fail "plan-context-injector.sh executing output" "Expected EXECUTING plan reference, got: $output"
fi

# -------------------------------------------------------
# Test 14: plan-context-injector.sh silent on no active plans
# -------------------------------------------------------
bold "Test 14: plan-context-injector.sh silent when no active plans"
setup_project
mkdir -p "$TMPDIR/.maestro/handoff"
cat > "$TMPDIR/.maestro/handoff/done.json" <<'HANDOFF'
{"status":"completed","topic":"Old plan","plan_destination":".maestro/plans/old.md"}
HANDOFF

output=$(CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/plan-context-injector.sh" 2>&1) || true
if [[ -z "$output" ]]; then
  pass "plan-context-injector.sh exits silently when no active plans"
else
  fail "plan-context-injector.sh silent exit" "Expected no output, got: $output"
fi

# -------------------------------------------------------
# Test 15: session-start.sh includes ACTIVE PLAN for executing handoff
# -------------------------------------------------------
bold "Test 15: session-start.sh includes ACTIVE PLAN from handoff"
setup_project
mkdir -p "$TMPDIR/.maestro/handoff"
cat > "$TMPDIR/.maestro/handoff/active.json" <<'HANDOFF'
{"status":"executing","topic":"Build dashboard","plan_destination":".maestro/plans/build-dashboard.md"}
HANDOFF

output=$(CLAUDE_PROJECT_DIR="$TMPDIR" bash "$SCRIPTS_DIR/session-start.sh" < /dev/null 2>&1) || true
if echo "$output" | jq -e '.hookSpecificOutput.additionalContext' > /dev/null 2>&1; then
  context=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  if [[ "$context" == *"ACTIVE PLAN: Build dashboard"* ]]; then
    pass "session-start.sh includes ACTIVE PLAN from handoff file"
  else
    fail "session-start.sh active plan content" "Missing ACTIVE PLAN in: $context"
  fi
else
  fail "session-start.sh active plan JSON" "Output: $output"
fi

# -------------------------------------------------------
# Summary
# -------------------------------------------------------
echo ""
bold "=== Results ==="
echo "  Total:  $TOTAL"
green "  Passed: $PASS"
if [[ $FAIL -gt 0 ]]; then
  red "  Failed: $FAIL"
  exit 1
else
  echo "  Failed: 0"
  green "All tests passed!"
fi
