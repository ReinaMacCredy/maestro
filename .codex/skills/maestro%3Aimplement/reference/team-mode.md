# Team Mode Protocol

## Overview

Team mode uses agent delegation to parallelize task execution. You (the skill runner) become the orchestrator. Workers (kraken/spark) handle implementation. Unlike parallel mode (which uses ephemeral sub-agents per wave), team mode maintains persistent workers that pull tasks from a shared queue.

## When to Use

- Track has 4+ tasks
- Tasks are complex enough to benefit from dedicated workers
- Tasks have moderate dependencies (workers can wait for blockers to clear)
- You want persistent workers that self-assign work rather than orchestrator-dispatched waves

**Do not use** when:
- Track has fewer than 4 tasks (single-agent is more efficient)
- All tasks are tightly sequential (workers will idle waiting)
- Runtime does not support agent delegation

## Prerequisites

Your runtime must support agent delegation (teams, subagents, or equivalent).
- Claude Code: enable Agent Teams via `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`
- Other runtimes: use your native delegation mechanism

## Setup

### 1. Create Team

Create a worker team named "implement-{track_id}" (description: "Implementing track: {track_description}"). Use whatever team/delegation API your runtime provides.

### 2. Create Tasks from Plan

Parse `plan.md` and create one task per implementation item:

Create a task:
- **Subject**: "Phase {N} Task {M}: {task_title}"
- **Description**: Include context, spec reference, workflow (TDD or ship-fast), expected files to modify, and acceptance criteria (tests pass, coverage meets threshold, no lint errors).
- **Active form**: "Implementing {task_title}"

Set dependencies between tasks so that task M is blocked by task M-1 within the same phase. Cross-phase tasks are always sequential.

**Dependency example:**
```
Phase 1:
  Task 1.1: Create database schema      (no deps)
  Task 1.2: Create model layer          (blocked by 1.1)
  Task 1.3: Create seed data script     (blocked by 1.1, independent of 1.2)

Phase 2:
  Task 2.1: Create API routes           (blocked by all Phase 1)
  Task 2.2: Add authentication          (blocked by 2.1)
```

Tasks 1.1 starts immediately. Tasks 1.2 and 1.3 can run in parallel once 1.1 completes. Phase 2 waits for all of Phase 1.

**BR mirroring**: If `.beads/` exists and `metadata.json` has `beads_epic_id`, the orchestrator also mirrors task state to BR. Workers do NOT interact with `br` directly -- the orchestrator handles BR state changes after verifying worker output.

### 3. Spawn Workers

Spawn workers based on track size (see Worker Sizing below).

**For TDD tasks** (features, new code):

Spawn a TDD worker (kraken) with the following prompt:

```
You are a TDD implementation worker on team 'implement-{track_id}'.

Your workflow:
1. Check the task list for available tasks (unblocked, no owner)
2. Claim one by setting owner to your name and status to in_progress
3. Read the task description for context
4. Follow TDD: write failing tests, implement to pass, refactor
5. Run tests and verify they pass
6. Mark task completed with a summary of changes
7. Check the task list for next available task
8. If no tasks available, notify the orchestrator and wait

Project context:
- Workflow: .maestro/context/workflow.md
- Tech stack: .maestro/context/tech-stack.md
- Track spec: .maestro/tracks/{track_id}/spec.md
- Style guides: .maestro/context/code_styleguides/ (if exists)
- Priority context: .maestro/notepad.md (## Priority Context section, if exists)

Important constraints:
- Do NOT commit -- the orchestrator commits after verification
- Do NOT modify .maestro/ files
- Do NOT run br/bv commands
- Report blockers immediately rather than guessing
```

**For quick-fix tasks** (bugs, small changes):

Spawn a quick-fix worker (spark) with the same structure but without strict TDD requirement. Spark workers implement first, then add tests.

### 4. Worker Sizing

| Track Tasks | Workers | Types | Rationale |
|-------------|---------|-------|-----------|
| 1-3 | 1 kraken | Single worker | Overhead of team setup exceeds benefit |
| 4-8 | 2 (1 kraken + 1 spark) | Mixed team | Kraken handles complex tasks, spark handles simple ones |
| 8+ | 3 (2 kraken + 1 spark) | Full team | Two krakens for parallel complex work |

