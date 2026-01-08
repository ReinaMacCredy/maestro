# Remember Facade

<!-- remember v2 -->

Quick entry point for handoff protocol operations.

## Primary Reference

→ [Handoff Skill](../../handoff/SKILL.md)

> Load the [designing skill](../../designing/SKILL.md) for session lifecycle

## Handoff Storage

| Aspect | Description |
|--------|-------------|
| Storage | `conductor/handoffs/<track>/` |
| Format | Structured JSON + Markdown summary |
| Audience | Same or different agent |
| TTL | 7 days (configurable) |

## Handoff Format

Save to: `conductor/handoffs/<track>/handoff_<timestamp>.json`

Required sections:
- **Intent** - What we're building and why
- **Progress** - Current state and completed work
- **Next Steps** - What remains to be done
- **Constraints & Ruled-Out** - What we decided NOT to do

→ [Template](handoff/template.md)

## Session Context

For within-session context, use `conductor/tracks/<track>/metadata.json`:
- Tracks current task, TDD phase, validation state
- Automatically updated by Conductor workflows

## See Also

- [Checkpoint Facade](checkpoint.md) - Progress checkpointing
- [Handoff Skill](../../handoff/SKILL.md) - Full handoff protocol

> Load the [designing skill](../../designing/SKILL.md) for session lifecycle (RECALL/REMEMBER)
