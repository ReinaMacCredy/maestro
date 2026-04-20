# maestro

Maestro is a local-first conductor for multi-agent software engineering. It gives you one CLI and one on-disk state model for missions, features, assertions, handoff launches, checkpoints, memory, and project context so separate agent sessions can collaborate without a server, database, or background daemon.

It is designed for a workflow where a human operator coordinates multiple terminals, while Maestro keeps the shared state disciplined and inspectable.

## Why Maestro

- Shared state lives on disk in `.maestro/`, not in chat history.
- Missions break work into milestones, features, and validation assertions.
- Native handoff launches build a self-contained markdown brief and start a fresh Codex or Claude run from the current repo state.
- Memory commands turn corrections and learnings into reusable guidance.
- Mission Control gives you a read-only TUI and JSON snapshots of current state.
- The runtime stays local-first: filesystem, git, config, and terminal tools.

## At A Glance

```mermaid
flowchart LR
  human["Human operator"]
  maestro["Maestro CLI + .maestro/ state"]
  workerA["Agent terminal A"]
  workerB["Agent terminal B"]
  tui["Mission Control"]

  human --> maestro
  maestro --> workerA
  maestro --> workerB
  workerA --> maestro
  workerB --> maestro
  maestro --> tui
```

Maestro is the shared state layer in the middle. The human moves between terminals; agents read and write through the same local CLI surface.

## What Maestro Is Not

- It is not a hosted orchestration service or remote agent platform.
- It is not tied to a single model vendor or harness.
- It does not require a database, queue, or network API to work.

The human operator is the bridge between terminals. Maestro is the shared state layer underneath that workflow.

## Core Concepts

| Concept | Purpose |
|---|---|
| Mission | The top-level unit of work with a lifecycle such as `draft`, `approved`, or `executing`. |
| Milestone | A phase within a mission. Milestones can act as work phases or validation gates. |
| Feature | A concrete piece of work assigned to an agent type, with verification steps and optional dependencies. |
| Assertion | A validation target tied to a feature. Assertions are updated to `passed`, `failed`, `blocked`, or `waived`. |
| Handoff | A persisted launch record plus markdown brief for starting a fresh Codex or Claude session from current mission or repo context. |
| Task | A Claude-style blocker-graph work item for the daily loop; lives at `.maestro/tasks/tasks.jsonl` independent of missions. |
| Reply | A worker's structured outcome record for a feature, optionally gated by behavioral principles. |
| Principle | A behavioral rule injected into worker prompts and scored against replies. Stored at `.maestro/principles.jsonl`. |
| Memory | Corrections, learnings, and compiled guidance that feed back into future worker prompts. |
| Checkpoint | A timestamped mission snapshot you can save and later restore. |
| Bundle | A portable `.mission.tar.gz` archive of a mission plus its artifacts for review or transfer. |
| Mission Control | A read-only dashboard for previewing mission state interactively or as JSON. |

## Mission Control Preview

Mission Control gives you a read-only terminal dashboard over the current Maestro state.

![Mission Control preview](assets/images/Misson_Control_Preview.png)

## How Work Flows

```mermaid
flowchart TD
  init["Initialize project"] --> plan["Create mission plan JSON"]
  plan --> mission["maestro mission create --file plan.json"]
  mission --> approve["maestro mission approve <mission-id>"]
  approve --> prompt["Optional: maestro feature prompt <feature-id> --mission <mission-id>"]
  prompt --> handoff["maestro handoff \"Implement auth-impl\" --provider codex"]
  handoff --> worker["Fresh Codex or Claude run starts with a persisted markdown brief"]
  worker --> progress["maestro feature update ... --status in-progress/review"]
  progress --> validate["maestro validate update ... --result passed|failed|waived"]
  validate --> checkpoint["maestro checkpoint save --mission <mission-id>"]
  checkpoint --> seal["maestro milestone seal <milestone-id> --mission <mission-id>"]
```

The loop is deliberately simple: define work, optionally inspect the current feature brief, launch a fresh handoff, update progress, validate the outcome, and checkpoint before sealing the milestone.

## Installation

### Requirements

