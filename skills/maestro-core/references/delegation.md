# Delegation Matrix

> **Clear separation of responsibilities between main thread and sub-agents.**

## Main Thread Responsibilities

The main thread acts as a router/orchestrator. It stays clean with minimal context.

| Responsibility | Description |
|----------------|-------------|
| **Understand intent** | Parse user request, identify task type |
| **Route to specialist** | Spawn appropriate sub-agent(s) |
| **Display summaries** | Show sub-agent results to user |
| **Confirm destructive actions** | Gate dangerous operations (delete, force-push) |
| **Handle errors** | Aggregate failures, present options |
| **Aggregate results** | Combine multi-agent outputs |
| **Track progress** | Monitor sub-agent completion |
| **Manage context** | Load inbox, create handoffs |

## Sub-Agent Responsibilities

Sub-agents are specialists. They do the actual work and report back.

| Responsibility | Description |
|----------------|-------------|
| **File reading** | Read and analyze source code |
| **Code analysis** | Understand implementation details |
| **Implementation** | Write new code, modify existing |
| **Security review** | Audit for vulnerabilities |
| **Research** | Explore codebase, find patterns |
| **Testing** | Create and run tests |
| **Documentation** | Write docs, update comments |
| **Refactoring** | Improve code structure |

## Responsibility Matrix

| Task | Main Thread | Sub-Agent |
|------|-------------|-----------|
| Parse user request | ✅ | ❌ |
| Route to specialist | ✅ | ❌ |
| Read files | ❌ | ✅ |
| Write files | ❌ | ✅ |
| Claim beads (bd update) | ✅ | ✅ (in orchestrator mode) |
| Close beads (bd close) | ✅ | ✅ (in orchestrator mode) |
| Reserve files | ❌ | ✅ |
| Send Agent Mail | ❌ | ✅ (required) |
| Display results | ✅ | ❌ |
| Confirm dangerous actions | ✅ | ❌ |
| Run tests | ❌ | ✅ |
| Git operations | ❌ | ✅ |

## Communication Flow

```
User Request
     │
     ▼
┌─────────────────┐
│   Main Thread   │
│  (Understands)  │
└────────┬────────┘
         │ Task() spawn
         ▼
┌─────────────────┐
│   Sub-Agent     │
│  (Does work)    │
└────────┬────────┘
         │ send_message()
         ▼
┌─────────────────┐
│   Agent Mail    │
│  (Stores)       │
└────────┬────────┘
         │ fetch_inbox()
         ▼
┌─────────────────┐
│   Main Thread   │
│  (Summarizes)   │
└────────┬────────┘
         │
         ▼
     User Response
```

## Sub-Agent Reporting

Every sub-agent MUST call `send_message()` before returning:

```python
send_message(
    project_key=project_key,
    sender_name=agent_name,
    to=["Orchestrator"],
    subject=f"Completed: {task_summary}",
    body_md="""
## Status
SUCCEEDED

## Files Changed
- path/to/file.ts (modified)

## Key Decisions
- Decision: rationale

## Issues (if any)
None
"""
)
```

## Anti-Patterns

| ❌ Don't | ✅ Do Instead |
|----------|---------------|
| Main thread reads many files | Spawn Research sub-agent |
| Main thread writes code | Spawn Execution sub-agent |
| Sub-agent displays results | Sub-agent sends via Agent Mail |
| Skip Agent Mail reporting | Always call send_message() |
| Main thread runs tests | Spawn Testing sub-agent |
| Sub-agent asks user questions | Return structured result, main handles |
