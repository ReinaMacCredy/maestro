# Spec: Unified DS Pipeline

## Overview

Merge the Design Session (`ds`) and Planning Pipeline (`pl`) into a single 8-phase unified pipeline with seamless transitions, consolidated research, and continuous context flow.

## Requirements

### Functional Requirements

#### FR1: Unified 8-Phase Pipeline
- **FR1.1:** `ds` command triggers the unified pipeline (phases 1-8)
- **FR1.2:** Phases execute in order: DISCOVER → DEFINE → DEVELOP → VERIFY → DECOMPOSE → VALIDATE → ASSIGN → READY
- **FR1.3:** No manual transition message between phases
- **FR1.4:** Phase state tracked in `metadata.json.pipeline.current_phase`

#### FR2: Mode-Aware Execution
- **FR2.1:** Complexity scoring determines mode (< 4 = SPEED, > 6 = FULL)
- **FR2.2:** SPEED mode executes phases 1, 2, 4, 8 only
- **FR2.3:** FULL mode executes all 8 phases
- **FR2.4:** User can escalate from SPEED to FULL with `[E]` option

#### FR3: Consolidated Research
- **FR3.1:** `research-start` hook runs at Phase 1 start
- **FR3.2:** `research-verify` hook runs between Phase 3 and 4 (FULL mode only)
- **FR3.3:** Total research time under 40 seconds
- **FR3.4:** Research results stored in `pipeline_context.research`

#### FR4: Context Flow
- **FR4.1:** `pipeline_context` object accumulates through all phases
- **FR4.2:** Context persisted in `metadata.json.pipeline`
- **FR4.3:** Each phase can read all prior phase outputs
- **FR4.4:** Context survives session interruption/resumption

#### FR5: A/P/C Checkpoints
- **FR5.1:** A/P/C menu shown after phases 1-4 in FULL mode
- **FR5.2:** Oracle audit runs automatically before Phase 4 A/P/C menu
- **FR5.3:** Phases 5-8 execute automatically (no A/P/C)
- **FR5.4:** Auto-planning confirmation gate before Phase 5

#### FR6: Oracle Revision Loop
- **FR6.1:** NEEDS_REVISION verdict halts at Phase 4 (FULL mode)
- **FR6.2:** User options: [R] Revise, [S] Skip, [A] Abort
- **FR6.3:** Maximum 2 revision retries before manual review
- **FR6.4:** Skip logs warning but allows continuation

#### FR7: Legacy Compatibility
- **FR7.1:** `--legacy` flag runs old DS behavior
- **FR7.2:** `pl` standalone runs phases 5-8 with existing design.md
- **FR7.3:** Error if `pl` run without design.md

### Non-Functional Requirements

#### NFR1: Performance
- Research hooks complete within 20s each
- Total pipeline overhead < 60s for SPEED mode
- Phase transitions < 1s

#### NFR2: Reliability
- Context persisted after each phase
- Resumable after interruption
- Graceful degradation if research times out

#### NFR3: Usability
- Progress indicator for research hooks
- Clear phase transition messages
- Helpful error messages with suggested fixes

## Interface Contracts

### Input: `ds` Command

```
ds [--unified] [--legacy] [--full] [--speed]

Options:
  --unified   Use new unified pipeline (default after migration)
  --legacy    Use old DS + PL separation
  --full      Force FULL mode regardless of complexity
  --speed     Force SPEED mode regardless of complexity
```

### Output: Artifacts

| Phase | Artifact | Location |
|-------|----------|----------|
| 4 (VERIFY) | design.md | `conductor/tracks/<id>/design.md` |
| 5 (DECOMPOSE) | beads | `.beads/*.md` |
| 7 (ASSIGN) | plan.md | `conductor/tracks/<id>/plan.md` |
| 8 (READY) | metadata.json | `conductor/tracks/<id>/metadata.json` |

### State: metadata.json.pipeline

```json
{
  "pipeline": {
    "version": 1,
    "current_phase": 4,
    "mode": "FULL",
    "preflight_completed": true,
    "started_at": "2026-01-08T10:00:00Z",
    "research": {
      "start": { "completed": true, "duration_ms": 18000 },
      "verify": { "completed": true, "duration_ms": 12000 }
    },
    "validation": {
      "checkpoints_passed": ["CP1", "CP2", "CP3"],
      "oracle_verdict": null,
      "retries": 0
    }
  }
}
```

## Dependencies

| Dependency | Type | Required For |
|------------|------|--------------|
| `bd` CLI | External | Phases 5-7 (beads operations) |
| `bv` CLI | External | Phase 6 (validation) |
| Oracle tool | Internal | Phase 4 (audit) |
| Task() | Internal | Research hooks |

## Acceptance Criteria

See design.md Section 6.
