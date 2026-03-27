# TypeScript Style Guide

## Types
- Prefer `interface` for object shapes and `type` for unions or intersections
- Avoid `any`; use `unknown` and narrow with type guards
- Use `readonly` for immutable data
- Prefer `const` assertions for literal types
- Use discriminated unions over optional fields for variant types

## Naming
- Types and interfaces: PascalCase
- Variables and functions: camelCase
- Constants: UPPER_SNAKE_CASE
- Enums: PascalCase for both enum names and members
- Files: kebab-case

## Functions
- Prefer arrow functions for callbacks and short expressions
- Use named functions for top-level declarations
- Add explicit return types for public API functions
- Use function overloads sparingly; prefer union types

## Async
- Always `await` promises; avoid fire-and-forget flows
- Use `Promise.all()` for parallel independent operations
- Handle errors with `try/catch` at the boundary rather than every call site
- Prefer `async/await` over `.then()` chains

## Imports
- Group imports by built-in, external, internal, then relative
- Use named imports instead of `import *`
- Avoid circular dependencies

## Nullability
- Prefer `undefined` over `null`
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid non-null assertions except in tests or tightly constrained cases

## Testing
- Use `describe` and `it` for structure
- Mock external dependencies, not internal modules
- Test error paths in addition to happy paths


# maestro -- CLI Harness for Agent-Optimized Development

## Getting Started

At the start of every session, call `maestro status --json`.
The status response includes a `playbook` field with stage-specific tools, skills, objectives, and anti-patterns.
Load recommended skills from `playbook.skills` with `maestro skill <name>`.
`skills.recommended` is also present for backward compatibility.

## Architecture

maestro is a **CLI-first harness** -- structured memory + workflow guardrails.
Claude Code is the orchestrator (spawning agents natively), maestro is the filing cabinet with opinions.
Agents interact via `maestro <command> --json` through Bash. Hooks inject context automatically.

- **6 task states**: pending, claimed, done, blocked, review, revision
- **89 CLI commands** across 14 domains, all with `--json` support
- **Plain file backend** (default), optional br sync
- **Hooks**: SessionStart (pipeline injection), PreToolUse:Agent (task spec injection)
- **Doctrine Compiler**: cross-feature learning from execution history, injected into workers via separate budget
- **Pipeline**: discovery --> research --> planning --> approval --> execution --> done (stages are skippable)
- **DCP budgets**: token-based (chars/4 estimation). Config supports both `*BudgetTokens` (preferred) and `*BudgetBytes` (backward compat, auto-derives tokens)
- **Skill stages**: built-in and external skills declare their pipeline stage via `stage:` frontmatter. Playbook auto-discovers external skills tagged for the current stage.

## Workflow Phases

| Phase | Trigger | MCP Tools / CLI Commands |
|-------|---------|--------------------------|
| Discovery | New feature request | `maestro feature-create`, `maestro memory-write` |
| Research | Feature exists | Agent subagents, `maestro memory-write` to capture findings |
| Planning | Research done | `maestro plan-write`, `maestro plan-read` |
| Approval | Plan written | `maestro plan-approve` |
| Execution | Plan approved | `maestro task-sync`, `maestro task-next`, `maestro task-claim`, `maestro task-done` |
| Completion | All tasks done | `maestro feature-complete`, `maestro memory-promote`, `maestro doctrine-approve` |

## Planning Mode

1. Load `maestro:design` and `maestro:parallel-exploration` skills
2. Research the codebase, save findings with `maestro memory-write`
3. Write the plan with `maestro plan-write`
4. Review comments with `maestro plan-read`
5. Approve with `maestro plan-approve`

## Execution Mode

1. `maestro task-sync` -- generate tasks from approved plan
2. `maestro task-next` -- find runnable tasks with compiled specs
3. `maestro task-claim` -- claim a task for an agent
4. Spawn Agent to implement (pre-agent hook auto-injects spec + worker rules)
5. `maestro task-done` -- mark complete with summary
6. Repeat until all tasks done

## Blocked Tasks

If a worker hits a blocker:
1. Worker runs `maestro task-block` with reason
2. Review blocker in `maestro status`
3. Resolve and run `maestro task-unblock` with decision

## Stale Claims

Claims expire after `claimExpiresMinutes` (default 120). Expired claims are auto-reset to pending when `maestro task-next` is called.

## CLI Interface

All commands accept `--json` for structured output. Use `maestro <command> --help` for full usage.

## CLI Commands (70)

Commands organized by domain:

### Feature (5)
`feature-create`, `feature-list`, `feature-info`, `feature-active`, `feature-complete`

### Plan (6)
`plan-write`, `plan-read`, `plan-approve`, `plan-revoke`, `plan-comment`, `plan-comments-clear`

