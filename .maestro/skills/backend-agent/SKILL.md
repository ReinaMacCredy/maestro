---
name: backend-agent
description: Implement Mission Control domain logic, runtime-state persistence, recovery orchestration, and cross-CLI adapters in the Maestro CLI
---

# Backend Agent

NOTE: startup and cleanup are handled by `agent-base`. This skill defines the work procedure for backend-heavy Mission Control reliability features.

## When to Use This Skill

Use for features involving:
- runtime-state types, validators, and storage adapters
- session-detection and host-normalization adapters
- recovery/retry orchestration and checkpoint semantics
- filesystem-backed audit/event/history persistence
- service wiring in `src/services.ts`
- domain invariants that must hold across retries, stale detection, and resume flows

## Required Skills

None.

## Work Procedure

1. Read the feature description, `fulfills` assertions, and the relevant `.factory/library/*.md` files, especially `architecture.md`, `runtime-recovery.md`, and `cross-cli.md`.
2. Identify the invariant you are changing before editing code: ownership uniqueness, stale detection, recovery audit preservation, session normalization, checkpoint safety, or similar.
3. Write failing unit tests first for the smallest missing behavior. Add integration coverage when the behavior must be observable through CLI commands.
4. Implement the domain/adapter/usecase change using existing project patterns:
   - Zod schemas + typed validator helpers
   - pure async usecases with ports passed in
   - filesystem adapters using `src/lib/fs.ts`
   - `MaestroError` hints for user-facing failures
5. Preserve backwards compatibility for older missions or missing files when feasible. Missing runtime data should degrade safely, not crash command flows.
6. Run the narrowest relevant tests while iterating, then finish with `bun run typecheck`. If the change affects user-visible command behavior indirectly, ensure there is CLI integration coverage proving it.
7. In the handoff, be explicit about which invariant was added, what persisted files changed, and how recovery/checkpoint/session behavior was verified.

## Example Handoff

```json
{
  "salientSummary": "Added explicit runtime-state persistence plus automatic recovery bookkeeping for stale agent ownership. Session normalization and checkpoint semantics were extended without regressing existing Claude/Codex flows.",
  "whatWasImplemented": "Introduced runtime-state domain types and filesystem storage, expanded session-detection adapters for additional hosts, implemented recovery logic that requeues features with preserved report and retry history, and extended checkpoint save/load so runtime recovery metadata is captured and restored safely.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "bun test tests/unit/domain tests/unit/adapters tests/unit/usecases --grep 'runtime|recover|checkpoint|session'",
        "exitCode": 0,
        "observation": "Runtime-state, session, recovery, and checkpoint invariants passed in unit coverage."
      },
      {
        "command": "bun test tests/integration/checkpoint-resume.test.ts tests/integration/session-sourcepath.test.ts",
        "exitCode": 0,
        "observation": "CLI-observable checkpoint and session behaviors matched the new backend logic."
      },
      {
        "command": "bun run typecheck",
        "exitCode": 0,
        "observation": "Domain, adapter, and usecase changes compile cleanly."
      }
    ],
    "interactiveChecks": []
  },
  "tests": {
    "added": [
      {
        "file": "tests/unit/usecases/recovery-runtime.test.ts",
        "cases": [
          {
            "name": "recoverable agent failure requeues feature with preserved report and appended retry history",
            "verifies": "VAL-RECOVERY-001"
          },
          {
            "name": "checkpoint restore does not reactivate expired runtime ownership as live",
            "verifies": "VAL-CKPT-003"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires a broader change to mission lifecycle semantics than the approved plan covers
- Runtime/recovery invariants conflict with existing persisted-state compatibility in a way that needs product-direction input
- Cross-CLI host support requires external environment behavior that cannot be reproduced or validated locally
