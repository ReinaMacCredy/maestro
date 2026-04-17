---
name: maestro:implement
description: "Execute track tasks following TDD workflow. Single-agent by default, --team for parallel Agent Teams, Sub Agent Parallels. Use when ready to implement a planned track."
argument-hint: "[<track-name>] [--team] [--parallel]"
---

# Implement -- Task Execution Engine

Execute tasks from a track's implementation plan, following the configured workflow methodology (TDD or ship-fast). Supports single-agent mode (default), team mode (`--team`), and parallel mode (`--parallel`).

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring. Optional -- auto-selects if only one track is pending.
- `--team`: Enable team mode with parallel workers (kraken/spark).
- `--parallel`: Enable parallel mode with Task sub-agents in isolated worktrees.
- `--resume`: Skip already-completed tasks (marked `[x]`) and continue from next `[ ]` task.

---

## Step 1: Mode Selection

Parse `$ARGUMENTS` for explicit flags, then validate the choice is appropriate.

### 1a: Explicit Flag Detection

- If contains `--team` --> team mode (see `reference/team-mode.md`)
- If contains `--parallel` --> parallel mode (see `reference/parallel-mode.md`)
- Otherwise --> single-agent mode (default)
- If contains `--resume` --> set resume flag (works with all modes)

### 1b: Mode Selection Checklist

Before executing, validate that the chosen mode fits the work. Use this checklist when the user has NOT specified a flag and you need to recommend, or to warn when an explicit flag conflicts with the task shape.

```
MODE SELECTION CHECKLIST

   Evaluate:                          Single    Parallel    Team
   ----------------------------------------------------------------
1. Task count                         1-3       3-8         4+
2. Independent tasks in phase?        any       2+ needed   2+ needed
3. File scope overlap between tasks?  n/a       low/none    low/none
4. Cross-task dependencies?           any       few         moderate ok
5. Task complexity                    any       moderate    high
6. Need human review between tasks?   yes       wave-end    task-end
7. Runtime supports Task tool?        n/a       required    n/a
8. Runtime supports Agent Teams?      n/a       n/a         required
```

**Decision rules:**

| Condition | Recommended Mode | Reason |
|-----------|-----------------|--------|
| 1-3 tasks, any dependency shape | Single | Overhead of parallelism exceeds benefit |
| 3-8 tasks, 2+ independent per phase, low file overlap | Parallel | Wave-based execution saves time |
| 4+ tasks, high complexity, need orchestration | Team | Workers handle complexity, orchestrator verifies |
| All tasks are sequential (each depends on previous) | Single | No parallelism possible regardless of count |
| Tasks touch many shared files | Single | File conflicts make parallel/team counterproductive |
| Mix of independent and dependent tasks | Parallel | Waves handle the dependency ordering naturally |

**Warn and suggest** if the user's explicit flag conflicts:

```
[!] --parallel specified but all 3 tasks are sequential (each depends on previous).
--> Falling back to single-agent mode. No parallelism benefit here.
```

```
[!] --team specified but only 2 tasks in this track.
--> Single-agent mode is more efficient for small tracks. Proceed with --team anyway? (y/n)
```

### 1c: Mode Comparison

| Aspect | Single | Parallel | Team |
|--------|--------|----------|------|
| Execution | Sequential in main session | Concurrent sub-agents in worktrees | Concurrent workers via delegation |
| Isolation | None (main worktree) | Git worktree per sub-agent | Shared worktree, task-level isolation |
| Who commits | Main session | Main session (after merge) | Orchestrator (after verification) |
| Failure recovery | Fix inline, retry | Retry failed task sequentially | Reassign or fix task |
| Best for | Small tracks, tight dependencies | Medium tracks, independent tasks | Large tracks, complex tasks |
| Overhead | None | Worktree setup + merge | Team setup + monitoring |

---

## Step 2: Track Selection

