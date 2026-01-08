# Plan: /conductor-finish Phase 4 Revision

## Epic 1: Schema & State Infrastructure

### [x] 1.1 Create finish_state.schema.json
**File:** `workflows/schemas/finish_state.schema.json`

Create JSON Schema with:
- `$schema`: `http://json-schema.org/draft-07/schema#`
- `phase`: integer, minimum 0, maximum 6
- `completed`: array of strings, enum values
- `startedAt`: string (ISO date-time)
- `skipCodemaps`: boolean
- `skipRefresh`: boolean

**Verify:** Schema validates against example from finish-workflow.md

### [x] 1.2 Update finish-state.json example in finish-workflow.md
**File:** `skills/conductor/references/finish-workflow.md`

Update lines 334-351:
- Change phase max from 5 to 6
- Add `"context-refresh"` to completed array example
- Add `skipRefresh` field

---

## Epic 2: Phase 0 - Validation Pre-Flight

### [x] 2.1 Add Phase 0 section to finish-workflow.md
**File:** `skills/conductor/references/finish-workflow.md`

Insert after "## Validation" section (around line 35):
```markdown
## Phase 0: Validation Pre-Flight

**Purpose:** Check for stale state and validate track integrity

### Resume Detection
1. Check for existing `finish-state.json` in track directory
2. If found and not corrupted:
   ```
   Previous run interrupted at Phase X. Resume? [Y/n]
   ```
3. If corrupted: warn and restart from Phase 1

### Track Validation
- Verify spec.md exists (warn if missing)
- Verify plan.md exists (warn if missing)
- Check for open beads (warn only, don't block)
```

---

## Epic 3: Phase 4 - Context Refresh

### [x] 3.1 Add Phase 4 section to finish-workflow.md
**File:** `skills/conductor/references/finish-workflow.md`

Insert after Phase 3 (Knowledge Merge), before current Phase 4 (Archive):

```markdown
## Phase 4: Context Refresh

**Purpose:** Update conductor context documents with shipped work

### 4.1 Update product.md

1. Read track's spec.md, extract feature description
2. Read conductor/product.md
3. Find existing completion section:
   - Search for headings containing: "Shipped", "Completed", "Done", "Features"
   - If none found: append `## Shipped Features` at end
4. Check if track already documented (by ID or title)
5. If not documented, append entry:
   ```markdown
   - [track_id] Feature description (completed YYYY-MM-DD)
   ```

### 4.2 Update tech-stack.md (detect + prompt)

1. Detect package manager(s) in project
2. Parse current dependencies from lockfiles
3. Compare against documented deps in tech-stack.md
4. If new deps found:
   ```
   New dependencies detected:
   + dependency-name (dev/prod)
   
   Add to tech-stack.md? [Y/n]
   ```
5. On confirm: append to appropriate section
6. On decline or no new deps: skip silently

### 4.3 Update tracks.md

1. Read conductor/tracks.md
2. Find track entry by ID in "## Active Tracks" section
3. If not found: warn, continue
4. If found:
   - Remove entry from Active section
   - Add to "## Completed Tracks" section
   - Change marker: `[ ]` or `[~]` → `[x]`
   - If Archive chosen: update link `tracks/` → `archive/`

### Skip Flag

If `--skip-refresh` provided:
```
Phase 4/6: Skipped (--skip-refresh)
```

### Progress
```
Phase 4/6: Refreshing context docs...
  → product.md: +1 shipped feature
  → tech-stack.md: skipped (no new deps)
  → tracks.md: moved to Completed
```
```

---

## Epic 4: Phase 5 - Archive Simplification

### [x] 4.1 Revise Archive Choice in finish-workflow.md
**File:** `skills/conductor/references/finish-workflow.md`

Replace S/H/K section (lines 194-209) with:

```markdown
### Archive Choice (A/K)

Prompt user:
```
Archive choice:
[A] Archive - move to conductor/archive/
[K] Keep - stay active in tracks/
> 
```

