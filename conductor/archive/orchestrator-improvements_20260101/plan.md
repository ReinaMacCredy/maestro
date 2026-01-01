# Plan: Orchestrator Skill Improvements

## Phase 1: Worker Protocol Enhancements

### 1.1 Track Thread Pattern
- [ ] 1.1.1 Add TRACK_THREAD variable to worker-prompt.md template
- [ ] 1.1.2 Add `summarize_thread(thread_id=TRACK_THREAD)` to START step
- [ ] 1.1.3 Add self-messaging template for COMPLETE step (Learnings/Gotchas/Next notes)

### 1.2 Per-Bead Loop Restructure
- [ ] 1.2.1 Restructure STEP 2 (EXECUTE) into explicit per-bead loop
- [ ] 1.2.2 Add START sub-step (register, read thread, reserve, claim)
- [ ] 1.2.3 Add WORK sub-step (implement, check inbox)
- [ ] 1.2.4 Add COMPLETE sub-step (close, report, save context, release)
- [ ] 1.2.5 Add NEXT sub-step (loop continuation logic)

### 1.3 AGENTS.md Tool Preferences
- [ ] 1.3.1 Add Tool Preferences section placeholder to worker-prompt.md
- [ ] 1.3.2 Document how to populate from project AGENTS.md

## Phase 2: Monitoring & Verification

### 2.1 Enhanced Monitoring
- [ ] 2.1.1 Update monitoring.md to prioritize `bv --robot-triage`
- [ ] 2.1.2 Add `jq '.quick_ref'` extraction pattern
- [ ] 2.1.3 Keep fetch_inbox/search_messages as secondary

### 2.2 Lingering Beads Check
- [ ] 2.2.1 Add verification step to workflow.md Phase 7 (before epic close)
- [ ] 2.2.2 Add `bd list --parent=<epic-id> --status=open` command
- [ ] 2.2.3 Add user prompt for handling lingering beads

## Phase 3: Auto-Detect Parallel Routing

### 3.1 Fix planTasks Population
- [ ] 3.1.1 Verify fb command saves planTasks to metadata.json
- [ ] 3.1.2 Verify fb command saves beadToTask mapping
- [ ] 3.1.3 Add crossTrackDeps array if missing

### 3.2 Implement Auto-Detect Logic
- [ ] 3.2.1 Add dependency graph analysis to implement.md
- [ ] 3.2.2 Add runtime verification via `bd list --json`
- [ ] 3.2.3 Add auto-generate Track Assignments logic
- [ ] 3.2.4 Add routing to orchestrator when ≥2 independent beads

## Phase 4: Documentation & Validation

### 4.1 Update References
- [ ] 4.1.1 Update orchestrator SKILL.md quick reference
- [ ] 4.1.2 Update conductor implement.md with auto-routing
- [ ] 4.1.3 Add new reference: auto-routing.md

### 4.2 Automated Verification
- [ ] 4.2.1 Run validation on all modified files
- [ ] 4.2.2 Test worker-prompt with sample track
- [ ] 4.2.3 Verify links and cross-references

## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.*, 1.3.* | skills/orchestrator/references/worker-prompt.md | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/orchestrator/references/monitoring.md, workflow.md | - |
| 3 | RedStone | 3.1.*, 3.2.* | skills/conductor/references/workflows/implement.md, beads-integration.md | 1.1.3 |
| 4 | PurpleMoon | 4.1.*, 4.2.* | skills/orchestrator/SKILL.md, references/*.md | 1.*, 2.*, 3.* |

## Cross-Track Dependencies

- Track 3 (3.2.1) depends on Track 1 (1.1.3) - needs track thread pattern defined first
- Track 4 depends on all other tracks - documentation update

## Estimated Duration

- Phase 1: 1.5 hours
- Phase 2: 0.5 hours
- Phase 3: 1 hour
- Phase 4: 0.5 hours
- **Total: ~3.5 hours**
