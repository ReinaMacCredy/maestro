---
name: maestro-core
description: Use when any Maestro skill loads - provides skill hierarchy, HALT/DEGRADE policies, and trigger routing rules for orchestration decisions
---

# Maestro Core - Workflow Router

Central hub for Maestro workflow skills. Routes triggers, defines hierarchy, and handles fallbacks.

## Skill Hierarchy

```
conductor (1) > orchestrator (2) > design (3) > beads (4) > specialized (5)
```

Higher rank wins on conflicts.

## Workflow Chain

```
ds → design.md → /conductor-newtrack → spec.md + plan.md → fb → beads → ci/co → implementation
```

## Routing Table

| Trigger | Skill | Description |
|---------|-------|-------------|
| `ds`, `/conductor-design` | [design](../design/SKILL.md) | Double Diamond design sessions |
| `/conductor-setup` | [conductor](../conductor/SKILL.md) | Initialize project |
| `/conductor-newtrack` | [conductor](../conductor/SKILL.md) | Create spec + plan from design |
| `ci`, `/conductor-implement` | [conductor](../conductor/SKILL.md) | Execute track (auto-routes to orchestrator) |
| `co`, `/conductor-orchestrate` | [orchestrator](../orchestrator/SKILL.md) | Parallel execution |
| `/conductor-finish` | [conductor](../conductor/SKILL.md) | Complete track |
| `fb`, `file-beads` | [beads](../beads/SKILL.md) | File beads from plan |
| `rb`, `review-beads` | [beads](../beads/SKILL.md) | Review filed beads |
| `bd ready` | [beads](../beads/SKILL.md) | Find available work |

## Fallback Policies

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` unavailable | HALT | `❌ Cannot proceed: bd CLI required` |
| `conductor/` missing | DEGRADE | `⚠️ Standalone mode - limited features` |
| Agent Mail unavailable | DEGRADE | `⚠️ Falling back to sequential execution` |

## Quick Reference

| Concern | Reference |
|---------|-----------|
| Complete workflow | [workflow-chain.md](references/workflow-chain.md) |
| All routing rules | [routing-table.md](references/routing-table.md) |
| Terms and concepts | [glossary.md](references/glossary.md) |

## Related Skills

- [design](../design/SKILL.md) - Double Diamond design sessions
- [conductor](../conductor/SKILL.md) - Context-driven development
- [orchestrator](../orchestrator/SKILL.md) - Multi-agent parallel execution
- [beads](../beads/SKILL.md) - Issue tracking and dependency graphs
- [writing-skills](../writing-skills/SKILL.md) - Creating new skills
- [sharing-skills](../sharing-skills/SKILL.md) - Contributing skills upstream
- [using-git-worktrees](../using-git-worktrees/SKILL.md) - Isolated workspaces
