# Design: Pre-Orchestration Token Optimization

> **Problem:** Before parallel workers start, the orchestrator consumes significant tokens on setup, skill loading, and redundant analysis.

## Context

From thread [T-019b7e6e-93b8-706b-894c-4333967cd5a8](http://localhost:8317/threads/T-019b7e6e-93b8-706b-894c-4333967cd5a8):

| Step | Token Cost | Cause |
|------|------------|-------|
| Skill loading (orchestrator + references) | ~5-7k | Eager load of 16+ reference files (workflow.md alone is ~650 lines) |
| Phase 2b: Execution Routing | ~300-500 | Re-analyzes file scopes (mostly gated already) |
| Phase 0/0.5: Preflight + Handoff | ~1-2k | `bv --robot-triage`, handoff resume |
| Agent Mail setup | ~500 | Full ensure_project + register ceremony |
| Conductor/Maestro-core skills | ~1-2k | Additional skill loads before orchestrator |
| **TOTAL PRE-SPAWN** | **~8-12k** | Before any worker does actual work |

## Oracle Audit Findings

**Key corrections from deep audit:**

1. **Token estimates were optimistic** - `workflow.md` alone is ~3-5k tokens, not the entire ~4k claimed
2. **Solution B already partially implemented** - `implement.md` Phase 2b has fast-path for Track Assignments
3. **Solution C is medium risk, not low** - Agent Mail protocol inconsistency (pre-register vs self-register)
4. **Missing sinks** - Preflight, Handoff, Conductor skill loads not addressed

## Solution Overview (Revised)

| Solution | Token Savings | Risk | Notes |
|----------|---------------|------|-------|
| A: Lazy Skill Loading | ~3-5k | Medium | Requires loader changes |
| B: Harden Trust plan.md | ~200-400 | Low | Mostly done, needs tightening |
| C: Streamlined Agent Mail | ~300-500 | **Medium** | Protocol consistency needed |
| D: Preflight/Handoff optimization | ~1k | Low | NEW - not in original design |
| **Realistic Combined** | **~5-7k (40-60%)** | Medium | Target: ~4-5k pre-spawn |

---

## Solution A: Lazy Skill Reference Loading

### Problem

Current flow loads ALL skill reference files immediately:

```
skill(name="orchestrator") 
  → SKILL.md loaded
  → 16 reference files auto-attached to context (~3-5k tokens)
```

Most references are never used in a given session.

### Design

**Add `## Lazy References` section to skills:**

```markdown
## Lazy References

References loaded on-demand when workflow step needs them:

| Trigger | Reference |
|---------|-----------|
| Phase 3: Initialize | [agent-mail.md](references/agent-mail.md) |
| Phase 4: Spawn | [worker-prompt.md](references/worker-prompt.md) |
| Conflict detected | [agent-coordination.md](references/agent-coordination.md) |
```

**Skill loader behavior:**

1. Load SKILL.md only (core instructions)
2. Parse `## Quick Reference` table for inline guidance
3. Load references ONLY when specific phase/trigger activates

### Implementation

Update orchestrator/SKILL.md:

```diff
- ## References
- 
- | Topic | File |
- |-------|------|
- | Full workflow | [workflow.md](references/workflow.md) |
- | Worker prompt | [worker-prompt.md](references/worker-prompt.md) |
- ...

+ ## Lazy References
+ 
+ Load references only when needed:
+ 
+ | When | Load |
+ |------|------|
+ | Always | SKILL.md (this file) |
+ | Phase 4 (spawn) | worker-prompt.md |
+ | Cross-track deps | agent-coordination.md |
+ | Conflict resolution | agent-mail.md |
```

### Token Impact

| Before | After | Savings |
|--------|-------|---------|
| ~4k (16 files) | ~500 (SKILL.md + 1-2 refs) | ~3.5k |

---

## Solution B: Harden Trust plan.md Track Assignments

### Current State (Per Oracle Audit)

**Good news:** `implement.md` Phase 2b ALREADY has a fast-path:

```python
# implement.md:128-138
if "## Track Assignments" in plan:
    return PARALLEL_DISPATCH  # Skips all analysis
```

The heavy beads/`bd list`/dependency analysis only runs in the auto-detect branch.

### Remaining Optimization

The confirmation prompt (`implement.md:200-231`) still uses `group_by_file_scope()` even when Track Assignments exist. Fix:

```python
# Phase 2b: Execution Routing (Tightened)

if "## Track Assignments" in plan:
    # Parse Track Assignments table directly
    tracks = parse_track_assignments(plan)
    
    # Use parsed tracks for confirmation (NOT group_by_file_scope)
    print(f"📊 {len(tracks)} tracks from plan.md:")
    for track in tracks:
        print(f"- Track {track.id}: {len(track.tasks)} tasks ({track.scope})")
    
    print("Run parallel? [Y/n]: ")
    return PARALLEL_DISPATCH if confirmed else SINGLE_AGENT
```

### Light Validation (Optional)

```python
# Sanity check without full dependency analysis
metadata = Read("conductor/tracks/<track-id>/metadata.json")
plan_tasks = set(track.tasks for track in tracks)
bead_tasks = set(metadata.beads.planTasks.keys())

if plan_tasks - bead_tasks:
    print(f"⚠️ Tasks in Track Assignments missing from beads: {plan_tasks - bead_tasks}")
```

### Token Impact (Revised)

| Before | After | Savings |
|--------|-------|---------|
| ~400 (grouping call) | ~100 (parse table) | ~200-300 |

**Note:** Original estimate of ~1.3k was overstated - the fast-path already exists.

---

## Solution C: Streamlined Agent Mail Setup (MEDIUM RISK)

### Problem

Current initialization runs multiple MCP calls:

```python
health_check()           # 1 call
ensure_project()         # 1 call  
register_agent()         # 1 call per agent (orchestrator + workers)
send_message()           # EPIC START to workers
```

### Protocol Inconsistency (Oracle Finding)

**Current docs are contradictory:**

| Doc | Says |
|-----|------|
| `agent-mail.md` | "MUST pre-register ALL workers before spawn" |
| `worker-prompt.md` | "Orchestrator has ALREADY registered you" |
| `worker-prompt.md` | Workers call `macro_start_session` in Step 1 |

**Problem:** If orchestrator pre-registers workers AND workers call `macro_start_session`, that's duplicate work. If we remove pre-registration, the EPIC START message fails (recipients don't exist).

