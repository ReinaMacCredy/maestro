# Unattended loop

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

WHEN: human says "use loop", "keep looping", away/asleep wording, or work-while-away wording -- keep going on the current work and loop the card store under full local autonomy until done.

The away-mode of the work loop: one long-lived session works the card store
until done. This is the orchestration *shape*; the full driver POLICY
(authorization, failure budget, boundaries, ledger, report) lives in
maestro-card `reference/loop.md` and is the authority. Read it before running
this.

## Shape

Per unit of work this adds nothing: every card is claimed, worked, and verified
exactly per `work.md`, test-first default included.

1. Start from the store, never from memory: `maestro status`, then
   `maestro loop work-lease --json`.
2. Parse the returned sidecar contract. If `status=leased`, work exactly the
   returned `selected_card`: `task complete --summary --claim --proof` ->
   `maestro task verify <id>`. If `status=dry` or `status=blocked`, reconcile
   from the returned inspect handles and do not launch a worker.
3. Commit each verified slice locally on the feature branch. Never push.
4. Before crossing a local autonomy gate, record/reconstruct run evidence:
   `autonomy_start` for run authority and `autonomy_action` for accept,
   prepare, unblock, local close, and hard-stop decisions.
5. When `maestro card ready` is dry, replenish: accept proposed contracts that
   pass the normal gate, prepare ready features with no tasks, then continue.
6. When a feature is locally verified and QA-slice/close gates pass, close it
   locally and record the action.
7. When nothing is workable, acceptable, preparable, unblockable, or closable
   inside the hard-stop boundary, stop and write the autonomy report.

## Boundaries (the night's hard stops)

- MAY: `feature accept`, `feature prepare`, `task unblock` for local Maestro
  blockers, `claim`, work, `complete`, `verify`, `note`, `block`, local
  per-slice commits, QA-slice, and local `feature close`.
- NEVER without explicit bounded run-scoped ship authority: push, tag, release,
  publish, archive, or any external ship action.
- NEVER even with ship authority: destructive git, secret rotation,
  platform/tool approval bypass, or guarded sidecar edits.
- If the loop fixes a loop-discovered issue, completion/ship needs durable
  recurrence-guard evidence: regression test, proof gate, QA checklist, harness
  rule, skill guidance update, or locked decision.

Full policy and exact ledger/report fields -> maestro-card `reference/loop.md`.

## Scheduler variant

An external scheduler (cron, launchd, a cloud schedule) can replace the
long-lived session: each firing runs `maestro loop work-lease --json`, launches
at most one worker from the returned contract, then exits, cold-starting from
the store with `maestro resume`. `claim` guards overlapping firings. Maestro
itself never schedules anything.

## Stop

Backlog dry or failure budget hit (3 consecutive blocks = systemic; end early).
The final message is the report: concise summary, compact autonomy action
table, blocked/hard-stop counts, local-close count, and run-ledger path. Full
policy -> maestro-card `reference/loop.md`.
