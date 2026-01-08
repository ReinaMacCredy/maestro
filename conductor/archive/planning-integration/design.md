# Design: Planning Pipeline + Orchestrator Enhancement

Generated: 2026-01-03
Mode: Design Session (ds)
Track ID: planning-integration

---

## 1. Problem Statement

Integrate two external workflow patterns into Maestro:
1. **Planning Pipeline** (`external/planning.md`) - Risk-based 6-phase planning with spikes
2. **Orchestrator Enhancement** (`external/orchestrator.md`) - Track thread pattern for bead-to-bead context

### Goals
- Add `pl` trigger for execution-focused planning (complement to `ds`)
- Unified design.md output from both `ds` and `pl` paths
- Spikes for HIGH risk validation before implementation
- Track threads for worker context preservation

### Non-Goals
- Replace existing `ds` workflow (complement, not replace)
- Change beads storage format

---

## 2. Discovery

### Existing Architecture

| Component | Location | Role |
|-----------|----------|------|
| maestro-core | `skills/maestro-core/` | Routing table, fallback policies |
| design | `skills/design/` | Double Diamond, A/P/C checkpoints |
| conductor | `skills/conductor/` | Track management, implement, finish |
| orchestrator | `skills/orchestrator/` | Multi-agent parallel execution |
| beads | `skills/beads/` | Issue tracking, bd CLI |

### Current Workflow
```
ds → design.md → cn → spec.md + plan.md → fb → ci/co
```

### Gaps Identified
- No risk-based planning phase
- No spike validation for HIGH risk items
- Workers lose context between beads
- No execution plan with track assignments before orchestrator

---

## 3. Approach

### Dual Routing Strategy

| Trigger | Mode | Use When |
|---------|------|----------|
| `ds` | Double Diamond | Unclear scope, need exploration |
| `pl` | Planning Pipeline | Clear feature, need risk analysis + execution plan |

### Routing Decision Heuristics

```
Explicit trigger?
├── "ds" → Double Diamond
├── "pl" → Planning Pipeline
└── None → Analyze intent:
    ├── Exploratory keywords → ds
    ├── Execution keywords → pl
    └── Ambiguous → ASK USER [D/P]
```

### Risk Classification (from Oracle)

| Level | Criteria | Verification |
|-------|----------|--------------|
| LOW | Pattern exists in codebase | Proceed |
| MEDIUM | Variation of existing pattern | Interface sketch |
| HIGH | Novel or external integration | Spike required |

---

## 4. Design

### 4.1 Updated Workflow Chain

```
User request
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│                    INTENT ROUTER (maestro-core)                 │
├─────────────────────────────────────────────────────────────────┤
│  ds (exploratory)              │       pl (execution-focused)  │
│  ───────────────               │       ────────────────────    │
│  • Double Diamond phases       │       • Discovery phase       │
│  • A/P/C checkpoints           │       • Synthesis (Oracle)    │
│  • Research at checkpoints     │       • Verification (spikes) │
│                                │       • Risk-based planning   │
└────────────────┬───────────────┴───────────────┬────────────────┘
                 │                               │
                 └───────────────┬───────────────┘
                                 ▼
                         design.md (unified)
                                 │
                                 ▼
                    cn (/conductor-newtrack)
                                 │
                                 ▼
                         spec.md + plan.md
                         (plan.md includes Track Assignments)
                                 │
                                 ▼
                         fb (file-beads)
                         (spike learnings embedded)
                                 │
                                 ▼
                         ci / co
                         (track threads for context)
```

### 4.2 Canonical Artifacts

| Artifact | Location | Source |
|----------|----------|--------|
| design.md | `conductor/tracks/<id>/design.md` | ds OR pl |
| spec.md | `conductor/tracks/<id>/spec.md` | cn |
| plan.md | `conductor/tracks/<id>/plan.md` | cn (includes Track Assignments) |
| spikes | `conductor/spikes/<track-id>/spike-xxx/` | pl Phase 3 |
| beads | `.beads/*.md` | fb |
| metadata | `conductor/tracks/<id>/metadata.json` | all phases |

