# Execution Routing

Execution routing determines how work is dispatched based on complexity.

## Routing Tiers

### Tier 1: Weighted Scoring
Score-based routing using complexity metrics:
- **SPEED** (<4): Single-agent, immediate execution
- **ASK** (4-6): Confirm approach before proceeding
- **FULL** (>6): Full design session required

### Tier 2: Compound Conditions
Context-aware routing:
- SINGLE_AGENT: Default for most work
- PARALLEL_DISPATCH: When 2+ independent tasks identified
- AUTONOMOUS: `ca` explicit OR `ralph.enabled == true`

## Integration

See [session-lifecycle.md](session-lifecycle.md) for full workflow context.
