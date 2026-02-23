# Verification Protocol

After each worker completes a task, verify before committing.

---

## Per-Task Verification

1. **Read modified files** — confirm they exist, match expectations
2. **Run tests** — use the project test command (e.g., `bun test`, `pytest`, `go test ./...`)
3. **Run lint/typecheck** — catch style and type errors
4. **Check acceptance criteria** — match against the plan's task requirements
5. **Freshness rule** — evidence older than 5 minutes is stale; re-run commands

## Fix Loop (max 3 iterations)

When verification fails:

**Attempt 1**: Send failure details back to the original worker. Wait for fix. Re-verify.

**Attempt 2**: Spawn a `build-fixer` agent targeted at the specific error. Re-verify.

**Attempt 3**: Spawn an `oracle` agent to diagnose root cause. Apply recommendation via `build-fixer` or original worker. Re-verify.

**Exit conditions** (stop the loop):
- Verification passes → mark task complete, commit, continue
- Same failure 3 times → mark task `failed`, log the error, move on
- No actionable fix identified → mark task blocked, ask user: Retry / Skip / Stop

**Rules:**
- Never loop more than 3 times on the same failure
- Never retry the exact same fix that already failed
- Never skip to the next task without logging the failure

## Auto-Commit

After a task passes verification:

1. **Stage only the task's files:**
   ```bash
   git add <file1> <file2> ...
   ```

2. **Commit with conventional format:**
   ```
   feat(<plan-scope>): <short task description>

   Plan: <plan-name>
   Task: <task subject>
   ```
   - Use appropriate prefix: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
   - `<plan-scope>` = slugified plan title (e.g., "Add User Auth" → `auth`)
   - Keep the first line under 72 characters

3. **Skip commit** if there are no file changes (e.g., research-only task).

4. **Annotate the plan** with the commit SHA:
   ```
   - [x] Task title <!-- commit: abc1234 -->
   ```
   Skip annotation for planless work (no plan file to update).

## Completion Gate

Before proceeding to quality gates (Step 5), ALL of these must be true:

- [ ] All plan checkboxes are `- [x]` (or marked failed/skipped)
- [ ] All verification commands from `## Verification` section pass
- [ ] No build or lint errors
- [ ] No test failures

If any fail, spawn a worker to fix, then re-run ALL verification from scratch.

## Handling Stuck Workers

If a worker appears stuck (no progress, no output):

1. Check if work is still in progress (look at file modifications, running processes)
2. If genuinely stuck, reassign the task to a new worker with full context
3. If the subagent tool doesn't support status checks, apply a reasonable timeout and retry
