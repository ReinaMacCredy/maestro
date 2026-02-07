---
name: maestro
description: AI agent workflow with interview-driven planning and team-based execution. Use /design to start planning, /work to execute.
---

# Maestro Workflow

> "Spend tokens once on a good plan; reuse it many times."

## Triggers

| Trigger | Action |
|---------|--------|
| `/design <request>` | Start Prometheus interview mode (supports `--quick`) |
| `/work` | Execute plan with Agent Teams (supports `--resume`) |
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
/design → prometheus (team lead) → detect libraries → fetch docs (Context7/WebSearch) → spawns explore/oracle → interview → leviathan (review) → plan file
```

1. User triggers `/design <description>`
2. Prometheus creates team if research needed
2.5. Loads prior wisdom from `.maestro/wisdom/` (if any)
2.7. Detects external library/framework mentions and fetches docs via Context7 MCP (falls back to WebSearch/WebFetch)
3. Spawns explore for codebase research (and web research when relevant)
4. Spawns oracle for architectural decisions
5. Conducts structured interview (one question at a time, multiple-choice options, incremental validation)
6. Draft updates in `.maestro/drafts/{topic}.md`
7. When clear, generate plan to `.maestro/plans/{name}.md`
8. Spawn leviathan to validate plan quality
9. Cleanup team

Quick mode (`--quick`) streamlines to: team → 1 explore → 1-2 questions → plan

## Execution Flow

```
/work → orchestrator (team lead) → spawn workers in parallel → workers self-claim tasks
```

1. User triggers `/work`
2. Orchestrator loads plan from `.maestro/plans/`
2.5. Validates plan structure and confirms with user before proceeding
2.7. Optionally creates a git worktree for isolated execution (prevents conflicts with concurrent sessions)
3. Creates tasks via TaskCreate with dependencies
4. Spawns 2-4 workers in parallel (kraken, spark)
5. Assigns first round, workers self-claim remaining via TaskList
6. Orchestrator verifies results, extracts wisdom to `.maestro/wisdom/`
7. Suggests `/review` for post-execution verification

Use `--resume` to skip already-completed tasks.

## State Directory

```
.maestro/
├── plans/     # Committed work plans
├── drafts/    # Interview drafts
└── wisdom/    # Accumulated learnings

.worktrees/        # Git worktrees for isolated plan execution (project root)
```

## Agents

| Agent | Purpose | Model | Team Lead? | Has Team Tools? |
|-------|---------|-------|------------|-----------------|
| `prometheus` | Interview-driven planner. Detects libraries and fetches docs via Context7 MCP. Has web research tools (WebSearch, WebFetch) | sonnet | Yes | Yes (full) |
| `orchestrator` | Execution coordinator | sonnet | Yes | Yes (full) |
| `kraken` | TDD implementation | sonnet | No | Yes (self-claim) |
| `spark` | Quick fixes | sonnet | No | Yes (self-claim) |
| `oracle` | Strategic advisor | opus | No | Yes (self-claim) |
| `explore` | Codebase search | sonnet | No | Yes (self-claim) |
| `leviathan` | Deep plan reviewer | opus | No | Yes (self-claim) |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | No | Yes (self-claim) |
| `progress-reporter` | Status tracking | haiku | No | Yes (self-claim) |

All agents have `TaskList`, `TaskGet`, `TaskUpdate`, `SendMessage` for team self-coordination. Only team leads have `Task`, `TeamCreate`, and `TeamDelete` for spawning.

## Agent Teams Setup

Requires experimental feature flag in `~/.claude/settings.json`:

```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

## Skill Interoperability

Maestro auto-detects installed skills and injects their guidance into worker prompts. This allows workers to follow project-specific conventions without manual configuration.

**Discovery locations:**
- Project: `.claude/skills/`
- Global: `~/.claude/skills/`

**Graceful degradation:** If no skills are found, workflows proceed normally without injection.

See [docs/SKILL-INTEROP.md](../../../docs/SKILL-INTEROP.md) for full details.

## Quick Reference

- **Design**: `/design add user authentication`
- **Execution**: `/work`
- **Research**: `@explore`, `@oracle`
- **Implementation**: `@tdd`, `@spark`
