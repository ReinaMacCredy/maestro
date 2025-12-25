# Design: Context Engineering Integration

## Overview

Integrate context engineering patterns from [Agent Skills for Context Engineering](https://github.com/muratcankoylan/Agent-Skills-for-Context-Engineering) into the Conductor workflow system.

## Problem Statement

Current Conductor workflow lacks:
1. **Design routing** - All tasks go through full Double Diamond regardless of complexity
2. **Execution routing** - No intelligent decision between sequential vs parallel execution
3. **Context lifecycle** - No structured recall/remember for session continuity
4. **Degradation detection** - No quality monitoring during work loops

## Solution: Extend Existing + Add Routing

### Approach: Extend, Don't Duplicate

Instead of creating parallel systems, extend existing flows:

| Need | Solution |
|------|----------|
| Checkpointing | Extend Progress Checkpointing with degradation signals |
| Session persistence | Extend Handoff Protocol with anchored format for SA mode |
| Design routing | Add COMPLEXITY_EXPLAINER to ds skill |
| Execution routing | Add TIER 1/2 pattern to agent-coordination |

### Design Routing (SPEED vs FULL)

**Trigger:** After `ds` skill activation, before Double Diamond phases

**Scoring (weighted, max 18):**
```python
DESIGN_SCORE = (
    (multiple_epics) * 3 +
    (cross_module) * 2 +
    (new_abstractions) * 3 +
    (external_deps) * 2 +
    (files_5plus) * 1 +
    (unclear_scope) * 2 +
    (security_auth) * 2 +
    (data_migration) * 3
)
```

**Routing:**
- Score < 4 → SPEED MODE (1 phase)
- Score 4-6 → ASK USER (soft zone, default FULL after 2 prompts)
- Score > 6 → FULL MODE (4 phases A/P/C)

### Execution Routing (SINGLE vs PARALLEL)

**Trigger:** implement.md Phase 2b, after track selection

**TIER 1 (weighted score):**
```python
TIER1_SCORE = (
    (epics > 1) * 2 +
    (has_parallel_markers) * 3 +
    (domains > 2) * 2 +
    (independent_tasks > 5) * 1
)
# PASS if score >= 5
```

**TIER 2 (compound conditions):**
```python
TIER2_PASS = (
    (files > 15 AND tasks > 3) OR
    (est_tool_calls > 40) OR
    (est_time > 30 AND independent_ratio > 0.6)
)
```

**Result:**
- TIER 1 < 5 → SINGLE_AGENT
- TIER 1 >= 5 AND TIER 2 PASS → PARALLEL_DISPATCH
- TIER 1 >= 5 AND TIER 2 FAIL → SINGLE_AGENT

### Context Lifecycle

**RECALL (session start):**
- Load `.conductor/session-context.md`
- Token budget display with thresholds (<20% warn, <10% force compress)
- Context contract check (Intent, Track ID, Key decisions)
- Cold start: create skeleton if file missing

**Extended Progress Checkpointing:**
- Existing: Token budget triggers (70%, 85%, 90%)
- Add: Degradation signals after each task
  - `tool_repeat`: same tool on same target >= threshold
  - `backtrack`: revisiting completed task
  - `quality_drop`: test failures increase OR lint errors
  - `contradiction`: conflicts with Decisions anchor
- 2+ signals → trigger compression

**Extended Handoff (REMEMBER):**
- SA Mode: Save to `.conductor/session-context.md`
- MA Mode: Use existing handoff files
- Anchored format with [PRESERVE] markers:
  - Intent [PRESERVE]
  - Constraints & Ruled-Out [PRESERVE]
  - Decisions Made (with Why)
  - Files Modified
  - Open Questions / TODOs
  - Current State
  - Next Steps

## Architecture

### Complete Flow After Integration

```
preflight-beads.md
    │
    + RECALL (load session-context.md)
    │
    ▼
ds skill (if design task)
    │
    + COMPLEXITY_EXPLAINER → SPEED or FULL
    │
    ▼
/conductor-newtrack → fb → rb
    │
    ▼
implement.md
    │
    + Phase 2b: EXECUTION ROUTING → SINGLE or PARALLEL
    │
    ▼
Phase 3: Work Loop
    │
    + Evaluate degradation after each task (extended checkpoint)
    │
    ▼
Phase 6: Sync
    │
    + Extended Handoff (anchored format)
```

### State Files

| File | Purpose | Mode |
|------|---------|------|
| `.conductor/session-context.md` | Human-readable context (cross-session) | SA + MA |
| `.conductor/session-state_{agent}.json` | Machine state (within-session) | SA + MA |
| `implement_state.json` | Includes `execution_mode` field | SA + MA |

## Files to Create

| File | Purpose | Size |
|------|---------|------|
| `workflows/context-engineering/session-lifecycle.md` | RECALL + ROUTE orchestration | Medium |
| `workflows/context-engineering/references/anchored-state-format.md` | Template | Small |
| `workflows/context-engineering/references/design-routing-heuristics.md` | Scoring | Small |
| `workflows/agent-coordination/patterns/execution-routing.md` | TIER 1/2 | Medium |
| `workflows/conductor/checkpoint.md` | Facade → Progress Checkpointing | Tiny |
| `workflows/conductor/remember.md` | Facade → Handoff Protocol | Tiny |

## Files to Modify

| File | Changes |
|------|---------|
| `workflows/beads/workflow.md` | Add `## Degradation Signals` section |
| `workflows/conductor/beads-session.md` | Add `## Anchored Format (SA Mode)` section |
| `workflows/implement.md` | Add Phase 2b, add degradation evaluation |
| `skills/design/SKILL.md` | Add COMPLEXITY_EXPLAINER section |
| `workflows/conductor/preflight-beads.md` | Add RECALL hook |
| `workflows/agent-coordination/workflow.md` | Add execution-routing to patterns |
| `workflows/README.md` | Add context-engineering links |

## v1 Scope

### Included
- ✅ RECALL (load session-context.md)
- ✅ DESIGN ROUTING (COMPLEXITY_EXPLAINER + SPEED/FULL)
- ✅ EXECUTION ROUTING (TIER 1 + TIER 2, no VALIDATE)
- ✅ Extended Progress Checkpointing (degradation signals)
- ✅ Extended Handoff (anchored format for SA mode)

### Deferred to v1.1
- ❌ VALIDATE dispatch (dependency graph, cycle detection)
- ❌ Advanced degradation (signal combinators, hysteresis)
- ❌ Multi-agent ownership model
- ❌ Memory TTL and compaction rules

## Acceptance Criteria

1. Design routing displays COMPLEXITY_EXPLAINER before ds phases
2. Score < 4 routes to SPEED, > 6 routes to FULL, 4-6 asks user
3. Execution routing evaluates TIER 1/2 before Phase 3
4. Degradation signals detected after each task completion
5. Session context saved in anchored format at session end
6. Cold start creates skeleton session-context.md
7. Facades discoverable at `workflows/conductor/checkpoint.md` and `remember.md`

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Score 4-6 user unavailable | Default to FULL after 2 prompts |
| Corrupted session-context.md | Validation step in RECALL |
| Degradation in parallel dispatch | Coordinator aggregates after merge |
| Breaking existing Progress Checkpointing | Additive changes only |

## LOE Estimate

| Phase | Time |
|-------|------|
| Create 6 files | 2h |
| Modify 7 files | 1.5h |
| Verify | 30min |
| **Total** | **4h** |

## Party Mode Reviews Completed

- [x] Initial scope review (3 experts)
- [x] Stress test: 11 issues identified and addressed
- [x] Integration gaps analysis
- [x] Final plan review with facades

## Design Approved

Ready for `/conductor-newtrack context-engineering_20251226`
