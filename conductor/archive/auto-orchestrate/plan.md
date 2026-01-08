# Implementation Plan: Auto-Orchestration After Filing Beads

## Epic 1: Core Auto-Orchestration Logic

### 1.1 Create auto-orchestrate.md reference file
- [ ] **1.1.1** Create `skills/beads/references/auto-orchestrate.md`
- [ ] **1.1.2** Document the graph analysis algorithm using `bv --robot-triage`
- [ ] **1.1.3** Document Track Assignment generation logic
- [ ] **1.1.4** Document idempotency check via `metadata.json.beads.orchestrated`

### 1.2 Extend FILE_BEADS.md with Phase 6
- [ ] **1.2.1** Add Phase 6 section to `skills/beads/references/FILE_BEADS.md`
- [ ] **1.2.2** Add step 6.1: Query graph via `bv --robot-triage --graph-root <epic-id> --json`
- [ ] **1.2.3** Add step 6.2: Generate Track Assignments from ready/blocked beads
- [ ] **1.2.4** Add step 6.3: Update `metadata.json.beads.orchestrated = true`
- [ ] **1.2.5** Add step 6.4: Call orchestrator with generated assignments

### 1.3 Add orchestrated flag to metadata schema
- [ ] **1.3.1** Update `skills/conductor/references/schemas/metadata.schema.json` with `orchestrated` field
- [ ] **1.3.2** Update metadata.json template in track-init-beads.md

## Epic 2: Orchestrator Integration

### 2.1 Accept auto-generated Track Assignments
- [ ] **2.1.1** Update `skills/orchestrator/SKILL.md` to document auto-generated assignments
- [ ] **2.1.2** Update `skills/orchestrator/references/workflow.md` Phase 1 to accept in-memory assignments
- [ ] **2.1.3** Add validation for auto-generated vs manual Track Assignments

### 2.2 Add final rb review phase
- [ ] **2.2.1** Update `skills/orchestrator/references/workflow.md` to add Phase 7: Final Review
- [ ] **2.2.2** Document spawning `rb` sub-agent after all workers complete
- [ ] **2.2.3** Add completion summary after rb finishes

## Epic 3: Agent Mail Fallback

### 3.1 Sequential fallback when Agent Mail unavailable
- [ ] **3.1.1** Add Agent Mail availability check to auto-orchestrate flow
- [ ] **3.1.2** Implement fallback to `/conductor-implement` (sequential)
- [ ] **3.1.3** Add warning message: "Agent coordination unavailable - running sequential"

## Epic 4: Documentation & Learnings

### 4.1 Update AGENTS.md with learnings
- [ ] **4.1.1** Add command: `bv --robot-triage --graph-root <epic-id> --json`
- [ ] **4.1.2** Add gotcha: `metadata.json.beads.orchestrated` for idempotency
- [ ] **4.1.3** Add pattern: Auto-orchestration after fb

### 4.2 Update CODEMAPS
- [ ] **4.2.1** Update `conductor/CODEMAPS/overview.md` with new flow
- [ ] **4.2.2** Add auto-orchestrate to data flow diagram
