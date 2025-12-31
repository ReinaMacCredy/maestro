# Thin Router

Main thread stays clean: understand intent → route to specialist → display summary. Sub-agents do actual work and report via Agent Mail.

## First Message Protocol

On session start, before routing:
```python
fetch_inbox(project_key, agent_name)  # Load context from prior sessions
```

## Intent → Agent Routing

| Intent Keywords | Agent Type | Description |
|-----------------|------------|-------------|
| `research`, `find`, `locate`, `where is` | Research | Codebase exploration |
| `review`, `check`, `audit`, `security` | Review | Code/security review |
| `plan`, `design`, `architect`, `structure` | Planning | Design decisions |
| `implement`, `build`, `create`, `add` | Execution | Code implementation |
| `fix`, `debug`, `investigate`, `trace` | Debug | Bug investigation |
| `test`, `verify`, `validate` | Testing | Test creation/verification |
| `refactor`, `improve`, `optimize` | Refactor | Code improvement |
| `document`, `explain`, `describe` | Docs | Documentation |
| `analyze`, `understand`, `how does` | Analysis | Code comprehension |
| `migrate`, `upgrade`, `convert` | Migration | Version/format updates |
| `configure`, `setup`, `install` | Config | Environment setup |
| `deploy`, `release`, `ship` | Deploy | Deployment tasks |
| `monitor`, `log`, `track` | Observability | Monitoring setup |
| `benchmark`, `performance`, `profile` | Performance | Performance analysis |
| `integrate`, `connect`, `hook` | Integration | System integration |

## Spawn Pattern

```python
Task(
    description=f"""You are {agent_name}, a {agent_type} specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## Protocol
1. Do the work
2. Send summary via Agent Mail before returning:
   send_message(project_key, agent_name, to=["Orchestrator"],
     subject=f"Completed: {task_summary}",
     body_md=summary_template)
3. Return structured result

## CRITICAL
You MUST call send_message() before returning.
""",
    prompt=user_request
)
```

## Summary Protocol

Sub-agents MUST send this before returning:

```markdown
## Status
SUCCEEDED | PARTIAL | FAILED

## Files Changed
- path/to/file1.ts (added/modified/deleted)
- path/to/file2.ts (added/modified/deleted)

## Key Decisions
- Decision 1: rationale
- Decision 2: rationale

## Issues (if any)
- Issue description
```

## Main Thread Responsibilities

| Main Thread | Sub-Agent |
|-------------|-----------|
| Understand intent | File reading |
| Route to specialist | Code analysis |
| Display summaries | Implementation |
| Confirm destructive actions | Security review |
| Handle errors | Research |
| Aggregate results | Testing |

## Delegation Matrix

See [delegation.md](delegation.md) for full responsibility matrix.
