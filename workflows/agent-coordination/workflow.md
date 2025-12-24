# Agent Coordination Workflow

Enable parallel agents to avoid file collisions and share context via agent_mail MCP.

## When This Applies

- Dispatching parallel subagents via Task tool
- Multiple terminal sessions on same codebase
- Handoff between sessions

## Core Protocol

1. **Coordinator reserves files** before dispatch
2. **Subagents receive coordination block** in prompt
3. **Conflicts = warn + skip** (optimistic approach)
4. **Coordinator releases** on completion
5. **Session end = handoff message** to inbox

## Patterns

| Pattern | Purpose |
|---------|---------|
| [parallel-dispatch](patterns/parallel-dispatch.md) | Reserve → dispatch → release flow |
| [subagent-prompt](patterns/subagent-prompt.md) | Coordination block for Task prompts |
| [session-lifecycle](patterns/session-lifecycle.md) | Session start/end handoff |
| [graceful-fallback](patterns/graceful-fallback.md) | Handle MCP failures |

## Examples

- [dispatch-three-agents](examples/dispatch-three-agents.md) - Annotated parallel dispatch

## Failure Modes

| Failure | Response |
|---------|----------|
| MCP unreachable | Warn, proceed without coordination |
| Reservation conflict | Warn, skip file |
| Stale reservation | TTL expires, auto-releases |

## Visible Feedback

Users see what's happening without needing to act:

```
🔒 Reserved: skills/foo/SKILL.md, skills/bar/SKILL.md (1h)
Dispatching 3 agents...
```

```
🔓 Released reservations
```

```
⚠️ Agent coordination unavailable - proceeding without file locks
```

## Verification

After implementing coordination:

1. **Collision test**: Dispatch 2 agents to same file → one warns about conflict
2. **Failure test**: Kill MCP mid-session → workflow continues with warning
3. **Handoff test**: End session, start new → inbox has handoff message
4. **Feedback test**: Check `🔒 Reserved` and `🔓 Released` appears in output