**Never spawn more than 3 workers.** Beyond 3, orchestration overhead dominates and workers contend for files.

---

## Orchestrator Responsibilities

The orchestrator (you) does NOT write code. You manage workers, verify output, commit changes, and handle blockers.

### Monitor Progress

After spawning workers, periodically check the task list for available/completed work.

**Monitoring cadence:**
- Check task list after each worker reports completion
- If no worker reports for an extended period, check for stuck tasks
- Watch for workers that claim a task but don't progress

**BR supplementary monitoring**: If `beads_epic_id` exists, use `br ready --parent {epic_id} --json` alongside `TaskList` to see which BR issues are unblocked. This supplements (does not replace) Agent Teams task monitoring.

### Verify Completed Tasks

When a worker reports task completion:

1. **Read the files they changed** -- review for correctness, style, spec compliance
2. **Run the test suite**:
   ```bash
   CI=true {test_command}
   ```
3. **If verification passes:**
   - Commit the changes:
     ```bash
     git add {changed_files}
     git commit -m "{type}({scope}): {description}"
     ```
   - Update plan.md: mark `[x] {sha}`
   - **BR mirror**: If `beads_epic_id` exists:
     ```bash
     br close {issue_id} --reason "sha:{sha7} | verified" --suggest-next --json
     ```
4. **If verification fails:**
   - Create a fix task with the specific failure details
   - Assign to the original worker (they have context) or a different worker if the original is stuck
   - Include the error output in the fix task description

### Handle Blockers

If a worker reports being blocked:

1. Read their blocker message
2. Classify the blocker:

| Blocker Type | Action |
|-------------|--------|
| Missing dependency from another task | Check if the dependency task is complete; if not, tell worker to wait |
| Unclear spec | Ask user for clarification, relay answer to worker |
| Tech decision needed | Make the decision (or ask user), update tech-stack.md, relay to worker |
| Environment/tooling issue | Diagnose and fix yourself, then tell worker to retry |
| Design flaw in plan | STOP all workers, re-evaluate with user |

3. Resolution options:
   - **Provide guidance**: Send a message to the worker with the answer/decision
   - **Reassign**: Move the task to a different worker who may have better context
   - **Handle yourself**: Fix the environment/config issue (but NEVER write implementation code as orchestrator)
   - **Escalate**: If the blocker reveals a plan flaw, stop workers and consult the user

### Handle Worker Failures

If a worker crashes, times out, or produces consistently broken output:

```
[!] Worker 'kraken-1' failed on Task 2.3 after 2 attempts.
--> Reassigning to kraken-2.
```

1. **First failure**: Ask the worker to retry with more specific guidance
2. **Second failure on same task**: Reassign to a different worker
3. **Third failure**: Stop and ask user -- likely a spec or design issue
4. **Worker completely unresponsive**: Spawn a replacement worker, reassign the stuck task

### Task Handoff Between Workers

When a dependency clears and a new task becomes available:

1. Check which workers are idle (no active task)
2. If an idle worker exists: the worker will self-assign from the task list
3. If all workers are busy: the task waits until a worker completes its current task
4. If a blocked worker becomes unblocked: notify the worker that the blocker is resolved

---

## Shutdown

After all tasks complete:

1. Verify all tasks are marked `[x]` in the task list
2. Request shutdown for each worker (e.g., "All tasks complete. You can stop.")
3. Wait for shutdown confirmations
4. Tear down the worker team
5. Proceed to Phase Completion Protocol

---

## Worked Example: Team Mode Execution

**Track**: "Add authentication system"
**Plan**: 6 tasks across 2 phases

```
Phase 1:
  Task 1.1: Create user model and migration
  Task 1.2: Create password hashing utility
  Task 1.3: Create JWT token service (depends on 1.1)

Phase 2:
  Task 2.1: Create login endpoint (depends on 1.1, 1.2, 1.3)
  Task 2.2: Create registration endpoint (depends on 1.1, 1.2)
  Task 2.3: Add auth middleware (depends on 1.3)
```

**Setup:**

