# Design: Unified DS Pipeline

## 1. Problem Statement

**Current state:** The Design Session (`ds`) and Planning Pipeline (`pl`) are conceptually integrated but have:
- **Redundant discovery**: DS DISCOVER + PL Phase 1 Discovery both research the codebase
- **Fragmented research**: 5 separate research hooks (session-start, CP3, CP4, PL-discovery, PL-synthesis)
- **Context loss at transition**: After DS DELIVER, PL starts fresh without carrying context
- **State machine fragmentation**: Two separate state machines that don't align
- **SPEED/FULL mode confusion**: Different behaviors create inconsistent mental models

**Desired state:** A unified 8-phase pipeline where:
- Research is consolidated to 2 strategic hooks (not 5)
- Context flows seamlessly between all phases
- Single state machine tracks entire journey
- SPEED mode gets proportional (not different) treatment
- Phase transitions are invisible to the user

## 2. Discovery Context

### Current Research Hooks (5 hooks)

| Hook | Trigger | Agents | Duration |
|------|---------|--------|----------|
| discover-hook | DS session start | Locator + Pattern + CODEMAPS | 10s |
| grounding-hook | DEVELOP→DELIVER (CP3) | Locator + Analyzer + Pattern | 15s |
| CP4 full verification | DELIVER end | All 5 + Impact | 20s |
| PL Discovery | PL Phase 1 | Task() agents (Architecture + Pattern + Constraints) | ~30s |
| PL Synthesis | PL Phase 2 | Oracle | ~20s |

**Total potential research time: ~95 seconds** (if all run sequentially)

### Current Phase Flow

```
DS:  DISCOVER → DEFINE → DEVELOP → DELIVER
                                      ↓
                              [transition message]
                                      ↓
PL:  Discovery → Synthesis → Verification → Decomposition → Validation → Track Planning
```

**10 total phases with a manual transition in the middle.**

## 3. Design Decisions

### Decision 1: Unified 8-Phase Model

Merge DS (4 phases) + PL (6 phases) into 8 unified phases:

| # | Phase | Type | Purpose | From |
|---|-------|------|---------|------|
| 1 | **DISCOVER** | Diverge | Explore problem + research context | DS |
| 2 | **DEFINE** | Converge | Frame problem + select approach | DS |
| 3 | **DEVELOP** | Diverge | Architecture + components | DS |
| 4 | **VERIFY** | Converge | Oracle audit + risk assessment | DS DELIVER + PL Synthesis merged |
| 5 | **DECOMPOSE** | Execute | Create beads (fb) | PL Phase 4 |
| 6 | **VALIDATE** | Execute | Dependency check (bv) | PL Phase 5 |
| 7 | **ASSIGN** | Execute | Track assignments | PL Phase 6 |
| 8 | **READY** | Complete | Handoff to ci/orchestrate | New |

**Removed/Merged:**
- ~~PL Discovery~~ → Absorbed into Phase 1 (DISCOVER)
- ~~PL Synthesis~~ → Absorbed into Phase 4 (VERIFY)
- ~~PL Verification~~ → Merged into Phase 6 (VALIDATE)
- ~~DS DELIVER~~ → Renamed to VERIFY, includes Oracle audit

### Decision 2: Consolidated Research (5 → 2 Hooks)

Two strategic research points instead of five:

| Hook | Trigger | Agents | Purpose |
|------|---------|--------|---------|
| **research-start** | Phase 1 (DISCOVER) start | Locator + Pattern + CODEMAPS + Architecture | All initial context gathering |
| **research-verify** | Phase 3→4 (DEVELOP→VERIFY) | Analyzer + Pattern + Impact + Web | Design verification before Oracle |

**Time savings:** Old: 95s worst case → New: 35s max

### Decision 3: Context Passing via Pipeline Context Object

Single `pipeline_context` object that accumulates through all phases:

