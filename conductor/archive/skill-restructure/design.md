# Design: Skill Restructure to Anthropic Standard

**Track ID:** skill-restructure
**Created:** 2026-01-01
**Completed:** 2026-01-01
**Status:** Complete

## Problem Statement

Restructure 7 existing workflow skills to follow Anthropic's skill-creator standard, using a Hub-and-Spoke architecture with maestro-core as the central router, while preserving workflow coherence and enabling marketplace distribution.

## Goals

1. **Distribution** - Package skills for sharing on Anthropic marketplace
2. **Token Efficiency** - Reduce context window usage (3 skills over 500 line limit)
3. **Standardization** - Follow official Anthropic patterns for maintainability

## Approach

**Hybrid Implementation:**
- Phase 0: Directory cleanup (symlink)
- Phase 1: Foundation (clone, create hub, pilot)
- Phase 2: Parallel rollout (refactor remaining skills)

**Architecture:** Hub-and-Spoke
- `maestro-core` = central router (~150 lines)
- All other skills = spokes (<500 lines each)

## SKILL.md Template

Based on Anthropic standard + claudekit patterns:

```markdown
---
name: skill-name
description: >
  What it does. When to trigger (specific phrases, commands).
version: 1.0.0
---

# Skill Title

One-line summary of purpose.

## Core Principles

- **Principle 1** - Brief explanation
- **Principle 2** - Brief explanation

## Quick Reference

| Topic | When to Use | Reference |
|-------|-------------|-----------|
| **Topic A** | Condition | [file.md](references/file.md) |

## Key Patterns

### Pattern Name
Brief description.

### Anti-Patterns
- Don't do X

## Guidelines

- Guideline 1

## Scripts

- [script.py](scripts/script.py) - Description

## Related

- [Related Skill](../related/SKILL.md)
```

## Directory Structure (Target)

```
skills/
├── skill-creator/                 # Cloned from Anthropic
│   ├── SKILL.md
│   ├── LICENSE.txt
│   ├── references/
│   │   ├── output-patterns.md
│   │   └── workflows.md
│   └── scripts/
│       ├── init_skill.py
│       ├── package_skill.py
│       └── quick_validate.py
│
├── maestro-core/                  # NEW: Hub skill
│   ├── SKILL.md                   # <200 lines
│   └── references/
│       ├── workflow-chain.md
│       ├── routing-table.md
│       └── glossary.md
│
├── beads/                         # REFACTORED
├── design/                        # REFACTORED  
├── conductor/                     # REFACTORED
├── orchestrator/                  # REFACTORED
├── writing-skills/                # REFACTORED
├── sharing-skills/                # REFACTORED
└── using-git-worktrees/           # REFACTORED

skills/ → symlink to skills/
```

## Refactoring Map

| Skill | Current | Target | Action |
|-------|---------|--------|--------|
| design | 745 | <100 | Move Double Diamond → references/ |
| conductor | 614 | <100 | Move commands → references/ |
| writing-skills | 765 | <100 | Move examples → references/ |
| orchestrator | 414 | <100 | Move 8-phase → references/ |
| beads | 160 | <100 | Minor cleanup |
| sharing-skills | 199 | <100 | Minor cleanup |
| using-git-worktrees | 219 | <100 | Minor cleanup |

## maestro-core Design

Central hub (~150 lines) containing:
- Workflow chain diagram
- Routing table (trigger → skill)
- Fallback policies
- Links to all spoke skills

NOT containing:
- Detailed workflows (live in spokes)
- Implementation details (live in references/)

## Implementation Phases

### Phase 0: Directory Cleanup
- Delete `skills/` contents
- Create symlink: `skills/` → `skills/`

### Phase 1: Foundation
1. Clone `skill-creator` from Anthropic
2. Create `maestro-core` hub skill
3. Pilot refactor: `beads`
4. Validate with `quick_validate.py`

### Phase 2: Parallel Rollout
- Track A: design, conductor (complex, related)
- Track B: orchestrator (medium)
- Track C: writing-skills, sharing-skills, using-git-worktrees (simple)

## Success Criteria

| # | Metric | Target | Validation |
|---|--------|--------|------------|
| 1 | SKILL.md line count | All ≤500 | `wc -l skills/*/SKILL.md` |
| 2 | Frontmatter valid | Pass | `quick_validate.py` |
| 3 | maestro-core size | ≤200 lines | `wc -l` |
| 4 | Internal links | All resolve | Link validation |
| 5 | Workflow chain | Works | End-to-end test |
| 6 | Distribution ready | Package succeeds | `package_skill.py` |

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Broken references | Low | Validation script |
| Lost functionality | Low | Pilot beads first |
| Symlink issues | Low | Test after creation |

## Dependencies

- Anthropic skill-creator repo (public)
- Existing skills/ directory
- bd CLI for beads validation

## Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Architecture | Hub-and-Spoke | Preserves workflow coherence |
| Hub skill | maestro-core (fresh) | Old version was prep for this |
| Canonical dir | skills/ | More recent, has scripts/ |
| .claude/skills | Symlink | Single source of truth |
| Template source | claudekit + Anthropic | Best of both patterns |
| Pilot skill | beads | Small but has full pattern |
| Rollout | Hybrid | Validate then parallelize |

## Party Mode Insights

From expert agent review:

- **Winston (Architect):** Keep maestro-core <200 lines, router-only
- **Paige (Tech Writer):** Add Related section for discoverability
- **Amelia (Developer):** Pilot with beads before full rollout

## Advanced Analysis

Resolved concerns:
- Reference loading: Add explicit "load reference" instructions
- Circular references: Spokes never re-route to hub
- AGENTS.md integration: Keep routing in both initially
- Token savings: ~5700 tokens saved across skills

## Next Steps

~~Run `/conductor-newtrack skill-restructure` to generate spec and plan.~~

## Completion Summary

**Phase 3 Validation Results (2026-01-01):**

| Skill | Lines | Valid | Hub Link |
|-------|-------|-------|----------|
| beads | 78 | ✅ | ✅ |
| design | 75 | ✅ | ✅ |
| conductor | 85 | ✅ | ✅ |
| orchestrator | 87 | ✅ | ✅ |
| writing-skills | 69 | ✅ | ✅ |
| sharing-skills | 66 | ✅ | ✅ |
| using-git-worktrees | 61 | ✅ | ✅ |
| maestro-core (hub) | 62 | ✅ | N/A |
| skill-creator | 356 | ✅ | N/A |

**Total:** 939 lines (down from ~3116 original = 70% reduction)

**All success criteria met:**
- ✅ All SKILL.md ≤500 lines (target ≤100 for refactored skills)
- ✅ All pass quick_validate.py
- ✅ maestro-core ≤200 lines (62 actual)
- ✅ All internal links resolve
- ✅ Bidirectional Related sections
- ✅ Workflow triggers verified (ds, fb, bd ready)
- ✅ skills/ symlink working
