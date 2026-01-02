# Beads Assignee Field

> **Purpose**: Document the assignee field requirements for parallel orchestration.

## Schema Changes

### New Fields

| Field | Type | Description |
|-------|------|-------------|
| `assignee` | `string \| null` | Agent name assigned to this bead |
| `assigned_at` | `datetime \| null` | Timestamp when assignment occurred |

### Database Schema

```sql
ALTER TABLE beads ADD COLUMN assignee TEXT;
ALTER TABLE beads ADD COLUMN assigned_at TIMESTAMP;
CREATE INDEX idx_beads_assignee ON beads(assignee);
```

### JSON Representation

```json
{
  "id": "my-workflow:3-zyci.3.1",
  "title": "Document assignee field requirements",
  "status": "in_progress",
  "assignee": "PinkHill",
  "assigned_at": "2026-01-03T01:59:19.737688+00:00"
}
```

## CLI Interface

### Assignment Flag

```bash
# Assign during update
bd update <bead_id> --assignee <agent_name>

# Assign during claim
bd update <bead_id> --status in_progress --assignee PinkHill

# Clear assignment
bd update <bead_id> --assignee ""
```

### Self-Assignment Shortcut

```bash
# Assign to current agent (requires session context)
bd update <bead_id> --assignee=self
```

## Query Patterns

### Filter by Assignee

```bash
# List beads assigned to specific agent
bd list --assignee PinkHill

# List beads assigned to current session agent
bd list --assignee=self

# List unassigned beads (ready for claiming)
bd list --assignee=none --status open

# Combine with status
bd list --assignee PinkHill --status in_progress
```

### JSON Output

```bash
bd list --assignee PinkHill --json
```

Returns array with assignee fields populated.

## Orchestrator Integration

### Worker Assignment Protocol

1. **Pre-assignment by Orchestrator**:
   ```bash
   # Orchestrator assigns bead to worker before spawning
   bd update my-workflow:3-zyci.3.1 --assignee PinkHill
   ```

2. **Worker Claims Own Beads**:
   ```bash
   # Worker queries assigned beads
   bd list --assignee=self --status open
   
   # Worker claims and starts work
   bd update <bead_id> --status in_progress
   ```

3. **Verification on Complete**:
   ```bash
   # Orchestrator verifies assignment before accepting
   bd show <bead_id> --json | jq '.assignee'
   ```

### Assignment Conflict Detection

When two workers attempt to claim the same bead:

```bash
# Worker A claims
bd update <bead_id> --assignee WorkerA

# Worker B queries their assignments
bd list --assignee=self  # Won't see the bead

# Prevents parallel claim conflicts
```

## Related

- [beads-atomic-claim.md](beads-atomic-claim.md) - Conditional updates for race handling
- [beads-stale.md](beads-stale.md) - Detecting abandoned assignments
- [workflow.md](workflow.md) - Full orchestration workflow
