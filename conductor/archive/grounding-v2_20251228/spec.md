# Grounding Trigger Implementation — Specification

## Overview

Add explicit grounding execution instructions to the design skill's phase transitions, enabling automatic verification at each stage of the Double Diamond workflow.

## Problem

The grounding system has comprehensive documentation (tiers, enforcement levels, UI blocks) but Claude doesn't execute grounding because SKILL.md lacks explicit actionable instructions at phase transitions.

## Solution

Add inline grounding trigger blocks to `skills/design/SKILL.md` after each phase section, with:
- Tool execution steps
- Confidence calculation logic
- Enforcement behavior (halt/warn/block)
- UI display templates

## Functional Requirements

### FR-1: Grounding Execution at Phase Transitions

At each Double Diamond phase transition, execute grounding:

| Transition | Tier | Enforcement | Tools |
|------------|------|-------------|-------|
| DISCOVER→DEFINE | Mini | Advisory ⚠️ | finder |
| DEFINE→DEVELOP | Mini | Advisory ⚠️ | finder, Grep |
| DEVELOP→DELIVER | Standard | Gatekeeper 🚫 | Grep, finder, web_search |
| DELIVER→Complete | Full | Mandatory 🔒 | All + Impact Scan |

### FR-2: State Tracking

Track grounding completion across phases in session:
```
grounding_state = {
    "DISCOVER→DEFINE": { completed: bool, confidence: str, timestamp: str },
    ...
}
```

Display state block at each transition showing completed/pending status.

### FR-3: Confidence Calculation

Calculate from tool results:
- 3+ matches → HIGH
- 1-3 matches → MEDIUM
- 0 matches → LOW
- Timeout → MEDIUM (degraded)
- Error → LOW

### FR-4: Enforcement Logic

| Level | Skip Behavior |
|-------|---------------|
| Advisory | Allowed, log warning |
| Gatekeeper | Allowed with banner, log |
| Mandatory | Block until `SKIP_GROUNDING: <reason>` |

### FR-5: Timeout Handling

| Tier | Soft | Hard | Behavior |
|------|------|------|----------|
| Light | 3s | 5s | Partial + proceed |
| Mini | 5s | 8s | Partial + proceed |
| Standard | 10s | 15s | Partial + warning |
| Full | 45s | 60s | Block + manual verify |

### FR-6: Edge Cases

1. **Truncation:** 100+ matches → show "showing top 10"
2. **Empty justification:** Reject, require actual reason
3. **Conditional skip:** No external refs → skip web_search
4. **Loop-back:** Reset that transition + subsequent states
5. **Network failure:** Degrade confidence, show ✗

## Non-Functional Requirements

### NFR-1: Performance
- Light/Mini tiers complete in <5s typical
- Standard tier complete in <15s typical
- Full tier complete in <60s typical

### NFR-2: User Experience
- Clear UI blocks with consistent formatting
- Actionable options ([R]un, [S]kip, [C]ancel)
- Visible state tracking across phases

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| 1 | Grounding block displayed at DISCOVER→DEFINE |
| 2 | Grounding block displayed at DEFINE→DEVELOP |
| 3 | Grounding block displayed at DEVELOP→DELIVER |
| 4 | DEVELOP→DELIVER halts if grounding not run |
| 5 | User can skip Gatekeeper with warning |
| 6 | DELIVER→Complete blocks on LOW confidence |
| 7 | Mandatory skip requires explicit justification |
| 8 | Timeout shows partial results + warning |
| 9 | State tracking displays across phases |
| 10 | 100+ matches shows truncation note |
| 11 | Empty justification rejected |
| 12 | Conditional tool skip when no external refs |
| 13 | Loop-back resets subsequent grounding state |
| 14 | Network failure shows degraded confidence |

## Out of Scope

- Intent classification (v2 routing optimization)
- Weighted confidence scoring (simplified to match count)
- Persistent grounding logs (session-only for now)
- Grounding cache across sessions

## Dependencies

- `skills/design/SKILL.md` (target file)
- `skills/design/references/grounding/` (existing docs)
