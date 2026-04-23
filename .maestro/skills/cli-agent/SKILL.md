---
name: cli-agent
description: Implement Mission Control CLI commands, Mission Control snapshot/TUI behavior, and user-visible recovery/operator workflows in Maestro
---

# CLI Agent

NOTE: startup and cleanup are handled by `agent-base`. This skill defines the work procedure for CLI-facing Mission Control reliability features.

## When to Use This Skill

Use for features involving:
- command registration in `src/index.ts`
- Commander subcommand groups and option handling
- `mission-control` JSON/text/TUI behavior
- operator-facing pause/resume/recover/retry/history flows
- compiled binary parity and user-visible output formatting
- PTY or one-frame Mission Control verification

## Required Skills

None.

## Work Procedure

1. Read the feature description, `fulfills` assertions, and the relevant `.factory/library/*.md` files, especially `architecture.md`, `user-testing.md`, and any mission-specific topic file for your area.
2. Sketch the user-visible CLI/TUI contract before editing code: commands, options, JSON fields, one-frame/TUI output changes, and failure hints.
3. Add failing integration or PTY-facing tests first for the intended user-visible behavior. Prefer temp git repos and Bun subprocesses over private helper-only coverage.
4. Implement command or snapshot/TUI changes as thin shells over usecases:
   - keep command handlers thin
   - use `getServices()`
   - route structured output through existing output helpers
   - keep source-run and compiled-binary behavior aligned
5. If Mission Control rendering changes, verify both JSON and operator-visible text/interactive output. Do not rely on JSON-only coverage for UI-facing recovery states.
6. Run narrow test files while iterating, then finish with `bun run typecheck`. If the feature changes user-facing CLI/TUI behavior, also run `bun run build`, `./dist/maestro --version`, and a compiled `./dist/maestro mission-control ...` check.
7. In the handoff, include the exact commands/PTY checks you ran and what operator-visible behavior they proved.

## Example Handoff

```json
{
  "salientSummary": "Updated Mission Control JSON and TUI views to show explicit runtime recovery state and added operator-facing retry history output. Verified both source-run and compiled `./dist/maestro` Mission Control surfaces against the same fixture mission.",
  "whatWasImplemented": "Extended the Mission Control snapshot and rendering layers to consume explicit runtime-state records, surfaced recoverable/stale/failed status in both JSON and one-frame/interactive views, added CLI coverage for operator recovery/history inspection, and kept compiled-binary output aligned with source-run behavior.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "bun test tests/integration/mission-control.test.ts tests/unit/tui",
        "exitCode": 0,
        "observation": "Mission Control JSON/TUI regressions passed, including runtime/recovery rendering expectations."
      },
      {
        "command": "bun run typecheck",
        "exitCode": 0,
        "observation": "Command, snapshot, and TUI changes compile cleanly."
      },
      {
        "command": "bun run build && ./dist/maestro --version && ./dist/maestro mission-control --preview",
        "exitCode": 0,
        "observation": "Compiled binary was rebuilt successfully and rendered the expected read-only Mission Control preview."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Run Mission Control in a PTY fixture mission with stale and recoverable runtime states present",
        "observed": "Interactive output distinguished active work from stale/recoverable work and matched the JSON snapshot for the same mission."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/integration/mission-control.test.ts",
        "cases": [
          {
            "name": "mission-control --json exposes runtime recovery context from explicit runtime records",
            "verifies": "VAL-OPS-001"
          },
          {
            "name": "compiled Mission Control reports the same runtime recovery state as source-run JSON snapshot",
            "verifies": "VAL-CROSS-003"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature needs a new operator workflow or command contract that is not specified in the approved mission
- Existing interactive/PTy helpers are insufficient and a broader testing-strategy change is required
- A cross-CLI or runtime-state decision would alter backend invariants beyond the feature budget
