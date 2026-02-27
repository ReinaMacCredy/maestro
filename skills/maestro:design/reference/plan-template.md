# Implementation Plan Template

Enhanced plan template with FR traceability. Same TDD structure as new-track but each task references which functional requirements it addresses.

```markdown
# Implementation Plan: {title}

> Track: {track_id}
> Type: {feature | bug | chore}
> Created: {YYYY-MM-DD}

## Phase 1: {phase title}

### Task 1.1: {task title}
**Addresses:** FR-1, FR-3
- [ ] Write failing tests for {task}
- [ ] Implement {task} to pass tests
- [ ] Refactor {task} (if needed)

### Task 1.2: {task title}
**Addresses:** FR-2
- [ ] Write failing tests for {task}
- [ ] Implement {task} to pass tests
- [ ] Refactor {task} (if needed)

### Phase 1 Completion Verification
- [ ] Run test suite for Phase 1 scope
- [ ] Verify coverage >= {threshold}%
- [ ] Manual verification: {step-by-step check}

## Phase 2: {phase title}
...

## Requirements Coverage Matrix

| FR | Description | Task(s) | Status |
|----|------------|---------|--------|
| FR-1 | {description} | 1.1, 2.3 | Covered |
| FR-2 | {description} | 1.2 | Covered |
| FR-3 | {description} | -- | [!] ORPHANED |
```

## TDD Pattern Injection

For TDD methodology, every implementation task gets three sub-tasks:

1. **Write failing tests** (Red)
   - Create test file if it does not exist
   - Write tests defining expected behavior from spec
   - Run tests -- MUST fail (confirms tests are meaningful)
   - Do NOT proceed until tests fail

2. **Implement to pass** (Green)
   - Write minimum code to make tests pass
   - Run tests -- MUST pass
   - Do NOT add code beyond what is needed to pass

3. **Refactor** (optional)
   - Improve code quality with passing tests as safety net
   - Run tests after refactoring -- MUST still pass

## Ship-fast Pattern (alternative)

For ship-fast methodology, implementation tasks get:

1. **Implement** -- Write the feature/fix code
2. **Add tests** -- Write tests covering the implementation
3. **Verify** -- Run tests, confirm passing

## Phase Completion Verification

Every phase ends with a verification meta-task:

1. **Automated test execution**
   - Announce exact command before running (e.g., `CI=true npm test`)
   - Run and report results
   - Max 2 fix attempts on failure; if still failing, halt and ask user

2. **Manual verification plan**
   - Generate step-by-step verification instructions
   - Include commands and expected outcomes
   - Frontend: start dev server, test UI interactions
   - Backend: verify API endpoints with curl/httpie
   - CLI: run commands with expected output

3. **User confirmation**
   - Wait for explicit user approval
   - Record checkpoint commit SHA

## Sizing Guidelines

Design tracks are shifted up one tier from new-track because deeper specs produce larger plans.

| Scope | Phases | Tasks/Phase |
|-------|--------|-------------|
| Medium (3-8 files) | 2-3 | 2-4 |
| Large (8-15 files) | 3-5 | 3-5 |
| XL (15+ files) | 4-6 | 3-6 |

## Dependency Rules

- Tasks within a phase are ordered by dependency
- No forward references (task N cannot depend on task N+1)
- Shared infrastructure tasks come first (models, schemas, config)
- UI tasks come after their backing API/logic tasks
- Integration tests come after unit tests

## FR Traceability Rules

- Every task MUST have an "Addresses: FR-N, FR-M" line
- Every FR in the spec MUST appear in at least one task's Addresses line
- The Requirements Coverage Matrix at the end validates complete coverage
- Any FR marked ORPHANED is a plan gap that MUST be resolved before approval
