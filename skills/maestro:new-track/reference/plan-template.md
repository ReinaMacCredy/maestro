# Plan Template

## Structure

```markdown
# Implementation Plan: {title}

> Track: {track_id}
> Type: {feature | bug | chore}
> Created: {YYYY-MM-DD}

## Phase 1: {phase title}

### Task 1.1: {task title}
- [ ] Write failing tests for {task}
- [ ] Implement {task} to pass tests
- [ ] Refactor {task} (if needed)

### Task 1.2: {task title}
- [ ] Write failing tests for {task}
- [ ] Implement {task} to pass tests
- [ ] Refactor {task} (if needed)

### Phase 1 Completion Verification
- [ ] Run test suite for Phase 1 scope
- [ ] Verify coverage >= {threshold}%
- [ ] Manual verification: {step-by-step check}

## Phase 2: {phase title}
...
```

## TDD Pattern Injection

For TDD methodology (from `workflow.md`), every implementation task gets three sub-tasks:

1. **Write failing tests** (Red)
   - Create test file if it doesn't exist
   - Write tests defining expected behavior from spec
   - Run tests -- MUST fail (confirms tests are meaningful)
   - Do NOT proceed until tests fail

2. **Implement to pass** (Green)
   - Write minimum code to make tests pass
   - Run tests -- MUST pass
   - Do NOT add code beyond what's needed to pass

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

| Scope | Phases | Tasks/Phase |
|-------|--------|-------------|
| Small (1-2 files) | 1 | 1-3 |
| Medium (3-8 files) | 2-3 | 2-4 |
| Large (8+ files) | 3-4 | 3-5 |

## Dependency Rules

- Tasks within a phase are ordered by dependency
- No forward references (task N cannot depend on task N+1)
- Shared infrastructure tasks come first (models, schemas, config)
- UI tasks come after their backing API/logic tasks
- Integration tests come after unit tests

