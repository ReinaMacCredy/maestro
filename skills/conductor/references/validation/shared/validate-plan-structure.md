---
description: Gate 3 - Validates plan.md structure before implementation begins
---

# Validate Plan Structure

Gate 3 of the validation pipeline. Validates that plan.md has proper structure for implementation.

## Initial Setup

Before beginning validation, establish context:

1. **Locate the plan file**: Find `conductor/tracks/<track-id>/plan.md`
2. **Identify the spec**: Cross-reference with `spec.md` for requirements alignment
3. **Check track metadata**: Read `metadata.json` for track state

## Validation Process

### Step 1: Context Discovery

Gather the plan document and understand its scope:

```bash
# Read the plan file
cat conductor/tracks/<track-id>/plan.md

# Check associated spec for cross-reference
cat conductor/tracks/<track-id>/spec.md
```

**Identify:**
- Total number of epics
- Total number of tasks
- Presence of verification section
- Task dependencies declared

### Step 2: Systematic Validation

Validate each structural element against quality gates:

#### Task Structure Validation

For each task, verify:

| Element | Requirement | Check |
|---------|-------------|-------|
| Acceptance Criteria | At least one measurable criterion | ✅/❌ |
| Complexity Rating | Valid value (S/M/L or 1-5) | ✅/❌ |
| Deliverable | Concrete output defined | ✅/❌ |
| Description | Clear, actionable description | ✅/❌ |
| Dependencies | Valid task IDs if declared | ✅/❌ |

**Task quality formula:**
```
Task Quality = (criteria_present + complexity_valid + deliverable_clear) / 3
Minimum threshold: 0.8 (80%)
```

#### Epic Structure Validation

For each epic, verify:

| Element | Requirement | Check |
|---------|-------------|-------|
| Atomic Tasks | Each task is independently completable | ✅/❌ |
| Dependencies | Logical ordering (no circular deps) | ✅/❌ |
| Task Order | Sequential execution makes sense | ✅/❌ |
| Epic Scope | Tasks relate to single coherent goal | ✅/❌ |
| Parallelization | Independent tasks identified | ✅/❌ |

**Epic quality formula:**
```
Epic Quality = (atomic_tasks + valid_deps + logical_order + coherent_scope) / 4
Minimum threshold: 0.75 (75%)
```

#### Verification Section Validation

The plan MUST include an Automated Verification section:

| Element | Requirement | Check |
|---------|-------------|-------|
| Section Exists | `## Automated Verification` or `## Verification` present | ✅/❌ |
| Commands Listed | At least one verification command | ✅/❌ |
| Commands Runnable | Commands are valid shell commands | ✅/❌ |
| Coverage | Commands cover tests, lint, typecheck | ✅/❌ |
| Success Criteria | Clear pass/fail definition | ✅/❌ |

### Step 3: Generate Validation Report

Compile findings into structured report:

```markdown
## Plan Structure Validation Report

**Track:** <track-id>
**Date:** <timestamp>
**Status:** PASS | FAIL | WARN

### Task Quality Summary

| Task ID | Acceptance Criteria | Complexity | Deliverable | Score |
|---------|---------------------|------------|-------------|-------|
| T1.1    | ✅                  | ✅         | ✅          | 100%  |
| T1.2    | ⚠️ Vague            | ✅         | ❌ Missing  | 33%   |

**Overall Task Quality:** X/Y tasks pass (Z%)

### Epic Quality Summary

| Epic ID | Atomic Tasks | Dependencies | Order | Scope | Score |
|---------|--------------|--------------|-------|-------|-------|
| E1      | ✅           | ✅           | ✅    | ✅    | 100%  |
| E2      | ⚠️ Large T2.3| ✅           | ✅    | ✅    | 75%   |

**Overall Epic Quality:** X/Y epics pass (Z%)

### Verification Section

- [ ] Section exists
- [ ] Commands listed
- [ ] Commands are runnable
- [ ] Test command present
- [ ] Lint command present
- [ ] Build/typecheck command present

**Verification Status:** PASS | FAIL

### Issues Found

1. **[TASK T1.2]** Missing deliverable definition
2. **[TASK T2.3]** Task too large - should be split
3. **[EPIC E2]** Circular dependency detected: T2.1 → T2.3 → T2.1

### Recommendations

1. Add concrete deliverable to T1.2: "Updated config file at path/to/config.json"
2. Split T2.3 into subtasks for atomic execution
3. Resolve circular dependency by reordering or restructuring
```

## Important Guidelines

### What Makes a Valid Task

- **Acceptance criteria must be testable**: "Works correctly" is not valid. "Returns 200 status for valid input" is valid.
- **Complexity must be appropriate**: Tasks rated L or 5 should be split unless truly atomic.
- **Deliverables must be concrete**: Files, functions, APIs, not abstract concepts.

### What Makes a Valid Epic

- **Single responsibility**: One epic = one feature area or capability
- **Ordered for execution**: First task provides foundation for subsequent tasks
- **No orphan tasks**: Every task either has dependencies or is a starting point

### Verification Section Requirements

The verification section must enable automated validation:

```markdown
## Automated Verification

Run these commands to verify implementation:

\`\`\`bash
# Unit tests
npm test

# Type checking
npm run typecheck

# Linting
npm run lint

# Integration tests (if applicable)
npm run test:integration
\`\`\`

**Success criteria:** All commands exit 0 with no errors.
```

## Validation Checklist

Before marking Gate 3 complete:

- [ ] All tasks have acceptance criteria
- [ ] All tasks have complexity ratings
- [ ] All tasks have deliverables
- [ ] All epics have valid dependency chains
- [ ] No circular dependencies exist
- [ ] Verification section exists
- [ ] Verification commands are runnable
- [ ] Overall task quality ≥ 80%
- [ ] Overall epic quality ≥ 75%

## LEDGER Integration

Record validation results in session LEDGER:

```markdown
## Gate 3: Plan Structure Validation

**Timestamp:** 2025-12-29T10:30:00Z
**Result:** PASS | FAIL

**Metrics:**
- Task Quality: X%
- Epic Quality: Y%
- Verification: Present/Missing

**Issues:** <count> issues found
**Blocking:** <list any blocking issues>
```

## Relationship to Other Commands

| Gate | Command | Purpose |
|------|---------|---------|
| Gate 1 | `/conductor-setup` | Project initialization |
| Gate 2 | `/conductor-newtrack` | Spec validation |
| **Gate 3** | **validate-plan-structure** | **Plan structure validation** |
| Gate 4 | validate-plan-execution | Implementation validation |
| Gate 5 | `/conductor-finish` | Track completion |

Gate 3 MUST pass before `/conductor-implement` proceeds to execution phase.

## Failure Modes

| Failure | Action | Resolution |
|---------|--------|------------|
| Task missing criteria | BLOCK | Add acceptance criteria |
| Epic has circular deps | BLOCK | Restructure dependency graph |
| No verification section | WARN | Add verification commands |
| Large tasks detected | WARN | Consider splitting |

**BLOCK = Cannot proceed to implementation**
**WARN = Can proceed with caution, should address**
