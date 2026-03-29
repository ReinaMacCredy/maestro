---
name: cli-worker
description: Implement Mission Control CLI commands, prompt generation, shipped skills, and user-visible command behavior in Maestro
---

# CLI Worker

NOTE: startup and cleanup are handled by `worker-base`. This skill defines the work procedure for CLI-facing Mission Control features.

## When to Use This Skill

Use for features involving:
- command registration in `src/index.ts`
- Commander subcommand groups and option handling
- human-readable and JSON output formatting
- worker prompt generation
- progress/reporting views
- shipped skill markdown content under `skills/built-in/`
- CLI integration tests and temp-repo smoke checks

## Required Skills

None.

## Work Procedure

1. Read the feature description, affected assertions, and `.factory/library/implementation-patterns.md`.
2. Sketch the CLI contract before editing code: command path, arguments/options, JSON behavior, and failure cases.
3. Add failing integration tests first for the user-visible behavior. Use temp git repos and Bun subprocesses rather than testing through private helpers alone.
4. Implement command handlers as thin shells over usecases:
   - register under the correct command group
   - use `getServices()`
   - route output through `output(...)`
   - keep error behavior consistent with existing commands
5. If the feature involves prompt generation or shipped skill files, verify the actual written markdown content, not just return values.
6. Run targeted integration/unit tests while iterating, then finish with the feature's verification steps. When CLI wiring changes substantially, include `bun run build`.
7. In the handoff, include the exact CLI commands you ran and what their output proved.

## Example Handoff

```json
{
  "salientSummary": "Implemented the Mission Control command group for mission lifecycle plus prompt JSON inheritance. Added CLI integration coverage for create/show/approve/update flows and verified generated prompts write to worker artifact paths.",
  "whatWasImplemented": "Added `mission`, `feature`, and `milestone` command registration in `src/index.ts`, implemented command handlers that call Mission Control usecases, introduced a `resolveJsonFlag()` helper for root/group/leaf `--json` behavior, and added integration tests using temp git repositories. Also updated built-in Mission Control skill files to match the shipped command syntax.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "bun test tests/integration/mission-commands.test.ts tests/integration/feature-commands.test.ts",
        "exitCode": 0,
        "observation": "Mission and feature CLI roundtrips passed in temp git repositories, including JSON output and invalid-transition failures."
      },
      {
        "command": "bun run src/index.ts mission create --file tmp/plan.json --json",
        "exitCode": 0,
        "observation": "CLI emitted parseable JSON with a generated mission ID and created `.maestro/missions/{id}` runtime state."
      },
      {
        "command": "bun run typecheck",
        "exitCode": 0,
        "observation": "Command registrations and output helpers compile cleanly."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Create a temp git repo, run `bun run src/index.ts feature prompt f1 --mission <id> --out /tmp/prompt.md`, and inspect the written prompt",
        "observed": "Prompt contained mission context, feature verification steps, and the worker skill body, and it was written both to `/tmp/prompt.md` and `.maestro/missions/{id}/workers/f1/prompt.md`."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/integration/mission-commands.test.ts",
        "cases": [
          {
            "name": "mission create initializes runtime state under .maestro/missions with generated ID",
            "verifies": "VAL-MISSION-001"
          },
          {
            "name": "maestro --json mission create emits parseable JSON",
            "verifies": "VAL-CROSS-002"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature needs a new lifecycle behavior that is not covered by the approved plan
- Existing deleted skill trees or legacy output contracts appear to require a broader migration decision
- A CLI-facing regression exists in pre-existing handoff/session flows that is unrelated to Mission Control scope
