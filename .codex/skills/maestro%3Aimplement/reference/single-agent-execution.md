# Single-Agent Execution Protocol

## When to Use

Single-agent mode is the default. Use it when:
- Track has 1-3 tasks
- All tasks are sequential (each depends on the previous)
- Tasks touch heavily overlapping files
- You want maximum control and review between each task

## Task Execution Loop

For each task in the queue, follow the workflow methodology from `workflow.md`.

**Loop structure:**
```
for each phase in plan:
  for each task in phase:
    6a.1: Mark in progress
    6a.2-6a.4: Red-Green-Refactor (TDD) or Implement-Test (ship-fast)
    6a.5-6a.6: Verify coverage and compliance
    6a.7-6a.9: Commit, attach summary, record SHA
  end
  Phase Completion Protocol
end
```

---

## TDD Methodology

### 6a.1: Mark Task In Progress

Edit `plan.md`: Change task checkbox from `[ ]` to `[~]`.

**Deferred context**: If deferred context (workflow.md, tech-stack.md) has not been loaded yet, load it now before executing the first task.

If `br_enabled`: see BR Mirror Protocol below.

### 6a.2: Red Phase -- Write Failing Tests

1. Identify what to test based on task description and spec
2. Create test file if it doesn't exist
3. Write tests defining expected behavior
4. Run test suite:
   ```bash
   CI=true {test_command}
   ```
