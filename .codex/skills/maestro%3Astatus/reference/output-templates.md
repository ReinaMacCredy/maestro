# Status Output Templates

Concrete output examples for each feature phase and condition. Use these as formatting references when presenting status to the user.

---

## Discovery Phase

Feature exists but no plan has been written yet.

```
# maestro status

feature: my-feature [planning]
plan:    none
tasks:   0/0 done
context: 3 files, ~2400 bytes

--> No plan yet. Start with `maestro:brainstorming` to explore the idea,
    then write the plan with `maestro plan-write --feature my-feature`.
```

**What to emphasize**: Context files (if any exist, the user has been doing discovery work). Suggest brainstorming or design skills. Do not show empty task sections.

---

## Planning Phase -- Draft With No Comments

Plan exists as a draft, no review comments yet.

```
# maestro status

feature: my-feature [planning]
plan:    draft
tasks:   0/0 done

--> Plan is drafted. Review it with `maestro plan-read --feature my-feature`.
    When satisfied, approve with `maestro plan-approve --feature my-feature`.
```

---

## Planning Phase -- Draft With Unreviewed Comments

Plan has review comments that need attention.

```
# maestro status

feature: my-feature [planning]
plan:    draft (3 comments)
tasks:   0/0 done

[~] 3 unreviewed comments on the plan.
    Read them: `maestro plan-read --feature my-feature`
    Address the feedback, revise the plan, then approve.
```

**What to emphasize**: Comment count is the key signal. Comments on a draft plan mean someone (or a previous session) left feedback. These must be addressed before approval.

---

## Pre-Execution Phase -- Plan Approved, No Tasks

Plan is approved but tasks have not been synced yet.

```
# maestro status

feature: my-feature [approved]
plan:    approved
tasks:   0/0 done

--> Plan approved. Generate tasks: `maestro task-sync --feature my-feature`
```

**What to emphasize**: This is a single-action state. The only thing to do is sync tasks. Keep the output minimal.

---

## Execution Phase -- Healthy Progress

Tasks are running with no problems detected.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   3/8 done, 1 in_progress, 4 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [done]          003-write-port
  [in_progress]   004-implement-usecase
  [pending]       005-add-validation
  [pending]       006-wire-cli-command
  [pending]       007-integration-tests
  [pending]       008-update-docs

--> Continue work on task: 004-implement-usecase
```

**What to emphasize**: Progress fraction and the in-progress task. Show full task list with status alignment. Done tasks can be collapsed if there are many (10+).

---

## Execution Phase -- Blocked Task

A task is blocked and requires a user decision.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   2/6 done, 0 in_progress, 3 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [blocked]       003-api-integration (blocked by: 002-create-adapter)
  [pending]       004-validation-logic
  [pending]       005-error-handling
  [pending]       006-tests

[!] Task 003-api-integration is BLOCKED.
    Read the blocker: `maestro task-report-read --feature my-feature --task 003-api-integration`
    Resume after decision: `maestro task-start --feature my-feature --task 003-api-integration --continue-from blocked --decision "<your decision>"`
```

**What to emphasize**: Blocked tasks get `[!]` markers and appear in a separate callout above the next-action. The user must make a decision before progress can continue.

---

## Execution Phase -- Zombie (Stale Task)

A task is marked in_progress but its session is stale or missing.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   1/5 done, 1 in_progress, 3 pending
  [done]          001-setup-schema
  [in_progress]   002-create-adapter
  [pending]       003-api-integration
  [pending]       004-validation
  [pending]       005-tests

zombies: 1 stale task(s) in_progress
  [!] 002-create-adapter -- session stale or missing, recover with task-start --force

[!] Stale task detected: 002-create-adapter
    The session for this task has expired or is missing.
    Recover: `maestro task-start --feature my-feature --task 002-create-adapter --force`
    This marks the stale attempt as failed and starts a fresh attempt.
```

**What to emphasize**: Zombies are the highest-priority problem. They block progress because the task appears in_progress but no worker is actually running. Always resolve zombies before starting new work.

---

## Execution Phase -- Failed Task

A task has failed and needs to be reset or investigated.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   3/7 done, 0 in_progress, 3 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [done]          003-write-port
  [failed]        004-implement-usecase
  [pending]       005-add-validation
  [pending]       006-wire-cli-command
  [pending]       007-tests

[~] Task 004-implement-usecase FAILED.
    Read the failure report: `maestro task-report-read --feature my-feature --task 004-implement-usecase`
    Reset to retry: `maestro task-update --feature my-feature --task 004-implement-usecase --status pending`
```

