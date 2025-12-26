# Plan: Context Engineering Integration

## Epic 1: Context Engineering Workflow Structure [PARALLEL]

Create the new workflow directory and core orchestration file.

### Task 1.1: Create directory structure
- [ ] Create `workflows/context-engineering/` directory
- [ ] Create `workflows/context-engineering/references/` subdirectory
- **Files**: `workflows/context-engineering/`
- **Verify**: Directory exists

### Task 1.2: Create session-lifecycle.md
- [ ] Write RECALL phase documentation
- [ ] Write ROUTE phase documentation (reference to design + execution routing)
- [ ] Include integration points with preflight and implement
- [ ] Add version header and metadata
- **Files**: `workflows/context-engineering/session-lifecycle.md`
- **Verify**: File exists, sections complete

### Task 1.3: Create anchored-state-format.md
- [ ] Document anchored format template
- [ ] Define [PRESERVE] marker rules
- [ ] Document section compression rules
- [ ] Add version header: `<!-- session-context v1 -->`
- **Files**: `workflows/context-engineering/references/anchored-state-format.md`
- **Verify**: Template complete

### Task 1.4: Create design-routing-heuristics.md
- [ ] Document weighted scoring criteria
- [ ] Include examples for SPEED vs FULL
- [ ] Document soft zone (4-6) behavior
- [ ] Add escalation rules
- **Files**: `workflows/context-engineering/references/design-routing-heuristics.md`
- **Verify**: Scoring table complete

---

## Epic 2: Execution Routing Pattern [PARALLEL]

Add execution routing to agent-coordination patterns.

### Task 2.1: Create execution-routing.md
- [ ] Document TIER 1 weighted scoring
- [ ] Document TIER 2 compound conditions
- [ ] Define SINGLE_AGENT vs PARALLEL_DISPATCH outcomes
- [ ] Reference parallel-dispatch.md for actual dispatch
- [ ] Add visible feedback examples
- **Files**: `workflows/agent-coordination/patterns/execution-routing.md`
- **Verify**: Pattern complete, integrates with parallel-dispatch

### Task 2.2: Update agent-coordination workflow.md
- [ ] Add execution-routing to patterns table
- [ ] Add brief description
- **Files**: `workflows/agent-coordination/workflow.md`
- **Verify**: Pattern listed in table

---

## Epic 3: Design Routing Integration

Integrate COMPLEXITY_EXPLAINER into ds skill.

### Task 3.1: Add COMPLEXITY_EXPLAINER to design skill
- [ ] Add complexity scoring section after trigger detection
- [ ] Add COMPLEXITY_EXPLAINER display format
- [ ] Add SPEED vs FULL routing logic
- [ ] Add soft zone (4-6) user prompt
- [ ] Add default to FULL after 2 prompts
- [ ] Add escalation marker `[E]`
- **Files**: `skills/design/SKILL.md`
- **Verify**: Manual test with `ds` command
- **depends**: Task 1.4 (heuristics reference)

---

## Epic 4: Implement.md Phase 2b

Add execution routing phase to implementation workflow.

### Task 4.1: Add Phase 2b to implement.md
- [ ] Insert Phase 2b: Execution Routing after Phase 2
- [ ] Reference execution-routing.md pattern
- [ ] Add execution_mode to implement_state.json
- [ ] Branch logic for SINGLE vs PARALLEL
- [ ] Add visible feedback output
- **Files**: `workflows/implement.md`
- **Verify**: Phase 2b documented, state updated
- **depends**: Task 2.1 (execution-routing pattern)

### Task 4.2: Add degradation evaluation hook
- [ ] Add "evaluate degradation after each task" to Phase 3
- [ ] Reference extended Progress Checkpointing
- **Files**: `workflows/implement.md`
- **Verify**: Hook documented
- **depends**: Task 5.1 (degradation signals)

---

## Epic 5: Extended Progress Checkpointing

Add degradation signals to existing checkpointing.

