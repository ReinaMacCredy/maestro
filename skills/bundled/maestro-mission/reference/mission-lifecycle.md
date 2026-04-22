# Mission Lifecycle

A mission passes through a small number of named states as it is planned, approved, executed, and closed out.

## States

- `draft`: mission scaffold exists; milestones and features may be incomplete; not ready for execution.
- `approved`: design has been signed off; features are scoped; ready to begin.
- `rejected`: the mission proposal was explicitly rejected.
- `executing`: at least one milestone or feature is actively being worked on.
- `paused`: execution was paused; agents should not pick up new features until resumed.
- `validating`: execution is complete and final assertions are being checked.
- `completed`: all milestones closed, final assertions passed or explicitly waived.
- `failed`: terminal failure; the mission closed without completing its goal.

## Transitions

Lifecycle transitions happen through `maestro mission` verbs or through upstream actions (e.g., marking the last assertion result as `passed` can auto-transition a mission from `validating` to `completed`).

Inspect current state:

```bash
maestro mission show <missionId>
```

## Milestones

A milestone groups related features and can act as a validation gate. Milestones carry:
- `kind`: `work` or `gate`
- `profile`: the agent-style profile (`planning`, `review`, `impl`, etc.) when the milestone is executed by agents
- Dependencies on other milestones

Inspect milestones inside a mission:

```bash
maestro mission show <missionId>
maestro feature list --mission <missionId> --json
```

## Features

A feature is a concrete piece of work inside a milestone. It carries:
- Agent type (who should execute it)
- Verification steps (assertions)
- Optional dependencies on other features

Inspect features:

```bash
maestro feature list --mission <missionId> --json
maestro feature prompt <featureId> --mission <missionId>
```

## Checkpoints

Checkpoints persist a snapshot of mission state at a particular boundary (end of milestone, before risky change, etc.) so later sessions can rewind or compare. Checkpoints are read-only artifacts under `.maestro/missions/<missionId>/checkpoints/`.

## Mission artifacts on disk

`.maestro/missions/<missionId>/` contains:
- Mission metadata (`mission.json`)
- Milestone and feature definitions
- Feature prompts rendered with injected memory
- Assertion results
- Checkpoint snapshots
- Handoff launches originating from the mission

These artifacts are repo-tracked and reviewable.
