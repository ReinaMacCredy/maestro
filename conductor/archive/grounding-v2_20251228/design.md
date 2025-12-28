# Grounding Trigger Implementation — Design Document

## Problem Statement

Grounding system has full documentation but **no trigger mechanism**. SKILL.md says "automatic" but Claude doesn't execute grounding because there are no explicit instructions at phase transitions.

**Root Cause:** Documentation describes behavior without actionable steps.

## Goals

1. **Inline Triggers** — Add explicit grounding execution steps to each phase transition
2. **State Tracking** — Track grounding completion across phases
3. **Enforcement Logic** — Implement halt/block behavior for Gatekeeper/Mandatory levels
4. **Graceful Degradation** — Handle timeouts and tool failures

## Success Criteria

| Metric | Current | Target |
|--------|---------|--------|
| Grounding executed at transitions | 0% | 100% |
| Gatekeeper blocks on skip | Never | Always |
| Mandatory blocks on low confidence | Never | Always |
| Timeout handling | Undefined | Graceful fallback |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              PHASE TRANSITION DETECTED                      │
│              (e.g., DEVELOP → DELIVER)                      │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              1. SELECT TIER                                 │
│  SPEED mode → Light | FULL mode → lookup transition         │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              2. EXECUTE GROUNDING                           │
│  Run tools per tier (Grep, finder, web_search)              │
│  Apply timeout (3s/5s/10s/45s based on tier)                │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              3. CALCULATE CONFIDENCE                        │
│  matches > 3 → HIGH | 1-3 → MEDIUM | 0 → LOW                │
│  timeout → MEDIUM (partial) | error → LOW                   │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              4. ENFORCE & DISPLAY                           │
│  Advisory → warn + proceed                                  │
│  Gatekeeper → block if not run                              │
│  Mandatory → block if LOW confidence                        │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. State Tracking

