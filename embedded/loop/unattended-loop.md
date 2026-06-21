# Unattended loop

>> ENTER UNATTENDED MODE NOW.
>> Keep going on what you're already doing -- carry the current work forward
>> and loop it autonomously until it's done: claim -> work -> verify -> commit
>> locally. Stay grounded in the maestro card store, not chat memory: the
>> feature and cards you're on now, then `maestro card ready` and the features
>> you've ACCEPTED (`feature prepare` their tasks).
>> NEVER push, accept, or close (those are the human's gates), and never run
>> destructive git. End with the report.

WHEN: human away/asleep -- keep going on the current work and loop the card store (ready + accepted features you prepare) unattended until done.

The away-mode of the work loop: one long-lived session works the card store
until done. This is the orchestration *shape*; the full driver POLICY
(kickoff, failure budget, boundaries, the report) lives in maestro-card
`reference/loop.md` and is the authority. Read it before running this.

## Shape

Per unit of work this adds nothing: every card is claimed, worked, and verified
exactly per `work.md`, test-first default included.

1. Start from the store, never from memory: `maestro status`, then
   `maestro card ready`.
2. `maestro card claim <id>` -> work -> `task complete --summary --claim --proof` ->
   `maestro task verify <id>`.
3. Commit each verified slice locally on the feature branch. Never push.
4. When `maestro card ready` is dry, replenish: find accepted features with no tasks
   (`maestro feature list`), `feature prepare <id> --draft`, review, apply with
   `prepare --from`, continue.
5. When nothing is workable or preparable, stop and write the report.

## Boundaries (the night's hard stops)

- MAY: `claim`, work, `complete`, `verify`, `prepare`, `note`, `block`, local
  per-slice commits on the feature branch.
- NEVER: `feature accept`, `feature close`, `archive`, push, tag, publish,
  destructive git. Accept and close are the awake human's gates.

A feature whose children are all verified is parked, not closed: confirm with
`feature close <id> --dry-run`, record CLOSE-READY in the report, move on.

## Scheduler variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs ONE iteration, then exits, cold-starting
from the store with `maestro resume`. `claim` guards overlapping firings.
Maestro itself never schedules anything.

## Stop

Backlog dry or failure budget hit (3 consecutive blocks = systemic; end early).
The final message is the report. Full policy -> maestro-card
`reference/loop.md`.
