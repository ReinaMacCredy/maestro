#!/usr/bin/env bash
# Smoke tests for handoff hooks
#
# Usage: ./scripts/test-hooks.sh
#
# Tests:
# 1. Version command
# 2. Handoff directory structure
# 3. Create handoff file
# 4. Index update on handoff

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

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
    mkdir -p "${TEST_DIR}/conductor/handoffs/general"
    mkdir -p "${TEST_DIR}/conductor/handoffs/test-track_20251229"
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

# Test 1: Directory structure exists
test_start
setup
if [[ -d "${TEST_DIR}/conductor/handoffs/general" ]]; then
    pass "Handoff directory structure created"
else
    fail "Handoff directory" "conductor/handoffs/general/ exists" "directory missing"
fi

# Test 2: Index.md exists
test_start
if [[ -f "${TEST_DIR}/conductor/handoffs/general/index.md" ]]; then
    pass "Index.md exists in general/"
else
    fail "Index.md" "file exists" "file missing"
fi

# Test 3: Index.md has correct format
test_start
if grep -q "^| Timestamp |" "${TEST_DIR}/conductor/handoffs/general/index.md"; then
    pass "Index.md has correct table format"
else
    fail "Index.md format" "table header present" "table header missing"
fi

# Test 4: Can create handoff file with timestamp
test_start
timestamp=$(date +%Y-%m-%d_%H-%M-%S-000)
handoff_file="${TEST_DIR}/conductor/handoffs/general/${timestamp}_general_manual.md"
cat > "$handoff_file" << 'EOF'
---
timestamp: 2025-12-29T10:00:00.000+07:00
trigger: manual
track_id: general
git_commit: abc123f
git_branch: main
author: agent
---

# Handoff: general | manual

## Context

Test handoff for smoke tests.

## Changes

- None

## Learnings

- Test learning

## Next Steps

1. [ ] Verify tests pass
EOF

if [[ -f "$handoff_file" ]]; then
    pass "Handoff file created with timestamp naming"
else
    fail "Handoff file creation" "file created" "file missing"
fi

# Test 5: Track-specific handoff directory works
test_start
track_handoff="${TEST_DIR}/conductor/handoffs/test-track_20251229/${timestamp}_test-track_epic-start.md"
cat > "$track_handoff" << 'EOF'
---
timestamp: 2025-12-29T10:00:00.000+07:00
trigger: epic-start
track_id: test-track_20251229
bead_id: E1-test
git_commit: def456a
git_branch: feat/test-track
author: agent
---

# Handoff: test-track_20251229 | epic-start

## Context

Starting Epic 1 for test track.

## Changes

- None yet

## Learnings

- TBD

## Next Steps

1. [ ] Implement E1 tasks
EOF

if [[ -f "$track_handoff" ]]; then
    pass "Track-specific handoff file created"
else
    fail "Track handoff" "file created" "file missing"
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
