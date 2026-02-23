# Task Model

Canonical task state model for CLI-agnostic orchestration. Defines how tasks flow through the system — states, transitions, and ownership — without referencing any specific CLI tools.

---

## Task Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier assigned by the task board |
| `subject` | string | Short, imperative title (e.g., "Fix auth redirect bug") |
| `description` | string | Full description with acceptance criteria, file paths, and constraints |
| `activeForm` | string | Present continuous form for progress display (e.g., "Fixing auth redirect") |
| `status` | enum | Current state — see State Machine below |
| `owner` | string | Agent name currently responsible; empty means unassigned |
| `blocks` | string[] | Task IDs that cannot start until this task completes |
| `blockedBy` | string[] | Task IDs that must complete before this task can start |

---

## State Machine

```
pending ──→ in_progress ──→ completed
   │              │
   │              ├──→ blocked ──→ pending  (when blocker resolves)
   │              │
   │              └──→ failed
   │
   └──→ deleted
```

### State Definitions

| State | Meaning |
|-------|---------|
| `pending` | Created, not yet started. Can be claimed by a worker. |
| `in_progress` | Actively being worked on by the current owner. |
| `blocked` | Cannot proceed; waiting on a dependency to resolve. |
| `completed` | Done and verified against acceptance criteria. |
| `failed` | Could not complete after retry attempts; needs intervention. |
| `deleted` | Removed — no longer relevant to the plan. |

### Valid Transitions

| From | To | Condition |
|------|----|-----------|
| `pending` | `in_progress` | Worker claims the task |
| `pending` | `deleted` | Orchestrator removes irrelevant task |
| `in_progress` | `completed` | Worker finishes and verifies |
| `in_progress` | `blocked` | Dependency discovered mid-work |
| `in_progress` | `failed` | Work cannot proceed after retries |
| `blocked` | `pending` | Blocking dependency resolves |
| `failed` | `in_progress` | Orchestrator reassigns for retry |

Transitions not listed above are invalid. A `completed` task cannot be reopened; create a new task instead.

---

## Ownership Model

### 1. Explicit Assignment

The orchestrator assigns a task to a specific worker before work begins:

```
task.update(id, { owner: "agent-name", status: "in_progress" })
```

Use this when task routing requires specific expertise (e.g., assigning build failures to build-fixer).

### 2. Self-Claim

Workers autonomously pull work from the queue:

1. Call `task.list()` to see all tasks
2. Filter for `status: "pending"`, empty `owner`, and empty `blockedBy`
3. Claim by calling `task.update(id, { owner: "self", status: "in_progress" })`
4. Begin work immediately after claiming

**Self-claim loop (workers):**

```
task.list()
  → find first pending, unblocked, unowned task
  → task.update(id, { owner: "self", status: "in_progress" })
  → do the work
  → task.update(id, { status: "completed" })
  → repeat
```

### 3. Reassignment

The orchestrator can reassign a stalled or failed task to a different worker:

```
task.update(id, { owner: "new-agent", status: "in_progress" })
```

Trigger reassignment when a heartbeat has not been seen for >10 minutes (see Heartbeat Protocol).

### 4. One Owner at a Time

A task has exactly one owner. Concurrent ownership is not allowed. Workers must not claim a task that already has an owner.

---

## Task Board Operations

Abstract operations — implementation maps to the runtime's native task primitives:

### `task.create(subject, description, activeForm?)`

Create a new task and add it to the board.

- `subject`: Short imperative title
- `description`: Full requirements including acceptance criteria
- `activeForm`: Optional present-continuous label shown during progress display
- Returns the assigned `id`

### `task.list()`

Return all tasks with their current status summary. Workers use this to find claimable work (pending, unblocked, no owner).

### `task.get(id)`

Return full task details for a specific task ID, including description, current state, owner, and dependency lists.

### `task.update(id, changes)`

Apply a partial update to a task. Supported change fields:

| Field | Purpose |
|-------|---------|
| `status` | Advance or change the task state |
| `owner` | Assign or transfer ownership |
| `description` | Append progress notes or heartbeat timestamps |
| `addBlocks` | Register tasks that this task blocks |
| `addBlockedBy` | Register tasks that block this task |

Only provide fields that are changing. Omitted fields are left unchanged.

---

## Heartbeat Protocol

Long-running tasks (>5 minutes) must emit periodic heartbeats so the orchestrator can detect stalls.

**Worker responsibility:** Append a timestamp to the task description every 5 minutes while work is ongoing.

```
task.update(id, {
  description: "<existing description>\nHeartbeat: 2026-02-23T10:15:00Z"
})
```

**Stall detection:** If a task has been `in_progress` for >10 minutes with no heartbeat update, the orchestrator should treat it as stalled and reassign.

---

## Dependency Resolution

When a task transitions to `blocked`:

1. Set `status: "blocked"` and record which dependency is blocking (in description)
2. When the blocking task reaches `completed`, the orchestrator sets the blocked task back to `status: "pending"`
3. Workers re-discover it on their next `task.list()` call

The `blockedBy` field is set at task creation time for known dependencies. Mid-work blocking (discovered during execution) is handled via status transitions and description notes.
