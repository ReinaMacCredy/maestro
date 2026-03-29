# Architecture

High-level architecture for Mission Control in Maestro.

**What belongs here:** Component relationships, data flow, storage boundaries, and invariants workers need before editing code.

---

## System Overview

Mission Control extends the existing handoff-oriented CLI with mission planning and execution state. The CLI remains the only executable surface. It persists and validates state; the human/AI orchestrator remains responsible for planning, spawning workers, and making decisions.

## Core Architectural Rule

**Maestro is a state machine and persistence layer, not an agent runtime.**

That means:
- Maestro stores mission plans, feature state, assertion state, checkpoints, prompts, and reports
- Maestro validates transitions and referential integrity
- Maestro can generate worker prompts from stored mission data and skill files
- Maestro does **not** spawn workers, pick strategies, manage git branches, or make autonomous decisions

## Main Components

### CLI command groups

- `mission`: create/show/list/approve/reject/update mission lifecycle
- `feature`: list/update/prompt feature lifecycle and worker prompt generation
- `milestone`: list/status/seal milestone progress
- `validate`: show/update assertion state
- `checkpoint`: save/load/list execution snapshots

### Usecase layer

Pure async functions coordinate domain validation and persistence through ports. Commands should stay thin and convert domain results into CLI output.

### Domain layer

Mission-specific domain files define:
- status unions and input types
- Zod validators
- ID generation
- state-machine transition guards
- helpful mission-specific errors

### Ports and adapters

Filesystem-backed ports persist the mission graph:
- mission metadata in `mission.json`
- one file per feature in `features/{featureId}.json`
- assertion array in `assertions.json`
- timestamped snapshots in `checkpoints/`
- worker prompt/report artifacts in `workers/{featureId}/`

## Data Flow

1. A complete mission plan enters through `maestro mission create`
2. The create usecase validates cross-references and dependency cycles
3. Filesystem adapters persist mission metadata plus per-feature and assertion files
4. Feature and assertion commands move entities through validated state transitions
5. Prompt generation reads stored mission state plus `.maestro/skills/{workerType}/SKILL.md`
6. Checkpoints snapshot metadata so execution can resume later

## Storage Boundaries

### Product runtime state

Mission Control runtime state must live under:

```text
.maestro/missions/{missionId}/
```

Expected structure:

```text
.maestro/missions/{missionId}/
  mission.json
  features/
    {featureId}.json
  assertions.json
  checkpoints/
    {timestamp}.json
  workers/
    {featureId}/
      prompt.md
      report.json
```

### Skill locations

- Built-in shipped skills live under `skills/built-in/`
- Per-project worker skills used by prompt generation live under `.maestro/skills/{workerType}/SKILL.md`

### Non-goal boundary

Do not put product runtime mission state under `.factory/`. In this repo, `.factory/` exists only to guide mission workers and validators.

## Invariants

1. Mission IDs follow the existing `YYYY-MM-DD-NNN` date-sequential format
2. Feature, milestone, and assertion IDs are user-supplied and must remain unique within a mission
3. All cross-references are validated before persistence
4. Each feature file is updated independently to avoid concurrent shared-file corruption
5. Milestones can seal only when their assertions are terminal (`passed` or `waived`)
6. `waived` assertions require a stored reason and are terminal
7. Generated worker prompts must remain structurally stable even when stored user content contains prompt-like text

## Data Flow

### Mission Creation Flow
1. User runs `maestro mission create --name X --objective Y`
2. CLI parses arguments and calls `createMission()` use case
3. Use case generates ID, validates input
4. MissionStoreAdapter writes to `.maestro/missions/{id}/mission.yaml`
5. CLI outputs mission ID

### Feature Addition Flow
1. User runs `maestro feature add --description "Do X" --skill worker`
2. CLI validates description and skill exist
3. FeatureStoreAdapter appends to `features.json`
4. Feature gets sequential ID within mission

### Execution Flow
1. User runs `maestro run`
2. System loads mission and features
3. Scheduler builds dependency graph
4. For each ready feature:
   - Select agent based on capabilities
   - Create handoff with context
   - Spawn agent process
   - Wait for completion report
5. At milestone end: inject scrutiny validator
6. After scrutiny: inject user testing validator
7. Milestone sealed when all assertions pass

## Key Abstractions

### State Machines

**Task State:**
```
pending → assigned → in-progress → review → done
   ↑                                      |
   └────────── retry (on failure) ────────┘
```

**Feature State:**
```
pending → in-progress → completed
   ↑            |
   └──── retry ─┘
```

**Mission State:**
```
planning → running → validating → completed
              ↓         ↓
           paused   failed
```

### Validation Flow

```
Milestone Complete
       ↓
┌──────────────────┐
│ Scrutiny Inject  │ (auto)
│ Review Features  │
│ Synthesize       │
└────────┬─────────┘
       ↓ (pass)
┌──────────────────┐
│ User Test Inject │ (auto)
│ Validate Asserts │
│ Update State     │
└────────┬─────────┘
       ↓ (pass)
   Milestone Sealed
```

## Storage Schema

### Mission Directory Structure
```
.maestro/missions/{mission-id}/
├── mission.yaml              # Mission metadata
├── features.json             # Feature list
├── validation-contract.md    # Behavioral assertions
├── validation-state.json     # Assertion statuses
├── events.yaml               # Timeline events
├── notes.yaml                # Mission notes
├── checkpoints/              # Execution checkpoints
│   ├── checkpoint-{timestamp}.yaml
│   └── ...
└── workers/                  # Worker assignments
    └── {worker-id}/
        ├── worker.yaml
        └── handoffs/
```

### File Formats

**mission.yaml:**
```yaml
id: "2026-03-29-abc123"
name: "Refactor Auth System"
objective: "Modernize authentication"
status: "running"
createdAt: "2026-03-29T10:00:00Z"
updatedAt: "2026-03-29T14:30:00Z"
description: "Full auth refactor..."
priority: "high"
```

**features.json:**
```json
{
  "features": [
    {
      "id": "2026-03-29-abc123-F001",
      "description": "Implement JWT validation",
      "skillName": "backend-worker",
      "status": "completed",
      "fulfills": ["VAL-M2-FEATURE-001"]
    }
  ]
}
```

**agents.yaml:** (global)
```yaml
agents:
  - slug: "claude-code"
    displayName: "Claude Code"
    configDir: ".claude"
    configFile: "CLAUDE.md"
    capabilities: ["typescript", "design"]
    fallback: ["codex"]
```

## Invariants

1. **ID Uniqueness** - All IDs are globally unique within their scope
2. **State Validity** - All state transitions are validated before application
3. **Storage Atomicity** - Critical writes are atomic (write temp + rename)
4. **Validation Completeness** - Milestones only seal when all assertions pass
5. **Agent Attribution** - All work attributed to executing agent
6. **Context Preservation** - Handoffs preserve full execution context

## Extension Points

### Adding New Agent Types
1. Add agent spec to `SUPPORTED_AGENTS` in domain
2. Add agent to `agents.yaml` if custom
3. Create agent-specific prompt template
4. Test handoff pickup flow

### Adding New Commands
1. Create command file in `src/commands/`
2. Register in `src/index.ts`
3. Add integration tests
4. Add to validation contract

### Adding New Storage Backends
1. Implement port interface (e.g., `MissionStorePort`)
2. Create adapter (e.g., `DatabaseMissionStoreAdapter`)
3. Update services.ts to inject new adapter
4. Add adapter-specific tests
