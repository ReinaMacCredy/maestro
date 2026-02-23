# Work Execution Workflow

Canonical 9-step specification for the `/work` orchestration workflow. This document uses only abstract capability names defined in `capabilities.md`. It is the single source of truth for execution flow, referenced by `SKILL.md` and runtime adapters.

No CLI-specific tool names appear in this document. Concrete mappings live in `reference/runtimes/`.

---

## Arguments

| Argument | Meaning |
|----------|---------|
| `<plan-name>` | Load a specific plan by name or title substring |
| `--resume` | Skip already-completed tasks (`- [x]`) |
| `--eco` | Cost-efficient model routing (haiku for simple tasks) |
| _(none)_ | Auto-load if one plan exists; prompt if multiple |

---

## Step 1: load_plan

**Purpose**: Locate and load the plan file that defines the work to be executed.

### 1a. Check for in-progress designs

```
fs.search(".maestro/handoff/*.json")
```

For each handoff file found, `fs.read` it. If any has `"status": "designing"`, emit a warning to the user via `prompt.chat` and block until they confirm whether to proceed or stop. This is a destructive gate — do not auto-proceed.

### 1b. Collect available plans

```
fs.search(".maestro/plans/*.md")      -- maestro plans
fs.search("~/.claude/plans/*.md")     -- native plans
```

Merge into a single list, tracking `source: "maestro" | "native"` per entry. If a filename exists in both directories, the `maestro` version takes precedence (exclude the native duplicate).

Filter native plans: exclude any native plan where all checkboxes are already `- [x]`.

### 1c. Resolve plan from argument

**If `<plan-name>` argument provided:**

1. Attempt exact filename match in `.maestro/plans/`
2. Attempt exact filename match in `~/.claude/plans/`
3. Attempt title-based substring match (case-insensitive) on native plan `#` headings — native filenames are random, so title matching is the primary user-facing reference
4. If found, load it and skip the selection prompt
5. If not found, apply planless heuristic: if the argument contains spaces, length > 40, or includes common action verbs (`add`, `fix`, `create`, `update`, `implement`, `refactor`, `remove`, `change`, `move`, `build`), treat it as a planless work description

   --> See `reference/planless-flow.md` for the planless execution path

   Otherwise stop with error: `Plan "{plan-name}" not found. Available plans: {list}`

**If no argument provided:**

- 0 plans: stop with error — `No plans found. Run /design to create a plan, or /work <description> to work directly.`
- 1 plan: load automatically
- Multiple plans: present via `DECIDE` (see below)

**Multi-plan selection:**

Cross-reference `.maestro/handoff/*.json` files. Find handoffs with `status: "complete"` sorted by `completed` timestamp descending. If the latest matches an existing plan file, mark it as recommended.

```
DECIDE(
  question: "Which plan would you like to execute?",
  options: [recommended plan first, then others, native plans labeled "(native)"],
  blocking: true,
  default: <recommended plan or first in list>
)
```

Each option shows: plan title (from first `#` heading), objective excerpt (first 80 chars of `## Objective` / `## Summary` / `## Context`), unchecked task count.

**Capabilities used**: `fs.search`, `fs.read`, `prompt.chat`, `DECIDE`

**Success criteria**: One plan file loaded, source (`maestro` or `native`) recorded, `plan-slug` derived from filename without `.md`.

---

## Step 2: confirm

**Purpose**: Validate the plan structure and get explicit user approval before committing to execution.

### 2a. Validate plan structure

**For maestro plans** (`source: "maestro"`):

| Section | Required | On Missing |
|---------|----------|------------|
| `## Objective` | Yes | Stop with error |
| `## Tasks` with at least one `- [ ]` | Yes | Stop with error |
| `## Verification` | Yes | Stop with error |
| `## Scope` | No | Warn and proceed |

**For native plans** (`source: "native"`):

| Section | Required | On Missing |
|---------|----------|------------|
| At least one `- [ ]` checkbox | Yes | Stop with error |
| Any descriptive section | No | Warn and proceed |
| `## Verification` | No | Warn and proceed |

If any required section is missing: stop — `Plan is missing required sections: {list}. Fix the plan or run /plan-template to scaffold one.`

### 2b. Check code style guides (non-blocking)

```
fs.grep("maestro:code-styleguides:start", "CLAUDE.md")
```

If not found: emit informational notice — `Tip: Run /styleguide to inject language-specific code style guides into your project's CLAUDE.md.` Do NOT block execution.

### 2c. Confirm execution

Present plan summary: title, source, objective, task count, scope (if present).