### Task (12)
`task-sync`, `task-list`, `task-next`, `task-info`, `task-claim`, `task-done`, `task-block`, `task-unblock`, `task-spec-read`, `task-spec-write`, `task-report-read`, `task-report-write`

### Memory (9)
`memory-write`, `memory-read`, `memory-list`, `memory-delete`, `memory-compile`, `memory-consolidate`, `memory-archive`, `memory-stats`, `memory-promote`

### Handoff (3)
`handoff-send`, `handoff-receive`, `handoff-ack`

### Graph (3)
`graph-insights`, `graph-next`, `graph-plan`

### Search (2)
`search-sessions`, `search-related`

### Doctrine (6)
`doctrine-list`, `doctrine-read`, `doctrine-write`, `doctrine-deprecate`, `doctrine-suggest`, `doctrine-approve`

### Config (3)
`config-get`, `config-set`, `config-agent`

### Toolbox (6)
`toolbox-add`, `toolbox-create`, `toolbox-install`, `toolbox-list`, `toolbox-remove`, `toolbox-test`

### Visual (2)
`visual`, `debug-visual`

### Other (13)
`init`, `install`, `status`, `agents-md`, `skill`, `skill-list`, `dcp-preview`, `ping`, `doctor`, `history`, `execution-insights`, `self-update`, `update`

All commands accept `--json`. Use `maestro <command> --help` for full usage.

## CLI Reference (Agent Use)

All commands accept `--json`. Always pass `--json` explicitly on every call.
Content params: prefer `--file <path>` for multi-line content. Alternatives: `--content "..."` (short text only) or `--stdin`.

### Session
maestro status --json                    # [read] orient: stage, playbook, next action
maestro skill <name>                     # [read] load skill into context

### Discovery
maestro feature-create <name> --json                 # [write]
maestro feature-active --json                        # [read]

### Research
maestro memory-write --name <n> --file <path> --json # [write]
maestro memory-read --name <n> --json                # [read]
maestro memory-list --json                           # [read]

### Planning
maestro plan-write --file <path> --json              # [write]
maestro plan-read --json                             # [read]
maestro plan-approve --json                          # [write]

### Execution
maestro task-sync --json                             # [write]
maestro task-next --json                             # [read]
maestro task-claim --task <id> --agent-id <name> --json  # [write]
maestro task-brief --task <id> --json                # [read]
maestro task-done --task <id> --file <summary> --json    # [write]
maestro task-block --task <id> --reason "..." --json     # [write]
maestro task-accept --task <id> --json               # [write]
maestro task-reject --task <id> --file <feedback> --json # [write]

### Done
maestro feature-complete --json                      # [write]
maestro memory-promote --name <n> --json             # [write]

## agentMemory Integration

maestro integrates with **agentMemory** (`~/Code/agentMemory/`), a separate repo that provides a workflow-aware retrieval engine for `.maestro/` memory files.

### What agentMemory Does
- **Read-only retrieval engine** -- indexes `.maestro/` memory `.md` files, never writes them
- **Sidecar index** at `.maestro/retrieval-index.json` with keyword tokens, checksums, optional embeddings
- **6-signal hybrid retrieval**: semantic (0.25), keyword BM25 (0.15), pipeline stage (0.20), dependency graph (0.20), execution feedback (0.15), recency (0.05)
- **MMR diversity selection** within token budgets
- **Feedback loop** via `.maestro/feedback.jsonl` -- correlates injected memories with task outcomes

### How It Connects
- Installed as a dependency: `"agent-memory": "file:../agentMemory"` in package.json
- Toolbox adapter at `src/infra/toolbox/tools/external/agent-memory/` (manifest + adapter)
- Adapter registered in `src/infra/toolbox/loader.ts` as `'agent-memory'`
- Container resolves `agentMemoryRetriever` when the toolbox detects it (optional, graceful fallback)
- `taskBrief()` in `src/app/tasks/task-brief.ts` delegates to `agentMemoryRetriever.compile()` when available
- Pre-agent hook passes the retriever through to `taskBrief()`
- When not installed, standard DCP `selectMemories()` runs unchanged

### Key Files
| File | Role |
|------|------|
| `src/infra/toolbox/tools/external/agent-memory/adapter.ts` | Thin wrapper calling agentMemory library |
| `src/infra/toolbox/tools/external/agent-memory/manifest.json` | Toolbox manifest (provides: null, priority: 200) |
| `src/container.ts` | Resolves `agentMemoryRetriever` from toolbox |
| `src/app/tasks/task-brief.ts` | Uses `compile()` for hybrid retrieval when available |
| `src/surfaces/hooks/pre-agent.ts` | Passes retriever to `taskBrief()` |

### Development
- agentMemory repo: `~/Code/agentMemory/` (GitHub: ReinaMacCredy/agentMemory)
- After changes to agentMemory, run `bun install` in maestro to pick up updates
- Both repos must typecheck independently: `bun run typecheck` in each
