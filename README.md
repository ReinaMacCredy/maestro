# maestro

Harness for long-running AI coding agents -- structured memory, cross-feature learning, and plan-approve-execute workflow.

An MCP plugin that gives AI coding agents persistent state, workflow guardrails, and a plan-first pipeline. All durable state lives under `.maestro/`.

## Quick Start

```bash
maestro init                                    # initialize project
maestro feature-create my-feature               # create feature
maestro plan-write --feature my-feature \
  --content "## Discovery\n..."                 # write plan
maestro plan-approve --feature my-feature       # approve plan
maestro task-sync --feature my-feature          # generate tasks from plan
maestro task-next --feature my-feature          # find next runnable task
maestro task-claim --feature my-feature \
  --task 01-example                             # claim task for an agent
maestro task-done --feature my-feature \
  --task 01-example --summary "What changed"    # mark task complete
```

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
| `dist/server.bundle.mjs` | MCP server |
| `dist/cli.js` | npm CLI entry |
| `dist/maestro` | Standalone binary |
| `hooks/*.mjs` | 5 Claude Code hook scripts |

Development mode: `bun src/surfaces/cli/index.ts <command>`.

## Architecture

maestro is a **pure MCP plugin** -- the AI agent is the orchestrator, maestro is the filing cabinet with opinions.

```text
surfaces/  -->  app/     -->  domain/ports/  <--  infra/adapters/
(CLI, MCP,      (rules,       (interfaces)       (implementations)
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
    cli/            # 71 CLI commands (citty framework)
    mcp/            # 26 MCP tool handlers
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

## MCP Tools (26)

All tools are prefixed `maestro_` in MCP. Tools use merged action/what params: mutating tools use `action`, read-only tools use `what`.

| Group | Tools | Count |
|-------|-------|-------|
| Feature | `feature` (create, complete), `feature_read` (list, info, active) | 2 |
| Plan | `plan` (write, approve, revoke, comment, comments_clear), `plan_read` | 2 |
| Task | `task` (sync, claim, done, accept, reject, block, unblock, spec_write, report_write), `task_read` (list, info, spec, report, next, brief) | 2 |
| Memory | `memory` (write, delete, promote, compress, consolidate, connect, archive), `memory_read` (read, list, stats, insights, compile) | 2 |
| Doctrine | `doctrine` (write, approve, suggest, deprecate), `doctrine_read` (list, read) | 2 |
| Handoff | `handoff` (send, ack), `handoff_read` (read, list, status, receive) | 2 |
| Graph | `graph` (insights, next, plan, discovery, reserve) | 1 |
| Search | `search` (sessions, related, similar) | 1 |
| Config | `config_get`, `config_set` | 2 |
| Visual | `visual` | 1 |
| DCP | `dcp` (preview, stats, config) | 1 |
| Workflow | `stage` (jump, skip, back) | 1 |
| Standalone | `status`, `ping`, `init`, `doctor`, `history`, `execution_insights`, `skill` | 7 |

## CLI Commands (71)

All commands accept `--json` for machine-readable output. Use `maestro <command> --help` for full usage.

| Domain | Commands | Count |
|--------|----------|-------|
| Feature | create, list, info, active, complete | 5 |
| Plan | write, read, approve, revoke, comment, comments-clear | 6 |
| Task | sync, list, next, info, claim, done, block, unblock, spec-read, spec-write, report-read, report-write | 12 |
| Memory | write, read, list, delete, compile, consolidate, archive, stats, promote | 9 |
| Doctrine | list, read, write, deprecate, suggest, approve | 6 |
| Graph | insights, next, plan | 3 |
| Handoff | send, receive, ack | 3 |
| Search | sessions, related | 2 |
| Config | get, set, agent | 3 |
| Toolbox | add, create, install, list, remove, test | 6 |
| Visual | visual, debug-visual | 2 |
| Other | init, install, ping, status, agents-md, skill, skill-list, dcp-preview, doctor, history, execution-insights, self-update, update | 13 |

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
