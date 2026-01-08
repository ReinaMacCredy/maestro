# Thin Router

Main thread stays clean: understand intent → route to specialist → display summary. Sub-agents do actual work and report via Agent Mail.

## First Message Protocol

On session start, before routing:
```bash
# Load context from prior sessions
toolboxes/agent-mail/agent-mail.js fetch-inbox \
  --project-key "$PROJECT_PATH" \
  --agent-name "$AGENT_NAME"
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

```bash
Task(
    description=f"""You are {agent_name}, a {agent_type} specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## Protocol
1. Do the work
2. Send summary via Agent Mail CLI before returning:
   toolboxes/agent-mail/agent-mail.js send-message \
     --project-key "$PROJECT_PATH" \
     --sender-name "$AGENT_NAME" \
     --to '["Orchestrator"]' \
     --subject "Completed: {task_summary}" \
     --body-md "$SUMMARY"
3. Return structured result

## CRITICAL
You MUST send message via Agent Mail CLI before returning.
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

See [agent-routing.md](agent-routing.md) for responsibility matrix and spawn patterns.
