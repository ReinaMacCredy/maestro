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
```python
# Read prior context from track thread
summary = summarize_thread(
  project_key="<path>",
  thread_id="track:<agent>:<epic>"
)

# Reserve files
file_reservation_paths(
  paths=["<file-scope>"],
  reason="<bead-id>"
)

# Claim bead
bash("bd update <bead-id> --status in_progress")
```

### 2. WORK ON BEAD
- Implement requirements
- Check inbox periodically
- Escalate blockers to epic thread

### 3. COMPLETE BEAD
```python
# Close bead
bash("bd close <bead-id> --reason completed")

# Report to orchestrator (epic thread)
send_message(
  to=["<Orchestrator>"],
  thread_id="<epic-id>",
  subject="[<bead-id>] COMPLETE",
  body_md="Done: <summary>. Next: <next-bead-id>"
)

# Save context for next bead (track thread - self message)
send_message(
  to=["<self>"],
  thread_id="track:<agent>:<epic>",
  subject="<bead-id> Complete - Context for next",
  body_md="""
## Learnings
- What worked well
- What was tricky

## Gotchas
- Edge cases discovered
- Things to avoid

## Next Notes
- Context for next bead
- Dependencies or setup needed
"""
)

# Release files
release_file_reservations()
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

```python
# Worker BlueLake after completing bead bd-11
send_message(
  to=["BlueLake"],  # Self-message
  thread_id="track:BlueLake:my-workflow:3-ktgt",
  subject="bd-11 Complete - Context for bd-12",
  body_md="""
## Learnings
- Stripe SDK requires raw body for webhook verification
- Use stripe.webhooks.constructEvent() not manual parsing

## Gotchas
- STRIPE_WEBHOOK_SECRET env var must be set in test env
- Don't JSON.parse the body before verification

## Next Notes
- bd-12 needs to implement the actual event handlers
- checkout.session.completed handler should create subscription record
- Reference spike code in conductor/spikes/billing-spike/webhook-test/
"""
)
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
