# Spec: Skill Restructure to Anthropic Standard

**Track ID:** skill-restructure
**Created:** 2026-01-01
**Status:** Draft

## Overview

Restructure 7 existing workflow skills to follow Anthropic's skill-creator standard using Hub-and-Spoke architecture with maestro-core as central router.

## Functional Requirements

### FR-1: Directory Cleanup
- FR-1.1: Delete contents of `.claude/skills/`
- FR-1.2: Create symlink `.claude/skills/` → `skills/`
- FR-1.3: Verify symlink resolves correctly

### FR-2: Clone Anthropic skill-creator
- FR-2.1: Clone `skills/skill-creator/` from `github.com/anthropics/skills`
- FR-2.2: Include all 7 files: SKILL.md, LICENSE.txt, 2 references, 3 scripts
- FR-2.3: Verify `quick_validate.py` runs successfully

### FR-3: Create maestro-core Hub
- FR-3.1: Create `skills/maestro-core/SKILL.md` ≤200 lines
- FR-3.2: Create `skills/maestro-core/references/workflow-chain.md`
- FR-3.3: Create `skills/maestro-core/references/routing-table.md`
- FR-3.4: Create `skills/maestro-core/references/glossary.md`
- FR-3.5: Hub contains routing table to all spoke skills

### FR-4: Refactor beads (Pilot)
- FR-4.1: Restructure `beads/SKILL.md` to template format
- FR-4.2: Add Core Principles, Quick Reference, Anti-Patterns, Related sections
- FR-4.3: Move detailed content to references/ if needed
- FR-4.4: Validate with `quick_validate.py`
- FR-4.5: Target: ≤100 lines

### FR-5: Refactor design
- FR-5.1: Restructure `design/SKILL.md` from 745 → ≤100 lines
- FR-5.2: Move Double Diamond details → `references/double-diamond.md`
- FR-5.3: Move A/P/C mechanics → `references/apc-checkpoints.md`
- FR-5.4: Move grounding → `references/grounding.md`
- FR-5.5: Add Related section linking to maestro-core

### FR-6: Refactor conductor
- FR-6.1: Restructure `conductor/SKILL.md` from 614 → ≤100 lines
- FR-6.2: Move command details → existing references/
- FR-6.3: Move validation gates → references/
- FR-6.4: Add Related section

### FR-7: Refactor writing-skills
- FR-7.1: Restructure `writing-skills/SKILL.md` from 765 → ≤100 lines
- FR-7.2: Move examples → references/
- FR-7.3: Add Related section

### FR-8: Refactor remaining skills
- FR-8.1: Refactor `orchestrator/SKILL.md` (414 → ≤100 lines)
- FR-8.2: Refactor `sharing-skills/SKILL.md` (199 → ≤100 lines)
- FR-8.3: Refactor `using-git-worktrees/SKILL.md` (219 → ≤100 lines)

### FR-9: Final Validation
- FR-9.1: All skills pass `quick_validate.py`
- FR-9.2: All internal links resolve
- FR-9.3: Workflow chain test: `ds → fb → bd ready` works

## Non-Functional Requirements

### NFR-1: Token Efficiency
- Each SKILL.md ≤500 lines (target: ≤100 lines)
- Total reduction: ~5700 tokens saved

### NFR-2: Anthropic Compatibility
- Frontmatter: only `name`, `description`, `version`
- No custom fields (metadata, compatibility, keywords removed)

### NFR-3: Template Consistency
- All skills follow same template structure
- Core Principles, Quick Reference, Anti-Patterns, Guidelines, Scripts, Related

## Acceptance Criteria

| ID | Criterion | Validation |
|----|-----------|------------|
| AC-1 | `.claude/skills/` is symlink to `skills/` | `ls -la .claude/skills` |
| AC-2 | `skills/skill-creator/` exists with 7 files | `ls skills/skill-creator/` |
| AC-3 | `skills/maestro-core/SKILL.md` ≤200 lines | `wc -l` |
| AC-4 | All 7 refactored skills ≤500 lines | `wc -l skills/*/SKILL.md` |
| AC-5 | All skills pass validation | `python quick_validate.py skills/*` |
| AC-6 | Workflow chain works | Manual test |

## Dependencies

- Anthropic skill-creator repo (public GitHub)
- Existing `skills/` directory
- `bd` CLI for beads integration

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Broken references | Low | Medium | Link validation script |
| Lost functionality | Low | High | Pilot beads first |
| Symlink issues | Low | Low | Test after creation |

## Out of Scope

- Rewriting skill logic/behavior
- Changing BMAD agent definitions
- Modifying `bd` CLI behavior
- Creating skills beyond maestro-core