```
DECIDE(
  question: "Execute this plan?",
  options: [
    {label: "Yes, execute", description: "Proceed with team creation and task execution"},
    {label: "Cancel", description: "Stop without executing"}
  ],
  blocking: true,
  default: "Cancel"
)
```

If user cancels, stop execution.

**Capabilities used**: `fs.read`, `fs.grep`, `DECIDE`, `prompt.chat`

**Success criteria**: Plan passes validation, user has confirmed execution.

---

## Step 3: init_coordination

**Purpose**: Establish the coordination context for this execution — team, handoff file, and optional worktree isolation.

### 3a. Create team

```
team.create("work-{plan-slug}", "Executing {plan title}")
```

This is optional per capability tier but MUST be done first on Tier 1 runtimes. Workers join this team to share the task board.

**Fallback (Tier 2/3)**: Skip `team.create`. Workers coordinate through the shared task board without a named team context.

### 3b. Write handoff file

```
fs.write(".maestro/handoff/{plan-slug}.json", {
  "topic": "{plan-slug}",
  "status": "executing",
  "started": "{ISO timestamp}",
  "plan_destination": "{plan path}",
  "source": "{maestro|native}"
})
```

Overwrite any existing handoff file for this slug (e.g., a prior `status: "complete"` from `/design`).

### 3c. Worktree isolation (optional)

If the user opts into isolated execution on a separate branch:

```
DECIDE(
  question: "Where should this plan execute?",
  options: [
    {label: "Worktree (isolated)", description: "New branch, safe for parallel"},
    {label: "Main tree", description: "Current directory"}
  ],
  blocking: true,
  default: "Main tree"
)
```

--> See `reference/worktree-isolation.md` for the full setup and cleanup protocol.

If worktree is used, record `"worktree": true` in the handoff file.

**Capabilities used**: `team.create`, `fs.write`, `DECIDE`

**Success criteria**: Team exists (or skipped per tier), handoff file written with `status: "executing"`.

---

## Step 4: create_tasks

**Purpose**: Convert every plan checkbox into a tracked task on the shared task board, with dependencies and priority context.

### 4a. Load priority context

```
fs.read(".maestro/notepad.md")
```

Extract items under `## Working Memory` tagged with priority levels (P0-P2). If found, these become hard constraints appended to every task description as `**Priority Context**: {items}`.

### 4b. Discover available skills

--> See `reference/skill-injection.md` for the full discovery and injection protocol.

Matching skills are appended to worker prompts as `## SKILL GUIDANCE` sections.

### 4c. Create tasks from plan checkboxes

For each `- [ ]` line in the plan:

```
task.create(
  subject: "{task title from checkbox}",
  description: "{full description, acceptance criteria, file paths, constraints, priority context}",
  activeForm: "{present-continuous form of title}"
)
```

For each task, extract file paths mentioned in the plan's `Files:` section (if any). Include an `**Owned files**: {file1, file2}` line in the description. This enables file-ownership enforcement during dispatch.

Set dependencies between tasks:

```
task.update(id, { addBlockedBy: ["{dependency-task-id}"] })
```

### 4d. Resume mode

**`--resume` flag**: Create tasks only for unchecked items (`- [ ]`). Skip `- [x]` items entirely.

Emit: `Resuming: {N} complete, {M} remaining`

**Default (no `--resume`)**: Create tasks for all checkboxes.

Emit: `Starting fresh: {N} tasks`

**Capabilities used**: `fs.read`, `task.create`, `task.update`

**Success criteria**: All target checkboxes have corresponding tasks on the board with correct dependencies.

---

## Step 5: dispatch_workers

**Purpose**: Spawn parallel workers and assign initial tasks, then let workers self-coordinate from the task board.

### 5a. Select worker types

For each task, choose a worker type based on task characteristics:

| Worker | When to use |
|--------|-------------|
| `kraken` | TDD, new features, multi-file changes, correctness-critical |
| `spark` | Quick fixes, single-file changes, config updates |
| `build-fixer` | Build/compile errors, lint failures, type check errors |
| `oracle` | Architecture decisions, strategic design |
| `explore` | Codebase research, pattern finding (read-only) |

**Eco mode (`--eco`)**: Use cost-efficient model routing. `spark` tasks use haiku tier; `kraken` tasks use sonnet tier; `oracle` and `leviathan` are not spawned; `explore` uses haiku.

Emit at start: `Ecomode: using cost-efficient model routing (haiku for simple, sonnet for complex)`

### 5b. Spawn all workers in parallel

Spawn 2-4 workers simultaneously — not sequentially. Each receives the delegation prompt format:

