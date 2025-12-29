# Spec: Validation Gates

## Overview

Tích hợp 5 validation gates vào Maestro lifecycle để verify design alignment và implementation completeness. Gates routing qua maestro-core, logic nằm trong `conductor/references/validation/`.

## Functional Requirements

### FR1: Validation Gate Registry

- **FR1.1**: Hệ thống phải hỗ trợ 5 validation gates:
  - Gate 1: validate-design (sau DELIVER)
  - Gate 2: validate-spec (sau spec.md generation)
  - Gate 3: validate-plan-structure (sau plan.md generation)
  - Gate 4: validate-plan-execution (sau TDD REFACTOR)
  - Gate 5: validate-completion (trước /conductor-finish)

- **FR1.2**: Mỗi gate phải có file riêng trong `conductor/references/validation/shared/`

- **FR1.3**: Gate lifecycle routing phải được định nghĩa trong `conductor/references/validation/lifecycle.md`

### FR2: LEDGER Integration

- **FR2.1**: LEDGER.md frontmatter phải có `validation` section:
  ```yaml
  validation:
    gates_passed: []
    current_gate: null
    retries: 0
    last_failure: null
  ```

- **FR2.2**: LEDGER.md body phải có `## Validation History` section để log failures

- **FR2.3**: Mỗi gate pass/fail phải update LEDGER state

### FR3: Behavior Matrix

- **FR3.1**: SPEED mode: Tất cả gates chỉ WARN, không HALT
- **FR3.2**: FULL mode:
  - design, plan-execution, completion: HALT + retry (max 2)
  - spec, plan-structure: WARN + continue
- **FR3.3**: Sau max retries (2): Escalate với message "Human review needed"

### FR4: Content Format

- **FR4.1**: Tất cả gate files phải theo humanlayer format:
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
- Validation phải complete trong < 30 seconds mỗi gate

### NFR2: Maintainability
- Không tạo skill mới, dùng existing conductor/references/
- Reuse existing gate.md verification logic

### NFR3: Compatibility
- Hoạt động với cả SPEED và FULL mode
- Không break existing workflow

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
