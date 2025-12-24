# Spec: /conductor-finish Phase 4 Revision

## Overview

Revise `/conductor-finish` to:
1. Simplify archive options from S/H/K to A/K
2. Add new Phase 4: Context Refresh (updates product.md, tech-stack.md, tracks.md)
3. Add Phase 0 validation pre-flight
4. Create finish_state.schema.json for resume capability

## Requirements

### R1: Simplify Archive Options (S/H/K → A/K)

**Current behavior:**
- [S] Soft - keep in tracks/, mark archived in metadata
- [H] Hard - move to conductor/archive/
- [K] Keep - don't archive, stay active

**New behavior:**
- [A] Archive - move folder to `conductor/archive/`
- [K] Keep - stay in `tracks/`, don't mark complete

### R2: Add Phase 4 - Context Refresh

Insert between Phase 3 (Knowledge Merge) and Phase 5 (Archive):

#### R2.1: Update product.md
- Extract feature/bugfix description from track's spec.md
- Find existing section (search for "Shipped", "Completed", "Done", or "Features")
- If no section found: append `## Shipped Features` at end
- Add track entry, skip if already documented

#### R2.2: Update tech-stack.md (detect + prompt)
- Scan for new dependencies (package.json/go.mod/etc. vs tech-stack.md)
- If diff detected, show user prompt with detected deps
- Only write on user confirmation
- Skip silently if no new deps

#### R2.3: Update tracks.md
- Find track entry by ID in "## Active Tracks" section
- Remove from Active section
- Add to "## Completed Tracks" section with `[x]` marker
- If [A] Archive chosen: update link path `tracks/` → `archive/`

### R3: Add Phase 0 - Validation Pre-Flight

Before running phases:
- Check for existing `finish-state.json`
- If found, offer Resume / Reset options
- Validate track integrity (spec.md, plan.md exist)

### R4: Create finish_state.schema.json

New schema file at `workflows/schemas/finish_state.schema.json`:
- `phase`: number (0-6)
- `completed`: array of strings enum
- `startedAt`: ISO timestamp
- `skipCodemaps`: boolean
- `skipRefresh`: boolean

### R5: Update Flags

| Flag | New Behavior |
|------|--------------|
| `--with-pr` | Chain after Phase 6 (was Phase 5) |
| `--skip-codemaps` | Skip Phase 6 (was Phase 5) |
| `--skip-refresh` | **NEW:** Skip Phase 4 |

### R6: Phase Renumbering

| Phase | Name | Was |
|-------|------|-----|
| 0 | Validation Pre-Flight | NEW |
| 1 | Thread Compaction | 1 |
| 2 | Beads Compaction | 2 |
| 3 | Knowledge Merge | 3 |
| 4 | Context Refresh | NEW |
| 5 | Archive | 4 |
| 6 | CODEMAPS Regeneration | 5 |

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

## Edge Cases

| Case | Handling |
|------|----------|
| Track has no spec.md | Warn, skip product.md update |
| product.md missing completion section | Search for "Shipped"/"Completed"/"Done"/"Features", else append new section |
| Track already in Completed section of tracks.md | Skip (idempotent) |
| Archive folder name already exists | Prompt: overwrite / rename with suffix / abort |
| tracks.md entry not found by ID | Warn, continue without tracks.md update |
| Run `/conductor-finish` twice on same track | Safe - all operations are idempotent |

## Out of Scope

- Legacy file migration (5 loose `.md` files in archive/ → history/designs/) - manual task
