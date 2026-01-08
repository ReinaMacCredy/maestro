# Agent Coordination

When Agent Mail CLI is available, agents can coordinate file access and share context.

## Session Protocol

**Session start:**
```bash
# Check inbox for context from previous sessions
toolboxes/agent-mail/agent-mail.js fetch-inbox \
  --project-key "$PROJECT_PATH" \
  --agent-name "$AGENT_NAME"
```

**Session end:**
```bash
# Send handoff message for next session
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "$PROJECT_PATH" \
  --sender-name "$SENDER_NAME" \
  --to '["RecipientAgent"]' \
  --subject "Handoff context" \
  --body-md "Context for next session..."
```

## Parallel Dispatch

Before dispatching parallel subagents:
1. Reserve files with `agent-mail.js file-reservation-paths`
2. Inject coordination block into Task prompts
3. Release reservations after completion

## Failure Handling

If Agent Mail CLI is unavailable:
- Proceed without coordination (work completion is mandatory)
- Show `⚠️ Agent coordination unavailable` warning
- Don't block on optional features
