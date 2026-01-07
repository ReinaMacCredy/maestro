# Design: DS-PL Integration

## 1. Problem Statement

**Current state:** `ds` (Design Session) and `pl` (Planning Pipeline) are separate workflows. After completing a design session, users must manually invoke `pl` to create specs and plans.

**Desired state:** `ds` automatically chains into `pl` after DELIVER phase completes, creating a seamless flow from design to planning.

## 2. Requirements

### Functional Requirements

1. After `ds` DELIVER phase (CP4) passes → auto-trigger full `pl` pipeline
2. `pl` runs all 6 phases (no skip/lite mode)
3. `pl` called independently continues to work as-is
4. User sees clear transition message between ds and pl phases

### Non-Functional Requirements

1. No breaking changes to existing `pl` behavior
2. No changes to Double Diamond phases (DISCOVER/DEFINE/DEVELOP/DELIVER)
3. Session can be long (ds + full pl) - acceptable tradeoff

## 3. Design Decisions

### Decision 1: Trigger Point

**Chosen:** After CP4 (Oracle Audit) passes in DELIVER phase.

**Rationale:** 
- Design must be validated before planning
- Oracle audit ensures design quality
- Natural handoff point

### Decision 2: pl Mode

**Chosen:** Full 6-phase execution, no skip.

**Rationale:**
- User explicitly requested full execution
- pl discovery may find additional context ds missed
- Simpler implementation (no conditional logic)

### Decision 3: Output Structure

**Chosen:** Standard outputs - `design.md` updated, `plan.md` created, beads filed.

**Rationale:**
- Consistent with existing `/conductor-newtrack` behavior
- No special handling needed

## 4. Technical Approach

### Flow Diagram

```
ds (Double Diamond)
├── DISCOVER (diverge)
├── DEFINE (converge)  
├── DEVELOP (diverge)
└── DELIVER (converge)
    └── CP4: Oracle Audit
        └── IF APPROVED:
            └── AUTO-TRIGGER pl pipeline
                ├── Phase 1: Discovery
                ├── Phase 2: Synthesis  
                ├── Phase 3: Verification
                ├── Phase 4: Decomposition (fb)
                ├── Phase 5: Validation (bv)
                └── Phase 6: Track Planning
```

### Files to Modify

| File | Change |
|------|--------|
| `design/SKILL.md` | Add pl auto-trigger after DELIVER |
| `design/references/double-diamond.md` | Document pl handoff in Phase 4 exit |
| `maestro-core/references/routing-table.md` | Update ds output description |

### Implementation Details

#### 1. Update double-diamond.md

Add after DELIVER exit criteria:

```markdown
## Post-DELIVER: Planning Pipeline

After Oracle Audit passes (APPROVED verdict):

1. Display transition message:
   ```
   ✅ Design approved. Transitioning to Planning Pipeline...
   ```

2. Execute full pl pipeline (6 phases)
3. Outputs: design.md (updated) + plan.md + beads
```

#### 2. Update design/SKILL.md

In "Session Flow" section, add step 7:

```markdown
7. **Plan** - After DELIVER approved, auto-run pl pipeline (6 phases)
```

Update "Next Steps" section to note auto-execution:

```markdown
## Next Steps (after design.md created)

> **Note:** If ds completed with Oracle approval, pl pipeline runs automatically.
> Manual commands only needed if ds was interrupted or pl standalone.
```

## 5. Acceptance Criteria

- [ ] `ds` FULL mode + CP4 APPROVED → automatically runs `pl` 6 phases
- [ ] `ds` SPEED mode → suggests `cn` (no auto-trigger)
- [ ] `pl` standalone works unchanged
- [ ] Transition message displayed between ds and pl
- [ ] Final output: design.md + plan.md + beads filed
- [ ] `metadata.json` tracks planning state per phase
- [ ] Interrupted pl can resume via `cn`
- [ ] No regressions in existing ds or pl behavior

## 6. Risks & Mitigations

| Risk | Level | Mitigation |
|------|-------|------------|
| Long session duration | LOW | Expected behavior for full workflow |
| CP4 failure blocks pl | LOW | Correct - design must be approved first |
| User wants to skip pl after ds | LOW | Can interrupt; pl optional via Ctrl+C |
| Ambiguous trigger in SPEED mode | MEDIUM | Auto-trigger only in FULL mode with APPROVED verdict |
| Partial planning state on failure | MEDIUM | Update metadata.json per phase; allow resume via `cn` |
| User mental model confusion | LOW | Clear docs: "pl ran automatically" vs "run cn manually" |

## 7. Trigger Rules (Oracle Critical)

### SPEED vs FULL Mode

| Mode | CP4 Result | Action |
|------|------------|--------|
| FULL | APPROVED | Auto-trigger pl (6 phases) |
| FULL | NEEDS_REVISION | HALT - fix design first |
| SPEED | APPROVED | Suggest `cn` (no auto-trigger) |
| SPEED | NEEDS_REVISION | WARN only, suggest `cn` |

**Rationale:** SPEED mode is for quick, small designs - auto-running full pl is too heavy.

### Failure/Interruption Behavior

| Scenario | Behavior |
|----------|----------|
| pl phase fails | Update `metadata.json` with failed phase; show error |
| User interrupts (Ctrl+C) | Partial progress saved in metadata |
| Resume | Run `cn` to continue from last completed phase |

### Invocation Path

```
ds DELIVER (CP4 APPROVED)
    │
    ▼
Call internal pl_pipeline() function
    │
    ├── context: current track directory
    ├── input: design.md from current track
    └── output: plan.md, .beads/ in same track
```

**NOT via `cn` command** - direct internal call to avoid redundant setup.

## 8. Out of Scope

- Changes to pl phases or logic
- Changes to Double Diamond phases
- Mini-DISCOVER for standalone pl
- Conditional skip of pl phases after ds

## Oracle Audit

**Date:** 2025-01-04

### 6-Dimension Summary

| Dimension | Status | Notes |
|-----------|--------|-------|
| Completeness | ✅ | Trigger rules, failure behavior, invocation path specified |
| Feasibility | ✅ | Reuses existing Oracle/beads/bv infrastructure |
| Risks | ✅ | SPEED/FULL mode handling, partial state recovery addressed |
| Dependencies | ✅ | Direct internal call, no `cn` command dependency |
| Ordering | ✅ | Linear flow: ds → CP4 → pl phases |
| Alignment | ✅ | Matches product goal of seamless design-to-planning |

### Verdict: APPROVED

All critical issues from initial review addressed.
