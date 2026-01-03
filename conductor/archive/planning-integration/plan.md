# Implementation Plan: Planning Pipeline + Orchestrator Enhancement

Track ID: planning-integration
Created: 2026-01-03
Epic: planning-integration

---

## Phase 1: Core Infrastructure

### 1.1 Add `pl` Routing to maestro-core

- [ ] **1.1.1** Update `maestro-core/SKILL.md` routing table
  - Add: `pl, /plan, "plan feature" | planning | 6-phase risk-based planning`
  - File: `skills/maestro-core/SKILL.md`
  
- [ ] **1.1.2** Update `maestro-core/references/routing-table.md`
  - Add `pl` trigger with full description
  - Add intent detection keywords
  - File: `skills/maestro-core/references/routing-table.md`

- [ ] **1.1.3** Update workflow chain diagram
  - Show dual path: ds → design.md ← pl
  - File: `skills/maestro-core/references/workflow-chain.md`

### 1.2 Create Planning Pipeline References

- [ ] **1.2.1** Create `conductor/references/planning/` directory
  - File: `skills/conductor/references/planning/`

- [ ] **1.2.2** Create `pipeline.md` - Main 6-phase flow
  - Phases: Discovery → Synthesis → Verification → Decomposition → Validation → Track Planning
  - File: `skills/conductor/references/planning/pipeline.md`

- [ ] **1.2.3** Create `design-template.md` - Unified design.md template
  - Sections: Problem, Discovery, Approach, Design, Spike Results, Track Planning
  - File: `skills/conductor/references/planning/design-template.md`

- [ ] **1.2.4** Create `spikes.md` - Spike workflow
  - Creation, execution, capture learnings
  - File: `skills/conductor/references/planning/spikes.md`

---

## Phase 2: Spike Infrastructure

### 2.1 Spike Storage

- [x] **2.1.1** Create `conductor/spikes/` directory with README
  - Done in design session
  - File: `conductor/spikes/README.md`

### 2.2 Spike Workflow Documentation

- [ ] **2.2.1** Document spike creation flow
  - bd create for spike bead
  - mkdir for spike directory
  - README.md template
  - File: `skills/conductor/references/planning/spikes.md`

- [ ] **2.2.2** Document spike execution via Task()
  - Time-box enforcement
  - Output location
  - Success criteria
  - File: `skills/conductor/references/planning/spikes.md`

- [ ] **2.2.3** Document spike learnings capture
  - bd close with result
  - Update design.md Section 5
  - Embed in beads
  - File: `skills/conductor/references/planning/spikes.md`

---

## Phase 3: Orchestrator Enhancement

### 3.1 Track Thread Protocol

- [ ] **3.1.1** Create `orchestrator/references/track-threads.md`
  - Thread ID format: `track:<agent>:<epic>`
  - Two-thread architecture (epic + track)
  - File: `skills/orchestrator/references/track-threads.md`

- [ ] **3.1.2** Update `orchestrator/SKILL.md` Quick Reference
  - Add track thread row
  - Reference track-threads.md
  - File: `skills/orchestrator/SKILL.md`

### 3.2 Enhanced Worker Protocol

- [ ] **3.2.1** Update `orchestrator/references/worker-prompt.md`
  - Add Spike Learnings section
  - Add structured context template (Learnings/Gotchas/Next)
  - File: `skills/orchestrator/references/worker-prompt.md`

- [ ] **3.2.2** Update `orchestrator/references/workflow.md`
  - Add Option C: From Planning Pipeline (pl)
  - Spike learnings extraction
  - File: `skills/orchestrator/references/workflow.md`

### 3.3 Planning Integration

- [ ] **3.3.1** Document spike learnings → worker prompt injection
  - Parse design.md Section 5
  - Map to beads by file scope
  - Inject into worker prompt template
  - File: `skills/orchestrator/references/workflow.md`

---

## Phase 4: Validation & Error Handling

### 4.1 Planning Validation Gates

- [ ] **4.1.1** Define planning-specific gates
  - discovery-complete (WARN)
  - risk-assessed (HALT if HIGH without spike)
  - spikes-resolved (HALT if unresolved)
  - execution-ready (HALT if missing learnings)
  - File: `skills/conductor/references/planning/pipeline.md`

### 4.2 Error Handling

- [ ] **4.2.1** Document spike failure scenarios
  - Timeout handling
  - NO result alternatives
  - Missing spike code fallback
  - File: `skills/conductor/references/planning/spikes.md`

- [ ] **4.2.2** Document spike integrity check
  - At planning complete
  - At orchestrator spawn
  - File: `skills/conductor/references/planning/spikes.md`

---

## Phase 5: Metadata & State

### 5.1 metadata.json Schema

- [ ] **5.1.1** Update metadata.json schema
  - Add `planning` section
  - Add planning state machine
  - File: `skills/conductor/references/schemas/metadata.schema.json`

- [ ] **5.1.2** Document planning state transitions
  - unplanned → discovery → synthesized → verified → decomposed → validated → track_planned → executing
  - File: `skills/conductor/references/planning/pipeline.md`

---

## Phase 6: Documentation & CODEMAPS

### 6.1 Update Documentation

- [ ] **6.1.1** Update `conductor/SKILL.md`
  - Add `pl` handoff reference
  - Add planning state mention
  - File: `skills/conductor/SKILL.md`

- [ ] **6.1.2** Update `conductor/CODEMAPS/overview.md`
  - Add planning pipeline to workflow diagram
  - Add `conductor/spikes/` to directory structure
  - File: `conductor/CODEMAPS/overview.md`

### 6.2 Integration Tests (Manual)

- [ ] **6.2.1** Test `pl` trigger routing
- [ ] **6.2.2** Test spike creation and execution
- [ ] **6.2.3** Test track thread read/write
- [ ] **6.2.4** Test spike learnings injection

---

## Track Assignments

| Track | Tasks | File Scope | Depends On |
|-------|-------|------------|------------|
| A | 1.1.1, 1.1.2, 1.1.3 | `skills/maestro-core/**` | - |
| B | 1.2.1, 1.2.2, 1.2.3, 1.2.4, 2.2.1, 2.2.2, 2.2.3, 4.1.1, 4.2.1, 4.2.2, 5.1.2 | `skills/conductor/references/planning/**` | - |
| C | 3.1.1, 3.1.2, 3.2.1, 3.2.2, 3.3.1 | `skills/orchestrator/**` | - |
| D | 5.1.1 | `skills/conductor/references/schemas/**` | B |
| E | 6.1.1, 6.1.2 | `skills/conductor/SKILL.md`, `conductor/CODEMAPS/**` | A, B, C |

---

## Automated Verification

```bash
# Verify routing table
grep -q "pl.*planning" skills/maestro-core/SKILL.md

# Verify planning directory exists
ls skills/conductor/references/planning/

# Verify spike directory exists
ls conductor/spikes/

# Verify track-threads.md exists
ls skills/orchestrator/references/track-threads.md

# Verify metadata schema updated
grep -q "planning" skills/conductor/references/schemas/metadata.schema.json
```

---

## Summary

| Phase | Tasks | Est. Hours |
|-------|-------|------------|
| 1. Core Infrastructure | 7 | 3 |
| 2. Spike Infrastructure | 3 | 1.5 |
| 3. Orchestrator Enhancement | 5 | 2.5 |
| 4. Validation & Error Handling | 3 | 1.5 |
| 5. Metadata & State | 2 | 1 |
| 6. Documentation | 4 | 2 |
| **Total** | **24** | **~12** |
