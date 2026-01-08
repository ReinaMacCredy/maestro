# Agent Routing

> **Routing tables, spawn patterns, and file reservation patterns for each agent category.**

## Overview

This document defines how to route tasks to specialized agents based on intent, and provides Task() spawn patterns and file reservation patterns for each category.

## Routing Tables

### Research Category

| Agent | Intent Keywords | Default Priority | File Reservation |
|-------|-----------------|------------------|------------------|
| Locator | `find`, `where`, `locate`, `search` | 1 | None (read-only) |
| Analyzer | `analyze`, `understand`, `how does` | 2 | None (read-only) |
| Pattern | `pattern`, `convention`, `example` | 3 | None (read-only) |
| Web | `docs`, `api`, `external`, `library` | 4 | None (read-only) |

### Review Category

| Agent | Intent Keywords | Default Priority | File Reservation |
|-------|-----------------|------------------|------------------|
| CodeReview | `review`, `check`, `quality` | 1 | None (read-only) |
| SecurityAudit | `security`, `audit`, `vulnerability`, `CVE` | 2 | None (read-only) |
| PerformanceReview | `perf`, `slow`, `memory`, `profile` | 3 | None (read-only) |

### Planning Category

| Agent | Intent Keywords | Default Priority | File Reservation |
|-------|-----------------|------------------|------------------|
| Architect | `design`, `architect`, `structure`, `system` | 1 | `conductor/tracks/**` |
| Planner | `plan`, `approach`, `strategy`, `breakdown` | 2 | `conductor/tracks/**` |

### Execution Category

| Agent | Intent Keywords | Default Priority | File Reservation |
|-------|-----------------|------------------|------------------|
| Implementer | `implement`, `build`, `create`, `new` | 1 | Task-specific scope |
| Modifier | `add`, `change`, `update`, `modify` | 2 | Task-specific scope |
| Fixer | `fix`, `bug`, `patch`, `error` | 3 | Task-specific scope |
| Refactorer | `refactor`, `improve`, `clean`, `simplify` | 4 | Task-specific scope |

### Debug Category

| Agent | Intent Keywords | Default Priority | File Reservation |
|-------|-----------------|------------------|------------------|
| Debugger | `debug`, `investigate`, `root cause` | 1 | None (read-only) |
| Tracer | `trace`, `follow`, `track`, `execution` | 2 | None (read-only) |

## Spawn Patterns

### Research Agent Spawn

```bash
Task(
    description=f"""You are {agent_name}, a Research specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## Protocol
1. Use finder, Grep, Read to explore codebase
2. Do NOT modify any files
3. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     project_key:"$PROJECT_PATH" \
     sender_name:"$AGENT_NAME" \
     to:'["Orchestrator"]' \
     subject:"Completed: {task_id}" \
     body_md:"$SUMMARY"
4. Return structured findings

## CRITICAL
You MUST send message via Agent Mail CLI before returning.
""",
    prompt=user_request
)
```

### Review Agent Spawn

```bash
Task(
    description=f"""You are {agent_name}, a Review specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## Protocol
1. Read and analyze code (do NOT modify)
2. Document findings with file:line references
3. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     project_key:"$PROJECT_PATH" \
     sender_name:"$AGENT_NAME" \
     to:'["Orchestrator"]' \
     subject:"Completed: {task_id}" \
     body_md:"$SUMMARY"
4. Return structured review

## CRITICAL
You MUST send message via Agent Mail CLI before returning.
""",
    prompt=user_request
)
```

### Planning Agent Spawn

```bash
Task(
    description=f"""You are {agent_name}, a Planning specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## File Scope
{file_patterns}

## Protocol
1. Reserve files:
   toolboxes/agent-mail/agent-mail.js file-reservation-paths \
     project_key:"$PROJECT_PATH" \
     agent_name:"$AGENT_NAME" \
     paths:'["{file_scope}"]' \
     exclusive:true
2. Read existing plans/specs
3. Create/update planning documents
4. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     project_key:"$PROJECT_PATH" \
     sender_name:"$AGENT_NAME" \
     to:'["Orchestrator"]' \
     subject:"Completed: {task_id}" \
     body_md:"$SUMMARY"
5. Release reservations
6. Return structured result

## CRITICAL
You MUST send message via Agent Mail CLI before returning.
""",
    prompt=user_request
)
```

