# maestro

Harness for long-running AI coding agents -- structured memory, cross-feature learning, and plan-approve-execute workflow.

A CLI-first tool that gives AI coding agents persistent state, workflow guardrails, and a plan-first pipeline. Agents interact via `maestro <command> --json` through Bash. All durable state lives under `.maestro/`.

## Quick Start

```bash
maestro init                                    # initialize project
maestro feature-create my-feature               # create feature
maestro memory-write --name findings \
  --file research.md                            # save research
maestro plan-write --file plan.md               # write plan (auto-detects active feature)
maestro plan-approve                            # approve plan
maestro task-sync                               # generate tasks from plan
maestro task-next --json                        # find next runnable task
maestro task-claim --task 01-example \
  --agent-id worker-1                           # claim task for an agent
maestro task-brief --task 01-example --json     # get compiled worker context
maestro task-done --task 01-example \
  --file summary.md                             # mark task complete
maestro feature-complete                        # close the feature
```

All commands auto-detect the active feature -- no need to pass `--feature` after `feature-create`.

## Prerequisites

- [bun](https://bun.sh) (runtime and package manager)
- [git](https://git-scm.com) (repo state and audit capture)

### Optional integrations

The following tools power optional adapters. maestro degrades gracefully when any are absent.

| Tool | Adapter | Purpose |
|------|---------|---------|
| [br](https://github.com/Dicklesworthstone/beads_rust) (beads_rust) | sync backend | Task tracking and bead persistence |
| [bv](https://github.com/Dicklesworthstone/beads_viewer) (beads viewer) | `bv-graph` | Dependency graph insights, next-task recommendations, execution plans |
| [cass](https://github.com/Dicklesworthstone/coding_agent_session_search) (Coding Agent Session Search) | `cass-search` | Full-text search across coding agent sessions (Claude Code, Codex, Cursor, Gemini) |
| [agent-mail](https://github.com/Dicklesworthstone/mcp_agent_mail) (MCP Agent Mail) | `agent-mail-handoff` | Cross-agent handoff notifications via HTTP API |

## Building

```bash
bun install
bun run build
```

Produces:

| Output | Purpose |
|--------|---------|
| `dist/maestro` | Standalone binary |
| `dist/cli.js` | npm CLI entry |
| `hooks/*.mjs` | 5 Claude Code hook scripts |
| `dist/server.bundle.mjs` | Plugin server (empty, for hook registration) |

Development mode: `bun src/surfaces/cli/index.ts <command>`.

## Architecture

maestro is a **CLI-first harness** -- the AI agent is the orchestrator, maestro is the filing cabinet with opinions. Agents call `maestro <command> --json` via Bash. Hooks inject context automatically.

```text
surfaces/  -->  app/     -->  domain/ports/  <--  infra/adapters/
(CLI,           (rules,       (interfaces)       (implementations)
 hooks)          DCP, skills)
```

```text
src/
  domain/           # Pure domain: ports (interfaces), types, errors
    ports/          # 11 port interfaces (task, feature, plan, memory, doctrine,
                    #   verification, graph, handoff, search, settings, host)
  app/              # Application layer: use cases, DCP, skills, workflow engine
    dcp/            # Dynamic Context Pruning (budget-based memory scoring)
    doctrine/       # Cross-feature learning compiler
    skills/         # Skill loader and registry generator
    tasks/          # Task state machine, graph, verification, spec builder
    workflow/       # Pipeline stages, playbook, execution insights
    ...             # Plus domain orchestration (features, plans, memory, etc.)
  infra/            # Infrastructure: adapters, settings, utilities
    adapters/       # Port implementations (fs-based + toolbox-provided)
    toolbox/        # Plugin registry and loader (br, bv, cass, agent-mail)
    utils/          # Filesystem, git, paths, validation, output
    visual/         # HTML/CSS renderer for visualization
  surfaces/         # External interfaces
    cli/            # 89 CLI commands (citty framework)
    mcp/            # Empty server shell (hooks require plugin recognition)
    hooks/          # 5 Claude Code hooks
  container.ts      # Immutable DI container -- wires ports to adapters
  services.ts       # Service locator (thin shim over container)
skills/             # Bundled SKILL.md workflow guides
hooks/              # Installable Claude Code hooks (build output)
```

The container (`createContainer()`) returns an `Object.freeze()`-d service object. Domain ports have zero dependencies on infrastructure. Optional ports (graph, search, handoff) resolve via toolbox at startup.

### Pipeline

```text
discovery --> research --> planning --> approval --> execution --> done
```

Stages are skippable. Hooks inject pipeline context automatically.

### Task Model

6 states: `pending` --> `claimed` --> `done` | `blocked` | `review` --> `revision`

Stale claims expire after a configurable timeout (default 120 min) and auto-reset to `pending` on `task-next`.

## CLI Commands (89)

All commands accept `--json` for structured output. All feature-scoped commands auto-detect the active feature. Use `maestro <command> --help` for full usage.

Content-accepting commands support three input modes: `--content "..."` (inline), `--file path` (from file), or `--stdin` (piped).

| Domain | Commands | Count |
|--------|----------|-------|
| Feature | create, list, info, active, complete | 5 |
| Plan | write, read, approve, revoke, comment, comments-clear | 6 |
| Task | sync, list, next, info, claim, done, block, unblock, accept, reject, brief, spec-read, spec-write, report-read, report-write | 15 |
| Memory | write, read, list, delete, compile, consolidate, archive, stats, promote, compress, connect, insights | 12 |
| Doctrine | list, read, write, deprecate, suggest, approve | 6 |
| Graph | insights, next, plan, discovery, reserve | 5 |
| Handoff | send, receive, ack, read, list, status | 6 |
| Search | sessions, related, similar | 3 |
| Skill | load, list, install, create, remove, sync | 6 |
| Stage | jump, skip, back | 3 |
| Config | get, set, agent | 3 |
| Toolbox | add, create, install, list, remove, test | 6 |
| Visual | visual, debug-visual | 2 |
| Other | init, install, ping, status, agents-md, dcp-preview, doctor, history, execution-insights, self-update, update | 11 |

## Hooks

Installable Claude Code hooks that integrate maestro into the agent lifecycle:

| Hook | File | Trigger | Purpose |
|------|------|---------|---------|
| SessionStart | sessionstart.mjs | Session begins | Inject pipeline state and recommended skills |
| PreToolUse:Bash | pretooluse.mjs | Before Bash tool | Remind agent to finish maestro tasks on git commit |
| PreToolUse:Agent | pre-agent.mjs | Before agent spawn | Inject task spec + DCP-scored memories + doctrine |
| PostToolUse | posttooluse.mjs | After tool execution | Track tool usage and state changes |
| PreCompact | precompact.mjs | Before context compaction | Preserve critical maestro state |

Install with `maestro install`.

## Skills

21 bundled workflow skills, all using `maestro:` colon-prefixed naming:

- `maestro:design` -- deep discovery and specification (16-step process with reference files)
- `maestro:implement` -- task execution with TDD, parallel, and team modes
- `maestro:review` -- track-aware code review with automated checks
- `maestro:brainstorming` -- creative exploration before implementation
- `maestro:parallel-exploration` -- parallel read-only exploration
- `maestro:dispatching` -- parallel agent dispatch
- `maestro:debugging` -- systematic debugging methodology
- `maestro:tdd` -- test-driven development guidance
- `maestro:verification` -- verification before completion
- `maestro:agents-md` -- AGENTS.md quality discipline and generation
- `maestro:docker` -- Docker container workflows
- `maestro:prompt-leverage` -- prompt engineering for AI agents
- `maestro:new-feature` -- create feature/bug tracks with spec and plan
- `maestro:note` -- capture decisions and context to persistent notepad
- `maestro:plan-review-loop` -- iterative adversarial plan review
- `maestro:revert` -- git-aware undo of track implementation
- `maestro:next-move` -- strategic analysis for highest-leverage next step
- `maestro:simplify` -- review changed code for reuse, quality, and efficiency
- `maestro:visual` -- generate visual explanations of systems and data

Load with `maestro skill <name>`, list with `maestro skill-list`.

Skills with `reference/` subdirectories support progressive disclosure:
```
maestro skill maestro:design --ref steps/step-01-init.md
```

Old skill names (e.g., `writing-plans`) are aliased with deprecation warnings.

## Data Layout

```text
.maestro/
  config.json                           # Project configuration
  doctrine/                             # Doctrine items (cross-feature operating rules)
    <name>.json                         # Structured rule with effectiveness metrics
  memory/                               # Global project-scoped memory files
  features/
    <feature>/
      feature.json                      # Feature metadata and state
      plan.md                           # Implementation plan
      comments.json                     # Plan review comments
      memory/                           # Feature-scoped memory files
      tasks/
        <task>/
          task.json                     # Task state, claims, summaries
          spec.md                       # Compiled task specification
          report.md                     # Task completion report
          doctrine-trace.json           # Doctrine injection trace
```

## License

MIT