Track grounding completion in session context (Claude's working memory):

```
grounding_state = {
    "DISCOVER→DEFINE": { "completed": true, "confidence": "HIGH", "timestamp": "..." },
    "DEFINE→DEVELOP": { "completed": true, "confidence": "MEDIUM", "timestamp": "..." },
    "DEVELOP→DELIVER": null,  // Not yet reached
    "DELIVER→COMPLETE": null
}
```

**Display at each transition:**
```
┌─ GROUNDING STATE ──────────────────────────┐
│ ✓ DISCOVER→DEFINE: HIGH                    │
│ ✓ DEFINE→DEVELOP: MEDIUM                   │
│ ○ DEVELOP→DELIVER: pending                 │
│ ○ DELIVER→COMPLETE: pending                │
└────────────────────────────────────────────┘
```

### 2. Confidence Calculation (from tool results)

| Tool Result | Confidence |
|-------------|------------|
| `finder` returns 3+ matches | HIGH |
| `Grep` returns 1-3 matches | MEDIUM |
| `web_search` returns results | MEDIUM |
| No matches found | LOW |
| Tool timeout (soft limit) | MEDIUM + warning |
| Tool error/failure | LOW |

**Composite rule:** Best result from any tool used.

### 3. Timeout Handling

| Tier | Soft Limit | Hard Limit | Behavior |
|------|------------|------------|----------|
| Light | 3s | 5s | Return partial, proceed |
| Mini | 5s | 8s | Return partial, proceed |
| Standard | 10s | 15s | Return partial + warning |
| Full | 45s | 60s | Block, require manual verify |

**On timeout:**
```
┌─ GROUNDING TIMEOUT ────────────────────────┐
│ ⚠️ Timeout after 10s (Standard tier)       │
│ Partial results: 2 matches found           │
│ Confidence: MEDIUM (degraded)              │
│ Proceeding with warning...                 │
└────────────────────────────────────────────┘
```

### 4. Skip Behavior by Enforcement Level

| Level | User says "skip" | Behavior |
|-------|------------------|----------|
| **Advisory** ⚠️ | Allowed | Log warning, proceed |
| **Gatekeeper** 🚫 | Allowed with warning | Log, show warning banner, proceed |
| **Mandatory** 🔒 | Requires justification | Block until `SKIP_GROUNDING: <reason>` |

**Gatekeeper skip warning:**
```
┌─ GROUNDING SKIPPED ────────────────────────┐
│ ⚠️ Proceeding without grounding            │
│ Risk: Design may conflict with codebase    │
│ Logged for audit.                          │
└────────────────────────────────────────────┘
```

**Mandatory skip (requires explicit input):**
```
┌─ GROUNDING REQUIRED ───────────────────────┐
│ 🔒 Cannot skip at DELIVER→Complete         │
│                                            │
│ To override, type:                         │
│ SKIP_GROUNDING: <your justification>       │
└────────────────────────────────────────────┘
```

### 5. Edge Case Handling

#### Truncation (100+ matches)
```
┌─ GROUNDING (Mini) ──────────────────────────┐
│ Query: [problem summary]                    │
│ Found: 100+ matches (showing top 10)        │
│ Confidence: HIGH                            │
│ Note: Results truncated for display         │
└─────────────────────────────────────────────┘
```

#### Empty Justification Rejection
If user types `SKIP_GROUNDING:` or `SKIP_GROUNDING: ` (empty/whitespace):
```
┌─ INVALID JUSTIFICATION ────────────────────┐
│ ❌ Justification cannot be empty            │
│                                            │
│ Please provide a reason:                   │
│ SKIP_GROUNDING: <actual reason here>       │
└────────────────────────────────────────────┘
```

#### Conditional Tool Skipping
- **No external refs in design:** Skip `web_search`, use repo-only
- **No history context needed:** Skip `find_thread`
- Display which tools were skipped:
```
┌─ GROUNDING (Standard) ─────────────────────┐
│ Sources: repo ✓ | web ⊘ (no external refs) │
│ Confidence: HIGH                           │
└────────────────────────────────────────────┘
```

#### Loop-Back State Reset
When user says "revisit [PHASE]":
1. Reset grounding state for that transition and all subsequent
2. Display updated state:
```
┌─ GROUNDING STATE (reset) ──────────────────┐
│ ✓ DISCOVER→DEFINE: HIGH                    │
│ ○ DEFINE→DEVELOP: reset (was MEDIUM)       │
│ ○ DEVELOP→DELIVER: pending                 │
│ ○ DELIVER→COMPLETE: pending                │
└────────────────────────────────────────────┘
```

#### Network Failure (web_search fails)
```
┌─ GROUNDING (Standard, degraded) ───────────┐
│ Sources: repo ✓ | web ✗ (network error)    │
│ Confidence: MEDIUM (degraded)              │
│ Note: Web verification skipped             │
└────────────────────────────────────────────┘
```

## Deliverables

| File | Action | Description |
|------|--------|-------------|
| `skills/design/SKILL.md` | UPDATE | Add inline grounding triggers at phase transitions |

### SKILL.md Changes

Add after each phase section (e.g., after "### Phase 1: DISCOVER"):

```markdown
#### Transition: DISCOVER → DEFINE

**GROUNDING EXECUTION (Mini, Advisory ⚠️):**

1. **Run:** `finder` with query: "similar problems to [problem statement]"
2. **Calculate confidence:**
   - 3+ matches → HIGH
   - 1-3 matches → MEDIUM
   - 0 matches → LOW
3. **Display:**
   ```
   ┌─ GROUNDING (Mini) ──────────────────────┐
   │ Query: [problem summary]                │
   │ Found: [N] matches                      │
   │ Confidence: [HIGH/MEDIUM/LOW]           │
   │ Status: ✓ Complete                      │
   └─────────────────────────────────────────┘
   ```
4. **Proceed** to A/P/C checkpoint

---

#### Transition: DEVELOP → DELIVER (Gatekeeper 🚫)

**GROUNDING EXECUTION (Standard):**

1. **Run in sequence:**
   - `Grep` for patterns mentioned in design
   - `finder` for affected files
   - `web_search` if external APIs referenced
2. **Timeout:** 10s soft, 15s hard
3. **Display result block**
4. **HALT if skipped:**
   ```
   ┌─ GROUNDING REQUIRED ────────────────────┐
   │ 🚫 Cannot proceed without grounding     │
   │                                         │
   │ [R]un grounding  [S]kip with warning    │
   └─────────────────────────────────────────┘
   ```
5. **Only show A/P/C after grounding complete or user skips**

---

#### Transition: DELIVER → Complete (Mandatory 🔒)

**GROUNDING EXECUTION (Full + Impact Scan):**

1. **Run parallel:**
   - Full cascade: repo → web → history
   - Impact scan: `finder` for all files in design
2. **Timeout:** 45s soft, 60s hard
3. **Block if:**
   - Confidence = LOW
   - All sources failed
   - User must type `SKIP_GROUNDING: <reason>` to override
4. **Display:**
   ```
   ┌─ GROUNDING (Full) ──────────────────────┐
   │ Sources: repo ✓ | web ✓ | history ✓     │
   │ Impact: 12 files identified             │
   │ Confidence: HIGH                        │
   │ Status: ✓ Verified                      │
   └─────────────────────────────────────────┘
   ```
```

## Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| 1 | Grounding block displayed at DISCOVER→DEFINE | Manual test |
| 2 | Grounding block displayed at DEFINE→DEVELOP | Manual test |
| 3 | Grounding block displayed at DEVELOP→DELIVER | Manual test |
| 4 | DEVELOP→DELIVER halts if grounding not run | Manual test |
| 5 | User can skip Gatekeeper with warning | Manual test |
| 6 | DELIVER→Complete blocks on LOW confidence | Manual test |
| 7 | Mandatory skip requires explicit justification | Manual test |
| 8 | Timeout shows partial results + warning | Manual test |
| 9 | State tracking displays across phases | Manual test |
| 10 | 100+ matches shows truncation note | Manual test |
| 11 | Empty justification rejected | Manual test |
| 12 | Conditional tool skip when no external refs | Manual test |
| 13 | Loop-back resets subsequent grounding state | Manual test |
| 14 | Network failure shows degraded confidence | Manual test |

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Claude ignores grounding instructions | Medium | High | Make instructions explicit, test thoroughly |
| Tool timeouts slow down session | Medium | Medium | Soft limits + partial results |
| User confusion about enforcement | Low | Medium | Clear UI blocks with options |

## Open Questions

1. ~~State tracking~~ → Solved: in-session state object
2. ~~Confidence calculation~~ → Solved: match count heuristic
3. ~~Timeout handling~~ → Solved: soft/hard limits per tier

## Estimated Effort

~2 hours:
- SKILL.md updates: 1-1.5 hrs
- Testing: 30 min

## Design Session Notes

- Party Mode identified 4 gaps: state tracking, confidence calculation, timeout handling, skip behavior
- All gaps addressed in updated design
- Simplified confidence calculation from 4-factor weighted score to match count heuristic (pragmatic for Claude execution)
