# Work Stealing Protocol

Dynamic load balancing via task reassignment between workers.

## When to Trigger

Work stealing activates when:
- **Load imbalance detected**: One worker has >2 more tasks than another
- **Worker idle**: A worker completes all assigned tasks while others have pending work
- **Worker stale**: A worker becomes unresponsive (>10 min without heartbeat)

## STEAL Message Format

Workers send steal requests via Agent Mail:

```python
send_message(
  project_key="<path>",
  sender_name="<idle_worker>",
  to=["<orchestrator>"],
  thread_id="<epic-id>",
  subject="[STEAL] Request work",
  importance="normal",
  body_md="""
## Steal Request

**From**: IdleWorker
**Current Load**: 0 tasks
**Capacity**: Ready for work

Requesting reassignment of pending tasks.
"""
)
```

## Orchestrator Response

When orchestrator receives STEAL request:

```python
# 1. Check for imbalanced workers
workers = get_worker_status()
overloaded = [w for w in workers if w.pending_tasks > 2]

if overloaded:
    # 2. Find stealable task (no cross-deps, not in_progress)
    victim = max(overloaded, key=lambda w: w.pending_tasks)
    task = get_stealable_task(victim)
    
    # 3. Reassign via Beads
    bash(f"bd update {task.bead_id} --assigned {idle_worker}")
    
    # 4. Notify both workers
    send_message(
      to=[idle_worker, victim.name],
      subject=f"[REASSIGN] {task.bead_id}",
      body_md=f"Task {task.bead_id} reassigned from {victim.name} to {idle_worker}"
    )
```

## Load Imbalance Detection

Orchestrator monitors during Phase 5:

```python
def check_imbalance():
    workers = fetch_worker_status()
    
    max_load = max(w.pending for w in workers)
    min_load = min(w.pending for w in workers)
    
    if max_load - min_load > 2:
        return ImbalanceDetected(
            overloaded=[w for w in workers if w.pending == max_load],
            underloaded=[w for w in workers if w.pending == min_load]
        )
    return None
```

| Metric | Threshold | Action |
|--------|-----------|--------|
| Load difference | >2 tasks | Trigger rebalance |
| Idle worker | 0 pending, others >0 | Offer work |
| Stale worker | No heartbeat >10 min | Reclaim tasks |

## Reassignment via Beads

Tasks are reassigned using `bd update`:

```bash
# Reassign task to new worker
bd update <bead_id> --assigned <new_worker>

# Worker claims reassigned task
bd update <bead_id> --status in_progress
```

## Worker Handling of STEAL

When a worker receives reassignment notification:

**As donor (losing task):**
```python
# Check if task was started
if task.status == "in_progress":
    # Cannot steal - already started
    reply_message(
      message_id=msg.id,
      sender_name=worker_name,
      body_md="Cannot reassign - task already in progress"
    )
else:
    # Acknowledge transfer
    pass  # No action needed, orchestrator handles reassignment
```

**As recipient (receiving task):**
```python
# 1. Reserve files for new task
file_reservation_paths(
  project_key=project_key,
  agent_name=worker_name,
  paths=[task.file_scope],
  ttl_seconds=3600
)

# 2. Claim the task
bash(f"bd update {task.bead_id} --status in_progress")

# 3. Execute normally
```

## Stealable Task Criteria

A task is stealable if:
1. Status is `ready` (not `in_progress` or `blocked`)
2. Has no unmet cross-track dependencies
3. File scope doesn't conflict with thief's current work
4. Not the last task of an almost-complete track

```python
def is_stealable(task, thief):
    return (
        task.status == "ready" and
        not has_unmet_deps(task) and
        not conflicts_with(task.file_scope, thief.reserved_files) and
        not is_last_in_track(task)
    )
```

## Anti-Patterns

| ❌ Don't | ✅ Do |
|----------|-------|
| Steal in_progress tasks | Only steal `ready` tasks |
| Steal without file reservation | Reserve files before starting |
| Ignore cross-track deps | Check dependencies before reassign |
| Steal from nearly-done worker | Prefer stealing from workers with >3 pending |

## Related

- [workflow.md](workflow.md) - Phase 5 monitoring loop
- [agent-coordination.md](agent-coordination.md) - Message protocols
- [worker-prompt.md](worker-prompt.md) - Worker 4-step protocol
