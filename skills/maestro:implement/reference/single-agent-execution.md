# Single-Agent Execution Protocol

## Task Execution Loop

For each task in the queue, follow the workflow methodology from `workflow.md`.

---

## TDD Methodology

### 6a.1: Mark Task In Progress

Edit `plan.md`: Change task checkbox from `[ ]` to `[~]`.

**Deferred context**: If deferred context (workflow.md, tech-stack.md) has not been loaded yet, load it now before executing the first task.

**BR mirror**: If `metadata.json` has `beads_epic_id`, also claim the corresponding BR issue:

```bash
br update {issue_id} --claim --json
```

Look up `{issue_id}` from `metadata.json` `beads_issue_map` using the task key (e.g., `P1T1`).

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

**BR mirror**: If `metadata.json` has `beads_epic_id`, also close the corresponding BR issue:

```bash
br close {issue_id} --reason "sha:{sha7} | tests pass | {evidence}" --suggest-next --json
```

Look up `{issue_id}` from `metadata.json` `beads_issue_map`. The `--suggest-next` flag returns newly unblocked issues.

---

## Ship-fast Methodology

Same flow but reordered:
1. Mark in progress
2. Implement the feature/fix
3. Write tests covering the implementation
4. Run tests, verify passing
5. Commit, attach summary, record SHA
