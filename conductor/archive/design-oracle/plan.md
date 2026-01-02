# Implementation Plan: Auto Oracle Design Review

## Phase 1: Oracle Agent Template

Create the Oracle agent that runs on non-Amp platforms.

### 1.1 Create Oracle Agent
- [x] **Task 1.1.1**: Create `skills/orchestrator/agents/review/oracle.md`
  - File: `skills/orchestrator/agents/review/oracle.md`
  - Acceptance: Agent template with 6-dimension prompt, input/output format, Agent Mail reporting
  - depends: none

### 1.2 Update Agent Registry
- [x] **Task 1.2.1**: Update `skills/orchestrator/agents/README.md` with Oracle entry
  - File: `skills/orchestrator/agents/README.md`
  - Acceptance: Oracle listed in review agents section
  - depends: 1.1.1

## Phase 2: CP4 Integration

Wire Oracle into the design session CP4 checkpoint.

### 2.1 Update Double Diamond
- [x] **Task 2.1.1**: Add Oracle call to CP4 section in `double-diamond.md`
  - File: `skills/design/references/double-diamond.md`
  - Acceptance: Lines 74-86 include Oracle trigger with platform detection logic
  - depends: 1.1.1

### 2.2 Update A/P/C Checkpoints
- [x] **Task 2.2.1**: Update [C] Continue behavior at DELIVER in `apc-checkpoints.md`
  - File: `skills/design/references/apc-checkpoints.md`
  - Acceptance: [C] at CP4 runs Oracle first, shows HALT/WARN based on findings
  - depends: 2.1.1

### 2.3 Update Design Skill
- [x] **Task 2.3.1**: Add Oracle to Session Flow step 5 in `design/SKILL.md`
  - File: `skills/design/SKILL.md`
  - Acceptance: Session Flow step 5 (Validate) mentions Oracle audit
  - depends: 2.2.1

## Phase 3: Validation Integration

Document Oracle as part of validation lifecycle.

### 3.1 Update Lifecycle
- [x] **Task 3.1.1**: Document Oracle gate at CP4 in `lifecycle.md`
  - File: `skills/conductor/references/validation/lifecycle.md`
  - Acceptance: Oracle gate documented with inputs, outputs, HALT/WARN rules
  - depends: 2.1.1

## Phase 4: Documentation

Update CODEMAPS and other docs.

### 4.1 Update CODEMAPS
- [x] **Task 4.1.1**: Add Oracle to agent directory in `conductor/CODEMAPS/skills.md`
  - File: `conductor/CODEMAPS/skills.md`
  - Acceptance: Oracle listed under `agents/review/` in Research Protocol section
  - depends: 1.2.1

---

## Automated Verification

```bash
# Verify Oracle agent exists
test -f skills/orchestrator/agents/review/oracle.md && echo "✓ Oracle agent created"

# Verify double-diamond updated
grep -q "Oracle" skills/design/references/double-diamond.md && echo "✓ double-diamond.md updated"

# Verify apc-checkpoints updated
grep -q "Oracle" skills/design/references/apc-checkpoints.md && echo "✓ apc-checkpoints.md updated"

# Verify design SKILL.md updated
grep -q "Oracle" skills/design/SKILL.md && echo "✓ design/SKILL.md updated"

# Verify lifecycle.md updated
grep -q "Oracle" skills/conductor/references/validation/lifecycle.md && echo "✓ lifecycle.md updated"

# Verify CODEMAPS updated
grep -q "oracle.md" conductor/CODEMAPS/skills.md && echo "✓ CODEMAPS updated"
```

## Summary

| Phase | Tasks | Files |
|-------|-------|-------|
| 1. Agent Template | 2 | oracle.md, README.md |
| 2. CP4 Integration | 3 | double-diamond.md, apc-checkpoints.md, SKILL.md |
| 3. Validation | 1 | lifecycle.md |
| 4. Documentation | 1 | CODEMAPS/skills.md |
| **Total** | **7** | **6 files** |

## Track Assignments

| Track | Tasks | Files | Depends On |
|-------|-------|-------|------------|
| A | 1.1.1, 1.2.1 | agents/review/oracle.md, agents/README.md | - |
| B | 2.1.1, 2.2.1, 2.3.1 | design skill refs | A |
| C | 3.1.1, 4.1.1 | validation + CODEMAPS | A |