**What to emphasize**: Failed tasks get `[~]` markers (medium severity -- they do not block other tasks from running if those tasks have no dependency on the failed one). Suggest reading the report to understand what went wrong before blindly retrying.

---

## Execution Phase -- Partial Task

A task completed partially and needs to be resumed.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   2/6 done, 0 in_progress, 3 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [partial]       003-api-integration
  [pending]       004-validation
  [pending]       005-error-handling
  [pending]       006-tests

[~] Task 003-api-integration is PARTIAL.
    Some work was completed but the task is not finished.
    Review progress: `maestro task-report-read --feature my-feature --task 003-api-integration`
    Resume: `maestro task-start --feature my-feature --task 003-api-integration --continue-from partial`
```

**What to emphasize**: Partial tasks represent interrupted work. The previous attempt made progress that should not be discarded. Always resume rather than restart.

---

## Execution Phase -- Multiple Runnable Tasks

Several tasks are ready to start, with no current work in progress.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   3/8 done, 0 in_progress, 5 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [done]          003-write-port
  [pending]       004-implement-usecase
  [pending]       005-add-validation
  [pending]       006-wire-cli-command
  [pending]       007-integration-tests
  [pending]       008-update-docs

--> 3 tasks are ready to start: 004-implement-usecase, 005-add-validation, 006-wire-cli-command
    Pick the highest-priority task and start with:
    `maestro task-start --feature my-feature --task <task-id>`
```

---

## Execution Phase -- All Pending But Dependency-Blocked

All tasks are pending and none are runnable due to dependency chains.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   0/4 done, 0 in_progress, 4 pending
  [pending]       001-setup-schema
  [pending]       002-create-adapter (blocked by: 001-setup-schema)
  [pending]       003-api-integration (blocked by: 002-create-adapter)
  [pending]       004-tests (blocked by: 003-api-integration)

--> Start the first task in the dependency chain:
    `maestro task-start --feature my-feature --task 001-setup-schema`
```

---

## Completion Phase

All tasks are done.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   6/6 done
  [done]          001-setup-schema
  [done]          002-create-adapter
  [done]          003-api-integration
  [done]          004-validation
  [done]          005-error-handling
  [done]          006-tests
context: 5 files, ~8200 bytes

--> All tasks complete. Review the implementation, then mark the feature done:
    `maestro feature-complete --feature my-feature`
```

**What to emphasize**: Completion is a clean state. Show the done count, context file count (as a record of decisions made), and the single next action. Collapse the task list if there are many done tasks (show count only).

---

## Compound Condition -- Zombie + Blocked

Multiple problems coexist. Prioritize by severity.

```
# maestro status

feature: my-feature [executing]
plan:    approved
tasks:   2/7 done, 1 in_progress, 4 pending
  [done]          001-setup-schema
  [done]          002-create-adapter
  [in_progress]   003-api-integration
  [blocked]       004-validation
  [pending]       005-error-handling
  [pending]       006-cli-command
  [pending]       007-tests

zombies: 1 stale task(s) in_progress
  [!] 003-api-integration -- session stale or missing, recover with task-start --force

[!] ZOMBIE: 003-api-integration -- stale session, no worker running.
    Recover first: `maestro task-start --feature my-feature --task 003-api-integration --force`

[!] BLOCKED: 004-validation -- waiting on a decision.
    After recovering the zombie, read the blocker:
    `maestro task-report-read --feature my-feature --task 004-validation`

    Address problems in priority order: zombies first, then blocked tasks.
```

**What to emphasize**: When multiple conditions coexist, list them in severity order (zombie > blocked > failed > partial). Number them or use explicit ordering language ("first", "then", "after that") so the user knows the sequence.

---

## Formatting Reference

### Status Markers

| Marker | Meaning                          | Used For                          |
|--------|----------------------------------|-----------------------------------|
| `[!]`  | High severity, blocks progress   | Zombies, blocked tasks            |
| `[~]`  | Medium severity, needs attention  | Failed tasks, partial tasks, plan comments |
| `-->`  | Informational, suggested action   | Next steps, recommendations       |
| `[ok]` | Healthy state                    | Optional, for explicit confirmation |

### Collapsing Rules

- **Done tasks**: If more than 8 done tasks, collapse to a single line: `[done] 12 tasks completed`
- **Pending tasks**: Always show individually (the user needs to see what is coming)
- **Problem tasks**: Never collapse. Always show individually with full context.
- **Context files**: Show count and bytes only. Do not list individual files.

### Progress Indicators

For execution phase, include a progress fraction:

- `3/8 done (37%)` -- simple and scannable
- Do not use ASCII progress bars unless specifically requested