```
## TASK
[Specific, atomic goal]

## EXPECTED OUTCOME
- [ ] File created/modified: [path]
- [ ] Tests pass: `[command]`
- [ ] No new errors

## CONTEXT
[Background, constraints, related files]

## SKILL GUIDANCE
[Only if matching skills found — see skill-injection.md]

## MUST DO
- [Explicit requirements]

## MUST NOT DO
- [Explicit exclusions]

## PERSISTENT CONTEXT
- If you discover a non-obvious constraint during implementation, append to .maestro/notepad.md under ## Working Memory.
- Priority Context items are hard constraints.

## KNOWLEDGE CAPTURE
- Emit <remember category="learning">...</remember> for non-trivial patterns discovered.
```

```
agent.spawn(role: "{worker-type}", prompt: "{delegation prompt}", model?: "{tier}")
```

### 5c. Initial task assignment

After spawning, explicitly assign the first task to each worker:

```
task.update("{task-id}", { owner: "{worker-name}", status: "in_progress" })
```

After the first round, workers self-claim from the task board using the self-claim loop (see `task-model.md` § Ownership Model).

**File ownership**: Do not assign tasks with overlapping file ownership to different workers simultaneously. If overlap is unavoidable, enforce ordering via `addBlockedBy`.

**Capabilities used**: `agent.spawn`, `task.update`, `task.list`

**Success criteria**: All workers spawned, each has one initial task assigned, remaining tasks are available for self-claim.

---

## Step 6: monitor_verify

**Purpose**: Verify each completed task, auto-commit working increments, handle stalls, gate on overall completion, and run security/critic reviews.

--> See `reference/verification-protocol.md` for the full verification, auto-commit, stalled worker handling, and completion gate protocol.

### 6a. Per-task verification

For each task transitioning to `completed`:

1. `fs.read` the modified files
2. `exec.command` to run tests/build
3. If verification passes: auto-commit the increment
4. If verification fails: `agent.message` the responsible worker with specific issues; revert task to `in_progress`

### 6b. Stall detection

Monitor heartbeats via `task.get(id)` on tasks that have been `in_progress` for >10 minutes. If no heartbeat update is present, treat the worker as stalled:

1. `task.update(id, { owner: "{new-worker}", status: "in_progress" })` — reassign to another worker
2. `agent.message` the new worker with full context

### 6c. Completion gate

Poll `task.list()` until all tasks reach `completed` or `failed`. Tasks in `failed` state require orchestrator intervention:

```
DECIDE(
  question: "Task {id} failed: {reason}. How to proceed?",
  options: [
    {label: "Retry", description: "Reassign to a worker"},
    {label: "Skip", description: "Mark as skipped and continue"},
    {label: "Stop", description: "Halt execution"}
  ],
  blocking: true,
  default: "Retry"
)
```

### 6d. Security review (auto)

--> See `reference/security-prompt.md` for the security review trigger and prompt.

Spawn a `security-reviewer` agent when the plan touches security-sensitive paths (auth, permissions, crypto, network).

### 6e. Critic review (optional)

Spawn a `critic` agent for final review when the plan has >5 tasks or touches >5 files:

```
agent.spawn(role: "critic", prompt: "Review all files modified in this execution. Run tests, check for issues, report APPROVE/REVISE verdict.")
```

If the critic returns **REVISE**: `agent.message` the responsible worker(s) with specific issues; wait for fixes before continuing.

If the critic returns **APPROVE** (or step is skipped): proceed to Step 7.

**Capabilities used**: `fs.read`, `exec.command`, `task.list`, `task.get`, `task.update`, `agent.message`, `agent.spawn`, `DECIDE`

**Success criteria**: All tasks `completed`, tests pass, security review passed (or skipped for non-sensitive plans), critic approved (or skipped for small plans).

---

## Step 7: extract_wisdom

**Purpose**: Record learnings and reusable patterns from this execution for future sessions.

--> See `reference/wisdom-extraction.md` for the full wisdom extraction and learned skills protocol.

Collect `<remember>` tags emitted by workers during execution. Write distilled learnings to `.maestro/wisdom/`:

```
fs.write(".maestro/wisdom/{plan-slug}.md", "{synthesized learnings}")
```

**Capabilities used**: `fs.read`, `fs.write`

**Success criteria**: Wisdom file written (or no learnings found — that is also a valid outcome).

---

## Step 8: cleanup

**Purpose**: Capture an execution summary, shut down workers, dissolve the team, and archive the completed plan.

### 8a. Auto-capture execution summary

Append to `.maestro/notepad.md` under `## Working Memory`:

```
- [{ISO date}] [work:{plan-slug}] Completed: {N}/{total} tasks. Files: {count modified}. Learned: {count skills extracted}. Security: {pass|fail|skipped}.
```

```
fs.write(".maestro/notepad.md", "{existing content + appended line}")
```

