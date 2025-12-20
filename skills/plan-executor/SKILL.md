# Plan Executor

Use when: User says `execute plan` and has a written plan to execute, either in current session or via subagents.

## Trigger Phrases

- `execute plan`
- `run the plan`
- `implement the plan`

## Execution Modes

### Mode 1: BATCH (Sequential in Current Session)

Use when:
- Tasks have dependencies
- Need tight coordination
- Plan is small (< 10 tasks)

```
For each task in plan:
  1. Mark task in-progress (TodoWrite)
  2. Implement task
  3. Verify task
  4. Mark task complete
  5. Continue to next
```

### Mode 2: SUBAGENT (Parallel via Task Tool)

Use when:
- Tasks are independent
- Plan is large (10+ tasks)
- Tasks touch different parts of codebase

```
1. Identify independent task groups
2. Dispatch Task() for each group
3. Collect results
4. Handle failures
5. Merge and verify
```

## Workflow

### Phase 1: Load Plan

1. Find plan file (check in order):
   - User-specified path
   - `conductor/tracks/<id>/plan.md`
   - `docs/plans/*.md` (most recent)
   - `.beads/` database via `bd ready`

2. Parse plan structure:
   - Phases/sections
   - Individual tasks
   - Dependencies between tasks

3. Expected plan format:
   ```markdown
   # Plan: <title>
   
   ## Context
   Brief description of what this plan implements.
   
   ## Tasks
   
   ### Phase 1: <phase-name>
   - [ ] Task 1.1: <description>
     - Acceptance: <criteria>
     - Depends: none
   - [ ] Task 1.2: <description>
     - Acceptance: <criteria>
     - Depends: 1.1
   
   ### Phase 2: <phase-name>
   - [ ] Task 2.1: <description>
     - Acceptance: <criteria>
     - Depends: Phase 1
   ```
   
   Key elements:
   - Checkbox syntax `[ ]` / `[x]` for tracking
   - `Depends:` field indicates sequential requirements
   - Tasks without dependencies can run in parallel
   - Acceptance criteria define "done"

### Phase Sizing Guidelines

Before executing, verify phase sizing aligns with agent cognitive limits:

| Metric | Target | Action if Wrong |
|--------|--------|-----------------|
| Phase size | 500-1000 lines | Split if too large |
| Task size | ~500 lines | Merge if too small |
| Work duration | 30-120 min per task | Re-scope if outside |

**Why sizing matters:**
- Large phases cause **context bombing** (agent skips critical detail)
- Vague tasks lead to **lazy summarization** ("implement auth" vs specific methods)
- Critical edge cases get paraphrased away in oversized work units

**Pre-execution check:**
1. Estimate lines of code per phase
2. If phase > 1000 lines: split into sub-phases
3. If task < 200 lines: consider merging with adjacent task

**Grounding reminder:** Before implementing tasks with external dependencies (APIs, libraries, frameworks), ground patterns against current documentation:
- Use `/ground` if available, otherwise use `web_search` to verify external API/library patterns
- For existing codebase patterns: use `Grep` and `finder` to confirm conventions

### Phase 2: Classify Execution Mode

```
IF plan has sequential dependencies:
  mode = BATCH
ELIF tasks are independent AND count > 5:
  mode = SUBAGENT  
ELSE:
  mode = BATCH
```

Ask user to confirm mode if ambiguous.

### Phase 3: Execute

#### BATCH Mode:
```
for task in tasks:
  todo_write(task, status="in-progress")
  implement(task)
  verify(task)  # tests, lint, typecheck
  todo_write(task, status="completed")
```

#### SUBAGENT Mode:
```
groups = partition_independent_tasks(tasks)
for group in groups:
  Task(
    description: "Implement: {group.summary}",
    prompt: """
      Context: {plan_context}
      Tasks: {group.tasks}
      
      For each task:
      1. Implement with TDD
      2. Verify (tests, lint)
      3. Report status
      
      Return: summary, files changed, issues encountered
    """
  )
```

### Phase 4: Verify All

1. Run full test suite
2. Run typecheck/lint
3. Check for conflicts between parallel work
4. Report overall status

### Phase 5: Update Tracking

```bash
# If using beads
bd update <epic-id> --notes "Plan executed. Completed: X/Y tasks"

# If using conductor
# Update plan.md with [x] markers and commit SHAs
```

## Output Format

```
PLAN: <plan-name>
MODE: BATCH | SUBAGENT
TASKS: <completed>/<total>

[Per-task status...]

RESULT: Success | Partial | Failed
ISSUES: <any problems encountered>
NEXT: <recommendation>
```

## Error Handling

- If task fails, mark as blocked with reason
- Continue with independent tasks if in SUBAGENT mode
- Stop and report if in BATCH mode (unless user overrides)
- Always run verification before claiming success
