# Verification Protocol — Work Phase

Follow the standard verification protocol in `.claude/lib/verification-checklist.md`. Run each check (BUILD, TEST, LINT, FUNCTIONALITY, TODO, ERROR_FREE) independently. Evidence older than 5 minutes is stale — re-run commands for fresh output.

Messages from teammates arrive automatically. After each teammate reports completion:

1. Read files claimed to be created/modified
2. Run tests claimed to pass: `Bash("bun test")` or project-specific command
3. Check for lint/type errors
4. Verify behavior matches the plan's acceptance criteria

If verification fails, message the teammate with feedback:

```
SendMessage(
  type: "message",
  recipient: "impl-1",
  content: "Tests fail: [error]. Fix and re-verify.",
  summary: "Test failure feedback"
)
```

### Verification Fix Loop

When verification fails for a task, run an inlined fix-and-verify cycle (max 3 iterations):

**Iteration 1**: Message the original worker with the failure details. Wait for their fix. Re-verify.
**Iteration 2**: If still failing, spawn a `build-fixer` targeted at the specific error. Re-verify.
**Iteration 3**: If still failing, spawn `oracle` to diagnose root cause. Apply oracle's recommendation via `build-fixer` or `kraken`. Re-verify.

**Exit conditions** (stop the loop):
- Verification passes → mark task complete, continue
- Same failure 3 iterations in a row → mark task as blocked, log the failure, continue with other tasks
- No actionable fix identified by oracle → mark task as blocked, notify user

**Do NOT**:
- Loop more than 3 times on the same failure
- Retry the exact same fix that already failed
- Skip to the next task without logging the failure

### Auto-Commit on Verified Task Completion

After a task passes verification (files confirmed, tests pass, lint clean), **immediately commit the changes**:

1. Stage only the files related to the completed task:
   ```bash
   git add <file1> <file2> ...
   ```

2. Commit with a descriptive message using the **plan title** (first `#` heading from the plan file) as the scope:
   ```bash
   git commit -m "$(cat <<'EOF'
   feat(<plan-title-as-scope>): <short description of what the task accomplished>

   Plan: <plan-name>
   Task: <task subject>

   Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
   EOF
   )"
   ```

   - Use conventional commit prefixes: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
   - `<plan-title-as-scope>` is derived from the plan's `#` heading, lowercased and shortened to a slug (e.g., plan titled "Code Styleguides — CLAUDE.md Injection" → scope `styleguides`, plan titled "Add User Auth Flow" → scope `auth`)
   - Keep the first line under 72 characters

3. If there are no changes to commit (e.g., task was research-only), skip the commit silently.

4. **Annotate the plan with the commit SHA** — After a successful commit, capture the short SHA and update the plan file so `/review` can trace each task to its commit:

   ```bash
   SHA=$(git rev-parse --short HEAD)
   ```

   Then use `Edit` to update the task's checkbox line in the plan file from:
   ```
   - [ ] Task N: Title
   ```
   to:
   ```
   - [x] Task N: Title <!-- commit: {SHA} -->
   ```

   This annotation is an HTML comment — invisible when rendered but machine-parseable by `/review`. If the task checkbox has already been marked `[x]` (e.g., on resume), still append the commit annotation if not already present.

   **Skip this step for planless work** (no plan file to annotate).

**This ensures each working increment is saved and the session never ends with 0 commits.**

### Handling Stalled Workers

If a worker stops reporting progress:

1. **Check task status**: `TaskGet(taskId: "N")` — look for tasks stuck in `in_progress`
2. **Check heartbeat**: Look for a `Heartbeat:` line in the task description. If the timestamp is older than 10 minutes, the worker is likely stalled.
3. **Send status check**:
   ```
   SendMessage(
     type: "message",
     recipient: "impl-1",
     content: "Status check — are you blocked on anything?",
     summary: "Worker status check"
   )
   ```
4. **Wait 2 minutes** for a response after sending the status check.
5. **Reassign if no response**: `TaskUpdate(taskId: "N", owner: "impl-2", status: "pending")`
6. **Resolve blockers**: If the task has a dependency issue, create a new task to resolve the blocker first

### Worker Heartbeat Protocol

Workers are expected to update their task description with a heartbeat timestamp every 5 minutes while working on long-running tasks. The orchestrator uses these heartbeats to detect stalled workers.

**Expected worker behavior**:
```
TaskUpdate(taskId: "N", description: "...existing description...\nHeartbeat: 2026-02-08T07:15:00Z")
```

**Stall detection rules**:
- Task `in_progress` for >10 minutes with no heartbeat update → considered stalled
- Task `in_progress` for >10 minutes with a recent heartbeat (<5 min old) → still working, do not interrupt
- Task `in_progress` for >10 minutes with a stale heartbeat (>10 min old) → likely stalled, send status check

### Completion Gate

Before declaring all tasks complete and proceeding to Step 7, the orchestrator MUST pass this gate:

1. **Zero pending tasks**: Run `TaskList()` and confirm no tasks are `pending` or `in_progress`
2. **Verification commands pass**: Run every verification command from the plan's `## Verification` section
3. **Fix failures**: If ANY verification fails, message the responsible worker to fix it or spawn a `build-fixer`
4. **Re-verify**: After fixes, re-run all verification commands from scratch

**Completion Checklist** (all must be true before proceeding):
- [ ] All tasks completed (`TaskList()` shows zero pending/in_progress)
- [ ] All verification commands pass (from plan's `## Verification` section)
- [ ] No build/lint errors
- [ ] No test failures

Only after all checks pass can the orchestrator proceed to Step 6.6 (security review, if applicable), Step 6.7 (critic review, if applicable), or Step 7 (Extract Wisdom).
