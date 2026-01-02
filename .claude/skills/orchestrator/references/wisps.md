# Wisps - Ephemeral Beads

> **Purpose**: Document wisp requirements for temporary, session-scoped beads that don't persist to git.

## Overview

Wisps are ephemeral beads for transient work that shouldn't clutter the permanent issue history. They exist only for the current session and are automatically cleaned up.

## Schema

### Bead Extension

```yaml
# Standard bead fields plus:
ephemeral: true  # Marks bead as a wisp
```

### Example Wisp

```yaml
id: wisp-1704307200000
title: "Investigate flaky test"
status: in_progress
ephemeral: true
created_at: "2026-01-03T12:00:00Z"
```

## CLI Interface

### Creating Wisps

```bash
# Create a wisp (ephemeral bead)
bd create "Quick investigation" --wisp

# Create with parent (for patrol tasks)
bd create "Check stale workers" --wisp --parent my-workflow:3-zyci
```

### Burning Wisps

```bash
# Remove a single wisp permanently
bd burn <wisp-id>

# Burn all wisps (session cleanup)
bd burn --all
```

### Squashing Wisps

```bash
# Convert wisp to permanent bead
bd squash <wisp-id>

# Squash with new title
bd squash <wisp-id> --title "Resolved: flaky test root cause"

# Squash and attach to parent
bd squash <wisp-id> --parent epic-123
```

## Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│                     WISP LIFECYCLE                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   CREATE (--wisp)                                           │
│        │                                                    │
│        ▼                                                    │
│   ┌─────────┐                                               │
│   │  WISP   │ ─── ephemeral: true                           │
│   └────┬────┘                                               │
│        │                                                    │
│   ┌────┴────────────────────┐                               │
│   │                         │                               │
│   ▼                         ▼                               │
│ USE                      DISCARD                            │
│ (work on task)           (not needed)                       │
│   │                         │                               │
│   ▼                         ▼                               │
│ ┌───────────────┐     ┌───────────┐                         │
│ │ SQUASH        │     │ BURN      │                         │
│ │ (→ permanent) │     │ (delete)  │                         │
│ └───────────────┘     └───────────┘                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### State Transitions

| From | Action | To |
|------|--------|-----|
| - | `bd create --wisp` | Wisp exists |
| Wisp | `bd burn` | Deleted (no trace) |
| Wisp | `bd squash` | Permanent bead |
| Wisp | Session end (auto) | Deleted |

## Git Exclusion

### Behavior

Wisps are **never committed** to git:

```bash
# .gitignore entry (automatic)
.beads/wisps/

# Or in .beads/.gitignore
wisps/
```

### Storage Location

```
.beads/
├── issues/           # Permanent beads (git tracked)
├── wisps/            # Ephemeral beads (git ignored)
│   ├── wisp-123.yaml
│   └── wisp-456.yaml
└── index.yaml        # Only references permanent beads
```

### Sync Behavior

```bash
# bd sync excludes wisps
bd sync  # Only syncs permanent beads

# Explicit wisp cleanup
bd burn --all --force  # Remove all wisps
```

## Use Cases

### 1. Patrol Tasks

Witness patrol creates wisps for transient checks:

```bash
# Patrol creates wisp for investigation
bd create "Check stale bead xyz" --wisp

# If issue found, squash to permanent
bd squash wisp-123 --title "BUG: Worker PinkHill unresponsive"

# If no issue, burn it
bd burn wisp-123
```

### 2. Quick Investigations

```bash
# Developer investigating issue
bd create "Why is test flaky?" --wisp

# After investigation
bd burn wisp-456  # Just curious, no action needed
# OR
bd squash wisp-456 --title "Fix flaky test timing"  # Found real issue
```

### 3. Temporary Placeholders

```bash
# Placeholder while gathering requirements
bd create "TBD: API design" --wisp

# Once requirements clear
bd squash wisp-789 --title "Design: User API endpoints"
```

## Integration with Patrol

### Patrol Wisp Pattern

```python
# Witness patrol creates wisps for each check
def patrol_cycle():
    # Create wisp for this patrol run
    wisp = bd_create("Patrol cycle check", wisp=True)
    
    try:
        # Run checks
        stale = check_stale_tasks()
        unblocked = check_unblocked_tasks()
        
        if issues_found:
            # Squash to permanent bead for tracking
            bd_squash(wisp.id, title="PATROL: Found stale tasks")
        else:
            # No issues, burn the wisp
            bd_burn(wisp.id)
    except Exception:
        # Burn on error (transient)
        bd_burn(wisp.id)
```

## Configuration

### Default Wisp TTL

```yaml
# .beads/config.yaml
wisps:
  auto_burn_after: 24h  # Auto-burn after 24 hours
  max_count: 50         # Maximum wisps before warning
  burn_on_session_end: true
```

## Related

- [witness-patrol.md](witness-patrol.md) - Uses wisps for patrol tracking
- [heartbeat.md](heartbeat.md) - Wisp cleanup on worker exit
- [beads-stale.md](beads-stale.md) - Stale detection (wisps exempt)
