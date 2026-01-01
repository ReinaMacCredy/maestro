# Remember Facade

<!-- remember v1 -->

Quick entry point for handoff protocol operations.

## Primary Reference

→ [Handoff Protocol](beads-session.md#handoff-protocol-ma-mode)
→ [Anchored Format (SA Mode)](beads-session.md#anchored-format-sa-mode)

## Quick Reference: SA vs MA

| Aspect | SA Mode | MA Mode |
|--------|---------|---------|
| Storage | `.conductor/session-context.md` | `.conductor/handoff_*.json` |
| Format | Anchored markdown | Structured JSON |
| Audience | Same agent, future session | Different agent |
| TTL | Persistent | 24 hours |

## SA Mode (Anchored Format)

Save to: `.conductor/session-context.md`

Required [PRESERVE] sections:
- **Intent** - What we're building and why
- **Constraints & Ruled-Out** - What we decided NOT to do

→ [Template](../../design/references/anchored-state-format.md)

## MA Mode (JSON Handoff)

Save to: `.conductor/handoff_<from>_to_<to>.json`

Use when passing context to another agent.

## See Also

- [Session Lifecycle](../../design/references/session-lifecycle.md) - RECALL/REMEMBER
- [Checkpoint Facade](checkpoint.md) - Progress checkpointing
