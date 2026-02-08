# AGENTS.md - Maestro Plugin

Interview-driven planning with Agent Teams execution.

## Workflow

```
/design → prometheus (interview) → leviathan (review) → plan file → /work → orchestrator → teammates
```

## Commands

| Trigger | Agent | Action |
|---------|-------|--------|
| `/design <request>` | prometheus | Interview-driven planning |
| `/work` | orchestrator | Execute plan via teams |
| `/review` | - | Post-execution review with auto-fix |
| `/setup-check` | - | Validate Maestro prerequisites |
| `/status` | - | Show current Maestro state |
| `/reset` | - | Clean stale Maestro state |
| `/plan-template <name>` | - | Scaffold blank plan |
| `/styleguide` | - | Detect languages and inject code style guides |
| `/setup` | - | Scaffold project context |
| `/pipeline <preset>` | - | Sequential agent chains |
| `/analyze <problem>` | - | Deep read-only investigation |
| `/note <text>` | - | Capture decisions to notepad |
| `/learner [<topic>]` | - | Extract reusable learned skills |
| `/security-review` | security-reviewer | Security analysis |
| `/ultraqa` | - | Iterative fix-and-verify loop |
| `/research <topic>` | - | Multi-stage research |
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
| `build-fixer` | Build/compile/lint error specialist | sonnet | No |
| `oracle` | Strategic advisor | opus | No |
| `critic` | Post-implementation reviewer | opus | No |
| `security-reviewer` | Security analysis specialist | opus | No |
| `explore` | Codebase search | sonnet | No |
| `leviathan` | Deep plan reviewer | opus | No |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No |
| `progress-reporter` | Status tracking | haiku | No |

Team leads have Task, TeamCreate, TeamDelete, SendMessage. Workers have TaskList, TaskGet, TaskUpdate, SendMessage for self-coordination.

## State

```
.maestro/
├── plans/      # Work plans
├── archive/    # Executed plans
├── drafts/     # Interview drafts
├── context/    # Project context files
├── handoff/    # Session recovery JSON
├── wisdom/     # Learnings
├── research/   # Research session state
└── notepad.md  # Persistent notes
```

## Rules

1. Orchestrator never edits directly — always delegates
2. Verify subagent claims — agents can make mistakes
3. TDD by default — use kraken for new features
4. Workers self-claim tasks — parallel, not sequential

## Links

- [Skill](.claude/skills/maestro/SKILL.md)
- [Agents](.claude/agents/)
