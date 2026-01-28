# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is an AI agent workflow skills plugin for Claude Code (and other AI coding agents like Amp, Codex CLI, Cursor, Gemini CLI). It provides context-driven development with structured planning, TDD execution, persistent issue tracking, and multi-agent orchestration.

**Core philosophy:** "Spend tokens once on a good plan; reuse it many times."

## Commands

### Validation

```bash
cat .claude-plugin/plugin.json | jq .     # Validate plugin manifest
./scripts/validate-links.sh               # Validate documentation links
./scripts/validate-anchors.sh             # Validate markdown anchors
```

### Beads CLI (external dependency)

```bash
bd ready --json                           # Find available work (always use --json)
bd show <id>                              # Read task context
bd update <id> --status in_progress       # Claim task
bd close <id> --reason completed          # Complete task
bd sync                                   # Sync to git
bv --robot-stdout                         # Beads validation (NEVER use bare bv - it hangs)
```

## Architecture

### Directory Structure

```
.claude/                     # Claude Code runtime configuration
├── agents/                  # Agent definitions (standalone + symlinks)
├── commands/                # Slash commands (/atlas-plan, /atlas-work, etc.)
├── hooks/                   # Hook configuration
├── plans/                   # Generated execution plans
├── scripts/                 # Hook scripts (symlinks to ../../scripts/)
└── skills/                  # Skill packages
    └── atlas/               # Main workflow skill
        └── references/agents/  # Atlas agent definitions

.claude-plugin/              # Plugin manifest (entry point)
├── plugin.json              # Plugin definition
└── .lsp.json                # LSP server configuration

.atlas/                      # Runtime workflow state
├── plans/                   # Committed work plans
├── drafts/                  # Interview drafts
├── notepads/                # Wisdom accumulation per plan
├── boulder.json             # Active execution state
└── ralph-loop.local.md      # Ralph autonomous loop state

toolboxes/                   # MCP-to-CLI wrappers
├── agent-mail/              # Agent coordination CLI
└── ralph/                   # Ralph autonomous loop
```

### Atlas Workflow System

The Atlas workflow is the primary planning and execution system. It uses specialized agents spawned via `Task()`.

**Planning chain:**
```
@plan → atlas-prometheus → atlas-metis (gap analysis) → atlas-momus (review) → plan file
```

**Execution chain:**
```
/atlas-work → atlas-orchestrator → atlas-leviathan/kraken/spark → verification → wisdom
```

### Agent Hierarchy

| Agent | Purpose | Model | Notes |
|-------|---------|-------|-------|
| `atlas-prometheus` | Strategic planner, interview mode | sonnet | Chains to metis, momus, oracle |
| `atlas-orchestrator` | Master delegator | sonnet | **Never works directly** - always delegates |
| `atlas-leviathan` | General implementation | sonnet | Terminal executor |
| `atlas-kraken` | TDD implementation, heavy refactors | sonnet | Red-green-refactor cycle |
| `atlas-spark` | Quick fixes, simple changes | sonnet | Lightweight, fast |
| `atlas-oracle` | Strategic advisor | opus | Read-only, high-IQ reasoning |
| `atlas-explore` | Codebase search | sonnet | Read-only |
| `atlas-librarian` | External docs/research | sonnet | Read-only |
| `atlas-metis` | Pre-planning consultant | sonnet | Gap analysis before planning |
| `atlas-momus` | Plan reviewer | sonnet | Ruthless critic, approves with "OKAY" |

### Hooks

Hooks intercept tool use and prompt submission:

- **UserPromptSubmit**: `keyword-detector.sh`, `registry-injector.sh`
- **PreToolUse (Write/Edit)**: `prometheus-guard.sh`, `orchestrator-guard.sh`
- **PostToolUse (Write/Edit)**: `git-diff-reporter.sh`, `comment-checker.sh`, `edit-recovery.sh`
- **PostToolUse (Task)**: `verification-injector.sh`, `empty-task-detector.sh`
- **Stop**: `todo-enforcer.sh`, `ralph-loop-handler.sh`

### Skill Structure

Skills follow Claude Code plugin spec:
- YAML frontmatter with `name` and `description` (required)
- Markdown body with workflow instructions
- `references/` subdirectory for detailed documentation
- Agents in `references/agents/` symlinked to `.claude/agents/`

## Triggers Quick Reference

| Trigger | Action |
|---------|--------|
| `/atlas-plan <request>` | Start Prometheus interview mode |
| `/atlas-work` | Execute plan in orchestrator mode |
| `/ralph-loop` | Autonomous execution until completion |
| `/cancel-ralph` | Stop Ralph loop |
| `@plan`, `ultraplan` | Prometheus interview mode |
| `@oracle` | Strategic advisor (opus) |
| `@explore` | Codebase search |
| `@librarian` | External research |
| `@momus` | Plan review |
| `@metis` | Pre-planning consultation |
| `@review` | Code quality review |
| `@docs` | Documentation writer |
| `@tdd` | TDD implementation (kraken) |

## Versioning

- **Plugin version**: Auto-bumped by CI (`feat:` → minor, `fix:` → patch, `feat!:` → major while 0.x)
- **Skill version**: Manual frontmatter update
- **Pre-1.0 semantics**: Breaking changes bump MINOR not MAJOR to prevent accidental 1.0.0

## Critical Rules

1. **Use `--json` with `bd`** - For structured output
2. **Use `--robot-*` with `bv`** - Bare `bv` hangs
3. **Orchestrator never edits directly** - Always delegates via Task()
4. **7-section prompts for Task()** - Mandatory format (50-200 lines)
5. **Verify subagent claims** - Always verify, agents can make mistakes
6. **TDD by default** - Never write production code without failing test first
7. **Commit `.beads/` with code** - Always commit together