### Design Options

**Option 1: Keep pre-registration (safer)**
- Orchestrator pre-registers all workers
- Workers skip `macro_start_session` (already registered)
- EPIC START message works
- **Con:** No token savings

**Option 2: Workers self-register (riskier)**
- Orchestrator does NOT pre-register workers
- Workers call `macro_start_session` to self-register
- EPIC START message must be deferred or use broadcast
- **Con:** Protocol change, message timing issues

**Option 3: Hybrid (recommended)**
- Orchestrator uses `macro_start_session` for itself only
- Workers are NOT pre-registered by orchestrator
- EPIC START message uses `thread_id` only (no specific recipients)
- Workers join thread via `macro_start_session`

### Recommended Implementation

```python
# Phase 3: Initialize (Hybrid approach)

# 1. Orchestrator session (single call)
result = macro_start_session(
    human_key=PROJECT_PATH,
    program="amp",
    model="<model>",
    task_description="Orchestrator for <epic-id>"
)
ORCHESTRATOR_NAME = result.agent.name

# 2. Create epic thread (no specific recipients)
send_message(
    project_key=PROJECT_PATH,
    sender_name=ORCHESTRATOR_NAME,
    to=[ORCHESTRATOR_NAME],  # Send to self, workers join later
    thread_id=EPIC_ID,
    subject="EPIC STARTED: <title>",
    body_md="Workers will join this thread via macro_start_session"
)

# 3. Workers self-register in their Task context
# See updated worker-prompt.md
```

### Required Doc Updates

| File | Change |
|------|--------|
| `workflow.md` Phase 3 | Remove worker pre-registration |
| `agent-mail.md` | Change "MUST pre-register" to "workers self-register" |
| `worker-prompt.md` | Remove "Orchestrator has ALREADY registered you" |

### Token Impact (Revised)

| Before | After | Savings |
|--------|-------|---------|
| ~500 (7 calls) | ~250 (3 calls) | ~250 |

**Note:** Savings are smaller than original ~350 estimate because workers still call `macro_start_session`.

---

## Solution D: Preflight/Handoff Optimization (NEW)

### Problem (Identified by Oracle)

`implement.md` Phase 0 and Phase 0.5 run unconditionally:

| Phase | Cost | What it does |
|-------|------|--------------|
| Phase 0: Beads Preflight | ~500 | `bv --robot-triage`, session state |
| Phase 0.5: Handoff Load | ~500-1k | `summarize_thread`, file reads |

These run for EVERY `/conductor-implement`, even when:
- Beads are already filed and ready
- No prior handoff exists
- Session is fresh (first run)

### Design

**Skip preflight when metadata indicates ready state:**

```python
# Phase 0: Conditional Preflight

metadata = Read("conductor/tracks/<track-id>/metadata.json")

if metadata.beads.status == "complete" and metadata.beads.orchestrated:
    # Beads already filed and orchestrated - skip triage
    print("✓ Using cached bead state from metadata.json")
else:
    # Run full preflight
    bv --robot-triage --graph-root <epic-id> --json
```

**Skip handoff for fresh sessions:**

```python
# Phase 0.5: Conditional Handoff

handoff_dir = "conductor/handoffs/<track_id>/"
recent_handoffs = glob(handoff_dir + "*.md", last=7days)

if not recent_handoffs:
    print("ℹ️ No prior handoff - fresh session")
    # Skip handoff load entirely
else:
    # Load most recent handoff
    ...
```

### Token Impact

| Before | After | Savings |
|--------|-------|---------|
| ~1.5k (always run) | ~200 (conditional) | ~1k |

