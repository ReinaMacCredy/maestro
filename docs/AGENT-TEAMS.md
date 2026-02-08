# Agent Teams Guide

How Maestro uses Claude Code's Agent Teams feature.

## Tools

### Team Management

| Tool | Purpose |
|------|---------|
| `TeamCreate` | Create a team and its task list |
| `TeamDelete` | Remove team and task directories |

### Spawning Teammates

```
Task(
  description: "implement auth module",
  name: "worker-1",
  team_name: "my-team",
  subagent_type: "kraken",
  prompt: "## TASK\n..."
)
```

### Task Coordination

All agents (leads AND workers) share the task list:

| Tool | Purpose |
|------|---------|
| `TaskCreate` | Create a task |
| `TaskList` | View all tasks |
| `TaskGet` | Get task details |
| `TaskUpdate` | Update status/owner |

### Communication

| Type | Purpose |
|------|---------|
| `SendMessage(type: "message")` | Direct message to one teammate |
| `SendMessage(type: "broadcast")` | Message all (use sparingly) |
| `SendMessage(type: "shutdown_request")` | Ask teammate to exit |

## Worker Self-Coordination

All workers (kraken, spark, build-fixer, explore, oracle) have team tools. After their first assigned task, they self-claim:

```
TaskGet(taskId)                          # Read task details
TaskUpdate(taskId, status: "in_progress") # Claim it
# ... do the work ...
TaskUpdate(taskId, status: "completed")   # Mark done
TaskList()                                # Find next unblocked task
TaskUpdate(nextId, owner: "my-name")      # Claim next
```

## Examples

### Planning Team

```
# 1. Create team
TeamCreate(team_name: "design-auth", description: "Planning auth")

# 2. Spawn researchers in parallel
Task(name: "researcher", team_name: "design-auth", subagent_type: "Explore", prompt: "Find auth patterns...")
Task(name: "advisor", team_name: "design-auth", subagent_type: "oracle", prompt: "Evaluate JWT vs sessions...")

# 3. Interview user while research happens
# 4. Synthesize and generate plan

# 5. Cleanup
SendMessage(type: "shutdown_request", recipient: "researcher")
SendMessage(type: "shutdown_request", recipient: "advisor")
TeamDelete(reason: "Planning complete")
```

### Execution Team

```
# 1. Create team
TeamCreate(team_name: "work-auth", description: "Implementing auth")

# 2. Create tasks
TaskCreate(subject: "Add login endpoint", description: "...", activeForm: "Adding login endpoint")
TaskCreate(subject: "Add auth middleware", description: "...", activeForm: "Adding auth middleware")

# 3. Spawn workers in parallel
Task(name: "impl-1", team_name: "work-auth", subagent_type: "kraken", prompt: "...")
Task(name: "impl-2", team_name: "work-auth", subagent_type: "spark", prompt: "...")

# 4. Assign first round
TaskUpdate(taskId: "1", owner: "impl-1", status: "in_progress")
TaskUpdate(taskId: "2", owner: "impl-2", status: "in_progress")

# 5. Workers complete tasks and self-claim remaining via TaskList
# 6. Lead verifies results (read files, run tests)

# 7. Cleanup
SendMessage(type: "shutdown_request", recipient: "impl-1")
SendMessage(type: "shutdown_request", recipient: "impl-2")
TeamDelete(reason: "Execution complete")
```

## Best Practices

1. **Spawn workers in parallel** — Multiple `Task` calls in one message
2. **Use descriptive names** — `auth-impl` not `worker-1`
3. **Include context in prompts** — Teammates don't inherit your conversation
4. **Verify all claims** — Read files and run tests after each task
5. **Let workers self-claim** — Assign first round, workers take the rest
6. **Clean up when done** — Always shutdown teammates then `TeamDelete`
