---
name: backend-worker
description: Implement Mission Control domain logic, storage adapters, and usecases in the Maestro CLI
---

# Backend Worker

NOTE: startup and cleanup are handled by `worker-base`. This skill defines the work procedure for backend-heavy Mission Control features.

## When to Use This Skill

Use for features involving:
- domain type definitions and validators
- state-machine transition logic
- filesystem storage adapters and ports
- service wiring in `src/services.ts`
- usecases whose main complexity is persistence or invariants
- test fixtures and mocks for Mission Control data

## Required Skills

None.

## Work Procedure

1. Read the feature description, `fulfills` assertions, and `.factory/library/*.md` files that affect your area.
2. Identify the exact domain invariants or storage behaviors that must change.
3. Write failing unit tests first for the smallest missing behavior. Cover both happy-path and invalid-transition / invalid-reference cases.
4. Implement the domain or adapter change using existing project patterns:
   - Zod schemas + `validateX()` wrappers
   - pure async usecases with ports passed in
   - filesystem adapters using `src/lib/fs.ts`
   - `MaestroError` hints for user-facing failures
5. If the feature touches command behavior indirectly, add or update integration tests proving the backend behavior is observable through the CLI.
6. Run the narrowest relevant test files while iterating, then finish with the feature's listed verification steps.
7. In the handoff, be explicit about which files changed, which invariants were added, and exactly how the behavior was verified.

## Example Handoff

```json
{
  "salientSummary": "Added Mission Control transition guards and filesystem stores for missions, features, assertions, and checkpoints. Verified adapter behavior with temp-directory tests and confirmed the new domain types compile cleanly.",
  "whatWasImplemented": "Implemented `src/domain/mission-state.ts`, `src/domain/mission-validators.ts`, new store ports, and filesystem adapters for mission state under `.maestro/missions/{id}`. Added create/read/update/list coverage for mission, feature, assertion, and checkpoint storage plus referential-integrity and cyclic-dependency validation.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "bun test tests/unit/domain/mission-state.test.ts tests/unit/domain/mission-validators.test.ts",
        "exitCode": 0,
        "observation": "Domain transition and validation coverage passed, including invalid transitions, dangling references, and waived assertions."
      },
      {
        "command": "bun test tests/unit/adapters/mission-store.adapter.test.ts tests/unit/adapters/feature-store.adapter.test.ts tests/unit/adapters/assertion-store.adapter.test.ts tests/unit/adapters/checkpoint-store.adapter.test.ts",
        "exitCode": 0,
        "observation": "Filesystem adapter tests passed in temp directories, including per-feature file layout and checkpoint sorting."
      },
      {
        "command": "bun run typecheck",
        "exitCode": 0,
        "observation": "Mission Control types and service wiring compile without TypeScript errors."
      }
    ],
    "interactiveChecks": []
  },
  "tests": {
    "added": [
      {
        "file": "tests/unit/domain/mission-state.test.ts",
        "cases": [
          {
            "name": "assertMissionTransition rejects invalid mission updates with valid-next-state hints",
            "verifies": "VAL-MISSION-003"
          },
          {
            "name": "assertAssertionTransition allows failed or blocked assertions to return to pending",
            "verifies": "VAL-VALIDATION-002"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires a new CLI contract or output shape that is not specified
- A required cross-reference or storage invariant conflicts with the approved plan
- Existing repository deletions or test failures suggest broader migration work than the feature budget allows
