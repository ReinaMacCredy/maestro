---
description: Gate 4 - Validates implementation against plan after TDD REFACTOR phase
---

# Validate Plan Execution

Gate 4 of the validation pipeline. Validates that implementation matches plan after completing TDD REFACTOR phase.

## Initial Setup

Before beginning validation, establish context:

1. **Confirm TDD phase**: Implementation must be in REFACTOR or complete
2. **Locate plan file**: `conductor/tracks/<track-id>/plan.md`
3. **Identify current task/epic**: From track's `metadata.json` or beads status
4. **Prepare verification environment**: Ensure clean working state

## Validation Process

### Step 1: Context Discovery

Gather implementation context through parallel research:

```bash
# Parallel execution - run these concurrently
git diff HEAD~1 --stat                    # Recent changes summary
git diff HEAD~1 -- '*.ts' '*.js' '*.py'   # Code changes
cat conductor/tracks/<track-id>/plan.md   # Plan requirements
bd show <bead-id> --json                  # Task context
```

**Research tasks (parallel):**
- [ ] Gather git diff of implementation changes
- [ ] Read plan.md task/epic requirements
- [ ] Check verification commands from plan
- [ ] Review acceptance criteria for current task

### Step 2: Systematic Validation

#### Run Automated Verification

Execute all verification commands from plan:

```bash
# From plan's Automated Verification section
npm test                 # Unit tests
npm run typecheck        # Type checking
npm run lint             # Linting
npm run build            # Build verification
```

**Capture for each command:**
- Exit code (0 = success)
- Error count
- Warning count
- Test pass/fail count

#### Code Review Against Plan

Compare implementation to plan requirements:

| Aspect | Check | Method |
|--------|-------|--------|
| Files changed | Match expected deliverables | `git diff --stat` |
| Functions added | Match task descriptions | Code inspection |
| APIs implemented | Match acceptance criteria | Endpoint testing |
| Tests written | Cover acceptance criteria | Test file review |
| Dependencies | No unexpected additions | `package.json` diff |

#### Deviation Analysis

Identify any deviations from plan:

1. **Expected but missing**: Items in plan not implemented
2. **Unexpected additions**: Code added not in plan
3. **Scope changes**: Implementation differs from spec

### Step 3: Generate Validation Report

```markdown
## Plan Execution Validation Report

**Track:** <track-id>
**Task/Epic:** <current-task-id>
**Date:** <timestamp>
**TDD Phase:** REFACTOR ✅

### Implementation Status

| Deliverable | Plan | Implemented | Status |
|-------------|------|-------------|--------|
| UserService class | T1.1 | ✅ | MATCH |
| Auth middleware | T1.2 | ✅ | MATCH |
| Database migration | T1.3 | ⚠️ Partial | DEVIATION |

**Overall:** 2/3 deliverables complete (67%)

### Automated Verification Results

| Command | Exit Code | Result | Details |
|---------|-----------|--------|---------|
| `npm test` | 0 | ✅ PASS | 45/45 tests pass |
| `npm run typecheck` | 0 | ✅ PASS | No type errors |
| `npm run lint` | 0 | ✅ PASS | No lint errors |
| `npm run build` | 0 | ✅ PASS | Build successful |

**Verification Status:** ALL PASS ✅

### Code Review Findings

#### Matches Plan

- ✅ UserService implements CRUD operations per T1.1
- ✅ Auth middleware validates JWT tokens per T1.2
- ✅ Error handling follows spec patterns

#### Deviations from Plan

- ⚠️ **T1.3 Migration**: Only up migration implemented, down migration missing
- ⚠️ **Extra file**: Added `utils/helpers.ts` not in plan (acceptable utility)

#### Potential Issues

- 🔍 **Performance**: N+1 query pattern in UserService.getAll()
- 🔍 **Security**: Rate limiting not implemented (not in plan but recommended)
- 🔍 **Tests**: Edge case for empty input not covered

### Acceptance Criteria Checklist

From plan task T1.1:

- [x] User can be created with valid data
- [x] User can be retrieved by ID
- [x] User can be updated
- [x] User can be deleted
- [ ] User cannot be created with duplicate email (MISSING)

**Criteria Met:** 4/5 (80%)

### Manual Testing Required

If automated verification passes, these require manual verification:

1. **UI Integration**: Verify frontend displays user data correctly
2. **Error Messages**: Confirm user-friendly error messages
3. **Performance**: Test with >100 concurrent requests

### Recommendations

1. **BLOCKING:** Implement duplicate email check for T1.1 acceptance
2. **BLOCKING:** Add down migration for T1.3
3. **NON-BLOCKING:** Consider N+1 query optimization
4. **NON-BLOCKING:** Add rate limiting in future task
```

