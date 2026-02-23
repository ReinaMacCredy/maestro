# Runtime Adapter: Generic Chat

**Tier**: Serial execution (Tier 3)
**Target CLIs**: Cursor, Windsurf, Aider, and any future CLI that exposes only basic filesystem and shell capabilities.

This adapter defines how the `/work` orchestration workflow degrades gracefully when agent spawning and a shared task board are unavailable. The orchestrator does all work directly, in dependency order, using the plan file as the source of state.

---

## Capability Mapping

| Capability | Available | Implementation |
|---|---|---|
| `agent.spawn` | no | Orchestrator does the work inline; no delegation |
| `agent.message` | no | Not needed — no workers to message |
| `agent.wait` | no | Not needed — execution is serial |
| `agent.close` | no | Not needed |
| `team.create` | no | Skipped |
| `team.delete` | no | Skipped |
| `task.create` | no | Use plan file checkboxes as task tracking |
| `task.list` | no | Read plan file checkboxes |
| `task.get` | no | Read plan file section |
| `task.update` | no | Edit plan file checkbox to mark done |
| `prompt.structured` | no | Fall through to `prompt.chat` |
| `prompt.chat` | yes | Ask via plain text; wait for reply |
| `fs.read` | yes | Read file contents |
| `fs.write` | yes | Write file contents |
| `fs.search` | yes | Glob pattern search |
| `fs.grep` | yes | Content search |
| `exec.command` | yes | Shell command execution |

---

## Serial Execution Model

Without worker spawning, the orchestrator processes tasks one at a time in dependency order.

### Execution Loop

```
1. Read plan file — parse task list from ## Tasks section
2. Build dependency graph from task ordering and explicit dependencies noted in the plan
3. For each task in topological order:
   a. Read full task description from plan
   b. Check: is this task complete? (checkbox ticked) — if yes, skip
   c. Confirm with user: "About to work on: <task>. Proceed?"
   d. Do the work directly (read files, edit files, run commands)
   e. Verify: run the task's verification step
   f. Mark checkbox in plan file as complete
   g. Commit if work passes verification
4. After all tasks complete, ask user to confirm plan is done
5. Move plan file to archive
```

### User Confirmation Pattern

Because there is no structured prompt tool, all confirmations use plain text:

```
[next task] <task subject>
<task description>

Proceed? (yes / skip / stop)
```

The orchestrator waits for user reply before continuing. Acceptable replies:
- `yes` / `y` / blank — proceed
- `skip` / `s` — mark skipped, move to next task
- `stop` / `n` / `no` — halt execution and report progress

---

## Plan-File-as-State Pattern

With no shared task board, the plan file's `## Tasks` section is the source of truth for progress.

### Checkbox conventions

```markdown
## Tasks

- [ ] Task A — description
- [x] Task B — description (completed)
- [-] Task C — description (skipped)
```

The orchestrator reads and writes these checkboxes directly as it works through the plan.

### State after interruption

If execution is interrupted (session ends, error, user stop), progress is preserved in the plan file checkboxes. On resume, the orchestrator reads the plan, finds the first unchecked task, and continues from there.

---

## Graceful Degradation Details

### No agent spawning → inline execution

The orchestrator takes on every worker role itself. For a plan that would normally spawn `kraken` for TDD and `spark` for config changes, the orchestrator applies those strategies sequentially in the same session.

Quality trade-offs in serial mode:
- No parallel execution — slower for large plans
- No role specialization — orchestrator applies best judgment across all task types
- No independent verification step — orchestrator self-reviews before marking complete

### No task board → plan file checkboxes

Progress tracking moves to the plan file. The orchestrator:
- Reads the task list from `## Tasks` on each iteration
- Marks `[x]` after completing a task
- Notes blockers as `[!]` with a short reason inline

### No structured prompts → chat confirmations

Wherever the workflow calls for a structured choice (e.g., "which verification steps to run?"), the orchestrator presents the options as a numbered plain-text list and waits for a number or text reply.

Example:
```
Verification options:
1. Run tests only
2. Run tests + lint
3. Skip verification

Enter choice (1/2/3):
```

---

## Verification in Serial Mode

Without a separate critic or build-fixer agent, the orchestrator handles verification inline:

1. After completing each task, run the verification command from the plan's `## Verification` section
2. If verification fails, attempt one fix cycle
3. If the fix cycle fails, mark the task as `[!]` (blocked) and report to the user before continuing

Maximum one auto-fix attempt per task. If it fails, escalate to the user rather than looping.

---

## Commit Strategy

Serial mode follows the same commit-after-each-verified-task rule as full orchestration:

```
complete task → verify passes → commit with task subject as message → continue
```

If verification fails and the user chooses to skip, do not commit the partial work.

---

## Tier Capabilities Available

| Capability | Available |
|---|---|
| `agent.spawn` | no |
| `agent.message` | no |
| `agent.wait` | no |
| `agent.close` | no |
| `team.create` | no |
| `team.delete` | no |
| `task.create` | no (use plan checkboxes) |
| `task.list` | no (read plan checkboxes) |
| `task.get` | no (read plan section) |
| `task.update` | no (edit plan checkbox) |
| `prompt.structured` | no (use prompt.chat) |
| `prompt.chat` | yes |
| `fs.read` | yes |
| `fs.write` | yes |
| `fs.search` | yes |
| `fs.grep` | yes |
| `exec.command` | yes |
