# Workflow Context

<!-- maestro-setup:generated:start -->
## Operating Model

- Humans steer intent, priority, risk, and final approval.
- Agents inspect evidence, implement scoped changes, test, document, and surface risks.
- Repository docs are the durable source of truth.

## Default Flow

1. Clarify the task from repo evidence and user intent.
2. Use `maestro-brainstorm` when design is unclear.
3. Use `maestro-mission` when implementation needs an approved plan.
4. Convert approved work into `maestro task` entries before execution when the repo uses Maestro tasks.
5. Implement, verify, update docs, and report residual risks.

## Completion Bar

- Requested acceptance criteria are met.
- Touched surface has relevant verification.
- Durable decisions are reflected in context docs.
- Remaining risks or follow-ups are explicit.
<!-- maestro-setup:generated:end -->

## User Notes

Add workflow notes here. This section is outside the managed block.