```typescript
interface PipelineContext {
  // Metadata
  id: string;
  mode: "SPEED" | "FULL";
  current_phase: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;
  preflight_completed: boolean;
  started_at: ISO8601;
  
  // Research (accumulated)
  research: {
    start: ResearchStartResult | null;
    verify: ResearchVerifyResult | null;
  };
  
  // Decisions (accumulated)
  decisions: {
    problem_statement: string | null;
    success_criteria: string[] | null;
    scope: { in: string[], out: string[] } | null;
    approach: { selected: string, rationale: string, alternatives: string[] } | null;
    architecture: ArchitectureSpec | null;
    risk_assessment: RiskMap | null;
  };
  
  // Spikes (Phase 4)
  spikes: {
    required: SpikeRef[];           // HIGH risk items needing spikes
    completed: SpikeResult[];       // Results from spike execution
    all_passed: boolean;            // True if all spikes YES
  } | null;
  
  // Artifacts (accumulated)
  artifacts: {
    design_md: string;
    spec_md: string | null;
    plan_md: string | null;
    beads: BeadRef[] | null;
    track_assignments: TrackAssignment[] | null;
  };
  
  // Validation state
  validation: {
    checkpoints_passed: ("CP1" | "CP2" | "CP3" | "CP4")[];
    oracle_verdict: "APPROVED" | "NEEDS_REVISION" | null;
    oracle_beads_review: "APPROVED" | "NEEDS_REVISION" | null;  // Phase 6
    retries: number;
    max_retries: 2;
    skip_accepted: boolean;
  };
  
  // Orchestration (Phase 8)
  orchestration: {
    mode: "parallel" | "sequential" | null;
    tracks: TrackAssignment[] | null;
    workers_spawned: string[] | null;      // Agent names
    workers_completed: string[] | null;
  } | null;
}

interface SpikeRef {
  id: string;
  question: string;
  risk_item: string;
  time_box_minutes: number;
  path: string;
}

interface SpikeResult {
  id: string;
  result: "YES" | "NO" | "PARTIAL" | "TIMEOUT";
  learnings: string[];
  approach?: string;
  blocker?: string;
}
```

### Decision 4: Unified State Machine

Single state progression with mode-aware transitions:

```
STATES: discover → define → develop → verify → decompose → validate → assign → ready

MODES:
  SPEED: discover → define → verify → ready  (skip develop, decompose, validate, assign)
  FULL:  All 8 phases
```

### Decision 5: SPEED Mode as Proportional Subset

| Mode | Phases | A/P/C | Research | Beads | Tracks |
|------|--------|-------|----------|-------|--------|
| **SPEED** | 1,2,4,8 | No | 1 hook (start only) | No | No |
| **FULL** | 1-8 | Yes | 2 hooks | Yes | Yes |

### Decision 6: Oracle Revision Loop

```
Oracle returns NEEDS_REVISION
       │
       ▼
  1. HALT execution (FULL mode only)
       │
       ▼
  2. Display Oracle issues with line references
       │
       ├──[R] Revise──→ User edits → Re-run Oracle (max 2 retries)
       │
       ├──[S] Skip──→ Log warning, continue (user accepts risk)
       │
       └──[A] Abort──→ Save progress, suggest resume later
```

## 4. Technical Approach

### 4.1 Phase Transition Rules

| From | To | Condition | Context Update |
|------|----|-----------|----|
| INIT | DISCOVER | Always | mode, id set |
| DISCOVER | DEFINE | CP1 pass (WARN OK) | research.start populated |
| DEFINE | DEVELOP | CP2 pass + FULL mode | decisions set |
| DEFINE | VERIFY | CP2 pass + SPEED mode | (skip DEVELOP) |
| DEVELOP | VERIFY | CP3 pass (WARN OK) | research.verify populated |
| VERIFY | DECOMPOSE | CP4 APPROVED + FULL mode | oracle_verdict = APPROVED |
| VERIFY | READY | CP4 pass + SPEED mode | (skip 5,6,7) |
| DECOMPOSE | VALIDATE | beads filed | artifacts.beads populated |
| VALIDATE | ASSIGN | bv passes | dependencies validated |
| ASSIGN | READY | tracks assigned | artifacts.track_assignments populated |

### 4.2 A/P/C Integration

A/P/C checkpoints appear at phase boundaries in FULL mode only:

| After Phase | A/P/C | Special Behavior |
|-------------|-------|------------------|
| 1 (DISCOVER) | Yes | [A] Advanced assumption audit |
| 2 (DEFINE) | Yes | [A] Scope stress-test |
| 3 (DEVELOP) | Yes | [A] Architecture deep-dive |
| 4 (VERIFY) | Yes | Oracle runs BEFORE showing menu |
| 5-8 | No | Automated execution |

