# Design: maestro-core

## Overview

Central orchestration skill for the Maestro plugin ecosystem. Defines skill loading hierarchy, HALT/DEGRADE fallback policies, and trigger routing rules.

## Problem Statement

When multiple Maestro skills match a trigger, there is no single source of truth defining which skill takes precedence, causing routing confusion and inconsistent HALT/DEGRADE behaviors.

## Solution

Create `maestro-core` as the central orchestrator that:
1. Defines 5-level skill hierarchy
2. Standardizes HALT vs DEGRADE decisions
3. Provides trigger disambiguation rules
4. Establishes Prerequisites pattern for skill dependencies

## Architecture

```
skills/maestro-core/
├── SKILL.md                    # ≤100 lines, orchestrator
└── references/
    ├── hierarchy.md            # 5-level priority + message formats
    └── routing.md              # Trigger disambiguation + decision rules
```

## Hierarchy (5 Levels)

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing decisions |
| 2 | conductor | Track orchestration |
| 3 | design | Design sessions |
| 4 | beads | Issue tracking |
| 5 | specialized | worktrees, sharing, writing |

## HALT vs DEGRADE Policy

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` CLI unavailable | HALT | ❌ Cannot proceed: bd CLI not found |
| `conductor/` missing | DEGRADE | ⚠️ Standalone mode |
| Village MCP unavailable | DEGRADE | ⚠️ Using single-agent mode |
| CODEMAPS missing | DEGRADE | ⚠️ No CODEMAPS found |

## Trigger Routing

| Trigger | Context | Routes To |
|---------|---------|-----------|
| "design a feature" | Any | design |
| "track this work" | conductor/ exists | conductor |
| "track this work" | no conductor/ | beads |
| "create task for" | conductor/ exists | conductor |
| "what's blocking" | Any | beads |

## Skill Updates

All 6 existing skills receive Prerequisites section:

```markdown
## Prerequisites

**REQUIRED SUB-SKILL:** [maestro-core](../maestro-core/SKILL.md)

Load maestro-core first for orchestration context.
```

### Design Skill Special Update

Change HALT to DEGRADE when `conductor/` missing:
- Show warning: "⚠️ Standalone mode - no Conductor context"
- Skip CODEMAPS loading
- Continue with Double Diamond

## Deliverables

| File | Action | Lines |
|------|--------|-------|
| skills/maestro-core/SKILL.md | Create | ~80 |
| skills/maestro-core/references/hierarchy.md | Create | ~60 |
| skills/maestro-core/references/routing.md | Create | ~80 |
| skills/design/SKILL.md | Update | +15 |
| skills/conductor/SKILL.md | Update | +8 |
| skills/beads/SKILL.md | Update | +8 |
| skills/using-git-worktrees/SKILL.md | Update | +8 |
| skills/sharing-skills/SKILL.md | Update | +8 |
| skills/writing-skills/SKILL.md | Update | +40 |

## Acceptance Criteria

1. [ ] `skills/maestro-core/SKILL.md` exists and ≤100 lines
2. [ ] `hierarchy.md` contains 5-level table
3. [ ] `routing.md` contains trigger disambiguation table
4. [ ] `ds` without conductor/ shows warning, not HALT
5. [ ] All 5 skills have Prerequisites with maestro-core
6. [ ] `writing-skills` has "Skill Dependencies" section
7. [ ] Quick-start example in maestro-core SKILL.md

## Effort

**Total: 10 hours**

| Phase | Task | Hours |
|-------|------|-------|
| 1 | maestro-core foundation | 3 |
| 2 | Design DEGRADE update | 1.5 |
| 3 | Skill Prerequisites updates | 3 |
| 4 | writing-skills documentation | 1.5 |
| 5 | Testing | 1 |

## Risks

| Risk | Mitigation |
|------|------------|
| Skills don't load maestro-core | Clear Prerequisites pattern + documentation |
| Hierarchy conflicts | routing.md covers common cases |
| Message inconsistency | Standardized formats in hierarchy.md |

## Design Session

- **Thread:** http://localhost:8317/threads/T-019b68ef-20a1-742c-8f63-4912e0c65d87
- **Date:** 2025-12-29
- **Participants:** Oracle, Winston, Wendy, Dr. Quinn, John, Paige, Amelia, Bob, Murat, Barry, Morgan, Mary, Sally

## Approval

✅ Design approved by Party Mode (Round 5)