### Task 5.1: Add Degradation Signals section
- [ ] Add `## Degradation Signals` section to beads/workflow.md
- [ ] Document per-tool thresholds (file_write: 3, bash: 3, search: 5, file_read: 10)
- [ ] Document signal definitions (tool_repeat, backtrack, quality_drop, contradiction)
- [ ] Document 2+ signals → trigger compression
- [ ] Document evaluation timing (after each task)
- **Files**: `workflows/beads/workflow.md`
- **Verify**: Section exists, thresholds documented

### Task 5.2: Create checkpoint.md facade
- [ ] Create thin facade file
- [ ] Link to Progress Checkpointing section
- [ ] Include quick reference
- **Files**: `workflows/conductor/checkpoint.md`
- **Verify**: File exists, links work

---

## Epic 6: Extended Handoff Protocol

Extend handoff for SA mode with anchored format.

### Task 6.1: Add Anchored Format section to beads-session.md
- [ ] Add `## Anchored Format (SA Mode)` section
- [ ] Document save location: `.conductor/session-context.md`
- [ ] Reference anchored-state-format.md template
- [ ] Add validation for PRESERVE sections
- **Files**: `workflows/conductor/beads-session.md`
- **Verify**: Section exists
- **depends**: Task 1.3 (anchored format template)

### Task 6.2: Create remember.md facade
- [ ] Create thin facade file
- [ ] Link to Handoff Protocol section
- [ ] Include quick reference for SA vs MA
- **Files**: `workflows/conductor/remember.md`
- **Verify**: File exists, links work

---

## Epic 7: RECALL Integration

Add RECALL hook to preflight.

### Task 7.1: Add RECALL hook to preflight-beads.md
- [ ] Add RECALL step after session state creation
- [ ] Load `.conductor/session-context.md` if exists
- [ ] Cold start: create skeleton with version header
- [ ] Display token budget with thresholds
- [ ] Verify context contract
- **Files**: `workflows/conductor/preflight-beads.md`
- **Verify**: Hook documented, cold start handled
- **depends**: Task 1.3 (anchored format template)

---

## Epic 8: Documentation Updates [PARALLEL]

Update documentation and indexes.

### Task 8.1: Update workflows/README.md
- [ ] Add context-engineering directory link
- [ ] Add brief description of routing system
- **Files**: `workflows/README.md`
- **Verify**: Links work

### Task 8.2: Create track state files
- [ ] Create metadata.json
- [ ] Create .track-progress.json
- [ ] Create .fb-progress.json
- **Files**: `conductor/tracks/context-engineering_20251226/`
- **Verify**: State files exist

---

## Dependency Graph

```
Epic 1 (Structure) ──┬──► Epic 3 (Design Routing)
                     │         │
                     │         ▼
                     │    Epic 4.1 (Phase 2b)
                     │         │
Epic 2 (Exec Route)──┘         │
                               ▼
                          Epic 4.2 (Degradation hook)
                               │
Epic 5 (Checkpointing) ────────┘
                               │
Epic 6 (Handoff) ◄─────────────┘
     │
     ▼
Epic 7 (RECALL)

Epic 8 (Docs) ──► [PARALLEL with all]
```

## Parallel Execution Groups

| Group | Epics | Dependencies |
|-------|-------|--------------|
| G1 | Epic 1, Epic 2, Epic 8 | None (can start immediately) |
| G2 | Epic 3, Epic 5, Epic 6 | After G1 |
| G3 | Epic 4, Epic 7 | After G2 |

## Estimated Time

| Epic | Time |
|------|------|
| Epic 1 | 30 min |
| Epic 2 | 20 min |
| Epic 3 | 25 min |
| Epic 4 | 20 min |
| Epic 5 | 20 min |
| Epic 6 | 15 min |
| Epic 7 | 20 min |
| Epic 8 | 10 min |
| **Total** | **2.5h** (with parallel: ~1.5h) |
