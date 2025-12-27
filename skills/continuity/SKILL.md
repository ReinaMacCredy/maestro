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

```
continuity load
```

**Behavior:**
1. Read `conductor/sessions/active/LEDGER.md` if exists
2. Check for stale ledger (>24h) → auto-archive if stale
3. Read most recent handoff from `conductor/sessions/archive/`
4. Display context summary

### continuity save

Update LEDGER.md with current state:

```
continuity save
```

**Sections updated:**
- `updated`: Current timestamp
- `State`: Current Done/Now/Next
- `Working Set`: Modified files
- `Key Decisions`: Any decisions made

### continuity handoff

Create handoff file and archive current session:

```
continuity handoff [reason]
```

**Reasons:** `manual` (default), `session-end`, `pre-compact`, `stale`

**Creates:** `conductor/sessions/archive/YYYY-MM-DD-HH-MM-<trigger>.md`

### continuity status

Display health check:

```
continuity status
```

**Shows:**
- Current LEDGER.md age
- Handoff count
- Last handoff date
- Index status (if artifact-index.db exists)

### continuity search

Search archived handoffs:

```
continuity search <query>
```

**Requires:** Python 3 + uv, SQLite with FTS5

**Uses:** `scripts/artifact-query.py`

## Data Storage

```
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