### 4.3 Phase 4: VERIFY - Spike Execution

When risk assessment identifies HIGH risk items, spikes are executed:

```
Risk Assessment in Phase 4:
   │
   ├── LOW items → Proceed
   ├── MEDIUM items → Interface sketch (inline)
   └── HIGH items → Spawn spike Task()
                         │
                         ▼
               ┌─────────────────────────────────┐
               │ For each HIGH risk item:        │
               │ 1. Create spike bead + dir      │
               │ 2. Spawn Task() with time-box   │
               │ 3. Wait for completion          │
               │ 4. Capture result (YES/NO)      │
               └─────────────────────────────────┘
                         │
                         ▼
               Oracle aggregates spike results
                         │
                         ▼
               Update design.md Section 5
                         │
               ┌─────────┴─────────┐
               │                   │
        All spikes YES      Any spike NO/TIMEOUT
               │                   │
               ▼                   ▼
           Continue           HALT - user decision
```

**Spike Task() Template:**
```python
Task(
  description="Spike: <question>",
  prompt="""
  Time-box: 30 minutes
  Output: conductor/spikes/<track>/<spike-id>/
  
  Success criteria:
  - Working throwaway code
  - Answer documented (YES/NO + details)
  - Learnings captured
  
  On completion:
  bd close <id> --reason "YES: <approach>" or "NO: <blocker>"
  """
)
```

### 4.4 Phase 6: VALIDATE - Oracle Final Review

After `bv` dependency checks, Oracle reviews beads for completeness:

```
Phase 6: VALIDATE
   │
   ├── Step 1: bv --robot-suggest (find missing deps)
   ├── Step 2: bv --robot-insights (detect cycles)
   ├── Step 3: bv --robot-priority (validate priorities)
   ├── Step 4: Fix issues (bd dep add/remove)
   │
   └── Step 5: Oracle Final Review
               │
               ▼
       ┌─────────────────────────────────────┐
       │ oracle(                             │
       │   task="Review beads completeness", │
       │   context="Check for gaps, unclear  │
       │            beads, missing deps",    │
       │   files=[".beads/"]                 │
       │ )                                   │
       └─────────────────────────────────────┘
               │
       ┌───────┴───────┐
       │               │
    APPROVED     NEEDS_REVISION
       │               │
       ▼               ▼
   Continue      Fix beads → re-validate
```

### 4.5 Phase 8: READY - Auto-Orchestration

After track assignment, optionally spawn worker agents:

```
Phase 8: READY
   │
   ├── Check plan.md for Track Assignments table
   │
   └── Count parallel tracks
               │
       ┌───────┴───────┐
       │               │
   ≥2 tracks      1 track
       │               │
       ▼               ▼
   ┌─────────────────────────────────────┐
   │ Orchestration prompt:               │
   │                                     │
   │ Ready to execute. Found N tracks:   │
   │ • Track A (BlueLake): 4 beads       │
   │ • Track B (GreenCastle): 3 beads    │
   │                                     │
   │ [O] Orchestrate (spawn workers)     │
   │ [S] Sequential (run ci manually)    │
   │                                     │
   │ Default: [O] after 30s              │
   └─────────────────────────────────────┘
       │
       ├── [O] Orchestrate
       │       │
       │       ▼
       │   For each track:
       │   ┌─────────────────────────────────────┐
       │   │ Task(                               │
       │   │   description="Track A: BlueLake",  │
       │   │   prompt="""                        │
       │   │   Agent: BlueLake                   │
       │   │   File scope: skills/...   │
       │   │   Beads: vou1.1 → vou1.2 → vou1.4  │
       │   │   Execute in order, TDD mode.      │
       │   │   Report via Agent Mail.           │
       │   │   """                               │
       │   │ )                                   │
       │   └─────────────────────────────────────┘
       │       │
       │       ▼
       │   Monitor via Agent Mail
       │       │
       │       ▼
       │   All workers complete → rb (review beads)
       │
       └── [S] Sequential
               │
               ▼
           Suggest: "Run `ci` to start implementation"
```

