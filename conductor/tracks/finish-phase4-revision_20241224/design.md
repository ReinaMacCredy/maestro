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
- Add to "Shipped Features" or "Completed" section in product.md
- Skip if already documented

### 4.2 Update tech-stack.md (conditional)
- Scan for new dependencies added during track implementation
- Compare current package.json/go.mod/etc. against tech-stack.md
- Append new deps if found
- Skip if no new deps

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

## Acceptance Criteria

- [ ] `/conductor-finish` prompts A/K (not S/H/K)
- [ ] Phase 4 updates `product.md` with shipped feature
- [ ] Phase 4 updates `tech-stack.md` if new deps detected
- [ ] Phase 4 moves track entry in `tracks.md` (Active → Completed)
- [ ] Archive moves folder to `conductor/archive/`
- [ ] `finish-workflow.md` reference updated to reflect 6 phases

## Edge Cases

| Case | Handling |
|------|----------|
| Track has no spec.md | Warn, skip product.md update |
| product.md missing "Shipped Features" section | Append `## Shipped Features` at end |
| Track already in Completed section of tracks.md | Skip (idempotent) |
| Archive folder name already exists | Prompt: overwrite / rename with suffix / abort |
| tracks.md entry not found by ID | Warn, continue without tracks.md update |
| Run `/conductor-finish` twice on same track | Safe - all operations are idempotent |

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
| product.md section format varies | Detect existing section pattern or append to end |