Create the file if it does not exist.

### 8b. Shutdown workers

Send a shutdown signal to each spawned worker:

```
agent.message("{worker-name}", "Work complete. Shut down when ready.")
agent.close("{worker-name}")
```

### 8c. Delete team

```
team.delete()
```

**Fallback (Tier 2/3)**: No-op — skip if `team.create` was skipped.

### 8d. Archive plan

**For maestro plans** (`source: "maestro"`):

```
exec.command("mkdir -p .maestro/archive/ && mv .maestro/plans/{slug}.md .maestro/archive/{slug}.md")
```

Emit: `Archived plan to .maestro/archive/{slug}.md`

**For native plans** (`source: "native"`):

Do NOT move or delete the native plan (managed by the host CLI). Instead, mark all checkboxes complete:

```
fs.read("~/.claude/plans/{slug}.md")
-- replace all `- [ ]` with `- [x]`
fs.write("~/.claude/plans/{slug}.md", "{updated content}")
```

Emit: `Marked native plan ~/.claude/plans/{slug}.md as complete`

### 8e. Update handoff file

```
fs.write(".maestro/handoff/{slug}.json", {
  "topic": "{slug}",
  "status": "archived",    -- "completed" for native plans
  "started": "{original timestamp}",
  "completed": "{ISO timestamp}",
  "plan_destination": "{final path}",
  "source": "{maestro|native}"
})
```

### 8f. Worktree cleanup

--> See `reference/worktree-isolation.md` for the worktree cleanup protocol.

**Capabilities used**: `fs.read`, `fs.write`, `exec.command`, `agent.message`, `agent.close`, `team.delete`

**Success criteria**: All workers shut down, team dissolved, plan archived or marked complete, handoff file updated to terminal status.

---

## Step 9: report

**Purpose**: Deliver a concise execution summary to the user.

Include:

- Tasks completed (N/total)
- Files created/modified
- Tests passing (command + result)
- Plan archived to `.maestro/archive/{slug}.md` (or native plan marked complete)
- Any issues or follow-ups

**If executed in a worktree** (handoff `"worktree": true`):

- Branch name: `maestro/{plan-slug}`
- Worktree path (if kept) or confirmation it was removed
- Merge instructions: `exec.command("git merge maestro/{plan-slug}")`

Suggest post-execution review:

```
To verify results against the plan's acceptance criteria, run:
  /review  (Codex: $review)
```

**Capabilities used**: `prompt.chat`

**Success criteria**: User has a complete, actionable summary of what was executed and what (if anything) needs follow-up.

---

## Cross-References

| Topic | Document |
|-------|----------|
| Abstract capabilities | `reference/core/capabilities.md` |
| Task state model and heartbeat | `reference/core/task-model.md` |
| DECIDE primitive | `reference/core/decisions.md` |
| Runtime capability mappings | `reference/runtimes/registry.md` |
| Verification loop (Step 6) | `reference/verification-protocol.md` |
| Security review trigger (Step 6d) | `reference/security-prompt.md` |
| Wisdom extraction (Step 7) | `reference/wisdom-extraction.md` |
| Worktree isolation (Steps 3c, 8f) | `reference/worktree-isolation.md` |
| Skill injection (Step 4b) | `reference/skill-injection.md` |
| Planless flow | `reference/planless-flow.md` |

---

## Capability Tier Adaptation

The workflow degrades gracefully across capability tiers. See `reference/core/capabilities.md` § Capability Tiers for tier definitions.

| Step | Tier 1 (Full) | Tier 2 (Partial) | Tier 3 (Serial) |
|------|--------------|-----------------|----------------|
| init_coordination | `team.create` + handoff | handoff only | handoff only |
| dispatch_workers | parallel `agent.spawn` | parallel `agent.spawn` | orchestrator executes inline |
| monitor_verify | `agent.message` for feedback | task description updates | inline, no delegation |
| cleanup | `agent.close` + `team.delete` | no-op close | no-op |

---

## Invariants

These rules hold across all tiers and modes:

1. **Orchestrator never edits files directly** — all file changes are delegated to workers (Tiers 1-2) or executed as the orchestrator's own inline task (Tier 3 only).
2. **Workers cannot edit plan files** — `.maestro/plans/` is read-only for all workers.
3. **One task owner at a time** — concurrent ownership is invalid (see `task-model.md` § Ownership Model).
4. **Destructive decisions block** — any DECIDE with irreversible consequences must wait for explicit user input; auto-default is not permitted.
5. **Commit after each verified task** — zero-commit sessions are a failure mode.
6. **Resume skips completed tasks** — `--resume` never recreates tasks for `- [x]` checkboxes.
