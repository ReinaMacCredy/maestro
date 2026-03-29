# User Testing

Validation surface, dry-run findings, and concurrency guidance for Mission Control.

**What belongs here:** Runtime validation surfaces, how validators should exercise them, and resource-cost guidance.

---

## Validation Surface

### Primary Surface: CLI commands and generated files

Mission Control is a CLI-only feature. Validators should exercise it through:

1. direct CLI invocations (`bun run src/index.ts ...`) inside temp git repositories
2. filesystem assertions against `.maestro/missions/{id}/...`
3. shipped skill file inspection under `skills/built-in/`

### Required checks

- command success/failure behavior
- JSON output shape and text output readability
- persisted mission/feature/assertion/checkpoint files
- regression coverage for pre-existing handoff/session/note/status/doctor commands

## Validation Readiness Dry Run

- `bun test` passed in the current environment before planning artifacts were written
- `bun run typecheck` passed in the current environment before planning artifacts were written
- Existing integration tests already use temp directories and Bun subprocesses, which matches the planned Mission Control validation surface
- No browser tooling, network services, or auth bootstrap were required for this mission

## Validator Tooling

Use direct shell/Bun execution rather than browser tooling:

```bash
bun run src/index.ts mission create --file plan.json --json
bun run src/index.ts milestone seal m1 --mission <id> --json
```

Prefer fresh temp git repositories for end-to-end flows.

## Validation Concurrency

### Host profile captured during planning

- CPU cores: 10
- RAM: 64 GB
- Memory pressure free: 87%
- Aggregate CPU load during dry run: roughly 27% of total capacity

### Max concurrent validators: 3

Rationale:
- Mission Control validation spawns multiple Bun subprocesses and file-heavy integration tests
- Some existing tests call local tools (`git`, optional `cass`) and can be noisy under high parallelism
- A cap of 3 keeps well within the 70% headroom rule while avoiding unnecessary contention

## Isolation Strategy

- Each validator should use its own temp git repository
- Do not run validation against the working repository
- Do not start background services or bind ports
- Clean up temp directories after each flow

## Critical Flows To Exercise

1. Mission creation from a complete JSON plan
2. Mission/feature/assertion transition validity and failure hints
3. Prompt generation using `.maestro/skills/{workerType}/SKILL.md`
4. Milestone sealing with both passing and blocking assertion states
5. Checkpoint save/load/list semantics
6. Regression coverage for handoff/session/note/status/doctor

## Evidence Expectations

- capture stdout/stderr for every CLI flow
- parse JSON responses instead of string-matching alone when `--json` is used
- inspect resulting files for mission state and checkpoint flows
- preserve failing output that names blocking assertion IDs or valid transitions
