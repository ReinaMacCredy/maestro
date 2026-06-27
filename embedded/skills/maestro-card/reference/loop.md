# Unattended Loop

>> ENTER UNATTENDED MODE NOW.
>> Keep going on what you're already doing -- carry the current work forward
>> under full local autonomy: accept proposed contracts, prepare tasks, unblock
>> local Maestro blockers, claim -> work -> verify -> commit locally, and close
>> locally verified features. Stay grounded in the maestro card store, not chat
>> memory: the feature and cards you're on now, then `maestro card ready`, then
>> proposed/ready features you can accept or prepare.
>> HARD STOP: any push/tag/publish/release/archive action not granted by an
>> explicit bounded run-scoped ship authority, destructive git, secret rotation,
>> or a platform/tool approval failure. End with the autonomy report.

The away mode of the work loop: one long-lived session carries the current
work forward and works the card store until done while the human is away. Per
unit of work this file adds nothing - every card is claimed, worked, and
verified exactly per [work.md](work.md), including its test-first default. This
file is the full unattended-loop policy authority: authorization, local
autonomy, replenishment, stops, audit ledger, boundaries, and the report.

## Kickoff

The human says some form of "use loop", "keep looping", "I am going away",
"I am going to sleep", "work while I am away", or "work the backlog while I'm
away" and leaves. Carry forward the work they were on - continue the current
feature and its cards, do not abandon them for a fresh queue. In
full-autonomy mode the away prompt is the local authorization to finalize
clean handoffs, accept proposed contracts, prepare tasks, unblock local
Maestro blockers, add dependencies when covered by the accepted contract or
blocker authority, work, verify, commit locally, and close locally verified
features without another human response. No stop time or unit cap is required,
ever; if the prompt states one ("until 07:00", "max 5 cards"), honor it,
never ask for one.

Start from the store, never from memory (the session can die; the store is the
only durable state): `maestro status`, then `maestro loop work-lease --json`.
`work-lease` is one sidecar tick: it selects one ready card in the requested
scope, claims it through the normal card claim policy, emits the existing
work-touch run evidence, and prints the bounded worker contract. It never
launches a worker, sleeps, polls, owns a queue, or schedules the next tick.
Long-lived agents may call it before each unit; an external sidecar may call it
once per cron/launchd/cloud firing.

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

For a new broad goal, draft or update a proposed feature, run
`feature finalize` to write the clean handoff, then accept it locally when the
contract is explicit enough for Maestro's normal `feature accept` gate to pass.
For a goal already backed by proposed, ready, or current work, continue into the
card loop below. For an ambiguous broad goal, draft with explicit assumptions:
record what you inferred in the feature/spec, turn material uncertainty into
questions or decision forks, and hard stop only when even a proposed feature
would be materially misleading, permission-sensitive, or outside the hard-stop
boundary.

Codex can run the resulting card loop directly. Claude Code should author a
Workflow script that performs the same store-grounded sequence. Both agents use
the same records and stop conditions.

## Loop

1. Record `autonomy_start` before crossing local gates. It carries
   `authority_ref`, `authority_summary`, `prompt_hash`, and `hard_stops`.
   Authority is run-scoped: it ends on dry stop, hard stop, terminal report, or
   a stale/abandoned run. It is not a repo mode, card mode, config flag, daemon,
   or scheduler.
2. `maestro loop work-lease --json` -> read the returned `selected_card` and
   `worker_prompt` -> work the card per [work.md](work.md). The returned JSON
   includes the card id, claim identity, stale-claim policy, allowed follow-up
   verbs, hard stops, recurrence-guard requirement, inspect/reconcile handles,
   run-event path, and ship authority status. The test-first rule applies
   unchanged: an observable `--check` is worked test-first; a skip is valid
   only for a non-behavioral check or an explore/spike lane, and the skip note
   names which. Skips are surfaced in the report.
3. Finish with `task complete --summary --claim --proof`, then
   `maestro task verify <id>`.
4. Commit each verified slice locally on the feature branch. Never push.
5. When a local lifecycle gate is crossed because of away authorization, record
   an `autonomy_action` before or immediately after the command. Include
   `action`, `target_kind`, `target_id`, `authority_ref`, compact
   `before_state`, normalized/redacted `command`, `result`, and compact
   `after_state`. Do not store full card snapshots.
