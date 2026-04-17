# Delegation Patterns

Patterns for dispatching workers across different task topologies. Choose the pattern that matches your dependency structure.

---

## Pattern 1: Single Worker

**When:** One task, or multiple tasks that share too much state to parallelize.

```
Orchestrator
    |
    v
  Worker A (task 1, 2, 3 sequentially)
    |
    v
  Review & merge
```

**Use when:**
- Tasks modify the same files
- Task 2 depends on Task 1's output
- Shared mutable state (database, config files, type definitions)
- Total work is small enough for one agent

**Workflow:**
```bash
maestro task-start --feature my-feature --task 01-the-task
# Worker completes
# Review output, merge
```

**Risk:** Bottleneck. If the worker blocks, everything stops.

**Mitigation:** Break the work into subtasks within the single worker's scope so progress is incremental.

---

## Pattern 2: Parallel Workers (Fan-Out)

**When:** 2+ tasks with zero shared state. The primary dispatch pattern.

```
Orchestrator
    |
    +---> Worker A (task 1)
    +---> Worker B (task 2)
    +---> Worker C (task 3)
    |
    v
  Review all, merge all
```

**Use when:**
- Different test files / different modules / different subsystems
- No worker needs another worker's output
- Workers will not edit the same files

**Workflow:**
```bash
# Verify tasks are runnable (all dependencies satisfied)
maestro status

# Dispatch all in parallel
maestro task-start --feature my-feature --task 01-fix-auth
maestro task-start --feature my-feature --task 02-fix-parser
maestro task-start --feature my-feature --task 03-fix-renderer
```

**Post-dispatch checklist:**
1. Wait for all workers to finish
2. Review each worker's summary
3. Check for file conflicts (did two workers edit the same file?)
4. Run full test suite before merging
5. Merge one at a time, re-testing after each merge

**Risk:** Merge conflicts if independence was misjudged.

**Mitigation:** Before dispatching, verify workers will not touch overlapping files. If unsure, dispatch sequentially.

---

## Pattern 3: Dependent Task Chain (Pipeline)

**When:** Tasks have sequential dependencies -- Task 2 needs Task 1's output.

```
Orchestrator
    |
    v
  Worker A (task 1: define types)
    |
    v
  Review & merge
    |
    v
  Worker B (task 2: implement using types)
    |
    v
  Review & merge
    |
    v
  Worker C (task 3: write integration tests)
    |
    v
  Review & merge
```

**Use when:**
- Task 2 imports from Task 1's output
- Types/interfaces must exist before implementation
- Integration tests need implementation to exist

**Workflow:**
```bash
# Phase 1
maestro task-start --feature my-feature --task 01-define-types
# Wait, review, merge

# Phase 2 (now runnable because 01 is done)
maestro status  # Verify 02 is runnable
maestro task-start --feature my-feature --task 02-implement
# Wait, review, merge

# Phase 3
maestro task-start --feature my-feature --task 03-integration-tests
# Wait, review, merge
```

**Optimization -- Hybrid pipeline:** If tasks 2a and 2b both depend on task 1 but not on each other, fan out after task 1:

```
Task 1 (types)
    |
    +---> Task 2a (implementation A)
    +---> Task 2b (implementation B)
    |
    v
Task 3 (integration tests, depends on 2a and 2b)
```

```bash
# Phase 1
maestro task-start --feature my-feature --task 01-types
# Merge

# Phase 2 (parallel)
maestro task-start --feature my-feature --task 02a-impl-a
maestro task-start --feature my-feature --task 02b-impl-b
# Merge both

# Phase 3
maestro task-start --feature my-feature --task 03-integration
```

**Risk:** Slow -- sequential by nature.

**Mitigation:** Maximize what can run in parallel at each phase. Use `maestro status` to identify all runnable tasks after each merge.

---

## Pattern 4: Blocked Task Recovery

**When:** A worker hits a blocker it cannot resolve (needs a design decision, missing dependency, ambiguous spec).

```
Orchestrator
    |
    v
  Worker A (task 1)
    |
    v
  BLOCKED: "Should we use REST or GraphQL?"
    |
    v
  Orchestrator gets user decision
    |
    v
  Worker A' (resumes task 1 with decision)
    |
    v
  Review & merge
```

**Detection:** `maestro status` shows the task as `blocked` with a reason.

**Recovery workflow:**
```bash
# 1. Read the blocker details
maestro task-report-read --feature my-feature --task 01-the-task

# 2. Present to user, get decision
# "Worker is blocked: [blocker reason]. What should we do?"

# 3. Resume with the decision
maestro task-start --feature my-feature --task 01-the-task \
  --continue-from blocked \
  --decision "Use REST -- we need browser compatibility"
```

**What NOT to do:**
- Do NOT guess the answer and resume without asking the user
- Do NOT discard the task and start fresh (partial work is preserved)
- Do NOT start other dependent tasks while this one is blocked

**Parallel blocked recovery:** If multiple workers block on independent questions, collect all questions, present them to the user at once, then resume all workers.

---

## Pattern 5: Failed Task Retry

**When:** A worker fails (crash, incorrect output, tests broken).

```
Orchestrator
    |
    v
  Worker A (task 1) --> FAILED
    |
    v
  Diagnose: what went wrong?
    |
    v
  Worker A' (retry with better context)
    |
    v
  Review & merge
```

**Diagnosis checklist:**
1. Read the worker's report: `maestro task-report-read --feature my-feature --task 01`
2. Check what files were changed
3. Identify: was the spec unclear? Was the task too broad? Did the worker misunderstand?

**Retry strategies:**

| Root Cause | Recovery |
|-----------|----------|
| Spec unclear | Rewrite task spec with more detail, restart |
| Task too broad | Split into smaller subtasks |
| Worker misunderstood | Add explicit constraints and examples to prompt |
| External failure (build broken) | Fix the environment, then restart |
| Stale worktree | Use `--force` to mark stale attempt failed and start fresh |

```bash
# For stale/stuck tasks
maestro task-start --feature my-feature --task 01-the-task --force

# For tasks needing spec update before retry
maestro task-spec-write --feature my-feature --task 01-the-task --content "..."
maestro task-start --feature my-feature --task 01-the-task
```

---

## Choosing the Right Pattern

```
Is there only 1 task?
  YES --> Pattern 1 (Single Worker)
  NO  --> Do tasks share state or files?
            YES --> Can you sequence them?
                      YES --> Pattern 3 (Pipeline)
                      NO  --> Pattern 1 (Single Worker -- too coupled to split)
            NO  --> Pattern 2 (Parallel Workers)

Did a worker block?  --> Pattern 4 (Blocked Recovery)
Did a worker fail?   --> Pattern 5 (Failed Retry)
```

## Independence Checklist

Before choosing Pattern 2 (Parallel), verify ALL of these:

- [ ] Workers will not edit the same files
- [ ] Workers will not import from each other's new code
- [ ] Workers will not modify shared type definitions
- [ ] Workers will not depend on the same mutable state (DB, config)
- [ ] Each worker's tests can run independently
- [ ] Merging in any order produces the same result

One "no" means you need Pattern 1 or Pattern 3 instead.