### 4.3 Unified design.md Structure

```markdown
# Design: <Feature Name>

Generated: <date>
Mode: <ds | pl>
Track ID: <track-id>

---

## 1. Problem Statement
<from ds DISCOVER or pl Discovery>

## 2. Discovery
### Architecture Snapshot
### Existing Patterns
### Technical Constraints
### External References

## 3. Approach
### Gap Analysis (pl mode)
### Risk Map (pl mode)
### Recommended Approach
### Alternative Approaches

## 4. Design
### Core Concept
### Key Decisions
### Interfaces

## 5. Spike Results (pl mode, if any)
### Spike: <question>
- Result: YES/NO
- Learnings: ...
- Code reference: conductor/spikes/<track>/spike-xxx/

## 6. Track Planning (pl mode, if parallel execution expected)
### Tracks Summary
| Track | Agent | Beads | File Scope |
|-------|-------|-------|------------|

(Full Track Assignments go into plan.md)
```

### 4.4 Planning Pipeline Phases (pl mode)

```
┌─────────────────────────────────────────────────────────────────┐
│ Phase 1: DISCOVERY                                              │
│ • Parallel Task() agents: architecture, patterns, constraints   │
│ • Librarian: external patterns                                  │
│ • Output: design.md Section 2                                   │
├─────────────────────────────────────────────────────────────────┤
│ Phase 2: SYNTHESIS                                              │
│ • Oracle: gap analysis, approach options, risk map              │
│ • Output: design.md Section 3                                   │
├─────────────────────────────────────────────────────────────────┤
│ Phase 3: VERIFICATION                                           │
│ • HIGH risk → create spike in conductor/spikes/<track>/         │
│ • Execute spikes via Task() with time-box (30 min default)      │
│ • Output: design.md Section 5, updated approach                 │
├─────────────────────────────────────────────────────────────────┤
│ Phase 4: DECOMPOSITION                                          │
│ • fb (file-beads) with spike learnings embedded                 │
│ • Output: .beads/*.md                                           │
├─────────────────────────────────────────────────────────────────┤
│ Phase 5: VALIDATION                                             │
│ • bv --robot-suggest, --robot-insights, --robot-priority        │
│ • Oracle final review                                           │
│ • Output: validated dependency graph                            │
├─────────────────────────────────────────────────────────────────┤
│ Phase 6: TRACK PLANNING                                         │
│ • bv --robot-plan for parallel tracks                           │
│ • Assign file scopes (non-overlapping)                          │
│ • Generate agent names                                          │
│ • Output: design.md Section 6, plan.md Track Assignments        │
└─────────────────────────────────────────────────────────────────┘
```

### 4.5 Validation Gates (pl mode)

| Gate | After Phase | Enforcement |
|------|-------------|-------------|
| discovery-complete | 1 | WARN |
| risk-assessed | 2 | HALT if HIGH without spike |
| spikes-resolved | 3 | HALT if unresolved |
| execution-ready | 6 | HALT if missing learnings |

### 4.6 Orchestrator: Track Thread Protocol