### 4.6 Auto-Planning Confirmation Gate

Before Phase 5:

```
Oracle audit APPROVED. Ready to auto-generate:
• Beads (.beads/*.md)
• Dependencies (bv validation)
• Track assignments (plan.md)

[C] Continue (auto-generate all)
[M] Manual (stop here, I'll run fb/bv/cn)
[P] Preview (show what would be generated)
```

### 4.7 `pl` Compatibility

| Scenario | Behavior |
|----------|----------|
| `pl` after `ds` completes Phase 4 | **DEPRECATED** - Not needed |
| `pl` standalone (no prior ds) | **ALIAS** - Runs Phases 5-8 with existing design.md |
| `pl --legacy` | Runs old 6-phase PL pipeline (deprecated) |

## 5. Files to Modify

| File | Change |
|------|--------|
| `skills/design/SKILL.md` | Update to 8-phase model, remove pl reference |
| `skills/design/references/unified-pipeline.md` | NEW - main pipeline doc |
| `skills/design/references/double-diamond.md` | Deprecate, redirect to unified |
| `skills/design/references/planning/pipeline.md` | Deprecate, redirect to unified |
| `skills/design/references/session-init.md` | Update to INIT preflight |
| `skills/design/references/apc-checkpoints.md` | Update checkpoint locations |
| `skills/conductor/references/research/hooks/research-start.md` | NEW - merged hook |
| `skills/conductor/references/research/hooks/research-verify.md` | NEW - merged hook |
| `skills/maestro-core/SKILL.md` | Update routing table |
| `skills/conductor/references/schemas/metadata.schema.json` | Add `pipeline` section |

## 6. Acceptance Criteria

### Core Pipeline
- [ ] `ds` triggers unified 8-phase pipeline
- [ ] SPEED mode runs phases 1,2,4,8 only
- [ ] FULL mode runs all 8 phases
- [ ] No "transition message" between design and planning
- [ ] Context flows via `pipeline_context` through all phases
- [ ] Single state machine in `metadata.json.pipeline`

### Research & Verification
- [ ] Research consolidated to 2 hooks (research-start, research-verify)
- [ ] Total research time < 40s (down from ~95s)
- [ ] HIGH risk items trigger spike Task() in Phase 4
- [ ] Spike results aggregated via Oracle
- [ ] Spike learnings embedded in beads

### Checkpoints & Gates
- [ ] A/P/C checkpoints at phases 1-4 in FULL mode
- [ ] Oracle audit in Phase 4 (with revision loop)
- [ ] Auto-planning gate before Phase 5 ([C]/[M]/[P])
- [ ] Oracle final review in Phase 6 (beads completeness)

### Automation
- [ ] Phases 5-8 automated (no user interaction unless error)
- [ ] Phase 8 auto-orchestration prompt ([O]/[S])
- [ ] Task() spawned per track when [O] selected
- [ ] Workers report via Agent Mail
- [ ] `rb` triggered after all workers complete

### Compatibility
- [ ] `pl` standalone still works (alias to phases 5-8)
- [ ] `--legacy` flag runs old DS behavior
- [ ] Error if `pl` run without design.md

## 7. Risks & Mitigations

| Risk | Level | Mitigation |
|------|-------|------------|
| Breaking existing `ds` behavior | HIGH | Feature flag: `--legacy` for old behavior |
| `pl` standalone users confused | MEDIUM | Clear deprecation notice |
| Context object too large | LOW | Summarize at phase boundaries |

## 8. Migration Path

1. **Phase 1:** Add unified pipeline under `--unified` flag (parallel)
2. **Phase 2:** Default to unified, `--legacy` available
3. **Phase 3:** Remove legacy, `pl` becomes alias

## 9. Out of Scope

- Changes to beads CLI (`bd`, `bv`, `fb`)
- Changes to orchestrator dispatch logic
- Changes to conductor-implement, conductor-finish
- Changes to handoff system
- Changes to Agent Mail integration

---

## Oracle Audit

**Verdict:** ✅ APPROVED

All critical issues from initial review have been resolved:
- INIT is preflight (not phase 0)
- Revision loop specified with retry limits
- `pl` compatibility clearly defined
- `pipeline` section added to metadata.json schema