1. Read `.maestro/tracks.md`. Parse status markers: `[ ]` = new, `[~]` = in-progress, `[x]` = complete. Support both `- [ ] **Track:` and legacy `## [ ] Track:` formats.
2. **If track name given**: Match by exact ID or case-insensitive substring on description. If multiple matches, ask user.
3. **If no track name**: Filter `[ ]`/`[~]` tracks. 0 = error, 1 = auto-select, multiple = ask user.
4. **Confirm selection**: Ask user to start or cancel.

## Step 3: Load Context

Load context in tiers to minimize upfront token cost:

### Essential (load immediately)
1. Read track plan: `.maestro/tracks/{track_id}/plan.md`
2. Read track spec: `.maestro/tracks/{track_id}/spec.md`

### Deferred (load at first task start)
3. Read workflow config: `.maestro/context/workflow.md`
4. Read tech stack: `.maestro/context/tech-stack.md`

### On-demand (load only if relevant to current task)
5. Read guidelines: `.maestro/context/guidelines.md` (if exists)
6. Read code style guides: `.maestro/context/code_styleguides/` (if exists)
7. Note matched skills from `.maestro/tracks/{track_id}/metadata.json` `"skills"` array. Reference their guidance when relevant to current task (skill descriptions are already in runtime context). **Graceful degradation**: if missing/empty, proceed without.
8. Read `.maestro/notepad.md` (if exists). Extract `## Priority Context` bullets. These are injected as constraints into task execution context. **Graceful degradation**: if missing or empty, skip.

## Step 4: Update Track Status

Edit `.maestro/tracks.md`: `[ ]` --> `[~]`. Update `metadata.json`: `"status": "in_progress"`.

## Step 4.5: BR Check

**BR check**: If `metadata.json` has `beads_epic_id`, set `br_enabled=true`. All BR operations below only apply when `br_enabled`. See `reference/br-integration.md` for commands.

If `br_enabled` and `.beads/` does not exist: `br init --prefix maestro --json`.

## Step 5: Build Task Queue

Parse `plan.md`: identify phases (`## Phase N`), tasks (`### Task N.M`), sub-tasks (`- [ ] ...`).
If `--resume`: skip tasks already marked `[x]`.

### Task Dependency Resolution

Dependencies are resolved in this priority order:

1. **BR dependencies** (if `br_enabled`): Use `bv -robot-plan -label "track:{epic_id}" -format json` to get dependency-respecting execution order. This is the most reliable source because dependencies are explicit.
2. **Plan structure**: Tasks within a phase execute in document order. Phases execute sequentially. Cross-phase tasks are always sequential.
3. **Inferred dependencies**: If a task's description references output from another task (e.g., "using the API from Task 1.1"), treat it as a dependency even if not explicitly marked.

**Dependency conflict detection:**
```
[!] Task 2.3 references "the schema from Task 2.1" but plan.md lists them as independent.
--> Treating Task 2.3 as dependent on Task 2.1. Adjust wave assignment.
```

If `br_enabled` and `--resume`: use `br list --status open --label "phase:{N}" --json` to identify remaining work. Fall back to plan.md parsing if `bv` is unavailable.

---

## Single-Agent Mode (Default)

### Step 6a: Execute Tasks Sequentially

Follow the TDD or ship-fast methodology for each task.
See `reference/single-agent-execution.md` for the full Red-Green-Refactor cycle (steps 6a.1-6a.9), ship-fast variant, skill injection protocol, and worked examples.
See `reference/tdd-workflow.md` for TDD best practices and anti-patterns.

### Step 7a: Phase Completion Verification

When the last task in a phase completes, run the Phase Completion Protocol.
See `reference/phase-completion.md` for details (coverage check, full test run, manual verification, user confirmation).

---

## Parallel Mode (--parallel)

See `reference/parallel-mode.md` for full protocol: plan analysis for task independence, wave-based sub-agent spawning with worktree isolation, result verification and merge, conflict detection, sequential fallback, and worked examples.

