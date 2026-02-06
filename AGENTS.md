# AGENTS.md - Maestro Plugin

Interview-driven planning with Agent Teams execution.

## Workflow

```
/design → prometheus (interview) → plan file → /work → orchestrator → teammates
```

## Commands

| Trigger | Agent | Action |
|---------|-------|--------|
| `/design <request>` | prometheus | Interview-driven planning |
| `/work` | orchestrator | Execute plan via teams |
| `/setup-check` | - | Validate Maestro prerequisites |
| `/status` | - | Show current Maestro state |
| `/review` | - | Post-execution plan verification |
| `/reset` | - | Clean stale Maestro state |
| `@tdd` | kraken | TDD implementation |
| `@spark` | spark | Quick fixes |
| `@oracle` | oracle | Strategic advice (opus) |
| `@explore` | explore | Codebase search |

## Agents

| Agent | Purpose | Model | Team Lead? |
|-------|---------|-------|------------|
| `prometheus` | Interview-driven planner | sonnet | Yes |
| `orchestrator` | Execution coordinator | sonnet | Yes |
| `kraken` | TDD implementation | sonnet | No |
| `spark` | Quick fixes | sonnet | No |
| `oracle` | Strategic advisor | opus | No |
| `explore` | Codebase search | sonnet | No |
| `plan-reviewer` | Plan quality gate | sonnet | No |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No |
| `progress-reporter` | Status tracking | haiku | No |

All agents have team coordination tools (TaskList, TaskGet, TaskUpdate, SendMessage). Only team leads have Task + Teammate for spawning.

## State

```
.maestro/
├── plans/     # Work plans
├── drafts/    # Interview drafts
└── wisdom/    # Learnings
```

## Rules

1. Orchestrator never edits directly — always delegates
2. Verify subagent claims — agents can make mistakes
3. TDD by default — use kraken for new features
4. Workers self-claim tasks — parallel, not sequential

## Links

- [Skill](.claude/skills/maestro/SKILL.md)
- [Agents](.claude/agents/)
