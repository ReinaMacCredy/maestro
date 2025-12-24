# Design: /conductor-finish Phase 4 Revision

## Problem Statement

The current `/conductor-finish` Phase 4 (Archive) has two issues:
1. **S/H/K fragmentation** - Three archive options create chaos (completed tracks scattered across `tracks/` and `archive/`)
2. **Missing updates** - `tracks.md`, `product.md`, and `tech-stack.md` are never updated on completion

## Discovery Summary

### Party Mode Insights (Winston, Amelia, Maya)

- State should be canonical: one location per lifecycle stage
- S/H/K creates query complexity and code path sprawl
- "Soft" solved a perceived need (easy reference) that `tracks.md` already handles

### Gap Analysis

| File | Created By | Updated By | Gap |
|------|------------|------------|-----|
| `tracks.md` | `/conductor-setup` | `/conductor-newtrack`, `/conductor-implement` | **No completion update** |
| `product.md` | `/conductor-setup` | `/conductor-refresh` | **No shipped feature update** |
| `tech-stack.md` | `/conductor-setup` | `/conductor-refresh` | **No new deps update** |
| `AGENTS.md` | `/conductor-finish` | `/conductor-finish` | ✓ OK |

## Solution

### Change 1: S/H/K → A/K

Simplify archive options:

| Option | Action | When to Use |
|--------|--------|-------------|
| **[A] Archive** | Move folder to `archive/`, update paths | Work is complete |
| **[K] Keep** | Stay in `tracks/`, don't mark complete | Need more work or pausing |

Remove **[S] Soft** entirely.

### Change 2: Add Phase 4 - Context Refresh

Insert new phase between Knowledge Merge (3) and Archive (5):

```
Phase 1: Thread Compaction
Phase 2: Beads Compaction
Phase 3: Knowledge Merge
Phase 4: Context Refresh   ← NEW
Phase 5: Archive           ← was Phase 4
Phase 6: CODEMAPS          ← was Phase 5
```

#### Phase 4: Context Refresh

```markdown
### 4.1 Update product.md
- Extract feature/bugfix description from track's spec.md
- Find existing section (search for "Shipped", "Completed", "Done", or "Features")
- If no section found: append `## Shipped Features` at end
- Add track entry, skip if already documented

### 4.2 Update tech-stack.md (detect + prompt)
- Scan for new dependencies (package.json/go.mod/etc. vs tech-stack.md)
- If diff detected, show user:
  ```
  New dependencies detected:
  + @types/node (dev)
  + zod
  
  Add to tech-stack.md? [Y/n]
  ```
- Only write on user confirmation
- Skip silently if no new deps

### 4.3 Update tracks.md
- Find track entry by ID in "## Active Tracks" section
- Remove from Active section
- Add to "## Completed Tracks" section with `[x]` marker
- If [A] Archive chosen: update link path `tracks/` → `archive/`
```

### Location Rules (Going Forward)

| Location | Contains | Format |
|----------|----------|--------|
| `conductor/tracks/` | Active work | Folders only |
| `conductor/archive/` | Completed tracks | Folders only |
| `history/designs/` | Standalone legacy designs | Files |
| `history/threads/` | Thread exports | Files |

## Out of Scope

- **Legacy file migration** - 5 loose `.md` files in `archive/` will be manually moved to `history/designs/`

## Additional Change: Deprecate /conductor-refresh

Move all refresh functionality into `/conductor-finish`:

| Current `/conductor-refresh` scope | New Location |
|-----------------------------------|--------------|
| `tech` | `/conductor-finish` Phase 4.2 |
| `product` | `/conductor-finish` Phase 4.1 |
| `workflow` | `/conductor-finish` Phase 4 (add 4.4) |
| `track [id]` | Remove (use `/conductor-revise` instead) |
| `codemaps` | `/conductor-finish` Phase 6 |
| `all` | `/conductor-finish` handles all on completion |

**Actions:**
1. Add Phase 4.4: Update workflow.md (detect CI/CD changes)
2. Remove `/conductor-refresh` command
3. Update SKILL.md to remove refresh references
4. Update references/workflows.md to remove refresh workflow
5. Remove `workflows/refresh.md` and `workflows/schemas/refresh_state.schema.json`
6. Update `workflows/README.md` to remove refresh references

## Acceptance Criteria

- [ ] `/conductor-finish` prompts A/K (not S/H/K)
- [ ] Phase 4.1 updates `product.md` with shipped feature (flexible section detection)
- [ ] Phase 4.2 detects new deps and prompts user before writing `tech-stack.md`
- [ ] Phase 4.3 moves track entry in `tracks.md` (Active → Completed)
- [ ] Archive moves folder to `conductor/archive/`
- [ ] `finish-workflow.md` reference updated to reflect 6 phases
- [ ] Phase 0 validation pre-flight added
- [ ] `workflows/schemas/finish_state.schema.json` created
- [ ] `--skip-refresh` flag documented
- [ ] `/conductor-refresh` command removed
- [ ] Phase 4.4 updates workflow.md (detect CI/CD changes)

## Edge Cases

| Case | Handling |
|------|----------|
| Track has no spec.md | Warn, skip product.md update |
| product.md missing "Shipped Features" section | Append `## Shipped Features` at end |
| Track already in Completed section of tracks.md | Skip (idempotent) |
| Archive folder name already exists | Prompt: overwrite / rename with suffix / abort |
| tracks.md entry not found by ID | Warn, continue without tracks.md update |
| Run `/conductor-finish` twice on same track | Safe - all operations are idempotent |

## State File Schema

### New: finish_state.schema.json

Create `workflows/schemas/finish_state.schema.json`:
- `phase`: number (1-6)
- `completed`: array of strings enum `["thread-compaction", "beads-compaction", "knowledge-merge", "context-refresh", "archive", "codemaps"]`
- `startedAt`: ISO timestamp
- `skipCodemaps`: boolean
- `skipRefresh`: boolean (NEW)

## Validation Pre-Flight

Add Phase 0 to `/conductor-finish`:
- Check for stale `finish-state.json`
- Offer Resume / Reset if found

## Flag Updates

| Flag | Current | After |
|------|---------|-------|
| `--with-pr` | Chain after Phase 5 | Chain after Phase 6 |
| `--skip-codemaps` | Skip Phase 5 | Skip Phase 6 |
| `--skip-refresh` | N/A | **NEW:** Skip Phase 4 |

## Documentation Updates Required

| Doc | Change |
|-----|--------|
| `references/finish-workflow.md` | Phase renumbering (4→5→6), A/K, new Phase 4 details |
| `references/workflows.md` | Update Phase 4 references if any |
| `SKILL.md` | Update phase count in overview (5→6) |

## Risks

| Risk | Mitigation |
|------|------------|
| Reopen archived track flow unclear | Manual: `mv archive/<id> tracks/` + update metadata.status |
| product.md missing completion section | Search for "Shipped"/"Completed"/"Done"/"Features", else append new section |
| tech-stack.md deps attribution unclear | Detect + prompt, don't auto-write |
