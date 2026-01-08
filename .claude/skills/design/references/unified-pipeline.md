# Unified DS Pipeline

> **Single 8-phase pipeline from problem discovery through execution-ready state.**

## Overview Diagram

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                              UNIFIED DS PIPELINE (8 PHASES)                              │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐                               │
│  │DISCOVER │ →  │ DEFINE  │ →  │ DEVELOP │ →  │ VERIFY  │   ← DESIGN (Human-Driven)     │
│  │(Diverge)│    │(Converge)    │(Diverge)│    │(Converge)                               │
│  │   🔬    │    │   🎯    │    │   🏗️    │    │   ✅    │                               │
│  └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘                               │
│       │              │              │              │                                     │
│      A/P/C         A/P/C         A/P/C         A/P/C + Oracle                            │
│       │              │              │              │                                     │
│  ╔════╧══════════════╧══════════════╧══════════════╧═════╗                               │
│  ║             research-start      research-verify       ║   ← RESEARCH HOOKS            │
│  ╚═══════════════════════════════════════════════════════╝                               │
│                                        │                                                 │
│                             ╔══════════╧══════════╗                                      │
│                             ║  Auto-Plan Gate     ║                                      │
│                             ║  [C]/[M]/[P]        ║                                      │
│                             ╚══════════╤══════════╝                                      │
│                                        │                                                 │
│  ┌───────────┐  ┌──────────┐  ┌────────┐  ┌─────────┐                                   │
│  │ DECOMPOSE │→ │ VALIDATE │→ │ ASSIGN │→ │  READY  │   ← EXECUTION (Automated)        │
│  │ (Execute) │  │ (Execute)│  │(Execute)│  │(Complete)                                  │
│  │    📦     │  │   🔍    │  │   📋    │  │   🚀    │                                   │
│  └─────┬─────┘  └────┬─────┘  └────┬────┘  └────┬────┘                                   │
│        │             │             │            │                                        │
│       fb           bv+Oracle    tracks     [O]/[S] prompt                                │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘

MODES:
  SPEED: 1 → 2 → 4 → 8  (skip 3,5,6,7)
  FULL:  1 → 2 → 3 → 4 → 5 → 6 → 7 → 8
```

## Phase Details

### Phase 1: DISCOVER (Diverge)

| Aspect | Value |
|--------|-------|
| **Purpose** | Explore problem space + gather research context |
| **Type** | Divergent thinking - expand possibilities |
| **Inputs** | User request, CODEMAPS, existing codebase |
| **Outputs** | Problem understanding, relevant files, patterns |
| **Checkpoint** | CP1 (A/P/C in FULL mode) |
| **Research Hook** | `research-start` fires here |
| **Mode** | Both SPEED and FULL |

**Research-Start Hook Agents:**
- Locator - Find relevant files
- Pattern - Identify existing patterns
- CODEMAPS - Load architecture context
- Architecture - Understand structure

---

### Phase 2: DEFINE (Converge)

| Aspect | Value |
|--------|-------|
| **Purpose** | Frame problem + select approach |
| **Type** | Convergent thinking - narrow to solution |
| **Inputs** | Research from Phase 1 |
| **Outputs** | Problem statement, success criteria, scope, approach |
| **Checkpoint** | CP2 (A/P/C in FULL mode) |
| **Research Hook** | None |
| **Mode** | Both SPEED and FULL |

**Decisions Set:**
- `problem_statement`
- `success_criteria[]`
- `scope.in[]` / `scope.out[]`
- `approach.selected` + `approach.rationale`

---

### Phase 3: DEVELOP (Diverge)

| Aspect | Value |
|--------|-------|
| **Purpose** | Architecture + component design |
| **Type** | Divergent thinking - explore implementations |
| **Inputs** | Approach from Phase 2 |
| **Outputs** | Architecture spec, component breakdown |
| **Checkpoint** | CP3 (A/P/C in FULL mode) |
| **Research Hook** | `research-verify` fires at end |
| **Mode** | **FULL only** (SPEED skips to Phase 4) |

**Research-Verify Hook Agents:**
- Analyzer - Deep code analysis
- Pattern - Validate pattern usage
- Impact - Assess change impact
- Web - External docs/examples

---

### Phase 4: VERIFY (Converge)

| Aspect | Value |
|--------|-------|
| **Purpose** | Oracle audit + risk assessment + spike execution |
| **Type** | Convergent thinking - validate design |
| **Inputs** | Architecture from Phase 3, research results |
| **Outputs** | Oracle verdict, risk map, spike results |
| **Checkpoint** | CP4 (A/P/C in FULL mode, Oracle runs BEFORE menu) |
| **Research Hook** | None (research-verify ran at Phase 3 end) |
| **Mode** | Both SPEED and FULL |

#### ⚠️ MANDATORY: Oracle Audit

**YOU MUST call `oracle()` at Phase 4. This is NOT optional.**

```python
# REQUIRED - Execute this exact call:
oracle(
    task="6-dimension design audit for Phase 4 VERIFY",
    context="""
    Review the design.md against these 6 dimensions:
    1. COMPLETENESS - All requirements addressed?
    2. CONSISTENCY - No contradictions?
    3. FEASIBILITY - Technically achievable?
    4. TESTABILITY - Can be verified?
    5. RISK - Blockers identified?
    6. SCOPE - Appropriate boundaries?
    
    Return: APPROVED or NEEDS_REVISION with specific issues.
    """,
    files=["conductor/tracks/<track>/design.md"]
)
```

**Oracle Response Handling:**
```
Oracle → APPROVED:       Continue to Phase 5
       → NEEDS_REVISION: [R] Revise / [S] Skip / [A] Abort
