# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is an AI agent workflow plugin for Claude Code. It provides interview-driven planning and team-based execution using Agent Teams.

**Core philosophy:** "Spend tokens once on a good plan; reuse it many times."

**Prerequisite:** Agent Teams must be enabled in `~/.claude/settings.json`:
```json
{ "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }
```

## Commands

- `/design <request>` — Interview-driven planning (supports `--quick` for streamlined mode)
- `/work [<plan-name>] [--resume]` — Execute plan with Agent Teams (parallel workers)
- `/setup-check` — Verify plugin prerequisites
- `/status` — Show Maestro state
- `/review` — Post-execution review with auto-fix (also supports planless git-diff mode)
- `/styleguide` — Detect project languages and inject code style guides into CLAUDE.md
- `/setup` — Scaffold project context (product definition, tech stack, guidelines)
- `/reset` — Clean stale state (teams, handoff files, drafts)
- `/plan-template <name>` — Scaffold blank plan with required sections
- `/pipeline <preset> | <agent1 -> agent2 'task'>` — Sequential agent chains with context passing
- `/analyze <problem or topic>` — Deep read-only investigation with structured report
- `/note [--priority <P0-P3>] <text>` — Capture decisions, context, and constraints to persistent notepad
- `/learner [--from-session | --from-diff | <topic>]` — Extract hard-won principles as reusable learned skills
- `/security-review [<files> | --diff [range]]` — Delegated security analysis with severity ratings
- `/ultraqa [--tests|--build|--lint|--typecheck|--custom '<cmd>']` — Iterative fix-and-verify loop (max 5 cycles, never commits)
- `/research <topic> [--auto|--stages <N>|--resume]` — Multi-stage research with parallel agents and synthesis

### Validation

```bash
cat .claude-plugin/plugin.json | jq .     # Validate plugin manifest
./scripts/validate-links.sh               # Validate documentation links
./scripts/validate-anchors.sh             # Validate markdown anchors
./scripts/test-hooks.sh                   # Test hook scripts
```

## Architecture

### Two-Phase Workflow

Both phases create Agent Teams. The skill SKILL.md files are the source of truth for each workflow — not agent definitions, not docs.

**Design phase** (`/design`): Design skill acts as thin team lead → spawns `prometheus` (built-in Plan type) in plan mode → prometheus detects external libraries and fetches docs via Context7 MCP (with WebSearch/WebFetch fallback) → spawns `explore` (built-in Explore type)/`oracle` for codebase research → conducts interview → `leviathan` reviews plan (full mode only) → user approves → plan saved to `.maestro/plans/`.

**Execution phase** (`/work`): Work skill acts as orchestrator → loads plan → creates tasks with dependencies → spawns 2-4 workers (`kraken`/`spark`) in parallel → workers self-claim tasks via TaskList → orchestrator verifies each result → commits after verified tasks → extracts wisdom → archives plan to `.maestro/archive/`.

### Source of Truth

| What | Where | NOT here |
|------|-------|----------|
| /design workflow | `.claude/skills/design/SKILL.md` | Agent definitions |
| /work workflow | `.claude/skills/work/SKILL.md` | Agent definitions |
| Agent identity + constraints | `.claude/agents/{name}.md` | Skills or docs |
| Skill overview + reference | `.claude/skills/maestro/SKILL.md` | README or CLAUDE.md |

Skills contain full workflows. Agent definitions are lean (identity + constraints only). No duplication between them.

### Hooks

Shell scripts in `.claude/scripts/` enforce workflow invariants via `.claude/hooks/hooks.json`:

| Hook | Trigger | Enforces |
|------|---------|----------|
| `orchestrator-guard.sh` | PreToolUse(Write/Edit) | Orchestrator cannot edit files directly — must delegate |
| `plan-protection.sh` | PreToolUse(Write/Edit) | kraken/spark cannot edit `.maestro/plans/` files |
| `verification-injector.sh` | PostToolUse(Task) | Reminds orchestrator to verify task results |
| `plan-validator.sh` | PostToolUse(Write) | Warns if plan is missing required sections |
| `wisdom-injector.sh` | PostToolUse(Read) | Surfaces wisdom files when a plan is read |
| `plan-context-injector.sh` | PreCompact | Injects active plan context into compaction summary |
| `session-start.sh` | SessionStart | Session initialization |
| `subagent-context.sh` | SubagentStart | Injects context into subagents |

### Runtime State

```
.maestro/
├── plans/      # Unexecuted work plans (active)
├── archive/    # Executed plans (moved here after /work completes)
├── drafts/     # Interview drafts (created during /design)
├── context/    # Project context files (product, tech stack, guidelines)
├── handoff/    # Session recovery JSON (design status, worktree metadata)
├── wisdom/     # Accumulated learnings from past executions
├── research/   # Research session state and findings
└── notepad.md  # Persistent notes (decisions, context, constraints)
```

### Skill Interoperability

Maestro auto-discovers installed skills from `.claude/skills/`, `~/.claude/skills/`, and plugin marketplaces. Matching skills are injected into worker prompts as `## SKILL GUIDANCE` sections. Discovery logic is in `.claude/lib/skill-registry.md`, matching logic in `.claude/lib/skill-matcher.md`.

## Agents

| Agent | Role | Model | Spawns |
|-------|------|-------|--------|
| `prometheus` (Plan) | Interview-driven planner | sonnet | — |
| `orchestrator` | Execution coordinator (team lead) | sonnet | kraken, spark, build-fixer, critic, explore |
| `kraken` | TDD implementation (worker) | sonnet | — |
| `spark` | Quick fixes (worker) | sonnet | — |
| `build-fixer` | Build/compile/lint error specialist (worker) | sonnet | — |
| `critic` | Post-implementation reviewer (read-only) | opus | — |
| `oracle` | Strategic advisor (read-only) | opus | — |
| `explore` (Explore) | Codebase search (read-only) | sonnet | — |
| `leviathan` | Deep plan reviewer | opus | — |
| `wisdom-synthesizer` | Knowledge consolidation | haiku | — |
| `progress-reporter` | Status tracking | haiku | — |
| `security-reviewer` | Security analysis specialist (read-only) | opus | — |

Team leads have `Task`, `TeamCreate`, `TeamDelete`, `SendMessage`. Workers have `TaskList`, `TaskGet`, `TaskUpdate`, `SendMessage` for self-coordination.

## Critical Rules

1. **Both phases use Agent Teams** — `/design` and `/work` both create teams and spawn teammates
2. **Orchestrator never edits directly** — Always delegates to kraken/spark (enforced by `orchestrator-guard.sh` hook)
3. **Workers cannot edit plans** — kraken/spark are blocked from `.maestro/plans/` (enforced by `plan-protection.sh` hook)
4. **Workers self-coordinate** — All agents have TaskList/TaskUpdate/SendMessage for parallel work
5. **Verify teammate claims** — Always read files and run tests after delegation
6. **TDD by default** — Use kraken for new features
7. **Plan required sections** — `## Objective`, `## Tasks` (with checkboxes), `## Verification` are mandatory