```
┌─────────────────────────────────────────────────────────────────┐
│                    TWO-THREAD ARCHITECTURE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  EPIC THREAD                      TRACK THREAD                  │
│  thread_id: <epic-id>             thread_id: track:<agent>:<epic>│
│  ─────────────────                ───────────────────────────── │
│  • Progress reports               • Bead-to-bead learnings      │
│  • Blockers                       • Gotchas discovered          │
│  • Cross-track issues             • Next bead hints             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Worker Per-Bead Loop:
┌─────────────────────────────────────────────────────────────────┐
│ 1. START BEAD                                                   │
│    • summarize_thread(thread_id="track:<agent>:<epic>")         │
│    • file_reservation_paths(paths=[scope], reason=bead)         │
│    • bd update <bead> --status in_progress                      │
├─────────────────────────────────────────────────────────────────┤
│ 2. WORK ON BEAD                                                 │
│    • Implement requirements                                     │
│    • Check inbox periodically                                   │
│    • Escalate blockers to epic thread                           │
├─────────────────────────────────────────────────────────────────┤
│ 3. COMPLETE BEAD                                                │
│    • bd close <bead> --reason "..."                             │
│    • send_message to orchestrator (epic thread): status         │
│    • send_message to self (track thread): learnings/gotchas     │
│    • release_file_reservations()                                │
├─────────────────────────────────────────────────────────────────┤
│ 4. NEXT BEAD                                                    │
│    • Loop to START with next bead                               │
│    • Read track thread for context!                             │
└─────────────────────────────────────────────────────────────────┘
```

### 4.7 metadata.json Planning State

```json
{
  "track": "<track-id>",
  "workflow": {
    "state": "DESIGN",
    "mode": "planning",
    "design_path": "pl"
  },
  "planning": {
    "state": "track_planned",
    "phases_completed": ["discovery", "synthesis", "verification", "decomposition", "validation", "track_planning"],
    "spikes": [
      {
        "id": "spike-001",
        "question": "Can Stripe SDK work with Node 18?",
        "result": "YES",
        "path": "conductor/spikes/<track>/spike-001/"
      }
    ],
    "risk_map": {
      "high": ["stripe-integration"],
      "medium": ["user-entity-changes"],
      "low": ["api-endpoints"]
    }
  },
  "beads": {
    "status": "filed",
    "planTasks": { ... }
  }
}
```

**Planning State Machine:**
```
unplanned → discovery → synthesized → verified → decomposed → validated → track_planned → executing → complete
```

---

## 5. Implementation Plan

### Files to Create

| File | Purpose |
|------|---------|
| `conductor/references/planning/pipeline.md` | 6-phase planning flow |
| `conductor/references/planning/spikes.md` | Spike workflow |
| `conductor/references/planning/design-template.md` | Unified design.md template |
| `orchestrator/references/track-threads.md` | Single source for track thread protocol |
| `conductor/spikes/.gitkeep` | Spike storage directory |

### Files to Update

| File | Changes |
|------|---------|
| `maestro-core/SKILL.md` | Add `pl` to routing table, update workflow chain |
| `maestro-core/references/routing-table.md` | Add `pl` trigger |
| `orchestrator/SKILL.md` | Reference track-threads.md |
| `orchestrator/references/workflow.md` | Add Option C (planning pipeline source) |
| `orchestrator/references/worker-prompt.md` | Add spike learnings section, enhance context structure |
| `conductor/SKILL.md` | Add `pl` handoff, planning state |

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Spike timeout | Auto-close with TIMEOUT, escalate to user |
| Spike result NO, no alternative | HALT, require user decision |
| All components HIGH risk | Suggest split into smaller features |
| Worker context overflow | Summarize after N beads, max-history policy |
| File scope overlap | Validation check before orchestrator spawn |
| Missing spike code | Fallback chain: embedded → design.md → Oracle reconstruct |

---

## 7. Success Criteria

- [ ] `pl` trigger works and routes correctly
- [ ] design.md generated from `pl` contains discovery + approach + spike results
- [ ] Spikes created in `conductor/spikes/<track>/`
- [ ] Spike learnings embedded in beads
- [ ] Workers read track thread before each bead
- [ ] Workers write structured context (learnings/gotchas/next) after each bead
- [ ] Orchestrator can spawn workers from plan.md Track Assignments
- [ ] metadata.json tracks planning state

---

## 8. Next Steps

After design approval:
1. `cn` → Generate spec.md + plan.md
2. `fb` → File beads for implementation
3. `ci` or `co` → Implement changes
