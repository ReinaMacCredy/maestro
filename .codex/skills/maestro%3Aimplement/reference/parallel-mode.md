# Parallel Mode Protocol

## Overview

Parallel mode uses Task sub-agents to execute independent plan tasks concurrently. You (the main session) analyze the plan for parallelism opportunities, spawn sub-agents in isolated worktrees, then verify and merge their results before committing. Sub-agents can read, write, and edit files but cannot commit, touch BR state, or modify plan.md.

## When to Use

- 3-8 tasks in a phase with 2+ independent tasks
- Tasks modify different files (low file scope overlap)
- Tasks do not depend on each other's output within the same wave
- Runtime supports the Task tool with worktree isolation

**Do not use** when all tasks are sequential, tasks heavily overlap in file scope, or the track has fewer than 3 tasks (overhead exceeds benefit).

## Prerequisites

Your runtime must support the Task tool for spawning sub-agents.
- Claude Code: Task tool with `isolation: "worktree"` parameter
- Other runtimes: use your native sub-agent/delegation mechanism with filesystem isolation

### Agent Mail (Optional)

If the `mcp-agent-mail` MCP server is available, parallel mode uses it for **advisory file reservations** to detect scope conflicts before spawning sub-agents. This replaces heuristic file-scope inference with explicit lock checks.

Setup (once per session, before first wave):
1. `ensure_project` with `human_key` set to the project's absolute path
2. `register_agent` for the main session (auto-generated name is fine)

If agent-mail is not available, parallel mode falls back to heuristic file-scope analysis (the default behavior described below).

---

## Step 6b: Analyze Plan for Parallelism

### 6b.1: Build Task Dependency Graph

Parse `plan.md` to identify all tasks in the current phase. For each task, determine:

1. **Explicit dependencies**: Tasks blocked by earlier tasks (sequential order in plan.md)
2. **File scope**: Infer which files each task will modify based on task title and sub-task descriptions
3. **Independence**: Two tasks are independent if:
   - Neither depends on the other's output
   - Their likely file scopes do not overlap
   - They are in the same phase (cross-phase tasks are always sequential)

4. **File reservation check** (if agent-mail available): After heuristic analysis, call `file_reservation_paths` with `exclusive: true` for each task's inferred file scope. If `conflicts` are returned (another task already reserved overlapping paths), move the conflicting task to a later wave. This replaces guesswork with concrete overlap detection.

### 6b.2: Classify Tasks into Waves

For each phase, partition tasks into waves:

**Wave 1**: All tasks with no dependencies within the phase
**Wave 2**: Tasks that depend only on Wave 1 tasks
**Wave N**: Tasks that depend only on tasks in earlier waves

Tasks within the same wave are independent and can run in parallel.

Report the parallelism plan to the user before executing:

```
[ok] Phase {N} parallelism analysis:
  Wave 1 (parallel): Task {A}, Task {B}
  Wave 2 (sequential): Task {C} (depends on {A})
  --> {M} tasks parallel, {K} sequential
```

### 6b.3: Fallback to Sequential

If ALL tasks in a phase are dependent (no parallelism possible), print:

```
[!] No independent tasks found in Phase {N}. Falling back to single-agent mode.
```

Execute the phase using the single-agent protocol from `reference/single-agent-execution.md`.

---

## Step 6c: Wave Execution Loop

For each wave in the current phase:

### 6c.1: Reserve Files and Spawn Sub-agents

**File reservations** (if agent-mail available): Before spawning, call `file_reservation_paths` for each task's inferred file scope with `exclusive: true` and a TTL of 7200 seconds. If any reservation returns `conflicts`, re-sequence the conflicting task into a later wave.

```
file_reservation_paths parameters:
  project_key: {project_absolute_path}
  agent_name: {registered_agent_name}
  paths: ["src/api/*.py", "tests/test_api.py"]  (task's inferred file scope)
  ttl_seconds: 7200
  exclusive: true
  reason: "Task {N.M}: {task_title}"
```

For each task in the wave (after reservations clear), spawn a Task sub-agent with worktree isolation:

```
Task tool parameters:
  subagent_type: "general-purpose"
  isolation: "worktree"
  mode: "bypassPermissions"
  prompt: <sub-agent prompt template below>
  description: "Task {N.M}: {task_title}"
  run_in_background: true  (for all but the last task in the wave)
```

Spawn ALL sub-agents in a single message (parallel tool calls) for maximum concurrency.

**Rate limit handling**: If Task tool returns a rate limit error, queue the remaining tasks and execute them sequentially after the current parallel batch completes.

### 6c.2: Sub-agent Prompt Template

Each sub-agent receives:

