# Plan: /conductor-finish Integration

## Epic 1: Core Infrastructure
*Setup foundational files and structure*

### Tasks

- [x] **1.1** Create `skills/conductor/references/finish-workflow.md` with 4-phase workflow documentation
- [x] **1.2** Create `skills/conductor/references/validation/` directory
- [x] **1.3** Move `commands/compact/judge-prompt.md` → `skills/conductor/references/validation/judge-prompt.md`
- [x] **1.4** Move `commands/compact/rubrics.md` → `skills/conductor/references/validation/rubrics.md`
- [x] **1.5** Create `skills/conductor/references/validation/README.md` explaining reserved status
- [x] **1.6** Delete `commands/compact/` directory
- [x] **1.7** Create `conductor/AGENTS.md` with template header

---

## Epic 2: Conductor SKILL.md Updates
*Add /conductor-finish command to conductor skill*

### Tasks

- [x] **2.1** Add `/conductor-finish [id]` to Slash Commands table with description and flags
- [x] **2.2** Add intent mappings: "finish track", "complete track", "doc-sync" → /conductor-finish
- [x] **2.3** Add "Track Completion" section documenting the 4-phase workflow
- [x] **2.4** Update Context Loading section to include conductor/AGENTS.md
- [x] **2.5** Add Epic Completion Behavior to set status = "ready_to_finish" after last epic

---

## Epic 3: finish-workflow.md Implementation
*Detailed workflow instructions for each phase*

### Tasks

- [x] **3.1** Write Trigger section (auto-trigger + manual + validation)
- [x] **3.2** Write Phase 1: Thread Compaction (fallback chain, LEARNINGS.md template, smart skip)
- [x] **3.3** Write Phase 2: Beads Compaction (bd compact usage, smart skip)
- [x] **3.4** Write Phase 3: Knowledge Merge (dedup logic, selective sync, git diff review)
- [x] **3.5** Write Phase 4: Archive (S/H/K prompt, commit, cleanup)
- [x] **3.6** Write Resume section (finish-state.json handling)
- [x] **3.7** Write Error Handling section (phase-specific behavior)
- [x] **3.8** Write Progress Feedback section (UI output format)

---

## Epic 4: Delete doc-sync Skill
*Remove deprecated skill*

### Tasks

- [x] **4.1** Delete `skills/doc-sync/SKILL.md`
- [x] **4.2** Delete `skills/doc-sync/` directory

---

## Epic 5: Update Related Documentation
*Sync all references to the new command*

### Tasks

- [x] **5.1** Update `skills/design/SKILL.md` pipeline diagram - replace DOCSYNC with conductor-finish
- [x] **5.2** Update root `AGENTS.md` - replace doc-sync mentions with /conductor-finish
- [x] **5.3** Update `conductor/tracks.md` to add this track

---

## Epic 6: Verification & Testing
*Ensure everything works correctly*

### Tasks

- [x] **6.1** Verify conductor skill loads without errors
- [x] **6.2** Verify all file paths are correct
- [x] **6.3** Verify no broken references to doc-sync
- [x] **6.4** Test /conductor-finish documentation is complete
- [x] **6.5** Run dry walkthrough of workflow

---

## Summary

| Epic | Tasks | Focus |
|------|-------|-------|
| 1 | 7 | Infrastructure setup |
| 2 | 5 | Conductor skill updates |
| 3 | 8 | Workflow documentation |
| 4 | 2 | doc-sync removal |
| 5 | 3 | Related docs |
| 6 | 5 | Verification |

**Total: 6 epics, 30 tasks**

## Dependencies

```
Epic 1 (Infrastructure)
    ↓
Epic 2 (SKILL.md) ←── Epic 3 (finish-workflow.md)
    ↓
Epic 4 (Delete doc-sync)
    ↓
Epic 5 (Update docs)
    ↓
Epic 6 (Verification)
```

## Implementation Notes

- Epics 2 and 3 can be done in parallel
- Epic 4 must wait for Epic 2 (doc-sync references replaced)
- Epic 6 should be done last to verify everything
