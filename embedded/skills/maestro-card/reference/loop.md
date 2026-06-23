# Unattended Loop

>> ENTER UNATTENDED MODE NOW.
>> Keep going on what you're already doing -- carry the current work forward
>> and loop it autonomously until it's done: claim -> work -> verify -> commit
>> locally. Stay grounded in the maestro card store, not chat memory: the
>> feature and cards you're on now, then `maestro card ready` and the features
>> you've ACCEPTED (`feature prepare` their tasks).
>> NEVER push, accept, or close (those are the human's gates), and never run
>> destructive git. End with the report.

The away mode of the work loop: one long-lived session carries the current
work forward and works the card store until done while the human is away. Per
unit of work this file adds nothing — every card is claimed, worked, and
verified exactly per [work.md](work.md), including its test-first default. This
file adds only the unattended policy: replenishment, stops, boundaries, and the
report.

## Kickoff

The human says some form of "keep working / work the backlog while I'm away"
and leaves. Carry forward the work they were on — continue the current feature
and its cards, do not abandon them for a fresh queue. A feature just
brainstormed is taken end-to-end only after the human's one `feature accept`
(the gate); then `feature prepare` mints its tasks and the loop works them. No
stop time or unit cap is required, ever; if the prompt states one ("until
07:00", "max 5 cards"), honor it, never ask for one.

Start from the store, never from memory (the session can die; the store is the
only durable state): `maestro status`, then `maestro card ready`.

If the kickoff is a broad goal instead of a named card or accepted feature,
infer a minimal GoalBrief before work starts:

```text
GoalBrief:
  outcome
  stop_condition
  constraints
```

The GoalBrief is transient. It must compile immediately into existing Maestro
records:

- `outcome` -> proposed feature description/request, or the current accepted
  feature if the goal clearly continues already-accepted work
- `stop_condition` -> acceptance/check candidates and proof expectations
- `constraints` -> non-goals, boundaries, or decision context

Do not create a goal file, goal command, hidden planner state, schema, MCP
tool, daemon, scheduler, or separate goal lifecycle. Existing feature, task,
decision, proof, and QA gates remain the only durable contract.

For a new broad goal, draft or update a proposed feature and stop at the human
`feature accept` gate before `prepare` or implementation work. For a goal that
is already backed by accepted/current work, continue into the card loop below.
For an ambiguous broad goal, draft with explicit assumptions: record what you
inferred in the feature/spec, turn material uncertainty into questions or
decision forks, and ask first only when even a proposed feature would be
materially misleading or permission-sensitive.

Codex can run the resulting card loop directly. Claude Code should author a
Workflow script that performs the same store-grounded sequence. Both agents use
the same records and stop conditions.

## Loop

1. `maestro card ready` -> `maestro card claim <id>` -> work the card per
   [work.md](work.md). The test-first rule applies unchanged: an observable
   `--check` is worked test-first; a skip is valid only for a non-behavioral
   check or an explore/spike lane, and the skip note names which. Skips are
   surfaced in the report.
2. Finish with `task complete --summary --claim --proof`, then
   `maestro task verify <id>`.
3. Commit each verified slice locally on the feature branch. Never push.
4. When `maestro card ready` is dry, replenish: find accepted features that lack
   tasks (`maestro feature list`, state `ready`, tasks 0), run
   `feature prepare <id> --draft`, review the draft, apply with
   `prepare --from`, and continue the loop.
5. When nothing is workable or preparable, stop and write the report.

A feature whose children are all verified is parked, not closed: confirm
with `maestro feature close <id> --dry-run`, record CLOSE-READY in the report,
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

Night NEVER: `feature accept`, `feature close`, `archive`, push, tag,
publish, destructive git operations, hand-editing `card.yaml` or guarded
sidecars. `accept` and `close` are the human's gates: contracts are frozen
and declared delivered only while someone is awake.

## Report

The session's final message is the report the human reads when they return:

- per unit: outcome plus TDD evidence (`verified, tdd: 3 cycles` or
  `verified, tdd: skipped - <reason>`) and the simplify outcome
  (`simplified` or `simplify: skipped - <non-code reason>`)
- blocked cards with reasons
- CLOSE-READY features with the exact command to run
  (`maestro feature close <id> --outcome "<one line>"`)
- features prepared overnight (replenishment)
- why the loop stopped (dry, cap reached, or failure budget)

The report is a summary, not the record: notes, proof, and events on the
cards carry the durable evidence.

## Scheduler Variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs ONE iteration of the loop above, then
exits. The card store is the only state between firings — cold-start with
`maestro resume` — and `claim` already guards overlapping firings against
double work. A firing that dies mid-card leaves its claim behind; the next
firing reclaims it once the claim crosses the existing 15-min stale TTL — the
same timeout that frees any abandoned claim, not a new mechanism. Rebuild the
night's account from durable state with `maestro query run` (its `--json`
carries the per-card trace and an honest interruption verdict); never
reconstruct the report from a dead firing's memory. Maestro itself
never schedules anything.

## Stop

- Do not ask the sleeping human questions; block the card and move on.
- Do not invent caps the kickoff prompt did not state.
- Do not keep working past 3 consecutive blocks.
- Do not let the night's results live only in conversation; every outcome
  lands on a card through the verbs.

## Hand-off

On return, human: review the report, `feature close` anything CLOSE-READY,
unblock or reassign blocked cards, accept the next contracts. Per-unit method
-> [work.md](work.md); proof -> [verify.md](verify.md).
