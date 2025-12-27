---
name: continuity
description: Session state preservation across sessions and compactions. Use when starting/ending sessions, before compaction, or when searching session history. Replaces session-compaction skill.
metadata:
  version: "1.0.0"
---

# Continuity

Automatic session state preservation across sessions and compactions.

## Triggers

- `continuity load` / `load context` - Load LEDGER.md + last handoff
- `continuity save` / `save state` - Update LEDGER.md
- `continuity handoff` / `create handoff` - Archive current state
- `continuity status` - Display health check
- `continuity search <query>` - Search archived handoffs

## Platform Behavior

### Claude Code (Automatic via Hooks)

When hooks are installed at `~/.claude/hooks/`:

| Event | Hook | Action |
|-------|------|--------|
| Session start | SessionStart | Auto-load LEDGER.md + last handoff |
| Before compact | PreCompact | Auto-create handoff |
| File edit | PostToolUse | Track modified files |
| Session end | Stop | Archive session |

### Amp Code (Manual with Reminders)

- Run `continuity load` at session start
- Run `continuity save` after significant changes
- Run `continuity handoff` before ending session
- amp.hooks remind you to save after file edits

### Codex (Manual Only)

- Run commands manually
- No automation available

## Commands

### continuity load

Load session context from LEDGER.md and last handoff:

```bash
continuity load
```

**Behavior:**
1. Read `conductor/sessions/active/LEDGER.md` if exists
2. Check for stale ledger (>24h) → auto-archive if stale
3. Read most recent handoff from `conductor/sessions/archive/`
4. Display context summary

### continuity save

Update LEDGER.md with current state:

```bash
continuity save
```

**Sections updated:**
- `updated`: Current timestamp
- `State`: Current Done/Now/Next
- `Working Set`: Modified files
- `Key Decisions`: Any decisions made

### continuity handoff

Create handoff file and archive current session:

```bash
continuity handoff [reason]
```

**Reasons:** `manual` (default), `session-end`, `pre-compact`, `stale`

**Creates:** `conductor/sessions/archive/YYYY-MM-DD-HH-MM-<trigger>.md`

### continuity status

Display health check:

```bash
continuity status
```

**Shows:**
- Current LEDGER.md age
- Handoff count
- Last handoff date
- Index status (if artifact-index.db exists)

### continuity search

Search archived handoffs:

```bash
continuity search <query>
```

**Requires:** Python 3 + uv, SQLite with FTS5

**Uses:** `scripts/artifact-query.py`

## Data Storage

```text
conductor/
├── sessions/
│   ├── active/
│   │   └── LEDGER.md        # Current session state (gitignored)
│   └── archive/
│       └── *.md             # Archived handoffs (committed)
└── .cache/
    └── artifact-index.db    # SQLite FTS5 index (gitignored)
```

## LEDGER.md Format

See [references/ledger-format.md](references/ledger-format.md)

### Conductor Session Fields

When integrated with Conductor, LEDGER.md frontmatter includes session state:

| Field | Type | Description |
|-------|------|-------------|
| `bound_track` | string \| null | Currently active track ID |
| `bound_bead` | string \| null | Currently claimed bead/task ID |
| `mode` | `SA` \| `MA` | Session mode (Single-Agent or Multi-Agent) |
| `tdd_phase` | `RED` \| `GREEN` \| `REFACTOR` \| null | Current TDD phase |
| `heartbeat` | ISO 8601 | Last activity timestamp |

These fields replace the deprecated `.conductor/session-state_*.json` files.

## Handoff Format

See [references/handoff-format.md](references/handoff-format.md)

## Amp Setup

See [references/amp-setup.md](references/amp-setup.md)

## Installing Hooks (Claude Code)

```bash
./scripts/install-global-hooks.sh
```

This installs TypeScript hooks to `~/.claude/hooks/`.

## Graceful Degradation

- Hooks never crash Claude (try/catch + exit 0)
- Missing directories created on demand
- Missing LEDGER.md starts fresh session
- Missing index prompts regeneration

## Concurrent Sessions

**Known limitation:** Multiple concurrent sessions on same codebase may conflict.

**Mitigation:** Each session uses timestamped handoffs. Last writer wins for LEDGER.md.

## Stale Detection

Ledgers older than 24 hours are automatically archived on next session start.

## Conductor Integration

Continuity is automatically chained into Conductor workflows.

### Auto-Triggers

| Workflow | Phase | Action |
|----------|-------|--------|
| `/conductor-implement` | Phase 0.5 | `continuity load` + track binding |
| `/conductor-finish` | Phase 6.5 | `continuity handoff track-complete` |
| `ds` (Design Session) | Initialization | `continuity load` (display prior context) |

### Track Binding

When working on a Conductor track, LEDGER.md binds to that track:

| Event | Frontmatter Update |
|-------|-------------------|
| `/conductor-implement <track>` | `bound_track: <track>` |
| `bd update <id> --status in_progress` | `bound_bead: <id>` |
| `bd close <id>` | `bound_bead: null` |
| `/conductor-finish` | `bound_track: null, bound_bead: null` |

### Track Switch Auto-Archive

If you switch tracks (bound_track differs from new track):
1. Current LEDGER.md is auto-archived to `conductor/sessions/archive/`
2. New LEDGER.md is created with new track binding
3. Previous context is preserved in archive

### Non-Blocking Guarantee

Continuity operations never block Conductor commands:
- Missing LEDGER.md → create fresh session
- Corrupted LEDGER.md → archive and create fresh
- Archive write fails → log warning, continue

### See Also

- [ledger-format.md](references/ledger-format.md) - Full LEDGER.md format with Conductor fields
- [preflight-beads.md](../conductor/references/conductor/preflight-beads.md) - Session initialization
- [beads-session.md](../conductor/references/conductor/beads-session.md) - Session lifecycle
