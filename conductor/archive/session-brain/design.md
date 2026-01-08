# Orchestrator Session Brain - Design

## Problem Statement

The Orchestrator skill lacks session awareness - multiple Amp sessions on the same project conflict silently because there's no auto-loading of context, no session identity management, and no coordination protocol between concurrent sessions.

## Solution

Add **Phase 0 (Preflight)** to Orchestrator workflow that:
- Auto-registers session identity with Agent Mail
- Detects active sessions via inbox analysis
- Warns on conflicts (track/files/beads)
- Prompts for stale session takeover

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Coordination mode | Always-on | Sessions should automatically participate |
| File reservations | Advisory (warn, don't block) | Don't break existing workflow |
| Session identity | Hybrid (`BlueLake-{timestamp}` internal, `BlueLake (session HH:MM)` display) | Unique + readable |
| Bead claiming | First wins, second sees "claimed by X" | Clear ownership |
| Context persistence | Auto-notify + lazy sync | Belt and suspenders |
| Session trigger | On `/conductor-implement`, `/conductor-orchestrate` | Starting commands only |
| `ds` behavior | Skip preflight | Design sessions always fresh |
| Stale threshold | 10 min inactive | Based on heartbeat protocol |
| Stale handling | Prompt: [T]ake over / [W]ait / [I]gnore | User decides |
| Agent Mail timeout | 3 seconds, then proceed with warning | Don't block on slow MCP |
| Scripts | Executable Python with JSON output | claudekit-skills pattern |
| Identity collision | Retry with incremented timestamp | Simple, reliable |
| Orphan cleanup | Auto-cleanup via message age | No manual intervention |
| Message window | Last 30 min only | Stale sessions irrelevant |
| Stuck beads on takeover | Prompt for manual, auto-reset on cleanup | Flexible |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ORCHESTRATOR PREFLIGHT                              │
│                         (Phase 0 - Session Brain)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  TRIGGER: /conductor-implement, /conductor-orchestrate                      │
│  SKIP:    ds, bd ready, bd show, bd list                                   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ STEP 1: IDENTITY                                                     │   │
│  │ • Generate session ID: {BaseAgent}-{timestamp}                       │   │
│  │ • Register with Agent Mail (persist in profile)                      │   │
│  │ • Store display name: "BlueLake (session 10:30)"                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              ↓                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ STEP 2: DETECT ACTIVE SESSIONS                                       │   │
│  │ • fetch_inbox() for recent messages (last 30 min)                    │   │
│  │ • Parse for [HEARTBEAT], [TRACK COMPLETE] subjects                   │   │
│  │ • Build active session list with tracks/files/beads                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              ↓                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ STEP 3: DISPLAY CONTEXT                                              │   │
│  │ • Show active sessions (if any)                                      │   │
│  │ • Warn on conflicts (track/files/beads)                              │   │
│  │ • Prompt for stale sessions (>10 min inactive)                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              ↓                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ STEP 4: PROCEED OR PROMPT                                            │   │
│  │ • No conflicts → proceed silently                                    │   │
│  │ • Conflicts → show warning, user chooses                             │   │
│  │ • Stale → takeover prompt [T]ake/[W]ait/[I]gnore                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              ↓                                              │
│              [EXISTING ORCHESTRATOR WORKFLOW Phase 1-7]                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Session Identity Model

```python
SESSION_IDENTITY = {
    "id": "BlueLake-1735689600",           # Internal (unique)
    "display": "BlueLake (session 10:30)", # Human readable
    "base_agent": "BlueLake",              # For grouping
    "created_ts": 1735689600,              # Unix epoch
    "track": "cc-v2-integration",          # Current track or null
    "beads_claimed": ["bd-101"],           # in_progress beads
    "files_reserved": ["src/api/**"],      # Active reservations
    "last_heartbeat": 1735690200,          # Last activity
    "status": "active"                     # active | stale | ended
}
```

## Session Lifecycle

```
START                    ACTIVE                      END
  │                        │                          │
  ▼                        ▼                          ▼
┌─────────┐  5 min   ┌───────────┐            ┌───────────┐
│ SESSION │ ──────── │ HEARTBEAT │ ────...──► │  SESSION  │
│  START  │          │           │            │    END    │
└─────────┘          └───────────┘            └───────────┘
     │                     │                        │
     │               >10 min gap                    │
     │                     ▼                        │
     │              ┌───────────┐                   │
     │              │   STALE   │                   │
     │              │ (takeover │                   │
     │              │  allowed) │                   │
     │              └───────────┘                   │
     │                                              │
     └──────────────────────────────────────────────┘
```

## File Structure

```
skills/orchestrator/
├── SKILL.md                          # MODIFY: Add Phase 0 section
├── scripts/                          # NEW: Executable Python scripts
│   ├── __init__.py
│   ├── preflight.py                  # Detect sessions, check conflicts
│   ├── session_identity.py           # ID generation, display formatting
│   ├── session_cleanup.py            # Auto-cleanup stale sessions
│   └── requirements.txt              # (empty - stdlib only)
├── references/
│   ├── workflow.md                   # MODIFY: Insert Phase 0
│   ├── preflight.md                  # NEW: Preflight protocol docs
│   ├── session-identity.md           # NEW: Identity format docs
│   └── patterns/
│       └── session-lifecycle.md      # MODIFY: Multi-session awareness
└── agents/
    └── README.md                     # MODIFY: Document session brain role
```

## Script Pattern

Scripts follow claudekit-skills pattern:
- Executable with shebang (`#!/usr/bin/env python3`)
- CLI with argparse subcommands
- JSON output for Claude to parse
- stdlib only (no external dependencies)
- Under 200 lines each

### preflight.py

```python
#!/usr/bin/env python3
"""
Orchestrator Session Preflight - Detect active sessions and conflicts.

Usage:
    python preflight.py detect --inbox-json <json>
    python preflight.py format-sessions --sessions-json <json>

Output: JSON with active_sessions, conflicts, recommendations
"""
```

## Conflict Handling

### Display Format

```
┌─ ACTIVE SESSIONS ──────────────────────────────────────────┐
│                                                            │
│ 🟢 BlueLake (session 10:30) - active                       │
│    Track: cc-v2-integration                                │
│    Beads: bd-101 (in_progress)                             │
│    Files: src/api/**                                       │
│    Last seen: 2 min ago                                    │
│                                                            │
│ 🟡 GreenCastle (session 09:15) - stale (12 min)            │
│    Track: auto-orchestrate                                 │
│    Beads: bd-201 (in_progress)                             │
│    Files: skills/orchestrator/**                           │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Conflict Prompt

```
┌─ CONFLICTS DETECTED ───────────────────────────────────────┐
│                                                            │
│ ⚠️  TRACK CONFLICT                                         │
│     BlueLake (session 10:30) is already on cc-v2-integration│
│                                                            │
│ Options:                                                   │
│ [P]roceed anyway - work on different files/beads           │
│ [S]witch track - pick a different track                    │
│ [W]ait - let other session finish first                    │
└────────────────────────────────────────────────────────────┘
```

### Stale Takeover Prompt

```
┌─ STALE SESSION DETECTED ───────────────────────────────────┐
│                                                            │
│ GreenCastle (session 09:15) inactive for 12 minutes        │
│                                                            │
│ Reserved files: skills/orchestrator/**                     │
│ Claimed beads: bd-201 (in_progress)                        │
│                                                            │
│ ⚠️  Warning: May have uncommitted work                      │
│                                                            │
│ [T]ake over - release reservations, reset beads to open    │
│ [W]ait - check again in 5 min                              │
│ [I]gnore - proceed without their files/beads               │
└────────────────────────────────────────────────────────────┘
```

## Acceptance Criteria

- [ ] Session 1 starts → registers, shows "no active sessions"
- [ ] Session 2 starts → shows Session 1 context
- [ ] Same track → warns "track conflict"
- [ ] Same bead claimed → shows "claimed by X"
- [ ] Session 1 stale (>10 min) → Session 2 sees takeover prompt
- [ ] Takeover accepted → beads reset to open, reservations released
- [ ] `ds` command → skips preflight entirely
- [ ] Agent Mail slow (>3s) → warns, proceeds without coordination

## Edge Cases Handled

| Edge Case | Solution |
|-----------|----------|
| Race condition on ID | Retry with incremented timestamp if name taken |
| Orphaned sessions | Auto-cleanup via message age (>10 min = stale) |
| Message volume | Only check last 30 min window |
| Stuck beads | Prompt for manual takeover, auto-reset on cleanup |
| Identity collision | Use millisecond timestamp, retry on conflict |

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Agent Mail required | LOW | Graceful fallback, 3s timeout |
| Message volume | LOW | 30 min window, stale cleanup |

## Party Mode Review

Reviewed by Winston (Architect), Amelia (Developer), Murat (Test Architect).

**Consensus:**
- Design is solid, Agent Mail as source of truth is right
- Hybrid identity approach approved
- Preflight as Phase 0 makes sense

**Incorporated recommendations:**
- Scripts are executable (claudekit-skills pattern)
- Use last_seen from any message type for stale detection
- Preflight is stateless (read stdin, output JSON, exit)
- Manual test script documented

## Next Steps

Run `/conductor-newtrack session-brain` to generate spec.md and plan.md.