---

## Combined Flow (Revised)

### Before (Current)

```
ci track
  │
  ├─ Load maestro-core skill (~500 tokens)
  ├─ Load conductor skill + refs (~1-2k tokens)
  ├─ Load orchestrator skill + 16 refs (~5-7k tokens)
  ├─ Phase 0: Beads Preflight (~500 tokens)
  ├─ Phase 0.5: Handoff Load (~500-1k tokens)
  ├─ Phase 2b: Check Track Assignments
  │    └─ (mostly skipped if Track Assignments exist)
  ├─ Confirmation prompt
  ├─ Phase 3: Agent Mail setup
  │    └─ health_check + ensure + register x4 (~500 tokens)
  └─ Phase 4: Spawn workers
  
  TOTAL: ~8-12k tokens before first worker starts
```

### After (Optimized)

```
ci track
  │
  ├─ Load maestro-core skill (~300 tokens - inline only)
  ├─ Load conductor SKILL.md only (~400 tokens)
  ├─ Load orchestrator SKILL.md only (~400 tokens)
  ├─ Phase 0: Skip preflight (metadata.beads.status=complete)
  ├─ Phase 0.5: Skip handoff (no recent handoffs)
  ├─ Phase 2b: Parse Track Assignments table (~100 tokens)
  ├─ Confirmation prompt
  ├─ Phase 3: macro_start_session (~150 tokens)
  └─ Phase 4: Spawn workers
  
  TOTAL: ~1.5-2.5k tokens before first worker starts
  
  SAVINGS: ~6-9k tokens (60-75% reduction)
```

### Realistic Expectations

| Scenario | Pre-spawn tokens |
|----------|-----------------|
| Best case (all optimizations) | ~1.5-2.5k |
| Typical case | ~3-4k |
| Worst case (first run, no Track Assignments) | ~6-8k |

---

## Implementation Plan (Revised)

### Phase 1: Harden Trust plan.md (LOW RISK) ✓ Mostly done

1. Update `implement.md` Phase 2b to use parsed Track Assignments for confirmation
2. Skip `group_by_file_scope()` when Track Assignments exist
3. Add light validation against `metadata.json.beads.planTasks`

**Files:** `conductor/references/workflows/implement.md`

### Phase 2: Preflight/Handoff Optimization (LOW RISK)

1. Add conditional preflight based on `metadata.beads.status`
2. Skip handoff load when no recent handoffs exist
3. Cache bead state in metadata.json for faster re-runs

**Files:** `conductor/references/workflows/implement.md`, `conductor/references/preflight-beads.md`

### Phase 3: Normalize Agent Mail Protocol (MEDIUM RISK)

1. **Decide model:** Pre-register vs workers self-register
2. Implement chosen model consistently across:
   - `workflow.md` Phase 3
   - `agent-mail.md` 
   - `worker-prompt.md`
3. Test EPIC START message timing

**Files:** `orchestrator/references/workflow.md`, `orchestrator/references/agent-mail.md`, `orchestrator/references/worker-prompt.md`

### Phase 4: Lazy Skill Loading (MEDIUM RISK)

1. Update Maestro loader to support `## Lazy References`
2. Add `## Lazy References` section to orchestrator SKILL.md
3. Keep compressed summaries in SKILL.md for critical flows
4. Define trigger conditions for each reference file
5. Validate with lint/tests

**Files:** `orchestrator/SKILL.md`, Maestro loader code

---

## Success Criteria (Revised)

| Metric | Current | Target | Notes |
|--------|---------|--------|-------|
| Pre-spawn tokens | ~8-12k | ~3-4k typical | 60% reduction |
| Best-case pre-spawn | N/A | ~1.5-2.5k | All optimizations |
| Time to first worker | ~30s | ~15s | Estimated |
| MCP calls before spawn | 7+ | 3-4 | macro_start_session |

## Risks (Oracle-Informed)

| Risk | Severity | Mitigation |
|------|----------|------------|
| Lazy loading misses needed reference | High | Keep inline summaries in SKILL.md |
| Loader change breaks existing skills | High | Support both `## References` and `## Lazy References` |
| Workers fail to self-register | Medium | Fallback to pre-registration |
| Track Assignments parsing errors | Low | Validate format, sanity check against beads |
| Stale metadata.beads.status | Low | Re-run preflight if bead commands fail |
| EPIC START message fails (no recipients) | Medium | Send to self, workers join thread |

## Dependencies

| Solution | Depends On |
|----------|-----------|
| B (Trust plan.md) | None - localized change |
| D (Preflight/Handoff) | None - localized change |
| C (Agent Mail) | MCP `macro_start_session` implementation |
| A (Lazy loading) | Maestro loader changes, stable workflow phases |

## Next Steps

1. **[C] Continue** to `/conductor-newtrack` to create spec + plan
2. **[↩ Back]** to refine any solution
3. **[S] Skip** to implementation directly (start with Phase 1+2, low risk)