```

#### ⚠️ MANDATORY: Spike Execution for HIGH Risk Items

**If risk assessment identifies HIGH risk items, YOU MUST spawn Task() for each:**

```python
# REQUIRED for each HIGH risk item:
Task(
    description=f"Spike: {risk_item.question}",
    prompt=f"""
    Time-box: 30 minutes
    Output: conductor/spikes/<track>/<spike-id>/
    
    Question to answer: {risk_item.question}
    
    Success criteria:
    - Working throwaway code demonstrating feasibility
    - Answer documented: YES (approach works) or NO (blocker found)
    - Learnings captured in spike/LEARNINGS.md
    
    On completion:
    bd close <spike-bead-id> --reason "YES: <approach>" or "NO: <blocker>"
    """
)
```

**Spike Results:**
```
All YES → Continue to Phase 5
Any NO/TIMEOUT → HALT for user decision
```

---

## Auto-Planning Confirmation Gate

Before entering Phase 5 (DECOMPOSE), display confirmation:

```
Oracle audit APPROVED. Ready to auto-generate:
• Beads (.beads/*.md)
• Dependencies (bv validation)
• Track assignments (plan.md)

[C] Continue (auto-generate all)
[M] Manual (stop here, I'll run fb/bv/cn)
[P] Preview (show what would be generated)

Default: [C] after 30s
```

### Behavior

| Choice | Action |
|--------|--------|
| **[C]** | Auto-generate beads, validate deps, assign tracks (phases 5-7) |
| **[M]** | Stop, suggest: "Run `fb` to file beads, `bv` to validate, then `cn`" |
| **[P]** | Show preview of beads/tracks that would be created, then prompt again |

### Skip Conditions

Gate is skipped if:
- SPEED mode (no beads/tracks)
- `--auto` flag passed to `ds`

---

### Phase 5: DECOMPOSE (Execute)

| Aspect | Value |
|--------|-------|
| **Purpose** | Create beads from design |
| **Type** | Automated execution |
| **Inputs** | Validated design from Phase 4 |
| **Outputs** | Filed beads in `.beads/` |
| **Checkpoint** | None (automated) |
| **Command** | `fb` (file beads) |
| **Mode** | **FULL only** (SPEED skips) |

**Auto-Plan Gate (before Phase 5):**
```
Oracle audit APPROVED. Ready to auto-generate:
• Beads (.beads/*.md)
• Dependencies (bv validation)
• Track assignments (plan.md)

[C] Continue (auto-generate all)
[M] Manual (stop here, I'll run fb/bv/cn)
[P] Preview (show what would be generated)
```

---

### Phase 6: VALIDATE (Execute)

| Aspect | Value |
|--------|-------|
| **Purpose** | Dependency check + Oracle beads review |
| **Type** | Automated execution |
| **Inputs** | Filed beads from Phase 5 |
| **Outputs** | Validated dependencies, Oracle approval |
| **Checkpoint** | None (automated) |
| **Commands** | `bv --robot-*`, Oracle |
| **Mode** | **FULL only** (SPEED skips) |

**Validation Steps:**
1. `bv --robot-suggest` - Find missing dependencies
2. `bv --robot-insights` - Detect cycles
3. `bv --robot-priority` - Validate priorities
4. Fix issues with `bd dep add/remove`
5. Oracle Final Review (beads completeness)

#### ⚠️ MANDATORY: Oracle Beads Review

**YOU MUST call `oracle()` after bv validation. This is NOT optional.**

```python
# REQUIRED - Execute this exact call:
oracle(
    task="Beads completeness review for Phase 6 VALIDATE",
    context="""
    Review the filed beads for:
    1. COVERAGE - All design tasks have corresponding beads?
    2. GRANULARITY - Beads are appropriately sized (not too large/small)?
    3. DEPENDENCIES - All dependencies correctly wired?
    4. CLARITY - Each bead has clear acceptance criteria?
    5. TESTABILITY - Each bead can be verified when complete?
    
    Return: APPROVED or NEEDS_REVISION with specific issues.
    """,
    files=[".beads/"]
)
```

**Oracle Response Handling:**
```
Oracle → APPROVED:       Continue to Phase 7
       → NEEDS_REVISION: Fix beads → re-validate (max 2 retries)
```

---

### Phase 7: ASSIGN (Execute)

| Aspect | Value |
|--------|-------|
| **Purpose** | Assign beads to tracks |
| **Type** | Automated execution |
| **Inputs** | Validated beads from Phase 6 |
| **Outputs** | Track assignments in `plan.md` |
| **Checkpoint** | None (automated) |
| **Mode** | **FULL only** (SPEED skips) |

**Track Assignment Output:**
```markdown
## Track Assignments

| Track | Agent | Beads | File Scope |
|-------|-------|-------|------------|
| A | BlueLake | vou1.1, vou1.2 | .claude/skills/design/** |
| B | GreenCastle | vou1.3, vou1.4 | .claude/skills/conductor/** |
```

---

### Phase 8: READY (Complete)

| Aspect | Value |
|--------|-------|
| **Purpose** | Handoff to implementation |
| **Type** | Terminal state |
| **Inputs** | Track assignments from Phase 7 |
| **Outputs** | Orchestration decision |
| **Checkpoint** | [O]/[S] prompt |
| **Mode** | Both SPEED and FULL |

**Orchestration Prompt (≥2 tracks):**
```
Ready to execute. Found N tracks:
• Track A (BlueLake): 4 beads
• Track B (GreenCastle): 3 beads

[O] Orchestrate (spawn workers)    ← Default after 30s
[S] Sequential (run ci manually)
```

**Single Track:**
- Suggest: `Run 'ci' to start implementation`

#### ⚠️ MANDATORY: Auto-Orchestration for Multiple Tracks

**If ≥2 parallel tracks exist and user selects [O], YOU MUST spawn Task() for each track:**

```python
# REQUIRED for each track when [O] selected:
for track in track_assignments:
    Task(
        description=f"Track {track.name}: {track.agent}",
        prompt=f"""
        Agent: {track.agent}
        Track: {track.name}
        File scope: {track.file_scope}
        Beads: {', '.join(track.beads)}
        
        Execute beads in dependency order using TDD:
        1. Register via macro_start_session()
        2. For each bead:
           - bd show <id> to read context
           - bd update <id> --status in_progress
           - Implement with RED-GREEN-REFACTOR
           - bd close <id> --reason completed
        3. Report completion via send_message()
        
        On completion:
        send_message(
            to=["Orchestrator"],
            subject="Track {track.name} complete",
            body="Completed beads: X, Y, Z. Files modified: ..."
        )
        """
    )
```

**After all workers complete:**
```python
# REQUIRED after all Task() workers return:
# 1. Run review beads
Bash(cmd="rb")

# 2. Summarize results
print("All tracks complete. Run `/conductor-finish` to finalize.")
```

---

## Mode Comparison

| Aspect | SPEED | FULL |
|--------|-------|------|
| **Phases** | 1, 2, 4, 8 | All 8 |
| **Total Phases** | 4 | 8 |
| **A/P/C Checkpoints** | No | Yes (CP1-CP4) |
| **Research Hooks** | 1 (start only) | 2 (start + verify) |
| **Beads Created** | No | Yes (Phase 5) |
| **Track Assignments** | No | Yes (Phase 7) |
| **Oracle Audit** | Yes (Phase 4) | Yes (Phase 4 + Phase 6) |
| **Spike Execution** | No | Yes (HIGH risk items) |
| **Time Estimate** | ~5 min | ~15-30 min |
| **Use Case** | Quick fixes, small features | Complex features, new systems |

### SPEED Mode Flow

```
DISCOVER ──────────→ DEFINE ──────────→ VERIFY ──────────→ READY
   │                   │                   │                 │
 research-start     decisions          Oracle only      suggest ci
                                       (no spikes)
```

### FULL Mode Flow

```
DISCOVER → DEFINE → DEVELOP → VERIFY → DECOMPOSE → VALIDATE → ASSIGN → READY
    │         │         │        │          │          │         │        │
 research  decisions  arch   Oracle+    fb beads    bv deps   tracks  [O]/[S]
  -start              +verify spikes                +Oracle
```

---

## Context Flow Specification

The `pipeline_context` object accumulates through all phases:

```typescript
interface PipelineContext {
  // Metadata
  id: string;                              // Track ID
  mode: "SPEED" | "FULL";                  // Execution mode
  current_phase: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;
  preflight_completed: boolean;            // INIT preflight done
  started_at: ISO8601;
  
  // Research (accumulated)
  research: {
    start: ResearchStartResult | null;     // Phase 1
    verify: ResearchVerifyResult | null;   // Phase 3 end
  };
  
  // Decisions (accumulated)
  decisions: {
    problem_statement: string | null;      // Phase 2
    success_criteria: string[] | null;     // Phase 2
    scope: { in: string[], out: string[] } | null;  // Phase 2
    approach: {                            // Phase 2
      selected: string,
      rationale: string,
      alternatives: string[]
    } | null;
    architecture: ArchitectureSpec | null; // Phase 3
    risk_assessment: RiskMap | null;       // Phase 4
  };
  
  // Spikes (Phase 4)
  spikes: {
    required: SpikeRef[];                  // HIGH risk items
    completed: SpikeResult[];              // Results
    all_passed: boolean;                   // True if all YES
  } | null;
  
  // Artifacts (accumulated)
  artifacts: {
    design_md: string;                     // Always present
    spec_md: string | null;                // Phase 4+
    plan_md: string | null;                // Phase 7+
    beads: BeadRef[] | null;               // Phase 5+
    track_assignments: TrackAssignment[] | null;  // Phase 7+
  };
  
  // Validation state
  validation: {
    checkpoints_passed: ("CP1"|"CP2"|"CP3"|"CP4")[];
    oracle_verdict: "APPROVED" | "NEEDS_REVISION" | null;
    oracle_beads_review: "APPROVED" | "NEEDS_REVISION" | null;
    retries: number;
    max_retries: 2;
    skip_accepted: boolean;                // User accepted Oracle skip
  };
  
  // Orchestration (Phase 8)
  orchestration: {
    mode: "parallel" | "sequential" | null;
    tracks: TrackAssignment[] | null;
    workers_spawned: string[] | null;
    workers_completed: string[] | null;
  } | null;
}

interface SpikeRef {
  id: string;
  question: string;
  risk_item: string;
  time_box_minutes: number;
  path: string;  // conductor/spikes/<track>/<spike-id>/
}

interface SpikeResult {
  id: string;
  result: "YES" | "NO" | "PARTIAL" | "TIMEOUT";
  learnings: string[];
  approach?: string;   // If YES
  blocker?: string;    // If NO
}
```

---

## State Machine Transitions

### State Diagram

```
                    ┌─────────────────────────────────────┐
                    │              INIT                   │
                    │         (preflight only)            │
                    └──────────────┬──────────────────────┘
                                   │ always
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                               DISCOVER                                       │
│                            (Phase 1)                                         │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │ CP1 pass (WARN OK)
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                DEFINE                                        │
│                            (Phase 2)                                         │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    │                             │
              CP2 + FULL                    CP2 + SPEED
                    │                             │
                    ▼                             │
┌──────────────────────────────┐                  │
│          DEVELOP             │                  │
│         (Phase 3)            │                  │
└──────────────┬───────────────┘                  │
               │ CP3 pass (WARN OK)               │
               ▼                                  │
┌─────────────────────────────────────────────────┴───────────────────────────┐
│                               VERIFY                                         │
│                            (Phase 4)                                         │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    │                             │
           CP4 APPROVED                    CP4 + SPEED
              + FULL                              │
                    │                             │
                    ▼                             │
┌──────────────────────────────┐                  │
│         DECOMPOSE            │                  │
│         (Phase 5)            │                  │
└──────────────┬───────────────┘                  │
               │ beads filed                      │
               ▼                                  │
┌──────────────────────────────┐                  │
│          VALIDATE            │                  │
│         (Phase 6)            │                  │
└──────────────┬───────────────┘                  │
               │ bv passes + Oracle               │
               ▼                                  │
┌──────────────────────────────┐                  │
│           ASSIGN             │                  │
│         (Phase 7)            │                  │
└──────────────┬───────────────┘                  │
               │ tracks assigned                  │
               ▼                                  │
┌─────────────────────────────────────────────────┴───────────────────────────┐
│                                READY                                         │
│                            (Phase 8)                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Transition Rules

| From | To | Condition | Context Update |
|------|----|-----------|----------------|
| INIT | DISCOVER | Always | `mode`, `id` set |
| DISCOVER | DEFINE | CP1 pass (WARN OK) | `research.start` populated |
| DEFINE | DEVELOP | CP2 pass + FULL mode | `decisions` set |
| DEFINE | VERIFY | CP2 pass + SPEED mode | (skip DEVELOP) |
| DEVELOP | VERIFY | CP3 pass (WARN OK) | `research.verify` populated |
| VERIFY | DECOMPOSE | CP4 APPROVED + FULL | `oracle_verdict = APPROVED` |
| VERIFY | READY | CP4 pass + SPEED mode | (skip 5,6,7) |
| DECOMPOSE | VALIDATE | beads filed | `artifacts.beads` populated |
| VALIDATE | ASSIGN | bv passes + Oracle | dependencies validated |
| ASSIGN | READY | tracks assigned | `artifacts.track_assignments` populated |

### Error Transitions

| State | Error | Action |
|-------|-------|--------|
| VERIFY | Oracle NEEDS_REVISION | [R] Revise → retry (max 2) |
| VERIFY | Spike NO/TIMEOUT | HALT for user decision |
| VALIDATE | bv cycle detected | Auto-fix or HALT |
| Any | Unrecoverable | [A] Abort → save progress |

---

## Research Hooks

### research-start (Phase 1)

**Trigger:** Start of DISCOVER phase
**Agents:** Locator, Pattern, CODEMAPS, Architecture
**Duration:** ~15s
**Output:** `research.start` in context

```python
Task(
  description="Research: Gather initial context",
  prompt="""
  Parallel agents:
  - Locator: Find files matching user request
  - Pattern: Identify existing patterns in codebase
  - CODEMAPS: Load architecture context
  - Architecture: Understand system structure
  
  Return: Merged research result
  """
)
```

### research-verify (Phase 3→4)

**Trigger:** End of DEVELOP phase (before VERIFY)
**Agents:** Analyzer, Pattern, Impact, Web
**Duration:** ~20s
**Output:** `research.verify` in context

```python
Task(
  description="Research: Verify design decisions",
  prompt="""
  Parallel agents:
  - Analyzer: Deep analysis of proposed changes
  - Pattern: Validate pattern usage
  - Impact: Assess change impact
  - Web: External docs, examples, best practices
  
  Return: Merged verification result
  """
)
```

---

## Related Files

| File | Purpose |
|------|---------|
| [../SKILL.md](../SKILL.md) | Main design skill entry point |
| [session-init.md](session-init.md) | INIT preflight procedures |
| [apc-checkpoints.md](apc-checkpoints.md) | A/P/C checkpoint details |
| [double-diamond.md](double-diamond.md) | Legacy Double Diamond (deprecated) |
| [../../conductor/references/planning/pipeline.md](../../conductor/references/planning/pipeline.md) | Legacy planning pipeline (deprecated) |
| [../../conductor/references/research/hooks/research-start.md](../../conductor/references/research/hooks/research-start.md) | Research-start hook details |
| [../../conductor/references/research/hooks/research-verify.md](../../conductor/references/research/hooks/research-verify.md) | Research-verify hook details |

---

## Quick Reference

```
SPEED (4 phases):  ds → DISCOVER → DEFINE → VERIFY → READY → ci
FULL (8 phases):   ds → DISCOVER → DEFINE → DEVELOP → VERIFY →
                       DECOMPOSE → VALIDATE → ASSIGN → READY → [O]/[S]

Research:          2 hooks (start + verify), ~35s max
Checkpoints:       A/P/C at phases 1-4 (FULL only)
Oracle:            Phase 4 (design) + Phase 6 (beads, FULL only)
Spikes:            Phase 4 for HIGH risk items (FULL only)
```

---

## Oracle Revision Loop

When Oracle returns NEEDS_REVISION at Phase 4 (FULL mode only):

```
Oracle returns NEEDS_REVISION
       │
       ▼
  1. HALT execution
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

### Prompt Template

```
⚠️ Oracle audit: NEEDS_REVISION

Issues found:
1. [Line 45] Missing error handling for edge case X
2. [Line 78] Unclear acceptance criteria for feature Y

[R] Revise - edit design.md and re-run Oracle
[S] Skip - accept risk and continue
[A] Abort - save progress for later

Retries remaining: 2
```

### Behavior

| Choice | Action |
|--------|--------|
| **[R]** | User edits design.md, Oracle re-runs (decrement retries) |
| **[S]** | Log `skip_accepted: true` in metadata, show warning, continue |
| **[A]** | Save `pipeline.current_phase: 4`, suggest `/conductor-handoff` |

### Max Retries

After 2 retries with NEEDS_REVISION:
```
Oracle still reports issues after 2 revision attempts.
[S] Skip anyway (not recommended)
[A] Abort and get human review
```

### SPEED Mode

In SPEED mode, Oracle issues are logged as warnings but do not halt:
```
⚠️ Oracle warnings (SPEED mode - non-blocking):
• Missing error handling for edge case X
Continuing to Phase 5...
```

---

## Phase Progress Indicator

Display progress at each phase transition:

### FULL Mode (8 phases)

```
┌─────────────────────────────────────────────────────────────┐
│ 📍 Phase 3/8: DEVELOP                                       │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━░░░░░░░░░░░░░░░░░░░░░ 37%        │
│                                                             │
│ ✅ DISCOVER  ✅ DEFINE  ▶️ DEVELOP  ○ VERIFY                │
│ ○ DECOMPOSE  ○ VALIDATE  ○ ASSIGN  ○ READY                 │
└─────────────────────────────────────────────────────────────┘
```

### SPEED Mode (4 phases)

```
┌─────────────────────────────────────────────────────────────┐
│ 📍 Phase 2/4: DEFINE (SPEED mode)                           │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━░░░░░░░░░░░░░░░░░░░░░ 50%        │
│                                                             │
│ ✅ DISCOVER  ▶️ DEFINE  ○ VERIFY  ○ READY                  │
└─────────────────────────────────────────────────────────────┘
```

### Phase Transition Message

At each phase transition:
```
────────────────────────────────────────────────────
📍 Entering Phase 4: VERIFY
   Purpose: Oracle audit + risk assessment
   Duration: ~5 minutes
────────────────────────────────────────────────────
```

### Compact Mode

For minimal output, show single line:
```
📍 [3/8] DEVELOP ━━━━━━━━━░░░░░░░░░░░ 37%
```
