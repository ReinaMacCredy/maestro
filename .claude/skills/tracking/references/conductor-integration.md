# Conductor Integration

When used with Conductor, beads operations are **automated via a facade pattern**.

## Facade Abstraction

Conductor commands call beads through a unified facade that:
- Validates bd availability
- Manages retry logic and error recovery
- Persists failed operations for later replay

**In the happy path, you never run manual bd commands** - Conductor handles:
- `preflight` → bd availability check
- `track-init` → create epic + issues from plan.md
- `claim` → bd update --status in_progress
- `close` → bd close --reason completed
- `sync` → bd sync with retry

## planTasks Mapping

`metadata.json.beads` contains bidirectional mapping between plan task IDs and bead IDs:

```json
{
  "beads": {
    "planTasks": { "1.1.1": "bd-42", "1.2.1": "bd-43" },
    "beadToTask": { "bd-42": "1.1.1", "bd-43": "1.2.1" }
  }
}
```

This enables:
- Track which plan tasks have beads
- Navigate from bead to plan context
- Detect orphan beads after plan revisions

## When Manual bd IS Appropriate

- Direct issue creation outside Conductor flow
- Ad-hoc queries (`bd search`, `bd list`)
- Debugging (`bd show <id>`)
- Recovery from failed automated operations

> **Cross-skill reference:** Load the [conductor](../../conductor/SKILL.md) skill for all 13 beads integration points.
