# User Testing

Validation surface, readiness notes, and concurrency guidance for the Mission Control reliability and cross-CLI orchestration mission.

**What belongs here:** Runtime validation surfaces, how validators should exercise them, and resource/cost guidance.

---

## Validation Surface

### Primary Surface: CLI commands and persisted mission state

Mission Control remains a CLI/TUI feature. Validators should exercise it through:

1. direct CLI invocations (`bun run src/index.ts ...`) inside temp git repositories
2. filesystem assertions against `.maestro/missions/{id}/...`
3. compiled binary checks using `./dist/maestro`
4. PTY/one-frame checks for Mission Control interactive behavior when runtime/recovery state must be rendered

### Required Checks

- command success/failure behavior
- JSON output shape and text output readability
- persisted runtime-state, retry-history, checkpoint, and event-log files
- regression coverage for existing session/handoff/Mission Control flows
- compiled binary parity for user-facing Mission Control output

## Validation Readiness

- Repo structure and CLI/TUI test surface are present.
- Core commands remain `bun test`, `bun run typecheck`, and `bun run build`.
- PTY/interactive Mission Control tests already exist in the repo and should be reused/extended.
- No browser tooling, auth setup, or external services are required for this mission.
- Planning encountered unstable custom Task-based droid delegation; agents and validators should treat agent-launch failures as environment blockers and return to the orchestrator instead of baking workarounds into product behavior.

## Validator Tooling

Prefer shell/Bun execution and PTY helpers:

```bash
bun run src/index.ts session --json
bun run src/index.ts mission-control --json
bun run src/index.ts checkpoint save --mission <id> --json
bun run build
./dist/maestro mission-control --json
```

Use `tuistory` only when a true interactive terminal capture is needed; otherwise shell + persisted file inspection is enough.

## Validation Concurrency

### Host Profile

- CPU cores: 10 logical
- RAM: 64 GB
- Mission surface is CLI-heavy with Bun subprocesses and PTY helpers

### Max Concurrent Validators: 3

Rationale:
- CLI/TUI validation can spawn multiple Bun subprocesses and PTY sessions
- PTY-based Mission Control checks are heavier than simple JSON command assertions
- A cap of 3 stays comfortably within the 70% headroom rule while leaving room for build/typecheck activity

## Isolation Strategy

- Each validator should use its own temp git repository and mission fixtures.
- Do not validate against the working repo.
- Do not start new external services or bind unrelated ports.
- Clean up temp directories after each flow.

## Critical Flows To Exercise

1. Runtime-state persistence for active/stale/failed/recoverable work
2. Session detection for Droid plus existing Claude/Codex regression coverage
3. Event-log normalization and graceful missing-hook behavior
4. Automatic recovery with preserved report and retry history
5. Mission Control JSON and interactive rendering of runtime/recovery state
6. Checkpoint save/load with runtime recovery metadata
7. Compiled `./dist/maestro` parity with source-run Mission Control state

## Evidence Expectations

- capture stdout/stderr for every CLI flow
- parse JSON responses when `--json` is used
- inspect persisted `.maestro/missions/{id}/...` files for runtime/recovery/checkpoint flows
- capture one-frame or PTY output for Mission Control rendering changes
- preserve build output and compiled binary output for compiled-surface assertions