| Choice | Action |
|--------|--------|
| **A** | Move track folder to `conductor/archive/`, update links |
| **K** | No change, track stays in `tracks/` (don't mark complete) |
```

### [x] 4.2 Update docSync record format
**File:** `skills/conductor/references/finish-workflow.md`

Update metadata.json example (around line 214):
- Change `"archiveChoice": "soft"` to `"archiveChoice": "archive"` or `"keep"`
- Remove "soft" as valid value

---

## Epic 5: Phase Renumbering & Flags

### [x] 5.1 Renumber all phase references
**File:** `skills/conductor/references/finish-workflow.md`

- Current "Phase 4: Archive" → "Phase 5: Archive"
- Current "Phase 5: CODEMAPS Regeneration" → "Phase 6: CODEMAPS Regeneration"
- Update all progress examples (e.g., "Phase 4/5" → "Phase 5/6")
- Update error handling table phase references

### [x] 5.2 Update flag documentation
**File:** `skills/conductor/references/finish-workflow.md`

Update Flags section (lines 18-20):
```markdown
**Flags:**
- `--with-pr` - Chain to finish-branch skill after Phase 6
- `--skip-codemaps` - Skip CODEMAPS regeneration (Phase 6)
- `--skip-refresh` - Skip Context Refresh (Phase 4)
```

### [x] 5.3 Update SKILL.md phase count
**File:** `skills/conductor/SKILL.md`

Find "/conductor-finish Workflow" section, update:
- "Runs 4 phases" → "Runs 6 phases"
- Update phase list to include Phase 0 and Phase 4

---

## Epic 6: Documentation Sync

### [x] 6.1 Update workflows.md if needed
**File:** `skills/conductor/references/workflows.md`

Search for references to finish phases and update any found.

### [x] 6.2 Update Full Progress Example
**File:** `skills/conductor/references/finish-workflow.md`

Update example (lines 385-415) to show 6 phases:
```
Phase 1/6: Extracting from 3 threads...
Phase 2/6: Compacting 5 beads...
Phase 3/6: Merging to conductor/AGENTS.md...
Phase 4/6: Refreshing context docs...
Phase 5/6: Preparing archive...
Phase 6/6: Regenerating CODEMAPS...
```

### [x] 6.3 Update Edge Cases table
**File:** `skills/conductor/references/finish-workflow.md`

Add new edge cases:
- `--skip-refresh flag` → Skip Phase 4 entirely
- `product.md missing completion section` → Search alternatives or append
- `tracks.md entry not found` → Warn, continue

---

## Epic 7: Deprecate /conductor-refresh

### [x] 7.1 Add Phase 4.4 workflow.md update to finish-workflow.md
**File:** `skills/conductor/references/finish-workflow.md`

Add to Phase 4 section:
```markdown
### 4.4 Update workflow.md (detect changes)

1. Scan `.github/workflows/` for CI/CD changes
2. Detect new linting/testing tools
3. Compare against workflow.md
4. If changes found, prompt user to update
```

### [x] 7.2 Remove /conductor-refresh from SKILL.md
**File:** `skills/conductor/SKILL.md`

- Remove from Slash Commands table
- Remove from Intent Mapping table
- Remove "Docs are outdated" / "Sync with codebase" intent

### [x] 7.3 Remove refresh workflow from workflows.md
**File:** `skills/conductor/references/workflows.md`

- Delete "## Workflow: Refresh" section (lines ~485-600)

### [x] 7.4 Update proactive behaviors in SKILL.md
**File:** `skills/conductor/SKILL.md`

- Remove "suggest `/conductor-refresh`" from stale context behavior
- Update to suggest running `/conductor-finish --skip-archive` or similar

---

## Epic 8: Cleanup Remaining References (Post-Implementation)

### [x] 8.1 Create workflows/finish.md
**File:** `workflows/finish.md`

Create consolidated workflow reference for /conductor-finish.

### [x] 8.2 Update workflows/README.md
Add finish.md to directory structure.

### [x] 8.3 Remove /conductor-refresh from structure.md
**File:** `skills/conductor/references/structure.md`

Remove refresh_state.json and /conductor-refresh references.

### [x] 8.4 Update design/SKILL.md
Remove /conductor-refresh reference for CODEMAPS.

---

## Verification

After all tasks complete:
1. [x] Read finish-workflow.md and verify 6 phases documented
2. [x] Verify schema file exists at workflows/schemas/finish_state.schema.json
3. [x] Verify SKILL.md mentions 6 phases
4. [x] Verify no references to S/H/K remain (only A/K)
5. [x] Verify `/conductor-refresh` removed from SKILL.md and workflows.md
6. [x] Verify workflows/finish.md exists
7. [x] Verify no stale /conductor-refresh references in skills/
