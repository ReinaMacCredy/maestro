# Planning Policy

<!-- maestro-setup:generated:start -->
## When A Plan Is Required

Create or update a plan before implementation when work involves:

- more than one subsystem
- a new feature with unknown implementation details
- schema, API, or public contract changes
- security, data, infrastructure, or deployment risk
- work expected to span multiple sessions

## Plan Location

Approved implementation plans live under `.maestro/missions/`.

## Planning Flow

1. Use `maestro-brainstorm` when the design is unclear.
2. Use `maestro-mission` after design approval.
3. Persist the approved plan to `.maestro/missions/<slug>.md`.
4. Convert plan phases into `maestro task` entries before implementation.
5. Use `maestro-handoff` only for cross-session transfer.

## Required Plan Content

- objective
- scope and non-goals
- affected files or subsystems
- phases and dependency order
- validation strategy
- risks and rollback
- approval gates

## Drift Rule

If implementation shows the plan is wrong, update the plan before continuing.
Do not create a parallel top-level planning system.
<!-- maestro-setup:generated:end -->

## User Notes

Add planning notes here. This section is outside the managed block.
