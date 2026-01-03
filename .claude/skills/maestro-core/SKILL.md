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
ds/pl → design.md → /conductor-newtrack → spec.md + plan.md → fb → beads → ci/co → implementation
```

## Routing Table

**CRITICAL:** After loading `maestro-core`, you MUST explicitly load the target skill via `skill(name="...")` before proceeding. This table only provides routing - it does NOT auto-load skills.

| Trigger | Skill to Load | Description |
|---------|---------------|-------------|
| `ds`, `/conductor-design` | `skill(name="design")` | Double Diamond design sessions |
| `pl`, `/plan`, "plan feature" | `skill(name="design")` | 6-phase risk-based planning |
| `/conductor-setup` | `skill(name="conductor")` | Initialize project |
| `cn`, `/conductor-newtrack` | `skill(name="conductor")` | Create spec + plan from design |
| `ci`, `/conductor-implement` | `skill(name="conductor")` | Execute track (auto-routes to orchestrator) |
| `co`, `/conductor-orchestrate` | `skill(name="orchestrator")` | Parallel execution |
| `/conductor-finish` | `skill(name="conductor")` | Complete track |
| `fb`, `file-beads` | `skill(name="beads")` | File beads from plan |
| `rb`, `review-beads` | `skill(name="beads")` | Review filed beads |
| `bd ready` | `skill(name="beads")` | Find available work |
| `/handoff`, `/conductor-handoff` | `skill(name="handoff")` | Session handoff (self-contained) |

### Routing Flow

```
1. User triggers command (e.g., `ci`)
2. Load maestro-core → get routing table
3. Look up trigger → find target skill
4. MUST call skill tool to load target skill
5. Follow loaded skill instructions
```

## Fallback Policies

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` unavailable | HALT | `❌ Cannot proceed: bd CLI required` |
| `conductor/` missing | DEGRADE | `⚠️ Standalone mode - limited features` |
| Agent Mail unavailable | HALT | `❌ Cannot proceed: Agent Mail required for coordination` |

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
- [handoff](../handoff/SKILL.md) - Session cycling and context preservation
- [writing-skills](../writing-skills/SKILL.md) - Creating new skills
- [sharing-skills](../sharing-skills/SKILL.md) - Contributing skills upstream
- [using-git-worktrees](../using-git-worktrees/SKILL.md) - Isolated workspaces
