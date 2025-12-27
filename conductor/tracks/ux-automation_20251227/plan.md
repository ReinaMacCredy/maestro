---
track_id: ux-automation_20251227
version: 1.0
status: draft
---

# Plan: UX Automation & State Machine

## Epic 1: Shared Reference Files

Create the foundational shared reference files that will be imported by commands.

### 1.1 Create state-machine.md
- [ ] Create `skills/conductor/references/shared/` directory
- [ ] Define state enum (INIT, DESIGNED, TRACKED, FILED, REVIEWED, IMPLEMENTING, DONE, ARCHIVED)
- [ ] Document valid transitions table with STRICT/SOFT classification
- [ ] Add transition validation pseudo-code

**Acceptance:** File exists with complete state definitions

### 1.2 Create suggestions.md
- [ ] Define state → suggestion mapping table
- [ ] Document output format template with box drawing
- [ ] Include examples for each state

**Acceptance:** File exists with all 8 states mapped

### 1.3 Create git-preflight.md
- [ ] Document branch detection logic (main/master check)
- [ ] Document clean/dirty state check
- [ ] Document branch creation flow with `-v2` fallback
- [ ] Include complete bash script

**Acceptance:** File exists with working bash script

---

## Epic 2: Metadata Schema Update

Update metadata.json schema to include workflow state tracking.

### 2.1 Update metadata.schema.json
- [ ] Add `workflow` object schema with required fields
- [ ] Add state enum validation
- [ ] Add history array schema
- [ ] Ensure backward compatibility (workflow is optional)

**Acceptance:** Schema validates new and old metadata.json files

### 2.2 Update metadata creation in newTrack.toml
- [ ] Add workflow object initialization in Phase 1.3
- [ ] Set initial state to INIT
- [ ] Add first history entry

**Acceptance:** New tracks have workflow object

---

## Epic 3: Git Preflight in newTrack

Add git branch checking before track creation.

### 3.1 Add Phase 0.5 to newTrack.toml
- [ ] Insert git preflight check after setup validation
- [ ] Detect current branch
- [ ] Check if on main/master
- [ ] Check for uncommitted changes (HALT if dirty)

**Acceptance:** Dirty main/master HALTs with error message

### 3.2 Implement branch creation prompt
- [ ] Prompt user: "On main. Create feat/{id}? [Y/n]"
- [ ] Check if branch already exists
- [ ] Offer `-v2` suffix if exists
- [ ] Run git fetch before checkout -b
- [ ] Store branch name in workflow.branch

**Acceptance:** Branch created and stored in metadata

---

## Epic 4: Auto-Archive in finish

Remove A/K prompt and implement auto-archive.

### 4.1 Remove A/K prompt from Phase 5
- [ ] Delete archive choice prompt code
- [ ] Default to archive behavior
- [ ] Keep backward-compatible with existing finish-state.json

**Acceptance:** No A/K prompt appears during finish

### 4.2 Implement --keep flag
- [ ] Add flag parsing in Phase 0
- [ ] Skip archive if --keep provided
- [ ] Set workflow.keep = true
- [ ] Set workflow.state = DONE (not ARCHIVED)

**Acceptance:** --keep flag prevents archiving

### 4.3 Add pre-archive validation
- [ ] Check for open beads before archiving
- [ ] HALT with count if open beads found
- [ ] Implement --force to bypass check
- [ ] Update workflow.state to ARCHIVED

**Acceptance:** Open beads HALT unless --force

---

## Epic 5: Suggestion Output

Add → Next suggestions to all commands.

### 5.1 Update finish.toml completion message
- [ ] Add suggestion box after completion summary
- [ ] Show `→ Next: ds (start new work)`
- [ ] Update workflow.state on completion

**Acceptance:** Finish shows suggestion box

### 5.2 Update newTrack.toml completion message
- [ ] Add suggestion box after track creation
- [ ] Show `→ Next: fb (file beads)` or `Start epic {id}`
- [ ] Update workflow.state to TRACKED

**Acceptance:** newTrack shows suggestion box

### 5.3 Update design.toml completion message
- [ ] Verify existing handoff block format
- [ ] Ensure `→ Next: /conductor-newtrack {id}` is shown
- [ ] Update workflow.state to DESIGNED (in metadata if track exists)

**Acceptance:** ds shows suggestion box (already implemented, verify)

### 5.4 Add suggestions to fb completion
- [ ] Add suggestion to beads skill output
- [ ] Show `→ Next: rb (review beads)`

**Acceptance:** fb shows suggestion

### 5.5 Add suggestions to rb completion
- [ ] Add suggestion to beads skill output
- [ ] Show `→ Next: bd ready (start work)`

**Acceptance:** rb shows suggestion

---

## Epic 6: State Transition Updates

Update commands to log state transitions.

### 6.1 Add transition logging to newTrack
- [ ] Update workflow.state to TRACKED after spec/plan generated
- [ ] Append to workflow.history

**Acceptance:** State logged in metadata

### 6.2 Add transition logging to fb
- [ ] Update workflow.state to FILED after beads created
- [ ] Append to workflow.history

**Acceptance:** State logged in metadata

### 6.3 Add transition logging to rb
- [ ] Update workflow.state to REVIEWED after review complete
- [ ] Append to workflow.history

**Acceptance:** State logged in metadata

### 6.4 Add transition logging to implement
- [ ] Update workflow.state to IMPLEMENTING when task claimed
- [ ] Update to DONE when all beads closed
- [ ] Append to workflow.history

**Acceptance:** State logged in metadata

### 6.5 Add transition logging to finish
- [ ] Update workflow.state to ARCHIVED after archive
- [ ] Append to workflow.history

**Acceptance:** State logged in metadata

---

## Summary

| Epic | Tasks | Est. Hours |
|------|-------|------------|
| Epic 1: Shared Reference Files | 3 | 1.0 |
| Epic 2: Metadata Schema Update | 2 | 0.5 |
| Epic 3: Git Preflight | 2 | 1.5 |
| Epic 4: Auto-Archive | 3 | 1.5 |
| Epic 5: Suggestion Output | 5 | 1.5 |
| Epic 6: State Transitions | 5 | 1.5 |
| **Total** | **20** | **7.5** |

## Dependencies

```
Epic 1 (Shared Files)
    ↓
Epic 2 (Schema) ──→ Epic 3 (Git Preflight)
    ↓                    ↓
Epic 6 (Transitions) ←───┘
    ↓
Epic 4 (Auto-Archive)
    ↓
Epic 5 (Suggestions)
```

## Execution Order

1. **Epic 1** - Foundation (no deps)
2. **Epic 2** - Schema (depends on Epic 1)
3. **Epic 3** - Git Preflight (depends on Epic 2)
4. **Epic 4** - Auto-Archive (can parallel with Epic 3)
5. **Epic 5** - Suggestions (depends on Epic 4)
6. **Epic 6** - Transitions (can parallel with Epic 5)
