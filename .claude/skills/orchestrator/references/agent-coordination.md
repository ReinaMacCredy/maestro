# Agent Coordination

When `agent_mail` MCP is available, agents can coordinate file access and share context.

## Session Protocol

**Session start:**
```bash
# Check inbox for context from previous sessions
fetch_inbox(project_key, agent_name)
```

**Session end:**
```bash
# Send handoff message for next session
send_message(project_key, sender_name, to, subject, body_md)
```

## Parallel Dispatch

Before dispatching parallel subagents:
1. Reserve files with `file_reservation_paths`
2. Inject coordination block into Task prompts
3. Release reservations after completion

## Failure Handling

If MCP is unavailable:
- Proceed without coordination (work completion is mandatory)
- Show `⚠️ Agent coordination unavailable` warning
- Don't block on optional features
