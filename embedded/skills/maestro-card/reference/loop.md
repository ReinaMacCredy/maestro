# Unattended Loop

The overnight/away mode of the work loop: one long-lived session works the
card store until done while the human sleeps. Per unit of work this file adds
nothing — every card is claimed, worked, and verified exactly per
[work.md](work.md), including its test-first default. This file adds only the
unattended policy: replenishment, stops, boundaries, and the morning report.

## Kickoff

The human says some form of "work the backlog while I sleep" and leaves. No
stop time or unit cap is required, ever. If the kickoff prompt states one
("until 07:00", "max 5 cards"), honor it; never ask for one.

Start from the store, not from memory: `maestro status`, then `maestro ready`.

## Loop

1. `maestro ready` -> `maestro claim <id>` -> work the card per
   [work.md](work.md). The test-first rule applies unchanged; a TDD skip
   needs its usual one-line noted reason and is surfaced in the morning
   report.
2. Finish with `task complete --summary --claim --proof`, then
   `maestro task verify <id>`.
3. Commit each verified slice locally on the feature branch. Never push.
4. When `maestro ready` is dry, replenish: find accepted features that lack
   tasks (`maestro feature list`, state `ready`, tasks 0), run
   `feature prepare <id> --draft`, review the draft, apply with
   `prepare --from`, and continue the loop.
5. When nothing is workable or preparable, stop and write the morning report.

A feature whose children are all verified is parked, not shipped: confirm
with `maestro feature ship <id> --dry-run`, record SHIP-READY in the report,
and move on.

## Failure Budget

- A card that fails 3 attempts is not ground down:
  `maestro task block <id> --reason "<what failed, 3 attempts>"`, then take
  the next card.
- 3 consecutive blocked cards means the problem is systemic (broken build,
  bad assumption), not the cards. End the night early and say so in the
  report.

## Boundaries

Night MAY: `claim`, work, `complete`, `verify`, `prepare`, `note`, `block`,
local per-step commits on the feature branch.

Night NEVER: `feature accept`, `feature ship`, `archive`, push, tag,
publish, destructive git operations, hand-editing `card.yaml` or guarded
sidecars. `accept` and `ship` are the human's gates: contracts are frozen
and declared delivered only while someone is awake.

## Morning Report

The session's final message is the report the human reads over coffee:

- per unit: outcome plus TDD evidence (`verified, tdd: 3 cycles` or
  `verified, tdd: skipped - <reason>`)
- blocked cards with reasons
- SHIP-READY features with the exact command to run
  (`maestro feature ship <id> --outcome "<one line>"`)
- features prepared overnight (replenishment)
- why the loop stopped (dry, cap reached, or failure budget)

The report is a summary, not the record: notes, proof, and events on the
cards carry the durable evidence.

## Scheduler Variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs ONE iteration of the loop above, then
exits. The card store is the only state between firings — cold-start with
`maestro resume` — and `claim` already guards overlapping firings against
double work. Maestro itself never schedules anything.

## Stop

- Do not ask the sleeping human questions; block the card and move on.
- Do not invent caps the kickoff prompt did not state.
- Do not keep working past 3 consecutive blocks.
- Do not let the night's results live only in conversation; every outcome
  lands on a card through the verbs.

## Hand-off

Morning, human: review the report, `feature ship` anything SHIP-READY,
unblock or reassign blocked cards, accept the next contracts. Per-unit method
-> [work.md](work.md); proof -> [verify.md](verify.md).
