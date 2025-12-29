---
name: continuity
description: "DEPRECATED - Use handoff system instead. This skill redirects to /create_handoff and /resume_handoff."
metadata:
  version: "2.0.0"
  deprecated: true
  replacement: "/create_handoff, /resume_handoff"
---

# Continuity (Deprecated)

> ⚠️ **This skill is deprecated.** Use the handoff system instead.

## Migration

The continuity skill (LEDGER.md-based session tracking) has been replaced by the HumanLayer-inspired handoff system.

### Old Commands → New Commands

| Old | New | Description |
|-----|-----|-------------|
| `continuity load` | `/resume_handoff` | Load prior session context |
| `continuity save` | `/create_handoff` | Save current session context |
| `continuity handoff` | `/create_handoff` (trigger: `manual`) | Create session handoff |

### Key Differences

1. **Git-committed** - Handoffs are committed to git (shareable with team)
2. **Standalone** - Works outside of Conductor workflows
3. **Structured** - YAML frontmatter with metadata (timestamp, trigger, git info)
4. **Per-track** - Handoffs organized by track in `conductor/handoffs/<track>/`

### New Commands

- `/create_handoff` - Create a handoff file with current context
- `/resume_handoff` - Find and load the most recent handoff
- `/conductor-handoff` - Alias for both (subcommand style)

### File Structure

```
conductor/handoffs/
├── general/                          # Non-track handoffs
│   ├── index.md
│   └── YYYY-MM-DD_HH-MM-SS-mmm_general_<trigger>.md
└── <track-id>/
    ├── index.md                      # Auto-generated log
    ├── YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
    └── archive/                      # After /conductor-finish
```

### Triggers

- `design-end` - After `/conductor-newtrack`
- `epic-start` - Before each CI epic
- `epic-end` - After each epic closes
- `pre-finish` - Start of `/conductor-finish`
- `manual` - User runs `/create_handoff`
- `idle` - After 30min inactivity gap

See [docs/handoff-system.md](../../docs/handoff-system.md) for complete documentation.