- [Bun](https://bun.sh/)
- Git
- A local agent harness in another terminal, such as Claude Code, Codex, Gemini CLI, or Droid CLI

### Install From Release

Install the latest published Maestro binary:

```bash
curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install.sh | bash
```

Install a specific published release:

```bash
MAESTRO_VERSION=<version> curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install.sh | bash
```

After installation, refresh to the latest published release with:

```bash
maestro update
```

### Build From Source

```bash
bun install
bun run build
```

This produces the compiled binary at `./dist/maestro`.

### Install Locally

```bash
bun run release:local
command -v maestro
maestro --version
```

If you also want to initialize global config and inject supported agent instruction blocks:

```bash
maestro install
```

`./dist/maestro` is the fresh repo build. `maestro` on your `PATH` is the installed local binary.

## Quick Start

### 1. Initialize a project

```bash
maestro init
```

This creates the local `.maestro/` workspace for the current repository.

### 2. Create a mission plan file

`mission create` expects a JSON plan file. A minimal example:

```json
{
  "title": "Add authentication",
  "description": "Ship the first authentication slice",
  "milestones": [
    {
      "id": "plan",
      "title": "Planning",
      "description": "Define the implementation approach",
      "order": 0,
      "kind": "work",
      "profile": "planning"
    },
    {
      "id": "implement",
      "title": "Implementation",
      "description": "Build and verify the feature",
      "order": 1,
      "kind": "work",
      "profile": "implementation"
    }
  ],
  "features": [
    {
      "id": "auth-plan",
      "milestoneId": "plan",
      "title": "Plan the auth flow",
      "description": "Define the login shape, risks, and acceptance criteria",
      "agentType": "codex-cli",
      "verificationSteps": [
        "Review the proposed flow with the team"
      ]
    },
    {
      "id": "auth-impl",
      "milestoneId": "implement",
      "title": "Implement the auth flow",
      "description": "Build the first working authentication slice",
      "agentType": "codex-cli",
      "dependsOn": [
        "auth-plan"
      ],
      "verificationSteps": [
        "Run build",
        "Run targeted tests",
        "Verify the login flow manually"
      ],
      "fulfills": [
        "auth-login-works"
      ]
    }
  ]
}
```

### 3. Create and approve the mission

```bash
maestro mission create --file plan.json
maestro mission list
maestro mission approve <mission-id>
```

### 4. Inspect the current feature brief (optional)

```bash
maestro feature list --mission <mission-id>
maestro feature prompt <feature-id> --mission <mission-id> --out worker-prompt.md
```

This writes the prompt to `worker-prompt.md` and also stores it under `.maestro/missions/<mission-id>/workers/<feature-id>/prompt.md`. `maestro handoff` does not require this step, but it is useful when you want to inspect the current feature context before launching a fresh agent run.

### 5. Launch a handoff

Launch a fresh Codex run for the next implementation slice:

```bash
maestro handoff \
  "Implement auth-impl for mission <mission-id> and run the listed verification steps before stopping" \
  --provider codex
```

Launches are detached by default. Maestro persists the handoff under `.maestro/launches/<id>/` and returns the launch record with the prompt path, log path, target directory, and provider details.

Useful variants:

```bash
maestro handoff "Review auth-impl before merge" --provider claude --worktree auth-review
maestro handoff "Finish auth-impl and wait for the result" --provider codex --wait --json
```

### 6. Track progress, validate, and seal

```bash
maestro feature update auth-impl --mission <mission-id> --status in-progress
maestro feature update auth-impl --mission <mission-id> --status review
maestro validate show --mission <mission-id>
maestro validate update auth-login-works --mission <mission-id> --result passed --evidence "bun test"
maestro checkpoint save --mission <mission-id>
maestro milestone seal implement --mission <mission-id>
```

## Handoffs

`maestro handoff "<task>"` builds a self-contained markdown brief and launches a fresh Codex or Claude run from the current repo state. Every launch is persisted under `.maestro/launches/<id>/` so the operator can inspect exactly what was sent and what the child process printed.

### What a launch contains

A launch record always includes:

- `id`, `provider`, and `model`
- `status`: `launching`, `launched`, `completed`, or `failed`
- `targetDir`: the directory handed to the external agent
- `promptPath`, `outputPath`, and the exact launched `command`
- optional `worktree` metadata when `--worktree` is used
- optional `pid` and `exitCode`, depending on detached vs `--wait` mode

The prompt itself is stored separately as markdown. Maestro always renders the same sections:

- `Task`
- `Context`
- `Relevant Files`
- `Current State`
- `What Was Tried`
- `Decisions`
- `Acceptance Criteria`
- `Constraints`

### How prompt context is chosen

- If exactly one actionable feature exists in the active mission, Maestro anchors the brief to that mission, milestone, feature, assertions, worker prompt or report artifacts, and the current git state.
- Otherwise it falls back to repository context: current branch, recent commits, and changed files.
- When `--worktree` is used, Maestro creates the sibling worktree first and appends that worktree path and branch information to the `Constraints` section.

### Launch status flow

```mermaid
flowchart LR
  launching["launching"] --> launched["launched"]
  launching --> completed["completed"]
  launching --> failed["failed"]
```

- Default mode returns as soon as the child process is started and records status `launched`.
- `--wait` blocks until the provider exits and records `completed` or `failed`.
- `--json` prints the persisted launch record for automation or debugging.

### Typical commands

Default Codex launch:

```bash
maestro handoff \
  "Implement auth-impl for mission <mission-id> and verify the touched surface area" \
  --provider codex
```

Claude launch in a sibling worktree:

```bash
maestro handoff \
  "Review auth-impl for regressions and missing tests" \
  --provider claude \
  --worktree auth-review
```

Foreground automation-friendly launch:

```bash
maestro handoff \
  "Finish auth-impl and return only after the tests pass" \
  --provider codex \
  --wait \
  --json
```

Use `--model` to override the provider default (`gpt-5.4` for Codex, `opus` for Claude), `--name` to label the launch, and `--base` when you need a specific base branch for a worktree handoff.

## Common Commands

| Command | Use it when you want to... |
|---|---|
| `maestro init` | Create local project state. |
| `maestro install` | Initialize global config and inject supported agent instruction blocks. |
| `maestro update` | Upgrade the local binary to the latest release and refresh agent instruction blocks. |
| `maestro doctor` | Check whether the local environment is configured correctly. |
| `maestro status` | Inspect the current Maestro state quickly. |
| `maestro mission create --file plan.json` | Create a mission from a plan file. |
| `maestro feature prompt <feature-id> --mission <mission-id>` | Generate the next worker prompt. |
| `maestro feature update <feature-id> --mission <mission-id> --status <status>` | Advance a feature through `pending`, `assigned`, `in-progress`, `review`, `done`, or `blocked`. |
| `maestro reply write <feature-id>` | Record a worker reply (outcome + optional report) for a feature. |
| `maestro handoff "<task>" --provider <provider>` | Build a markdown brief from current repo or mission context and launch a fresh agent run. |
| `maestro handoff "<task>" --worktree [slug] --wait --json` | Launch in a sibling worktree, wait for completion, and return structured metadata. |
| `maestro mission-control --preview` | Render a read-only dashboard preview in the terminal. |
| `maestro mission-control --json` | Get a machine-readable snapshot of mission state. |
| `maestro mission-control --render-check --size 120x40` | Validate TUI render integrity non-interactively. |
| `maestro task ready` | List actionable pending tasks with no unresolved blockers. |
| `maestro task claim <id>` | Take ownership of a task for the current session. |
| `maestro task update <id> --status in_progress` / `--status completed --reason "..."` | Start or finish a task. |
| `maestro task block <id> <blockedTaskIds...>` | Record that one task blocks others. |
| `maestro principle list` / `principle add` | Inspect or register a behavioral principle. |
| `maestro bundle export <mission-id> --out ./review.mission.tar.gz` | Package a mission + artifacts as a portable archive. |
| `maestro bundle inspect <path>` | Print a mission bundle's manifest without extracting. |
| `maestro memory-correct <rule>` | Capture a correction that should influence future runs. |
| `maestro memory-compile` | Turn raw learnings into reusable guidance. |
| `maestro ratchet-check` | Run the regression ratchet suite. |

Run `maestro <command> --help` for full flags and examples.

## Mission Control

Mission Control is a read-only dashboard over Maestro state. It supports:

- Interactive TTY mode with `maestro mission-control`
- Single-frame previews with `maestro mission-control --preview`
- Machine-readable snapshots with `maestro mission-control --json`
- Render validation with `maestro mission-control --render-check --size 120x40`

Available preview screens include:

- `dashboard`
- `features`
- `dependencies` (mission-only)
- `config`
- `memory`
- `graph`
- `agents`
- `dispatch` (mission-only)
- `events`
- `tasks`
- `timeline` (mission-only)
- `principles`
- `help`

Aliases: `feat`, `deps`, `cfg`, `mem`, `agent`, `event`, `task`, `principle`. Mission-only screens are skipped automatically when running in home mode.

For non-interactive environments, prefer `--preview`, `--preview all`, or `--json`.

## Architecture

```mermaid
flowchart TD
  cli["CLI commands"] --> usecases["Use cases"]
  usecases --> domain["Domain models + validators"]
  usecases --> ports["Ports"]
  ports --> adapters["Filesystem, git, config, session adapters"]
  adapters --> state[".maestro/ state + local environment"]
  state --> tui["Mission Control snapshots"]
```

The codebase follows a hexagonal shape: commands call use cases, use cases depend on domain rules and ports, and adapters implement those ports against the local filesystem and environment.

## Storage Model

Maestro stores project-local state in `.maestro/` and user-level defaults in `~/.maestro/`.

```text
.maestro/
├── config.yaml
├── launches/
├── memory/
│   ├── corrections/
│   ├── learnings/
│   └── ratchet/
├── missions/
│   └── <mission-id>/
│       ├── mission.json
│       ├── assertions.json
│       ├── checkpoints/
│       ├── features/
│       └── workers/
├── tasks/
│   ├── tasks.jsonl
│   └── candidates/
├── principles.jsonl
└── notes.json

~/.maestro/
├── config.yaml
└── graph/
    └── projects.json
```

The design is intentionally transparent: state is inspectable, diffable, and easy to back up. `.maestro/tasks/**` and `.maestro/principles.jsonl` are intentionally repo-tracked so the daily queue and behavioral rules are reviewed like any other code change; `.maestro/missions/**` and `.maestro/launches/**` stay ignored as local orchestration artifacts. Legacy `.maestro/handoffs/**` folders can remain on disk, but Maestro no longer reads them and `status` or `doctor` will warn when they are present.

## Codebase Layout

Maestro is organized as a feature-first hexagonal codebase:

- `src/features/<name>/` -- each feature is a bounded context containing its own `commands/`, `usecases/`, `domain/`, `ports/`, `adapters/`, plus a `services.ts` composition factory and `index.ts` public surface. Current features: `ratchet`, `handoff` (markdown prompt building plus Codex or Claude launch orchestration), `notes`, `graph`, `session`, `memory`, `mission` (with `feature/`, `validation/`, `checkpoint/` subfolders and behavioral principles), `agent` (library-only; composes worker prompts and manages harness config injection), `task` (Claude-style blocker graph for the daily loop), `reply` (worker reply ingest with principle gating), and `bundle` (portable mission archive).
- `src/infra/` -- plumbing that isn't a feature: init, doctor, status, install, update, uninstall, and mission-control commands, config and git ports/adapters, and infra-owned domain types.
- `src/shared/` -- generic utilities with no domain knowledge: filesystem, YAML, shell, path safety, and output formatting under `lib/`; cross-cutting primitives like IDs and UI config under `domain/`; plus top-level `errors.ts`, `version.ts`, and `version-format.ts`.
- `src/tui/` -- read-only rendering and input for Mission Control; consumes features through their public surfaces. See `src/tui/README.md` for the contributor-oriented TUI architecture walkthrough.
- `src/services.ts` -- composition root that wires every feature's adapters into a single service object.
- `src/index.ts` -- Commander CLI entry point.

Cross-feature imports must go through `@/features/<name>`, which resolves to the feature's `index.ts`. Deep imports across feature boundaries are forbidden and enforced by `bun run check:boundaries` in CI. Two features have explicit, scoped exceptions:

- `agent` may import from `mission`, `memory`, and `handoff` through their public surfaces, because agent composes prompts from mission context, memory hints, and the native handoff launcher APIs.
- `bundle` may import from `mission`, `reply`, `handoff`, and `session` through their public surfaces, because bundle is a read-only aggregator that snapshots every mission artifact, including handoff launches, into a portable archive.

The runtime is intentionally narrow: filesystem-backed stores, git integration, config handling, and a terminal UI. There is no database adapter or network service in the main workflow.

## Development

```bash
bun run build
bun run typecheck
bun test
bun run tui:dev
bun run release:local
```

Useful verification commands for CLI and TUI work:

```bash
./dist/maestro --version
maestro --version
maestro --help
maestro mission-control --preview --size 120x40 --format plain
maestro mission-control --render-check --size 120x40
```

## Supported Agent Config Injection

`maestro install` and `maestro update` can inject instruction blocks for:

- Claude Code
- Codex
- Gemini CLI
- Droid CLI

These instruction blocks help each harness read and write shared Maestro state consistently.

## License

MIT
