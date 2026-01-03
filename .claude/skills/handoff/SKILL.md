---
name: handoff
description: >
  Hand off to a fresh session. Use when context is full, you've finished
  a logical chunk of work, or need a fresh perspective. Integrates with Maestro workflow.
version: "1.0.0"
author: "Adapted from Gas Town"
---

# Handoff - Session Cycling

Hand off your current session while preserving work context.

## Triggers

| Command | Action |
|---------|--------|
| `/handoff` | Auto-detect mode (create or resume) |
| `/handoff [message]` | Create handoff with context notes |
| `/handoff create` | Force CREATE mode |
| `/handoff resume` | Force RESUME mode |
| `/handoff resume <track>` | Resume specific track |

**Aliases:** `/conductor-handoff` → `/handoff`

## When to Use

- Context getting full (approaching token limit)
- Finished a logical chunk of work
- Need a fresh perspective on a problem
- Session cycling requested

## Quick Start

```bash
# Simple handoff
/handoff

# With context notes
/handoff "Found bug in auth.go line 145"

# Resume in new session
/handoff resume
```

## What Persists vs Resets

| Persists | Resets |
|----------|--------|
| Beads state (`bd`) | Conversation context |
| Git state | TodoWrite items |
| Handoff files | In-memory state |
| Agent Mail messages | |

## Implementation

See references for detailed workflows:
- [create.md](references/create.md) - CREATE mode (9 steps)
- [resume.md](references/resume.md) - RESUME mode (9 steps)
- [template.md](references/template.md) - Handoff file template

## Auto-Detect Logic

```
IF first_message AND recent_handoff_exists(<7d)
  → RESUME mode
ELSE IF handoff_exists(>=7d)
  → WARN "Stale handoff" → CREATE mode
ELSE
  → CREATE mode
```

## Directory Structure

```
conductor/handoffs/
├── <track-id>/
│   ├── index.md                    # Log of all handoffs
│   └── YYYY-MM-DD_HH-MM_*.md       # Individual handoffs
└── general/
    └── index.md
```

## Agent Mail Integration

- **Subject:** `[HANDOFF:<trigger>] <track> - <context>`
- **Thread:** `handoff-<track_id>`
- **Search:** `search_messages(query="HANDOFF")`

## Error Handling

| Scenario | Action |
|----------|--------|
| Agent Mail unavailable | Markdown-only |
| `bd` unavailable | Skip Beads sync |
| Stale handoff (>7d) | Warn, suggest fresh start |
| No handoffs found | Suggest `/handoff create` |

## Related Skills

- [beads](../beads/SKILL.md) - Issue tracking
- [maestro-core](../maestro-core/SKILL.md) - Workflow routing
