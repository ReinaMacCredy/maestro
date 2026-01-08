# Track Thread Protocol

Bead-to-bead context preservation using self-addressed messages.

## Two-Thread Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    TWO-THREAD ARCHITECTURE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  EPIC THREAD                      TRACK THREAD                  │
│  thread_id: <epic-id>             thread_id: track:<agent>:<epic>│
│  ─────────────────                ───────────────────────────── │
│  • Progress reports               • Bead-to-bead learnings      │
│  • Blockers                       • Gotchas discovered          │
│  • Cross-track issues             • Next bead hints             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Thread ID Format

| Thread Type | Format | Example |
|-------------|--------|---------|
| Epic | `<epic-id>` | `my-workflow:3-ktgt` |
| Track | `track:<agent>:<epic>` | `track:BlueLake:my-workflow:3-ktgt` |

## Per-Bead Loop

### 1. START BEAD
```bash
# Read prior context from track thread
toolboxes/agent-mail/agent-mail.js summarize-thread \
  --project-key "$PROJECT_PATH" \
  --thread-id "track:$AGENT:$EPIC"

# Reserve files
toolboxes/agent-mail/agent-mail.js file-reservation-paths \
  --project-key "$PROJECT_PATH" \
  --agent-name "$AGENT" \
  --paths '["<file-scope>"]' \
  --reason "<bead-id>"

# Claim bead
bd update <bead-id> --status in_progress
```

### 2. WORK ON BEAD
- Implement requirements
- Check inbox periodically
- Escalate blockers to epic thread

### 3. COMPLETE BEAD
```bash
# Close bead
bd close <bead-id> --reason completed

# Report to orchestrator (epic thread)
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "$PROJECT_PATH" \
  --sender-name "$AGENT" \
  --to '["<Orchestrator>"]' \
  --thread-id "<epic-id>" \
  --subject "[<bead-id>] COMPLETE" \
  --body-md "Done: <summary>. Next: <next-bead-id>"

# Save context for next bead (track thread - self message)
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "$PROJECT_PATH" \
  --sender-name "$AGENT" \
  --to '["<self>"]' \
  --thread-id "track:$AGENT:$EPIC" \
  --subject "<bead-id> Complete - Context for next" \
  --body-md "## Learnings
- What worked well
- What was tricky

## Gotchas
- Edge cases discovered
- Things to avoid

## Next Notes
- Context for next bead
- Dependencies or setup needed"

# Release files
toolboxes/agent-mail/agent-mail.js release-file-reservations \
  --project-key "$PROJECT_PATH" \
  --agent-name "$AGENT"
```

### 4. NEXT BEAD
- Loop to START with next bead
- **Read track thread for context!**

## Context Structure

Messages to track thread MUST include:

| Section | Purpose |
|---------|---------|
| Learnings | What worked, patterns discovered |
| Gotchas | Pitfalls, edge cases, things to avoid |
| Next Notes | Context specifically for next bead |

## Example

```bash
# Worker BlueLake after completing bead bd-11
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "$PROJECT_PATH" \
  --sender-name "BlueLake" \
  --to '["BlueLake"]' \
  --thread-id "track:BlueLake:my-workflow:3-ktgt" \
  --subject "bd-11 Complete - Context for bd-12" \
  --body-md "## Learnings
- Stripe SDK requires raw body for webhook verification
- Use stripe.webhooks.constructEvent() not manual parsing

## Gotchas
- STRIPE_WEBHOOK_SECRET env var must be set in test env
- Don't JSON.parse the body before verification

## Next Notes
- bd-12 needs to implement the actual event handlers
- checkout.session.completed handler should create subscription record
- Reference spike code in conductor/spikes/billing-spike/webhook-test/"
```

## Benefits

1. **No context loss** - Each bead picks up where last left off
2. **Learnings persist** - Gotchas discovered early prevent issues later
3. **Spike references** - Track thread can point to spike code
4. **Searchable** - Agent Mail FTS5 can find historical context

## Related

- [worker-prompt.md](worker-prompt.md) - Worker protocol
- [workflow.md](workflow.md) - Full orchestrator workflow
- [agent-mail.md](agent-mail.md) - Messaging protocol
