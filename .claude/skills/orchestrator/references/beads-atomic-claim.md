# Beads Atomic Claim (Conditional Updates)

> **Purpose**: Document conditional update mechanism for race-free bead claiming in parallel environments.

## Overview

When multiple workers attempt to claim the same bead simultaneously, race conditions can occur. The `--expect-status` flag enables atomic conditional updates that fail gracefully when preconditions aren't met.

## CLI Interface

### Expect-Status Flag

```bash
# Only update if bead is currently in expected status
bd update <bead_id> --status in_progress --expect-status open

# Claim with assignment atomically
bd update <bead_id> --status in_progress --assignee PinkHill --expect-status open
```

### Behavior

| Current Status | Expected | New Status | Result |
|---------------|----------|------------|--------|
| `open` | `open` | `in_progress` | ✅ Success |
| `in_progress` | `open` | `in_progress` | ❌ Fail (status mismatch) |
| `closed` | `open` | `in_progress` | ❌ Fail (status mismatch) |

## Race Condition Handling

### The Problem

Without conditional updates:

```
Time    Worker A                    Worker B
────────────────────────────────────────────────
T1      bd show bead-1 → open       bd show bead-1 → open
T2      bd update --status in_prog  bd update --status in_prog
T3      ✓ Success                   ✓ Success (!)
        
Result: Both workers think they own bead-1 → conflict
```

### The Solution

With `--expect-status`:

```
Time    Worker A                              Worker B
─────────────────────────────────────────────────────────────────
T1      bd update --expect-status open        bd update --expect-status open
T2      ✓ Success (was open)                  ❌ Fail (now in_progress)
T3      (proceeds with work)                  (tries next bead)

Result: Only one worker claims bead-1 → no conflict
```

## Failure Scenarios

### Exit Codes

| Code | Meaning | Action |
|------|---------|--------|
| `0` | Success | Proceed with work |
| `1` | Status mismatch | Skip bead, try next |
| `2` | Bead not found | Log error, continue |
| `3` | Network/DB error | Retry with backoff |

### Error Output

```bash
$ bd update bead-1 --status in_progress --expect-status open
Error: Precondition failed - expected status 'open', found 'in_progress'
Exit code: 1
```

### JSON Error Response

```json
{
  "success": false,
  "error": "precondition_failed",
  "expected_status": "open",
  "actual_status": "in_progress",
  "bead_id": "my-workflow:3-zyci.3.3"
}
```

## Worker Claiming Protocol

### Recommended Pattern

```bash
#!/bin/bash
# Worker claiming protocol with retry

BEADS=$(bd list --assignee=self --status open --json | jq -r '.[].id')

for BEAD_ID in $BEADS; do
  # Atomic claim attempt
  if bd update "$BEAD_ID" --status in_progress --expect-status open; then
    echo "Claimed: $BEAD_ID"
    # Do work...
    bd close "$BEAD_ID" --reason completed
  else
    echo "Skipped: $BEAD_ID (already claimed)"
  fi
done
```

### With Assignment Verification

```bash
# Double-check: verify assignment before claiming
ASSIGNEE=$(bd show "$BEAD_ID" --json | jq -r '.assignee')
if [ "$ASSIGNEE" = "$MY_AGENT_NAME" ]; then
  bd update "$BEAD_ID" --status in_progress --expect-status open
fi
```

## Orchestrator Usage

### Pre-Assignment Flow

```
┌──────────────┐
│ Orchestrator │
└──────┬───────┘
       │
       ▼ (1) Pre-assign to worker
┌──────────────────────────────────────────┐
│ bd update bead-1 --assignee WorkerA      │
└──────┬───────────────────────────────────┘
       │
       ▼ (2) Spawn worker
┌──────────────┐
│   Worker A   │
└──────┬───────┘
       │
       ▼ (3) Query assigned beads
┌──────────────────────────────────────────┐
│ bd list --assignee=self --status open    │
└──────┬───────────────────────────────────┘
       │
       ▼ (4) Atomic claim
┌──────────────────────────────────────────────────────────┐
│ bd update bead-1 --status in_progress --expect-status open │
└──────────────────────────────────────────────────────────┘
```

### Recovery After Stale Detection

```bash
# Witness patrol detects stale bead
bd list --stale=1h --json

# Force reassign with atomic update
bd update stale-bead --assignee NewWorker --expect-status in_progress
```

## Combining with Other Flags

```bash
# Full atomic claim with all fields
bd update bead-1 \
  --status in_progress \
  --assignee PinkHill \
  --expect-status open \
  --notes "Starting work on documentation"
```

## Related

- [beads-assignee.md](beads-assignee.md) - Pre-assignment prevents most races
- [beads-stale.md](beads-stale.md) - Detect abandoned claims for recovery
- [agent-coordination.md](agent-coordination.md) - Worker coordination patterns
