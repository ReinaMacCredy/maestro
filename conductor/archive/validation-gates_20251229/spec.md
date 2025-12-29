# Spec: Validation Gates

## Overview

Integrate 5 validation gates into Maestro lifecycle to verify design alignment and implementation completeness. Gates route through maestro-core, logic resides in `conductor/references/validation/`.

## Functional Requirements

### FR1: Validation Gate Registry

- **FR1.1**: System must support 5 validation gates:
  - Gate 1: validate-design (after DELIVER)
  - Gate 2: validate-spec (after spec.md generation)
  - Gate 3: validate-plan-structure (after plan.md generation)
  - Gate 4: validate-plan-execution (after TDD REFACTOR)
  - Gate 5: validate-completion (before /conductor-finish)

- **FR1.2**: Each gate must have its own file in `conductor/references/validation/shared/`

- **FR1.3**: Gate lifecycle routing must be defined in `conductor/references/validation/lifecycle.md`

### FR2: LEDGER Integration

- **FR2.1**: LEDGER.md frontmatter must have `validation` section:
  ```yaml
  validation:
    gates_passed: []
    current_gate: null
    retries: 0
    last_failure: null
  ```

- **FR2.2**: LEDGER.md body must have `## Validation History` section to log failures

- **FR2.3**: Each gate pass/fail must update LEDGER state

### FR3: Behavior Matrix

- **FR3.1**: SPEED mode: All gates only WARN, no HALT
- **FR3.2**: FULL mode:
  - design, plan-execution, completion: HALT + retry (max 2)
  - spec, plan-structure: WARN + continue
- **FR3.3**: After max retries (2): Escalate with message "Human review needed"

### FR4: Content Format

- **FR4.1**: All gate files must follow humanlayer format:
  - Initial Setup
  - Validation Process (3 steps)
  - Important Guidelines
  - Validation Checklist
  - LEDGER Integration
  - Relationship to Other Commands

### FR5: Missing File Handling

- **FR5.1**: Missing validation source files → WARN + continue checking others
- **FR5.2**: Missing required files (product.md, tech-stack.md) → WARN + skip that check
- **FR5.3**: Aggregate all warnings in final report

## Non-Functional Requirements

### NFR1: Performance
- Validation must complete in < 30 seconds per gate

### NFR2: Maintainability
- No new skill creation, use existing conductor/references/
- Reuse existing gate.md verification logic

### NFR3: Compatibility
- Works with both SPEED and FULL mode
- Does not break existing workflow

## Acceptance Criteria

### AC1: Design Validation Gate
- [ ] Runs after DELIVER phase
- [ ] Checks design vs product.md goals
- [ ] Checks design vs tech-stack.md constraints
- [ ] Checks pattern consistency with CODEMAPS
- [ ] FULL mode: HALT on failure
- [ ] SPEED mode: WARN on failure

### AC2: Spec Validation Gate
- [ ] Runs after spec.md generation
- [ ] Checks spec captures design intent
- [ ] WARN on failure (both modes)

### AC3: Plan Structure Validation Gate
- [ ] Runs after plan.md generation
- [ ] Checks tasks have acceptance criteria
- [ ] Checks "Automated Verification" section exists
- [ ] WARN on failure (both modes)

### AC4: Plan Execution Validation Gate
- [ ] Runs after TDD REFACTOR phase
- [ ] Checks implementation matches plan
- [ ] Runs verification commands
- [ ] Follows humanlayer format exactly
- [ ] FULL mode: HALT on failure

### AC5: Completion Validation Gate
- [ ] Runs before /conductor-finish
- [ ] Checks all beads closed
- [ ] Checks no uncommitted changes
- [ ] FULL mode: HALT on failure

### AC6: LEDGER Integration
- [ ] validation state tracked in frontmatter
- [ ] Failures logged in Validation History section
- [ ] Retry counter works correctly

### AC7: Retry & Escalation
- [ ] Max 2 retries before escalate
- [ ] Escalation shows clear message
- [ ] Context preserved between retries

## Out of Scope

- New skill creation
- Auto-fix misalignment (report only)
- Validate dependencies (use existing `bd dep`)
- UI/CLI changes