### Execution Agent Spawn

```bash
Task(
    description=f"""You are {agent_name}, an Execution specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## File Scope
{file_patterns}

## Beads
{bead_list}

## Protocol
1. Reserve files:
   toolboxes/agent-mail/agent-mail.js file-reservation-paths \
     project_key:"$PROJECT_PATH" \
     agent_name:"$AGENT_NAME" \
     paths:'["{file_scope}"]' \
     exclusive:true
2. For each bead:
   - bd update <id> --status in_progress
   - Implement the code
   - Run tests/verification
   - bd close <id> --reason completed
3. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     project_key:"$PROJECT_PATH" \
     sender_name:"$AGENT_NAME" \
     to:'["Orchestrator"]' \
     subject:"Completed: {task_id}" \
     body_md:"$SUMMARY"
4. Release reservations
5. Return structured result

## CRITICAL
- You CAN claim and close beads using bd CLI
- You MUST send message via Agent Mail CLI before returning
""",
    prompt=user_request
)
```

### Debug Agent Spawn

```bash
Task(
    description=f"""You are {agent_name}, a Debug specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## Protocol
1. Use systematic debugging approach
2. Read logs, traces, code (do NOT modify unless fixing)
3. Document root cause analysis
4. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     project_key:"$PROJECT_PATH" \
     sender_name:"$AGENT_NAME" \
     to:'["Orchestrator"]' \
     subject:"Completed: {task_id}" \
     body_md:"$SUMMARY"
5. Return structured findings

## CRITICAL
You MUST send message via Agent Mail CLI before returning.
""",
    prompt=user_request
)
```

## File Reservation Patterns

### By Category

| Category | Reservation Mode | TTL | Rationale |
|----------|------------------|-----|-----------|
| Research | None | - | Read-only operations |
| Review | None | - | Read-only operations |
| Planning | Exclusive | 3600s | Document creation/update |
| Execution | Exclusive | 3600s | Code modification |
| Debug | None (usually) | - | Read-only until fix identified |

### Task-Specific Scopes

Map from plan.md File Scope column to reservation patterns:

| File Scope Pattern | Reservation Pattern |
|-------------------|---------------------|
| `src/api/**` | `["src/api/**"]` |
| `skills/orchestrator/**` | `["skills/orchestrator/**"]` |
| `tests/**/*.test.ts` | `["tests/**/*.test.ts"]` |
| `docs/` | `["docs/**"]` |

### Conflict Prevention

When spawning multiple execution agents:

1. **Non-overlapping scopes**: Spawn in parallel
2. **Overlapping scopes**: Spawn sequentially or split scope
3. **Same file**: Use `agent-mail.js file-reservation-paths` to serialize access

```python
# Check for conflicts before spawn
def check_scope_conflicts(tracks):
    for i, t1 in enumerate(tracks):
        for t2 in tracks[i+1:]:
            if scopes_overlap(t1.scope, t2.scope):
                # Add dependency or split scope
                resolve_conflict(t1, t2)
```

## Agent Directory Reference

See [../agents/README.md](../agents/README.md) for complete agent profiles:

- [Research Agents](../agents/research/)
- [Review Agents](../agents/review/)
- [Planning Agents](../agents/planning/)
- [Execution Agents](../agents/execution/)
- [Debug Agents](../agents/debug/)

## Integration with Workflow

This routing table is used in [workflow.md](workflow.md) Phase 3 (Spawn Workers) to determine:

1. Which agent type to spawn based on task intent
2. What spawn pattern to use
3. Whether file reservations are needed
4. What the agent's protocol should be

## Summary Protocol Reference

All spawn patterns include the mandatory `agent-mail.js send-message` call before returning. See [summary-protocol.md](summary-protocol.md) for the complete summary format.
