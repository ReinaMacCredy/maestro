# maestro

Cross-agent handoff and mission control CLI.

Maestro is a plan-based orchestration layer for multi-agent software engineering. It combines handoff workflows with mission state, milestone progress, validation assertions, worker prompts, checkpoints, and lightweight project notes.

## Overview

Maestro manages two primary concerns:

1. **Cross-agent handoffs** — pass work between agents with preserved context
2. **Mission Control** — track plan execution across missions, milestones, features, assertions, and checkpoints

## Installation

Build from source:

```bash
bun install
bun run build
```

The build outputs to `dist/maestro` as a standalone executable.

To install globally and inject agent instruction blocks:

```bash
bun run build
./dist/maestro install
```

## Commands

### Core Project Setup

| Command | Description |
|---------|-------------|
| `maestro init` | Initialize `.maestro/` in the current project |
| `maestro init --global` | Initialize global config at `~/.maestro/` |
| `maestro install` | Initialize global config and inject agent instruction blocks |
| `maestro update` | Update the installed binary and refresh agent instruction blocks |
| `maestro update --agents-only` | Refresh agent instruction blocks without rebuilding the binary |
| `maestro uninstall` | Remove agent instruction blocks and the installed binary/config |
| `maestro uninstall --agents-only` | Remove only the injected instruction blocks |

### Cross-Agent Handoffs

| Command | Description |
|---------|-------------|
| `maestro handoff --prompt <agent> --task "..."` | Create a handoff and generate a receiving-agent prompt |
| `maestro handoff --prompt [agent]` | Generate a prompt for the latest pending handoff without creating a new one |
| `maestro handoff --list [--all]` | List active handoffs, optionally including completed ones |
| `maestro handoff-pickup [--claim] [--agent <name>]` | Pick up work from an existing handoff |
| `maestro handoff-dig "<query>"` | Search handoff history via CASS |
| `maestro handoff-drop <id>` | Remove a specific handoff |
| `maestro handoff-cleanup` | Delete all handoffs |
| `maestro handoff-report --content "..."` | Report completion on picked-up work |
| `maestro session` | Detect the current agent session |
| `maestro session -q` | Print only the session ID |
| `maestro status` | Show initialization, git, CASS, and pending handoff state |
| `maestro doctor` | Run environment and configuration health checks |
| `maestro note --content "..."` | Append a timestamped project note |
| `maestro note --list` | List saved project notes |

### Mission Control

Mission Control adds plan-based orchestration with milestones, features, assertions, worker prompts, and checkpoints.

#### Mission lifecycle

| Command | Description |
|---------|-------------|
| `maestro mission create --file plan.json` | Create a mission from a plan file |
| `maestro mission list [--status <status>] [--limit <n>]` | List missions with optional filtering |
| `maestro mission show <id>` | Show mission details with milestone progress and compact summary |
| `maestro mission approve <id>` | Approve a draft mission |
| `maestro mission reject <id>` | Reject a draft mission |
| `maestro mission update <id> --status <status>` | Update mission status |
| `maestro mission update <id> --title <title>` | Update mission title |
| `maestro mission update <id> --description <desc>` | Update mission description |

#### Feature management

| Command | Description |
|---------|-------------|
| `maestro feature list --mission <id>` | List features for a mission |
| `maestro feature list --mission <id> --milestone <mid> --status <status>` | Filter features by milestone and/or status |
| `maestro feature update <id> --mission <mid> --status <status>` | Update feature status |
| `maestro feature update <id> --mission <mid> --report @report.json` | Attach a worker report |
| `maestro feature update <id> --mission <mid> --status pending --retry-reason "..."` | Reset a feature to pending with retry context |
| `maestro feature prompt <id> --mission <mid>` | Generate a worker prompt and persist it under mission workers state |
| `maestro feature prompt <id> --mission <mid> --out /path/prompt.md` | Also write the prompt to a custom file |

#### Milestone tracking

| Command | Description |
|---------|-------------|
| `maestro milestone list --mission <id>` | List milestones with feature and assertion progress |
| `maestro milestone status <id> --mission <mid>` | Show detailed milestone status |
| `maestro milestone seal <id> --mission <mid>` | Seal a milestone once all assertions are terminal |

#### Validation

| Command | Description |
|---------|-------------|
| `maestro validate show --mission <id>` | Show assertions for a mission |
| `maestro validate show --mission <id> --milestone <mid>` | Show assertions for a single milestone |
| `maestro validate update <id> --mission <mid> --result <result>` | Update an assertion result |
| `maestro validate update <id> --mission <mid> --result waived --reason "..."` | Waive an assertion with a required reason |

#### Checkpointing

| Command | Description |
|---------|-------------|
| `maestro checkpoint save --mission <id>` | Save a timestamped mission state snapshot |
| `maestro checkpoint list --mission <id>` | List checkpoints (newest first) |
| `maestro checkpoint load --mission <id>` | Restore the latest checkpoint snapshot |

### Global Option

- `--json` — output structured JSON at the root command, group, or leaf command level

## Storage Layout

Maestro stores runtime state under `.maestro/` in the project root:

```text
.maestro/
├── handoffs/                    # Pending and completed handoffs
├── missions/
│   └── {missionId}/
│       ├── mission.json         # Mission metadata
│       ├── features/
│       │   └── {fid}.json       # Per-feature state
│       ├── assertions.json      # Assertions stored per mission
│       ├── checkpoints/
│       │   └── {cid}.json       # Timestamped snapshots
│       └── workers/
│           └── {fid}/
│               ├── prompt.md    # Generated worker prompt
│               └── report.json  # Persisted worker report
├── context/                     # Cross-mission context files
└── settings.json                # User preferences
```

Mission creation uses a staging directory internally before finalizing mission state. `.maestro/` is runtime data and is typically added to `.gitignore`.

## Plan File Format

Missions are created from JSON plan files:

```json
{
  "title": "Feature Implementation",
  "description": "Add user authentication",
  "milestones": [
    {
      "id": "m1",
      "title": "Database Schema",
      "description": "Create user tables",
      "order": 0
    }
  ],
  "features": [
    {
      "id": "f1",
      "title": "User migration",
      "milestoneId": "m1",
      "description": "Create users table",
      "workerType": "backend-worker",
      "preconditions": "Mission approved and schema review complete",
      "expectedBehavior": "Users table exists with required indexes",
      "verificationSteps": ["Table exists", "Indexes created"],
      "fulfills": ["ASSERT-001"]
    }
  ]
}
```

## Development

### Build

```bash
bun run build
```

### Type Check

```bash
bun run typecheck
```

### Test

```bash
bun test
```

### Deploy

```bash
bun run deploy
```

## Requirements

- Bun runtime
- Git repository for handoff and session-aware workflows
- Optional: CASS for handoff search

## License

MIT