5. Confirm tests FAIL (this validates they're meaningful)
6. If tests pass unexpectedly: the behavior already exists. Skip to refactor or mark complete.
7. Do NOT proceed to implementation until tests fail.

### 6a.3: Green Phase -- Implement to Pass

1. Write minimum code to make tests pass
2. Run test suite:
   ```bash
   CI=true {test_command}
   ```
3. Confirm tests PASS
4. If tests fail: debug and fix. Max 3 attempts. If still failing, ask user for help.

### 6a.4: Refactor (Optional)

1. Review the implementation for code smells
2. Improve readability and structure
3. Run tests again to confirm still passing
4. Skip if implementation is already clean

### 6a.5: Verify Coverage

If `workflow.md` specifies a coverage threshold:
```bash
CI=true {coverage_command}
```

Check that new code meets the threshold. If not, add more tests.

### 6a.6: Check Tech Stack Compliance

If the task introduced a new library or technology not in `tech-stack.md`:
1. STOP implementation
2. Inform user: "This task uses {new_tech} which isn't in the tech stack."
3. Ask: Add to tech stack or find an alternative?
4. If approved: update `.maestro/context/tech-stack.md`
5. Resume

### 6a.7: Commit Code Changes

```bash
git add {changed_files}
git commit -m "{type}({scope}): {description}"
```

Commit message format:
- `feat(scope):` for new features
- `fix(scope):` for bug fixes
- `refactor(scope):` for refactoring
- `test(scope):` for test-only changes

### 6a.8: Attach Summary (if configured)

If `workflow.md` specifies git notes:
```bash
git notes add -m "Task: {task_name}
Phase: {phase_number}
Changes: {files_changed}
Summary: {what_and_why}" {commit_hash}
```

If commit messages: include summary in the commit message body.

### 6a.8.5: Capture Task Notes

If the task produced a non-obvious decision, constraint, or learning during implementation:

1. Append a bullet to `## Working Memory` in `.maestro/notepad.md`
2. Format: `- [{date}] [{track_id}:{task_name}] {insight}`
3. Only capture durable insights -- not routine status. Skip if nothing notable.

### 6a.9: Record Task SHA

Edit `plan.md`: Change task marker from `[~]` to `[x] {sha}` (first 7 characters of commit hash). Do NOT commit plan.md here -- plan state changes are batched and committed at phase completion.

If `br_enabled`: see BR Mirror Protocol below.

---

## Ship-fast Methodology

Same flow but reordered:
1. Mark in progress
2. Implement the feature/fix
3. Write tests covering the implementation
4. Run tests, verify passing
5. Commit, attach summary, record SHA

---

## Worked Example: Single-Agent TDD

**Track**: "Add user email validation"
**Plan**: Phase 1 has 3 tasks:
- Task 1.1: Create email validator module
- Task 1.2: Integrate validator into registration form
- Task 1.3: Add error messages for invalid emails

**Execution flow:**

```
[ok] Starting Track: add-user-email-validation (single-agent mode)
[ok] Loaded plan.md (3 tasks, 1 phase)

--- Task 1.1: Create email validator module ---
[~] Marked in progress
RED:   Created tests/email-validator.test.ts
       - rejects empty string
       - rejects missing @
       - accepts valid email
       Run: CI=true bun test tests/email-validator.test.ts
       [ok] 3 tests FAIL (expected -- module doesn't exist)

GREEN: Created src/utils/email-validator.ts
       Run: CI=true bun test tests/email-validator.test.ts
       [ok] 3 tests PASS

REFACTOR: Extracted regex to named constant. Tests still pass.

COMMIT: git add src/utils/email-validator.ts tests/email-validator.test.ts
        git commit -m "feat(validation): add email validator module"
[x] Task 1.1 complete (sha: a1b2c3d)

--- Task 1.2: Integrate validator into registration form ---
[~] Marked in progress
RED:   Added test in tests/registration.test.ts
       - form rejects invalid email on submit
       Run: CI=true bun test tests/registration.test.ts
       [ok] 1 test FAILS (validator not wired up)

GREEN: Imported validator in src/routes/register.ts
       Run: CI=true bun test tests/registration.test.ts
       [ok] All tests PASS

COMMIT: git commit -m "feat(registration): integrate email validation"
[x] Task 1.2 complete (sha: d4e5f6g)

--- Task 1.3: Add error messages for invalid emails ---
[~] Marked in progress
RED:   Added tests for error message rendering
       [ok] Tests FAIL (no error UI exists)

GREEN: Added error display component
       [ok] Tests PASS

COMMIT: git commit -m "feat(registration): add validation error messages"
[x] Task 1.3 complete (sha: h7i8j9k)

--- Phase 1 Completion ---
[ok] Full test suite: 47 tests, all passing
[ok] Coverage: 94% (threshold: 80%)
Presenting manual verification plan to user...
```

---

## Failure Recovery (Single-Agent)

### Test Won't Pass After 3 Attempts

```
[!] Task 1.2 test "form rejects invalid email" fails after 3 fix attempts.
    Error: TypeError: validator.validate is not a function
--> STOPPING. Asking user for help.
```

Possible causes:
- Spec is ambiguous about the expected API
- Task 1.1 produced a different interface than Task 1.2 expects
- Missing dependency or import issue

Recovery: Ask user to clarify, then fix and continue. Do NOT skip the task.

### Build Breaks on Later Task

If Task 1.3 breaks code from Task 1.1:

1. Run the full test suite to identify all failures
2. Determine if the break is in Task 1.3's changes or a latent issue
3. If Task 1.3 caused it: fix within Task 1.3 before committing
4. If latent issue: create a fix sub-task, execute it, then continue

### Unexpected Behavior Already Exists

If the Red phase shows tests already passing:

```
[!] Task 1.1 tests pass immediately -- email validator already exists.
--> Behavior already implemented. Checking if it matches spec...
```

1. Read the existing implementation
2. Compare against the spec
3. If matches: mark task complete, note in task notes
4. If differs: write tests for the MISSING behavior, then implement

---

## BR Mirror Protocol

Only applies when `br_enabled` (set in Step 4.5 of SKILL.md).

### On task start (6a.1)

Claim the corresponding BR issue:

```bash
br update {issue_id} --claim --json
```

Look up `{issue_id}` from `metadata.json` `beads_issue_map` using the task key (e.g., `P1T1`).

### On task complete (6a.9)

Close the corresponding BR issue:

```bash
br close {issue_id} --reason "sha:{sha7} | tests pass | {evidence}" --suggest-next --json
```

Look up `{issue_id}` from `metadata.json` `beads_issue_map`. The `--suggest-next` flag returns newly unblocked issues.
