# Maestro Quick Reference

## Commands

| Command | Description |
|---------|-------------|
| `/design <request>` | Interview-driven planning (team-based) |
| `/work` | Execute plan via Agent Teams |
| `/setup-check` | Validate Maestro prerequisites |
| `/status` | Show current Maestro state |
| `/review` | Post-execution plan verification |
| `/reset` | Clean stale Maestro state |

## Triggers

| Trigger | Agent |
|---------|-------|
| `/design <request>` | prometheus (team lead) |
| `/work` | orchestrator (team lead) |
| `/setup-check` | - |
| `/status` | - |
| `/review` | - |
| `/reset` | - |
| `@tdd` | kraken |
| `@spark` | spark |
| `@oracle` | oracle (opus) |
| `@explore` | explore |

## Agents

| Agent | Purpose | Model | Team Lead? |
|-------|---------|-------|------------|
| `prometheus` | Interview-driven planner | sonnet | Yes |
| `orchestrator` | Execution coordinator | sonnet | Yes |
| `kraken` | TDD implementation | sonnet | No |
| `spark` | Quick fixes | sonnet | No |
| `oracle` | Strategic advisor | opus | No |
| `explore` | Codebase search | sonnet | No |
| `leviathan` | Deep plan reviewer | opus | No |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No |
| `progress-reporter` | Status tracking | haiku | No |

## State

```
.maestro/
├── plans/     # Work plans from /design
├── drafts/    # Interview drafts
└── wisdom/    # Accumulated learnings
```

## Links

- [Skill Definition](.claude/skills/maestro/SKILL.md)
- [Agent Definitions](.claude/agents/)
- [Architecture](docs/ARCHITECTURE.md)
- [Agent Teams Guide](docs/AGENT-TEAMS.md)
