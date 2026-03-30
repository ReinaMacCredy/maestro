# maestro

Cross-agent handoff and mission control CLI.

Maestro is a plan-based orchestration layer for multi-agent software engineering. It provides persistence, state transitions, and validation for mission execution while remaining agnostic to which agents do the planning and execution.

## Overview

Maestro manages two primary concerns:

1. **Cross-agent handoffs** — Pass work between agents with context preservation
2. **Mission Control** — Plan-based execution state for complex multi-milestone projects

## Installation

Build from source:

```bash
bun install
bun run build
```

The build outputs to `dist/maestro` as a standalone executable.

## Commands

### Cross-Agent Handoffs

| Command | Description |
|---------|-------------|
| `maestro init` | Initialize `.maestro/` directory structure |
| `maestro handoff [--session <id>] [--task "..."]` | Create a new handoff for another agent |
| `maestro handoff-pickup [--claim] [--agent <name>]` | Pick up work from an existing handoff |
| `maestro handoff-dig "<query>"` | Search handoff history via CASS |
| `maestro handoff-drop <id>` | Remove a specific handoff |
| `maestro handoff-cleanup` | Delete all handoffs |
| `maestro handoff-report --content "..."` | Report completion on picked-up work |
| `maestro status` | Show current session and repository status |
| `maestro doctor` | Run health checks (git, CASS, config) |
| `maestro session -q` | Get current session ID |
| `maestro note --content "..."` | Create timestamped notes |

### Mission Control

Mission Control adds plan-based orchestration with milestones, features, assertions, and checkpoints.

**Mission lifecycle:**

| Command | Description |
|---------|-------------|
| `maestro mission create --file plan.json` | Create a mission from a plan file |
| `maestro mission list [--status <s>]` | List missions with optional filter |
| `maestro mission show <id>` | Show mission details with progress |
| `maestro mission approve <id>` | Approve a draft mission |
| `maestro mission reject <id>` | Reject a draft mission |
| `maestro mission update <id> --status <s>` | Update mission status |

**Feature management:**

| Command | Description |
|---------|-------------|
| `maestro feature list --mission <id>` | List features for a mission |
| `maestro feature update <id> --mission <m>` | Update feature status or attach report |
| `maestro feature prompt <id> --mission <m>` | Generate worker prompt for a feature |

**Milestone tracking:**

| Command | Description |
|---------|-------------|
| `maestro milestone list --mission <id>` | List milestones with progress |
| `maestro milestone status <id> --mission <m>` | Show detailed milestone status |
| `maestro milestone seal <id> --mission <m>` | Seal milestone (requires terminal assertions) |

**Validation:**

| Command | Description |
|---------|-------------|
| `maestro validate show --mission <id>` | Show assertions for a mission |
| `maestro validate update <id> --mission <m>` | Update assertion status |

**Checkpointing:**

| Command | Description |
|---------|-------------|
| `maestro checkpoint save --mission <id>` | Save timestamped mission state snapshot |
| `maestro checkpoint list --mission <id>` | List checkpoints (newest first) |
| `maestro checkpoint load --mission <id>` | Read latest checkpoint snapshot (metadata only) |

### Global Options

- `--json` — Output as JSON (works at root, group, or leaf level)

## Storage Layout

Maestro stores all runtime state under `.maestro/` in the project root:

```
.maestro/
├── handoffs/              # Pending and completed handoffs
├── missions/
│   └── {missionId}/       # Per-mission state
│       ├── mission.json   # Mission metadata
│       ├── features/
│       │   └── {fid}.json # Per-feature state
│       ├── assertions/
│       │   └── {aid}.json # Per-assertion state
│       ├── checkpoints/
│       │   └── {cid}.json # Timestamped snapshots
│       └── workers/
│           └── {fid}/
│               └── prompt.md  # Generated worker prompts
├── context/               # Cross-mission context files
└── settings.json        # User preferences
```

The `.maestro/` directory is added to `.gitignore` automatically — it contains runtime state only.

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

### Deploy (build + copy to PATH)

```bash
bun run deploy
```

## Requirements

- Bun runtime (latest)
- Git repository (for handoffs and session detection)
- Optional: CASS binary for handoff search

## License

MIT