```
[ok] Created team: implement-auth-system
[ok] Created 6 tasks with dependencies
[ok] Spawning workers:
  kraken-1: TDD worker (complex tasks)
  spark-1:  Quick-fix worker (simpler tasks)
```

**Execution flow:**

```
--- Phase 1 ---

kraken-1 claims Task 1.1 (user model)
spark-1  claims Task 1.2 (password hashing)
  --> 1.1 and 1.2 have no deps, run in parallel

[ok] spark-1 completes Task 1.2 (4 tests pass)
  Orchestrator verifies: tests pass, code looks good
  git commit -m "feat(auth): add password hashing utility"
  [x] Task 1.2 (sha: a1b2c3d)

spark-1 has no available tasks (1.3 blocked by 1.1). Waiting.

[ok] kraken-1 completes Task 1.1 (6 tests pass)
  Orchestrator verifies: tests pass, migration runs cleanly
  git commit -m "feat(auth): add user model and migration"
  [x] Task 1.1 (sha: e4f5g6h)

  --> Task 1.3 is now unblocked
spark-1 claims Task 1.3 (JWT service)

[ok] spark-1 completes Task 1.3 (5 tests pass)
  Orchestrator verifies: OK
  git commit -m "feat(auth): add JWT token service"
  [x] Task 1.3 (sha: i7j8k9l)

--- Phase 1 Complete ---
Full test suite: 15 tests pass
Phase completion verification: OK

--- Phase 2 ---

kraken-1 claims Task 2.1 (login endpoint)
spark-1  claims Task 2.3 (auth middleware)
  --> 2.2 is blocked by 1.1+1.2 (both done), so it's available
  --> but spark-1 chose 2.3 first. kraken-1 or spark-1 will pick up 2.2 next.

[!] spark-1 reports blocker on Task 2.3:
    "JWT_SECRET not defined in test environment"
  Orchestrator: Added JWT_SECRET to .env.test, told spark-1 to retry
  spark-1 retries Task 2.3

[ok] kraken-1 completes Task 2.1 (8 tests pass)
  git commit -m "feat(auth): add login endpoint"
  [x] Task 2.1 (sha: m0n1o2p)

kraken-1 claims Task 2.2 (registration endpoint)

[ok] spark-1 completes Task 2.3 (4 tests pass, after retry)
  git commit -m "feat(auth): add auth middleware"
  [x] Task 2.3 (sha: q3r4s5t)

spark-1 has no more tasks. Waiting.

[ok] kraken-1 completes Task 2.2 (7 tests pass)
  git commit -m "feat(auth): add registration endpoint"
  [x] Task 2.2 (sha: u6v7w8x)

--- Phase 2 Complete ---
Full test suite: 34 tests pass

--- Shutdown ---
[ok] All 6 tasks complete. Shutting down workers.
[ok] Team implement-auth-system disbanded.
```

---

## Comparison: Parallel Mode vs. Team Mode

| Aspect | Parallel Mode | Team Mode |
|--------|--------------|-----------|
| Worker lifecycle | Ephemeral (per wave) | Persistent (full track) |
| Task assignment | Orchestrator assigns per wave | Workers self-assign |
| Isolation | Git worktree per sub-agent | Shared worktree |
| Commit flow | Merge worktrees, then commit | Orchestrator commits after each task |
| Best for | Medium tracks, clear independence | Large tracks, complex dependencies |
| Failure handling | Retry sequentially in main session | Retry with same or different worker |
| Overhead | Worktree creation + merge per wave | Team setup + monitoring |

---

## Anti-patterns

| Don't | Do Instead |
|-------|-----------|
| Edit code directly as orchestrator | Delegate to workers -- orchestrator manages, never implements |
| Spawn too many workers (>3) | Match worker count to task count (see sizing table) |
| Skip verification | Always verify (read files + run tests) before committing |
| Let workers commit | Orchestrator commits after verification |
| Ignore worker messages | Respond promptly to unblock workers |
| Let workers interact with BR | Orchestrator handles all BR state changes |
| Force workers onto tasks | Workers self-assign -- guide with priorities, not force |
| Continue after 3 failures on same task | STOP and escalate to user -- likely a spec/design issue |
| Spawn replacement without diagnosing | Understand why the worker failed before spawning another |