---

## Team Mode (--team)

See `reference/team-mode.md` for full protocol: team creation, task delegation, worker spawning, monitoring, verification, shutdown, and worked examples.

---

## Step 8: Track Completion

When ALL phases are complete, run the Track Completion Protocol.
See `reference/track-completion.md` for details (mark complete, skill effectiveness recording, cleanup, final commit, summary).

---

## Failure Recovery

These recovery procedures apply across all modes. Mode-specific recovery is documented in the respective reference files.

### Worker/Sub-agent Failure

When a worker or sub-agent fails during task execution:

```
FAILURE TRIAGE

1. Read the error output
2. Classify the failure:

   Build error         --> Fix the code, retry the task
   Test failure        --> Debug the test or implementation, retry
   Missing dependency  --> Install/configure, retry
   Unclear spec        --> STOP, ask user for clarification
   Infrastructure      --> Check environment, retry once, then STOP
   Repeated failure    --> STOP after 3 attempts on same task
```

### Retry vs. Manual Fix Decision

| Signal | Action |
|--------|--------|
| Test failure with clear error message | Retry: fix the code and re-run |
| Same test fails 3 times | STOP: ask user -- likely a spec or design issue |
| Build error in generated/config code | Manual fix in main session, then continue |
| Worker reports blocker | Assess blocker, provide decision, re-dispatch |
| Merge conflict after parallel wave | Sequential retry for conflicting tasks |
| Rate limit or infrastructure error | Wait and retry once, then fall back to sequential |

### Re-dispatch vs. Fix-in-Place

**Re-dispatch** (spawn a new worker/sub-agent) when:
- The failure was environmental (timeout, rate limit, infra)
- The task is independent and can run cleanly from scratch
- The previous attempt left no partial state

**Fix-in-place** (main session fixes directly) when:
- The failure is a small, obvious bug (typo, missing import)
- Partial work exists and is mostly correct
- Re-dispatching would repeat substantial correct work

### Stale Task Recovery

If `maestro status` shows a task stuck in `[~]` (in-progress) with no active worker:

```
[!] Task 2.1 is marked in-progress but no worker is active.
--> This is a stale task from a crashed/interrupted session.
```

Recovery:
1. Check if partial work exists (uncommitted files, partial implementation)
2. If partial work is salvageable: use `--resume` to continue from current state
3. If partial work is broken: reset to last commit, mark task `[ ]`, re-execute
4. If using BR: `br update {issue_id} --status open --json` to unblock downstream

---

## Quality Gates

Apply these quality checks during and between task execution.

### Post-Batch Hygienic Review

After each task checkpoint report, ask the operator if they want a Hygienic code review for the latest task. If yes, run a review subagent to inspect the implementation changes and apply feedback before starting the next task.

### When to Stop and Ask for Help

**STOP executing immediately when:**
- Hit a blocker mid-task (missing dependency, test fails, instruction unclear)
- Plan has critical gaps preventing starting
- You don't understand an instruction
- Verification fails repeatedly (3+ attempts)
- A task produces side effects not anticipated by the plan

**Ask for clarification rather than guessing.**

### When to Revisit Earlier Steps

**Return to Review (Step 2) when:**
- The operator updates the plan based on your feedback
- Fundamental approach needs rethinking

**Re-planning signals:**
- Multiple tasks are failing due to a shared incorrect assumption
- The codebase structure diverges significantly from what the plan expected
- Dependencies between tasks are discovered that the plan did not account for
- A completed task invalidated assumptions in upcoming tasks

**Don't force through blockers** -- stop and ask.

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- **You are here.** Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Implementation consumes the `plan.md` created by `/maestro:new-track`. Each task produces atomic commits, which `/maestro:review` can analyze to verify correctness against the spec. Run `/maestro:status` to check progress mid-implementation, or `/maestro:revert` to undo if something goes wrong.
