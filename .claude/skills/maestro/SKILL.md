---
name: maestro
description: AI agent workflow with interview-driven planning and team-based execution. Use /design to start planning, /work to execute.
---

# Maestro Workflow

> "Spend tokens once on a good plan; reuse it many times."

## Triggers

| Trigger | Action |
|---------|--------|
| `/design <request>` | Start Prometheus interview mode |
| `/work` | Execute plan with Agent Teams |
| `/setup-check` | Validate Maestro prerequisites |
| `/status` | Show current Maestro state |
| `/review` | Post-execution plan verification |
| `/reset` | Clean stale Maestro state |
| `@tdd` | TDD implementation (kraken) |
| `@spark` | Quick fixes |
| `@oracle` | Strategic advisor (opus) |
| `@explore` | Codebase search |

## Planning Flow

```
/design → prometheus (team lead) → spawns explore/oracle → interview → plan file
```

1. User triggers `/design <description>`
2. Prometheus creates team if research needed
3. Spawns explore for codebase research
4. Spawns oracle for architectural decisions
5. Conducts interview with user
6. Draft updates in `.maestro/drafts/{topic}.md`
7. When clear, generate plan to `.maestro/plans/{name}.md`
8. Spawn plan-reviewer to validate plan quality
9. Cleanup team

## Execution Flow

```
/work → orchestrator (team lead) → spawn workers in parallel → workers self-claim tasks
```

1. User triggers `/work`
2. Orchestrator loads plan from `.maestro/plans/`
3. Creates tasks via TaskCreate with dependencies
4. Spawns 2-4 workers in parallel (kraken, spark)
5. Assigns first round, workers self-claim remaining via TaskList
6. Orchestrator verifies results, extracts wisdom to `.maestro/wisdom/`

## State Directory

```
.maestro/
├── plans/     # Committed work plans
├── drafts/    # Interview drafts
└── wisdom/    # Accumulated learnings
```

## Agents

| Agent | Purpose | Model | Team Lead? | Has Team Tools? |
|-------|---------|-------|------------|-----------------|
| `prometheus` | Interview-driven planner | sonnet | Yes | Yes (full) |
| `orchestrator` | Execution coordinator | sonnet | Yes | Yes (full) |
| `kraken` | TDD implementation | sonnet | No | Yes (self-claim) |
| `spark` | Quick fixes | sonnet | No | Yes (self-claim) |
| `oracle` | Strategic advisor | opus | No | Yes (self-claim) |
| `explore` | Codebase search | sonnet | No | Yes (self-claim) |
| `plan-reviewer` | Plan quality gate | sonnet | No | Yes (self-claim) |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No | Yes (self-claim) |
| `progress-reporter` | Status tracking | haiku | No | Yes (self-claim) |

All agents have `TaskList`, `TaskGet`, `TaskUpdate`, `SendMessage` for team self-coordination. Only team leads have `Task` and `Teammate` for spawning.

## Agent Teams Setup

Requires experimental feature flag in `~/.claude/settings.json`:

```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

## Quick Reference

- **Design**: `/design add user authentication`
- **Execution**: `/work`
- **Research**: `@explore`, `@oracle`
- **Implementation**: `@tdd`, `@spark`
