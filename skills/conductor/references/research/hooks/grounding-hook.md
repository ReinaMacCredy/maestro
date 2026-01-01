# Grounding Hook — Replace Tiered Grounding with Research Agents

## Purpose

Replace the existing tiered grounding system with parallel research agents for more comprehensive verification.

## Trigger

At DEVELOP → DELIVER phase transition (replaces "Standard" tier grounding).

## Integration Point

```
DEVELOP phase complete
    │
    ▼
┌─────────────────────────┐
│  GROUNDING HOOK         │  ◄── THIS HOOK (replaces grounding.md)
│  Parallel verification  │
└─────────────────────────┘
    │
    ▼
DELIVER phase begins
(design verified)
```

## What This Replaces

### Old Grounding (Sequential)

```
finder → Grep → web_search
    ↓        ↓        ↓
  5s      10s      15s   = 30s total
```

### New Research (Parallel)

```
┌─────────────┬─────────────┬─────────────┐
│  Locator    │  Analyzer   │  Pattern    │  = 15s total
└─────────────┴─────────────┴─────────────┘
         ↓
    Synthesize (5s)
```

## Execution Protocol

### Step 1: Extract Verification Targets

From design developed so far:
- Components mentioned
- Patterns proposed
- Files to be modified
- External dependencies

### Step 2: Spawn Parallel Agents

| Agent | Task |
|-------|------|
| Locator | Verify proposed file locations exist |
| Analyzer | Confirm interfaces match design |
| Pattern | Verify proposed patterns match existing conventions |
| Web (if external deps) | Verify API/library documentation |

### Step 3: Calculate Confidence

```
┌─ VERIFICATION RESULT ──────────────────────┐
│ Phase: DEVELOP → DELIVER                   │
│ Agents: 4 spawned, 4 completed             │
│ Duration: 12s                              │
├────────────────────────────────────────────┤
│ VERIFIED:                                  │
│ ✓ [src/auth/jwt.ts] exists, interface OK   │
│ ✓ Error handling matches project pattern   │
│ ✓ Stripe API docs confirmed                │
├────────────────────────────────────────────┤
│ CONFLICTS:                                 │
│ ⚠ Design uses `AuthError`, codebase uses   │
│   `AuthenticationError` - recommend align  │
├────────────────────────────────────────────┤
│ Confidence: HIGH (3/4 verified)            │
└────────────────────────────────────────────┘
```

### Step 4: Enforcement

| Confidence | Action |
|------------|--------|
| HIGH | Proceed to DELIVER |
| MEDIUM | Warning, proceed |
| LOW | Block, require resolution |

## Enforcement Levels (Preserved)

| Phase Transition | Level | Behavior |
|------------------|-------|----------|
| DISCOVER→DEFINE | Advisory ⚠️ | Warn, proceed |
| DEFINE→DEVELOP | Advisory ⚠️ | Warn, proceed |
| DEVELOP→DELIVER | Gatekeeper 🚫 | Block if not run |
| DELIVER→Complete | Mandatory 🔒 | Block if low confidence |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| enabled | true | Enable research-based grounding |
| timeout | 15s | Max verification duration |
| max_agents | 5 | Parallel agent limit |
| min_confidence | MEDIUM | Required confidence to proceed |

## Backwards Compatibility

### Old Command Still Works

```
/ground <question>
```

Routes to research protocol instead of old grounding.

### Old Output Format Preserved

```
┌─ GROUNDING RESULT ─────────────────────┐
│ Tier: standard → research              │
│ ...                                    │
└────────────────────────────────────────┘
```

## Benefits Over Old Grounding

| Aspect | Old | New |
|--------|-----|-----|
| Speed | Sequential (30s) | Parallel (15s) |
| Coverage | Single query | Multi-aspect |
| Context | Isolated | Synthesized |
| Output | Answer | Verification report |

## Error Handling

| Error | Action |
|-------|--------|
| Timeout | Partial results, warn |
| Agent failure | Continue with others |
| All fail | Fallback to manual verify |
| Conflict detected | Display, require resolution |

## Related

- [protocol.md](../protocol.md) - Main research protocol
- [agents/](../../../../orchestrator/agents/) - Agent definitions