## Important Guidelines

### Verification Command Execution

**Every verification command MUST be run fresh:**

```text
[OK] CORRECT: Run command → Read output → Report result
[X] WRONG: Assume previous run still valid
[X] WRONG: Report success without running
```

Reference: [Verification Gate](../../verification/gate.md) - "No completion claims without fresh verification evidence"

### Git Diff Analysis

Use structured diff analysis:

```bash
# Summary of changes
git diff HEAD~1 --stat

# Full diff for code files
git diff HEAD~1 -- '*.ts' '*.tsx' '*.js' '*.jsx'

# Check for unexpected file changes
git diff HEAD~1 --name-only | grep -v test
```

### Acceptance Criteria Validation

Each acceptance criterion must be:

1. **Traced**: Connected to specific test or verification
2. **Verified**: Evidence of passing verification
3. **Documented**: Recorded in validation report

## Validation Checklist

Before marking Gate 4 complete:

- [ ] TDD REFACTOR phase confirmed
- [ ] All verification commands executed
- [ ] All verification commands pass (exit 0)
- [ ] Git diff reviewed for unexpected changes
- [ ] Each deliverable verified against plan
- [ ] Acceptance criteria checklist complete (≥90%)
- [ ] Deviations documented and justified
- [ ] No blocking issues remain
- [ ] Manual testing items identified

## metadata.json Integration

Update the track's `metadata.json` file:

```text
ON VALIDATION START:
  Update metadata.json.validation:
    "current_gate": "plan-execution"

ON VALIDATION COMPLETE (PASS):
  Update metadata.json.validation:
    "gates_passed": [..., "plan-execution"]
    "current_gate": null
    "retries": 0

ON VALIDATION COMPLETE (FAIL):
  Update metadata.json.validation:
    "last_failure": "<failure reason>"
    "retries": <current + 1>
```

Example metadata.json.validation state:
```json
{
  "validation": {
    "gates_passed": ["design", "spec", "plan-structure", "plan-execution"],
    "current_gate": null,
    "retries": 0,
    "last_failure": null
  }
}
```

## Relationship to Other Commands

| Gate | Command | Purpose |
|------|---------|---------|
| Gate 1 | `/conductor-setup` | Project initialization |
| Gate 2 | `/conductor-newtrack` | Spec validation |
| Gate 3 | validate-plan-structure | Plan structure validation |
| **Gate 4** | **validate-plan-execution** | **Implementation validation** |
| Gate 5 | `/conductor-finish` | Track completion |

Gate 4 validates after each task/epic completion during `/conductor-implement`.

## TDD Integration

Gate 4 runs after TDD REFACTOR phase:

```text
RED → GREEN → REFACTOR → Gate 4 Validation
                              ↓
                    Pass? → Next task
                    Fail? → Fix issues → Re-validate
```

**TDD Phase Check:**
```bash
# Confirm metadata.json shows REFACTOR phase
cat conductor/tracks/<track-id>/metadata.json | jq '.tdd.phase'
```

## Failure Modes

| Failure | Severity | Action |
|---------|----------|--------|
| Tests fail | BLOCK | Fix tests before proceeding |
| Typecheck errors | BLOCK | Resolve type errors |
| Lint errors | WARN | Fix or document exceptions |
| Missing deliverable | BLOCK | Complete implementation |
| Criteria <90% | WARN | Document gaps, assess impact |
| Unexpected changes | WARN | Review and justify |

**BLOCK = Cannot complete task, must fix**
**WARN = Can proceed with documented justification**

## Parallel Validation Pattern

For efficiency, run independent validations in parallel:

```bash
# Parallel execution group 1 (automated)
npm test &
npm run typecheck &
npm run lint &
wait

# Parallel execution group 2 (research)
git diff HEAD~1 --stat &
bd show <id> --json &
wait
```

This reduces validation time while maintaining thoroughness.