6. When `maestro card ready` is dry, replenish: find proposed or ready features
   that can proceed locally. For proposed features, satisfy the normal
   QA-baseline and accept gate; for ready features with no tasks, run
   `feature prepare <id> --draft`, review the draft, apply with
   `prepare --from`, and continue the loop.
7. A feature whose children are all verified may be closed locally when the
   normal QA-slice and `feature close` gates pass. Record the local close as an
   `autonomy_action`. This is local delivery only: no push, tag, release,
   publish, archive, or external announcement unless the original prompt or
   accepted card contract granted explicit run-scoped ship authority naming
   scope, target, allowed external actions, hard stops, and required evidence.
   Absent, partial, stale, or overbroad authority fails closed.
8. When nothing is workable, acceptable, preparable, unblockable, or closable
   inside the hard-stop boundary, stop and write the report.

## Failure Budget

- A card that fails 3 attempts is not ground down:
  `maestro task block <id> --reason "<what failed, 3 attempts>"`, then take
  the next card.
- 3 consecutive blocked cards means the problem is systemic (broken build,
  bad assumption), not the cards. End the night early and say so in the
  report.

## Boundaries

Night MAY: `feature finalize`, `feature accept`, `feature prepare`,
`task unblock` for local Maestro blockers, dependency additions covered by the
accepted contract or blocker authority, `claim`, work, `complete`, `verify`,
`note`, `block`, local per-step commits on the feature branch, QA-slice, and
`feature close` for locally verified features.

Night NEVER without explicit bounded run-scoped ship authority: push, tag,
release, publish, archive, or any external ship action. Night NEVER even with
ship authority: destructive git operations, secret rotation, bypassing a
platform/tool approval failure, or hand-editing `card.yaml` or guarded
sidecars. Platform/tool approval failures are hard stops even under full local
autonomy.

If the autonomous worker fixes an issue discovered during the loop, it must
record a durable recurrence guard before completion or ship: a regression
test, proof gate, QA checklist entry, harness friction rule, skill guidance
update, or locked decision. The final report names the guard evidence.

Autonomy evidence is an audit layer only. The normal card, feature, task, QA,
proof, decision, and run stores remain authoritative. If ledger text and card
state disagree, trust the owning store and treat the mismatch as a report
defect to fix.

## Report

The session's final message is the report the human reads when they return:

- per unit: outcome plus TDD evidence (`verified, tdd: 3 cycles` or
  `verified, tdd: skipped - <reason>`) and the simplify outcome
  (`simplified` or `simplify: skipped - <non-code reason>`)
- blocked cards with reasons
- local closes with outcome, plus any close-ready-but-not-closed feature and
  the exact command to run (`maestro feature close <id> --outcome "<one line>"`)
- features prepared overnight (replenishment)
- why the loop stopped (dry, cap reached, failure budget, or hard stop)
- autonomous action table: action, target, result, redacted command, before and
  after state
- blocked/hard-stop counts, local-close count, and the run-ledger path

The report is a summary, not the record: notes, proof, card state, QA state,
and run events carry the durable evidence. `maestro query run` and
`maestro query run --json` rebuild the compact autonomy report from
`autonomy_start` and `autonomy_action` events plus normal card state.

## Scheduler Variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs ONE iteration of the loop above, then
exits. The sidecar tick is `maestro loop work-lease --json`; parse the JSON,
launch at most one worker from the returned contract, then stop until the next
external tick. The card store and run ledger are the only state between
firings - cold-start with `maestro resume` - and `claim` already guards
overlapping firings against double work. A firing that dies mid-card leaves
its claim behind; the next firing reclaims it once the claim crosses the
existing 15-min stale TTL - the same timeout that frees any abandoned claim,
not a new mechanism. Rebuild the night's account from durable state with
`maestro query run` (its `--json` carries the per-card trace, autonomy summary,
ledger paths, and an honest interruption verdict); never reconstruct the
report from a dead firing's memory. Maestro itself never schedules anything.

## Stop

- Do not ask the sleeping human questions; unblock or block locally according
  to the authority and hard-stop boundary, then move on.
- Do not invent caps the kickoff prompt did not state.
- Do not keep working past 3 consecutive blocks.
- Do not let the night's results live only in conversation; every outcome
  lands on a card through the verbs.

## Hand-off

On return, human: review the report and ledger, inspect local closes, unblock
or reassign any hard-stop cards, and decide whether to push, tag, release,
publish, or archive. Per-unit method -> [work.md](work.md); proof ->
[verify.md](verify.md).