```
You are executing a single task from a maestro implementation plan.

## Your Task
Phase {N}, Task {M}: {task_title}

### Sub-tasks
{list of sub-tasks from plan.md}

## Context
- Track spec: Read `.maestro/tracks/{track_id}/spec.md`
- Workflow methodology: Read `.maestro/context/workflow.md`
- Tech stack: Read `.maestro/context/tech-stack.md`
- Guidelines: Read `.maestro/context/guidelines.md` (if exists)

## Instructions
Follow the {TDD/ship-fast} methodology:
{methodology steps from workflow.md}

## Constraints
- You MUST NOT run git commit, git add, or any git write operations
- You MUST NOT modify .maestro/ files (plan.md, metadata.json, notepad.md)
- You MUST NOT run br/bv commands
- You MUST NOT call agent-mail MCP tools (file reservations are managed by the main session)
- You CAN read any file, write new files, edit existing files, and run tests
- You CAN install dependencies if needed (but prefer existing stack)
- After completing all sub-tasks, report what you changed and test results

## Deliverable
When done, provide:
1. List of files created/modified
2. Test results (command run + output)
3. Any issues encountered
```

### 6c.3: Wait for Completion

Wait for all sub-agents in the wave to complete. The system will notify when each finishes.

**Timeout handling**: If a sub-agent does not complete within a reasonable time (no fixed limit -- watch for the system notification), check its status. If stuck, note it for sequential retry.

### 6c.4: Collect Results

For each completed sub-agent:

1. Read the sub-agent's result message
2. Note the worktree path and branch returned
3. Record which files were changed and test outcomes

---

## Step 6d: Verify and Merge

### 6d.1: Verify Each Sub-agent's Work

For each completed sub-agent, in the main worktree:

1. **Cherry-pick or merge the worktree branch**:
   ```bash
   git merge --no-commit {worktree_branch}
   ```
   Or selectively apply changes if conflicts exist.

2. **Re-read changed files** to confirm correctness
3. **Re-run the test suite**:
   ```bash
   CI=true {test_command}
   ```
4. If tests pass: proceed to commit
5. If tests fail: reject this sub-agent's work and queue the task for sequential retry

### 6d.2: Conflict Detection

If multiple sub-agents in a wave modified the same file:

1. **Non-overlapping hunks**: Auto-merge via git's merge strategy
2. **Overlapping hunks**: Stop and present the conflict to the user:
   ```
   [!] File conflict in {file_path}:
     Task {A} modified lines {X-Y}
     Task {B} modified lines {X-Z}
   --> Resolve manually or retry tasks sequentially?
   ```
3. If user chooses sequential retry: queue both tasks for sequential execution, discard worktree results

### 6d.3: Commit Wave Results

After all sub-agents in the wave are verified:

```bash
git add {all_changed_files}
git commit -m "{type}({scope}): {description} [parallel: tasks {list}]"
```

One commit per wave. Include `[parallel: tasks N.M, N.K]` suffix to indicate which tasks were parallelized.

### 6d.4: Update Plan State

For each successfully completed task in the wave:

1. Edit `plan.md`: Change task marker from `[ ]` to `[x] {sha}` (first 7 characters of the wave commit hash)
2. **BR mirror**: If `metadata.json` has `beads_epic_id`, close the corresponding BR issues:
   ```bash
   br close {issue_id} --reason "sha:{sha7} | parallel wave" --suggest-next --json
   ```

```bash
git add .maestro/tracks/{track_id}/plan.md
git commit -m "maestro(plan): mark wave tasks complete [parallel]"
```

### 6d.5: Clean Up Worktrees and Reservations

After successful merge and commit, worktrees from completed sub-agents are automatically cleaned up by the Task tool. Verify cleanup:

```bash
git worktree list
```

Remove any stale worktrees:
```bash
git worktree prune
```

**Release file reservations** (if agent-mail was used): Call `release_file_reservations` for the main agent to free all reserved paths for the completed wave. This allows the next wave's tasks to reserve the same files if needed.

```
release_file_reservations parameters:
  project_key: {project_absolute_path}
  agent_name: {registered_agent_name}
```

---

## Step 6e: Handle Failures

### Sub-agent Failure

If a sub-agent fails (returns error or produces broken code):

1. Log the failure:
   ```
   [!] Sub-agent for Task {N.M} failed: {error summary}
   --> Retrying sequentially in main session
   ```
2. Queue the task for sequential execution using the single-agent protocol
3. Continue processing successful sub-agents from the same wave

### Mixed Results

If some sub-agents in a wave succeed and others fail:

1. Verify and merge successful results first
2. Commit successful work
3. Retry failed tasks sequentially
4. This ensures progress is not lost due to individual failures

### Merge Failure After Verification

If tests pass in the worktree but fail after merging into main:

1. The conflict is between the merged code and main branch state
2. Identify which task's changes cause the failure
3. Revert the merge: `git merge --abort` or `git reset --hard HEAD`
4. Re-execute the failing task sequentially with full main branch context
5. The sub-agent lacked visibility into other wave results -- sequential execution fixes this

