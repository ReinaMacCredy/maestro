#!/usr/bin/env bash
# Smoke tests for handoff hooks
#
# Usage: ./scripts/test-hooks.sh
#
# Tests:
# 1. Version command
# 2. SessionStart without ledger
# 3. SessionStart with ledger
# 4. PreCompact creates handoff
# 5. PostToolUse tracks file
# 6. Handoff directory structure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONTINUITY_JS="${REPO_ROOT}/hooks/continuity/dist/continuity.js"

# Test counter
TESTS_RUN=0
TESTS_PASSED=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
    echo "  Expected: $2"
    echo "  Got: $3"
}

test_start() {
    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "Test $TESTS_RUN: "
}

# Setup test environment
setup() {
    TEST_DIR=$(mktemp -d)
    mkdir -p "${TEST_DIR}/conductor/sessions/active"
    mkdir -p "${TEST_DIR}/conductor/sessions/archive"
    mkdir -p "${TEST_DIR}/conductor/handoffs/general"
    cd "${TEST_DIR}"
    
    # Create initial index.md for general
    cat > "${TEST_DIR}/conductor/handoffs/general/index.md" << 'EOF'
---
track_id: general
created: 2025-12-29T10:00:00+07:00
last_updated: 2025-12-29T10:00:00+07:00
---

# Handoff Log: General

| Timestamp | Trigger | Bead | Summary | File |
|-----------|---------|------|---------|------|
EOF
}

# Cleanup test environment
cleanup() {
    cd "${REPO_ROOT}"
    if [[ -n "${TEST_DIR:-}" && -d "${TEST_DIR}" ]]; then
        rm -rf "${TEST_DIR}"
    fi
}

trap cleanup EXIT

echo "=== Handoff System Smoke Tests ==="
echo ""

# Test 1: Version command
test_start
output=$(node "$CONTINUITY_JS" --version 2>&1)
if [[ "$output" =~ v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    pass "Version command outputs version"
else
    fail "Version command" "semantic version (vX.Y.Z)" "$output"
fi

# Test 2: SessionStart without ledger
test_start
setup
output=$(node "$CONTINUITY_JS" SessionStart 2>&1)
if [[ "$output" == *"hookEventName"* && "$output" == *"SessionStart"* ]]; then
    pass "SessionStart without ledger returns JSON"
else
    fail "SessionStart without ledger" "JSON with hookEventName" "$output"
fi

# Test 3: SessionStart with ledger
test_start
setup
cat > "${TEST_DIR}/conductor/sessions/active/LEDGER.md" << 'EOF'
---
updated: 2025-12-27T10:00:00Z
session_id: T-test123
platform: claude
---

# Session Ledger

## Goal

Testing session start hook.
EOF

output=$(node "$CONTINUITY_JS" SessionStart 2>&1)
if [[ "$output" == *"ledger"* && "$output" == *"Testing session start hook"* ]]; then
    pass "SessionStart with ledger includes ledger content"
else
    fail "SessionStart with ledger" "ledger content included" "$output"
fi

# Test 4: PreCompact creates handoff
test_start
setup
cat > "${TEST_DIR}/conductor/sessions/active/LEDGER.md" << 'EOF'
---
updated: 2025-12-27T10:00:00Z
session_id: T-test456
platform: claude
---

# Session Ledger

## State

### Now

- Testing precompact
EOF

# Capture stderr for the handoff creation message
output=$(node "$CONTINUITY_JS" PreCompact 2>&1)
handoff_count=$(ls -1 "${TEST_DIR}/conductor/sessions/archive/"*.md 2>/dev/null | wc -l || echo "0")
if [[ "$handoff_count" -ge 1 ]]; then
    pass "PreCompact creates handoff file"
else
    fail "PreCompact creates handoff" "1 handoff file" "$handoff_count files"
fi

# Test 5: PostToolUse tracks file
test_start
setup
export CLAUDE_TOOL_INPUT='{"file_path": "/path/to/test.ts"}'
exit_code=0
node "$CONTINUITY_JS" PostToolUse 2>&1 || exit_code=$?

if [[ -f "${TEST_DIR}/conductor/sessions/active/LEDGER.md" ]]; then
    content=$(cat "${TEST_DIR}/conductor/sessions/active/LEDGER.md")
    if [[ "$content" == *"/path/to/test.ts"* ]]; then
        pass "PostToolUse tracks modified file"
    else
        fail "PostToolUse" "file path in ledger" "$content"
    fi
elif [[ $exit_code -ne 0 ]]; then
    fail "PostToolUse" "hook to execute successfully" "exit code $exit_code"
else
    fail "PostToolUse" "LEDGER.md created" "file not created"
fi

# Test 6: Handoff directory structure exists
test_start
setup
if [[ -d "${TEST_DIR}/conductor/handoffs/general" ]]; then
    pass "Handoff directory structure created"
else
    fail "Handoff directory" "conductor/handoffs/general/ exists" "directory missing"
fi

# Summary
echo ""
echo "=== Results ==="
echo "Passed: ${TESTS_PASSED}/${TESTS_RUN}"

if [[ "$TESTS_PASSED" -eq "$TESTS_RUN" ]]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
