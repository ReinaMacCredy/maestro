# Spec: Beads Consolidation

## Overview

Refactor 3 beads-related skills into a unified architecture with thin skill entry point and centralized workflow logic.

## Requirements

### Functional Requirements

#### FR1: Unified Skill Entry Point
- Single `skills/beads/SKILL.md` handles all triggers: `bd`, `fb`, `rb`, `beads`, `file-beads`, `review-beads`
- Thin stub (~50 lines) delegates to workflow files
- Description must be keyword-rich for AI trigger matching

#### FR2: Centralized Workflow Logic
- All logic moves to `workflows/beads/`
- Main entry: `workflow.md` (lowercase)
- Supporting docs: `references/` directory (UPPERCASE naming)
- Total 10 files in references/

#### FR3: File Renaming
- `file-beads.md` → `FILE_BEADS.md`
- `review-beads.md` → `REVIEW_BEADS.md`
- Follows existing UPPERCASE convention in references/

#### FR4: Trigger Preservation
- `fb` must load `workflows/beads/references/FILE_BEADS.md`
- `rb` must load `workflows/beads/references/REVIEW_BEADS.md`
- `bd` must load `workflows/beads/workflow.md`

#### FR5: Documentation Updates
- 14 active files must be updated
- Replace `file-beads skill` → `beads skill`
- Update Mermaid nodes: `FB["fb (file-beads)"]` → `FB["fb"]`
- Merge table rows where appropriate

#### FR6: Broken Link Cleanup
- CLI_REFERENCE.md: Remove 12 lines with broken links
- Lines: 281-282, 370, 403, 556-561

### Non-Functional Requirements

#### NFR1: Hard Link Handling
- `skills/` and `.claude/skills/` are hard-linked
- Updating one updates both automatically
- Must delete both locations for removed skills

#### NFR2: Backwards Compatibility
- Keep `(file beads)` parenthetical in prose for readability
- Triggers remain unchanged for users

#### NFR3: Rollback Capability
- Must be able to revert via git checkout
- No destructive operations until verification passes

## Acceptance Criteria

| ID | Criterion | Test Method |
|----|-----------|-------------|
| AC1 | `fb` loads beads skill and executes FILE_BEADS.md | Manual: type `fb`, verify behavior |
| AC2 | `rb` loads beads skill and executes REVIEW_BEADS.md | Manual: type `rb`, verify behavior |
| AC3 | `bd` commands work unchanged | Manual: run `bd ready`, `bd list` |
| AC4 | Only 1 beads-related skill folder exists | `ls skills/ \| grep beads \| wc -l` = 1 |
| AC5 | `workflows/beads/` contains 10+ files | `ls workflows/beads/references/ \| wc -l` >= 10 |
| AC6 | No old references in active docs | Grep returns 0 matches |
| AC7 | CLI_REFERENCE.md has no broken links | Manual verification |

## Out of Scope

- Archive documentation (historical, not updated)
- New features (pure refactor)
- Plugin manifest changes
- Automated testing (manual verification only)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| AI trigger matching fails | Medium | High | Keyword-rich description |
| Broken internal references | Low | Medium | Grep verification |
| User confusion | Low | Low | Triggers unchanged |

## Dependencies

- None (self-contained refactor)
