# Maestro Quick Reference

## Commands

| Command | Description |
|---------|-------------|
| `/design <request>` | Interview-driven planning (supports `--quick`) |
| `/work [<plan-name>] [--resume]` | Execute plan via Agent Teams |
| `/review` | Post-execution review with auto-fix (supports planless git-diff mode) |
| `/setup-check` | Validate Maestro prerequisites |
| `/status` | Show current Maestro state |
| `/reset` | Clean stale Maestro state |
| `/plan-template <name>` | Scaffold blank plan with required sections |
| `/styleguide` | Detect languages and inject code style guides |
| `/setup` | Scaffold project context (product, tech stack, guidelines) |
| `/pipeline <preset>` | Sequential agent chains with context passing |
| `/analyze <problem>` | Deep read-only investigation with structured report |
| `/note [--priority <P0-P3>] <text>` | Capture decisions and context to persistent notepad |
| `/learner [--from-session \| <topic>]` | Extract principles as reusable learned skills |
| `/security-review [<files> \| --diff]` | Security analysis with severity ratings |
| `/ultraqa [--tests\|--build\|--lint]` | Iterative fix-and-verify loop (max 5 cycles) |
| `/research <topic> [--auto]` | Multi-stage research with parallel agents |

## Triggers

| Trigger | Agent |
|---------|-------|
| `/design <request>` | prometheus (team lead) |
| `/work` | orchestrator (team lead) |
| `@tdd` | kraken |
| `@spark` | spark |
| `@oracle` | oracle (sonnet) |

`explore` is used as an internal teammate by orchestrated workflows.

## Agents

| Agent | Purpose | Model | Team Lead? |
|-------|---------|-------|------------|
| `prometheus` | Interview-driven planner | sonnet | Yes |
| `orchestrator` | Execution coordinator | sonnet | Yes |
| `kraken` | TDD implementation | sonnet | No |
| `spark` | Quick fixes | sonnet | No |
| `build-fixer` | Build/compile/lint error specialist | sonnet | No |
| `oracle` | Strategic advisor | sonnet | No |
| `critic` | Post-implementation reviewer | sonnet | No |
| `security-reviewer` | Security analysis specialist | sonnet | No |
| `explore` | Codebase search | haiku | No |
| `leviathan` | Deep plan reviewer | sonnet | No |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No |
| `progress-reporter` | Status tracking | haiku | No |

## State

```
.maestro/
├── plans/      # Work plans from /design
├── archive/    # Executed plans (moved after /work completes)
├── drafts/     # Interview drafts
├── context/    # Project context files (product, tech stack, guidelines)
├── handoff/    # Session recovery JSON
├── wisdom/     # Accumulated learnings
├── research/   # Research session state and findings
└── notepad.md  # Persistent notes (decisions, context, constraints)
```

## Links

- [Skill Definition](.claude/skills/maestro/SKILL.md)
- [Agent Definitions](.claude/agents/)
- [Architecture](docs/ARCHITECTURE.md)
- [Agent Teams Guide](docs/AGENT-TEAMS.md)
- [Skill Interop](docs/SKILL-INTEROP.md)