### Orphaned Worktrees

On `--resume` with `--parallel`, check for stale worktrees from a previous interrupted run:

```bash
git worktree list --porcelain | grep -A1 "worktree.*\.claude/worktrees"
```

If found, prune them:
```bash
git worktree prune
```

---

## Worked Example: Parallel Execution

**Track**: "Add REST API endpoints for user management"
**Phase 2** has 5 tasks:
- Task 2.1: Create GET /users endpoint
- Task 2.2: Create GET /users/:id endpoint
- Task 2.3: Create POST /users endpoint
- Task 2.4: Add input validation middleware
- Task 2.5: Add pagination to GET /users

**Dependency analysis:**

```
Task 2.1: Modifies src/routes/users.ts, tests/users.test.ts     | no deps
Task 2.2: Modifies src/routes/users.ts, tests/users.test.ts     | no deps
Task 2.3: Modifies src/routes/users.ts, tests/users.test.ts     | no deps
Task 2.4: Modifies src/middleware/validation.ts                  | no deps
Task 2.5: Modifies src/routes/users.ts (depends on 2.1 output)  | depends on 2.1

[!] Tasks 2.1, 2.2, 2.3 all modify src/routes/users.ts
--> File scope overlap detected. Cannot parallelize all three.
```

**Wave assignment:**

```
[ok] Phase 2 parallelism analysis:
  Wave 1 (parallel): Task 2.1, Task 2.4
    - 2.1: src/routes/users.ts (GET /users only)
    - 2.4: src/middleware/validation.ts (no overlap)
  Wave 2 (parallel): Task 2.2, Task 2.3
    - Both touch users.ts but in different handler functions
    - Acceptable risk -- hunks unlikely to overlap
  Wave 3 (sequential): Task 2.5
    - Depends on 2.1 (needs the GET /users handler to add pagination)
  --> 4 tasks parallel across 2 waves, 1 sequential
```

**Execution:**

```
--- Wave 1 ---
Spawning sub-agent: Task 2.1 (GET /users) [worktree: .claude/worktrees/task-2.1]
Spawning sub-agent: Task 2.4 (validation) [worktree: .claude/worktrees/task-2.4]

[ok] Task 2.4 complete: src/middleware/validation.ts created, 6 tests pass
[ok] Task 2.1 complete: GET /users handler added, 4 tests pass

Merging wave 1...
  git merge --no-commit worktree/task-2.1  [ok]
  git merge --no-commit worktree/task-2.4  [ok]
  CI=true bun test  [ok] 10 new tests pass, 0 regressions
  git commit -m "feat(api): add GET /users and validation middleware [parallel: 2.1, 2.4]"
[x] Wave 1 complete (sha: m1n2o3p)

--- Wave 2 ---
Spawning sub-agent: Task 2.2 (GET /users/:id)
Spawning sub-agent: Task 2.3 (POST /users)

[ok] Task 2.2 complete: 3 tests pass
[!] Task 2.3 failed: "Cannot find module '../models/user'"
--> Sub-agent needed a model that doesn't exist yet.
--> Queuing Task 2.3 for sequential retry.

Merging wave 2 (partial)...
  git merge --no-commit worktree/task-2.2  [ok]
  CI=true bun test  [ok]
  git commit -m "feat(api): add GET /users/:id [parallel: 2.2]"
[x] Task 2.2 complete (sha: q4r5s6t)

--- Task 2.3 (sequential retry) ---
Executing in main session with full context...
Created src/models/user.ts (was missing from task scope)
Created POST /users handler
[ok] 5 tests pass
[x] Task 2.3 complete (sha: u7v8w9x)

--- Wave 3 ---
Task 2.5: Adding pagination to GET /users (sequential, depends on 2.1)
[ok] 3 tests pass
[x] Task 2.5 complete (sha: y0z1a2b)

--- Phase 2 Complete ---
```

---

## Phase Completion

After all waves in a phase complete (all tasks verified and committed), run the standard Phase Completion Protocol from `reference/phase-completion.md`.

The phase completion check runs against the main worktree, which now contains all merged results from parallel sub-agents.

---

## Anti-patterns

| Don't | Do Instead |
|-------|-----------|
| Let sub-agents commit | Main session commits after verification |
| Skip verification of sub-agent output | Always re-read files and re-run tests |
| Spawn sub-agents for dependent tasks | Only parallelize independent tasks |
| Ignore file scope overlap | Check for potential conflicts before spawning |
| Force all tasks parallel | Fall back to sequential when parallelism adds no value |
| Spawn too many sub-agents (>4) | Limit wave size to 3-4 concurrent sub-agents |
| Skip file reservations when agent-mail is available | Reserve paths before spawning to catch conflicts early |
| Retry failed sub-agents in parallel again | Sequential retry gives full context and avoids repeating the same failure |
| Ignore partial wave failures | Merge successful work, retry failures sequentially |
