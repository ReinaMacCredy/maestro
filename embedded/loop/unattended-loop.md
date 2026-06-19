# Unattended loop

WHEN: the human is away or asleep and a backlog should be worked without them.

The away-mode of the work loop: one long-lived session works the card store
until done. This is the orchestration *shape*; the full driver POLICY
(kickoff, failure budget, boundaries, the morning report) lives in maestro-card
`reference/loop.md` and is the authority. Read it before running this.

## Shape

Per unit of work this adds nothing: every card is claimed, worked, and verified
exactly per `work.md`, test-first default included.

1. Start from the store, never from memory: `maestro status`, then
   `maestro ready`.
2. `maestro claim <id>` -> work -> `task complete --summary --claim --proof` ->
   `maestro task verify <id>`.
3. Commit each verified slice locally on the feature branch. Never push.
4. When `maestro ready` is dry, replenish: find accepted features with no tasks
   (`maestro feature list`), `feature prepare <id> --draft`, review, apply with
   `prepare --from`, continue.
5. When nothing is workable or preparable, stop and write the morning report.

## Boundaries (the night's hard stops)

- MAY: `claim`, work, `complete`, `verify`, `prepare`, `note`, `block`, local
  per-slice commits on the feature branch.
- NEVER: `feature accept`, `feature ship`, `archive`, push, tag, publish,
  destructive git. Accept and ship are the awake human's gates.

A feature whose children are all verified is parked, not shipped: confirm with
`feature ship <id> --dry-run`, record SHIP-READY in the report, move on.

## Scheduler variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs ONE iteration, then exits, cold-starting
from the store with `maestro resume`. `claim` guards overlapping firings.
Maestro itself never schedules anything.

## Stop

Backlog dry or failure budget hit (3 consecutive blocks = systemic; end early).
The final message is the morning report. Full policy -> maestro-card
`reference/loop.md`.
