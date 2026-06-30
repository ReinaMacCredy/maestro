# Task Work

The work loop for executable Tasks. A Task is the atomic unit of implementation.
Cards are the high-level durable work objects. Feature, Bug, Chore, Custom,
Decision, Idea, and Progress are CardKinds / workflow kinds on those cards.
Progress is the lightweight Task container for small same-session work.

## Use

- Find work: `maestro card ready`, `maestro card list --parent <feature>`.
- Create or prepare work: `task create`, `task explore`, `task accept`.
- Pick up work: `task claim --next` (sequenced queue with dependency context)
  or `maestro task start <ref-or-id>` / `maestro task claim <id>` for one ready
  task.
- Record progress: `task update --summary` and/or `--claim`;
  `maestro card note <id> "<text>"` for running notes.
- Finish work: `task complete --summary --claim --proof`, then verify.
- Handle pauses or terminal outcomes: `block`, `unblock`, `reject`,
  `abandon`, `supersede`.
- Act on harness improvement proposals surfaced by `status`, `task next`, or
  `harness list`.

Recipe checkpoint: executable work uses `maestro loop show work`. Use that
recipe as the shape for perceive -> choose -> act -> observe -> learn ->
continue while the concrete lifecycle writes still go through task/card/proof
verbs. If a custom card/run recipe is needed, keep the same six phases, current
Maestro verbs, hard stops, and continue output.

## Do

When native Maestro MCP tools are available, use them for the normal Task loop:

```text
maestro_task_create -> maestro_task_explore -> maestro_task_accept
maestro_task_claim -> maestro_task_update -> maestro_task_complete
maestro_verify
```

Use `maestro_card_ready`, `maestro_card_list`, and `maestro_card_show` for
orientation. Use the CLI loop below when MCP is unavailable or a needed verb is
not exposed as an MCP tool. MCP tool schemas come from the host; CLI
signatures live in [cli.md](cli.md).

```sh
maestro task create        # mint the card; seed --check with the observable result
maestro task explore
maestro task accept        # locks acceptance
maestro task claim --next  # prints feature and dependency context
maestro task update        # record progress: summary and/or evidence claim
maestro task complete      # summary + claim + proof; auto-verifies
maestro task verify
```

`verify` and `show` can omit `<id>` when `MAESTRO_CURRENT_TASK` is set.

Use `--lane` as a routing convention, not a schema enum. Standard lanes are
`implement`, `explore`, `review`, `audit`, `light`, and `normal` for unrouted
default work; extend the vocabulary only by team convention. A `light` card is
one whose change carries no real logic to test: mechanical or structural code
with behavior held constant, config-only, docs-only, or a throwaway spike. A
change that adds or alters observable behavior is not light, whatever its size.
Existing states still own workflow meaning: exploring is a task state,
brainstorm/design work belongs in a feature card or SPEC, and planning usually
happens before `feature prepare`.

When a card's locked `--check` names observable behavior, that check is the
test: STOP and work it test-first per [tdd.md](tdd.md) — one failing test,
minimal code to green, repeat, then refactor — before writing implementation
code. The skip is valid only when the `--check` is non-behavioral
(docs/markdown/config-only), the lane is explore/spike, or the card is
`--lane light` (no real logic to test); the skip note must name which case
applies. "Non-testable" is not a free judgment call: a locked observable
`--check` is, by definition, testable.
Task handoff surfaces (`task create`, `task show`, `task accept`, and `task
claim`) render `implement_method`, `method_reason`, and `proof_required` before
work starts. `TDD required` means finish with matching RED and GREEN proof
claims. `TDD skipped` means finish with one claim naming the printed skip
reason plus the relevant verification evidence.

Before you write implementation code, climb the reach-ladder (HARNESS Code
style: skip/YAGNI -> stdlib -> native platform -> installed dependency ->
one-liner -> minimal new code) and stop at the lowest rung that solves the
card; name the rung you reached in the completion summary. `maestro lean` sets
how strictly to climb (lite/full/ultra/off) and `maestro lean review` walks the
diff against the ladder; the simplify pass below tidies whatever still sits too
high.

`--lane light` is a marker, never a grant: it does not exempt a card from
verification. If the `--check` names behavior the change introduces or alters,
the card is not light -- keep test-first. The Evidence Gate below is unchanged
for a light card; a code light card (mechanical/structural) still proves
behavior held constant -- the existing suite/build stays green (the normal
GREEN claim, minus the RED-first ordering) -- not merely the skip reason, while
a docs/config-only light card records the skip reason as its claim.

After the implementation is green and before `task complete --proof`, run the
simplify pass on the working-tree diff per [simplify.md](simplify.md): tidy the
change (reuse, dead code, wrong altitude) in place and keep it green. The
assessment runs on every card; you only edit when it finds something. On a
test-first card that pass IS the red-green-refactor step -- do it once, not
twice. Skip the assessment only for a purely non-code (docs/config) diff or a
`--lane light` card (nothing to clean, or the change is itself the cleanup),
and name that reason in the completion summary.

## User Steering During Work

User corrections are durable steering, not loose chat. Do not pause just
because the user corrected you. Pause only when the correction is ambiguous and
continuing could create wrong or hard-to-reverse work.

Classify and continue by risk:

| Signal | Action |
| --- | --- |
| Clear correction | Record `maestro event intervention --note "<correction>"`, update the current Task/Card if needed, and keep moving. |
| Unclear but low-risk | State the assumption, record it, and continue. |
| Unclear and scope/risk-changing | Ask one concise question before changing scope, contract, schema, lifecycle, release behavior, or deleting/overwriting work. |

Route the correction to the durable place it changes truth:

- Current execution detail -> `maestro task update <id> --summary ...` or
  `maestro task note <id> ...`.
- New executable work -> create a new Task under the same Card.
- Parent scope or acceptance changes -> amend the parent Feature/Card contract;
  do not silently rewrite the current Task.
- Fork/choice -> create or lock a Decision Card.
- Behavior gap -> record QA evidence, then create a Bug or Task when it needs
  executable follow-up.

Do not create a "next task" for every correction. Create the next Task only
when the correction is separate executable work.

## Simple Task Board

For simple work that does not need the full feature/card pipeline, use the
low-ceremony Task surface. There is no `todo` namespace and no task-specific
second lifecycle.

With Maestro hooks installed, the first write-like `PreToolUse` in an
implementation session auto-creates or reuses that session's Progress card,
adds one low Task if needed, starts it, and binds the session to the Progress
card. This does not fire for read-only hooks or when `MAESTRO_CURRENT_TASK` is
already set. Without hooks, do the same explicitly:

MCP: `maestro_task_add` -> `maestro_task_start` -> `maestro_task_done`.

```sh
maestro task add "fix typo"      # creates a ready Task inside progress.yml
maestro task list                # shows live rows with REF numbers
maestro task start <ref>         # marks it in_progress and takes ownership
maestro task done <ref> --proof "fixed typo"  # records proof and verifies it
```

`task add` is for small work. It creates or reuses the current actor's Progress
card under `.maestro/cards/<progress-id>/` and appends a TaskRecord row to
`progress.yml`. The Task is immediately ready to start, and `--id-only` prints
only the new Task id. For simple Chore-owned work, attach it with `--card
<chore-id>`; that path remains card-backed for compatibility. Feature, Bug, and
Custom card work should be prepared into Tasks through the card/feature prepare
path.

Progress is still a card, but the rows inside `progress.yml` are Tasks, not
CardTypes. Keep a Task in Progress while it only needs executable tracking. Lift
it into a card-backed Task/Bug/Custom/Chore when it needs its own lifecycle
record, facets (`spec.md`, `qa.md`, `notes.md`), discussion/history, child
tasks, product/defect/custom identity, or governance beyond execution.

Focus discipline (the task tool's one-active-item rule):

- `task start` is the "start before working" step. It records ownership and
  moves the Task to `in_progress` in one move. Use the `REF` from
  `task list`, or a stable id from `task list --json` / `task add --id-only`.
- Keep one Task `in_progress` per session at a time. If another task is active,
  the active board surfaces it; finish or pause the first when you switch.
- `task done <ref-or-id> --proof "<evidence>"` only works for low-ceremony
  standalone or Chore-owned Tasks with no explicit verification gate. If a Task
  has checks, a verify command, or belongs to Feature/Bug/Custom work, use
  `task complete --summary --claim --proof`, then `task verify`.

Board view:

```sh
maestro task list             # live Tasks; done hidden
maestro task list --mine      # only Tasks claimed by this actor
maestro task list --all       # include done/terminal history
maestro task list --json      # machine-readable refs plus stable ids
```

`maestro task list` includes current actor/session Progress-backed Tasks and
legacy card-backed Tasks. Human output uses ordinal `REF` values for
`task show/start/done`; stable ids stay in `progress.yml` and `--json`.
`maestro card list --type progress` shows the Progress card itself; low Tasks
do not appear as card rows.

The board reads:

| task tool   | maestro status | board glyph        |
| ----------- | -------------- | ------------------ |
| pending     | `ready`        | `○` (ready)   |
| in_progress | `in_progress`  | `◐` (active)  |
| completed   | `verified`     | `✓` (done)    |

`maestro watch` renders the board live.

## Evidence Gate

`complete --proof` records proof text and auto-runs verification. Verification
passes only when:

- task state is `needs_verification`
- at least one non-empty completion claim exists
- at least one proof source exists
- every claim text matches some proof or event text after whitespace
  normalization
- every configured verify command exits 0

Reliable closeout. A test-first card records the red→green pair as two claims,
each with matching proof:

```sh
maestro task complete <id> \
  --summary "<what changed>" \
  --claim "RED: test_<behavior> failed before impl" \
  --claim "GREEN: cargo test: 41 passed, 0 failed" \
  --proof "RED: test_<behavior> failed before impl" \
  --proof "GREEN: cargo test: 41 passed, 0 failed"
```

A card that took the test-first skip records a single claim naming the locked
skip reason instead.

Use concrete observed claims. A vague claim fails even when the work is real.
Use `maestro event create --task-id <id> --claim "<claim>"` only to repair or
add manual evidence after the default proof path is insufficient.

## Blockers And Terminal Verbs

```sh
maestro card dep add <child> <blocker>  # child waits on blocker (card edge)
maestro task block                      # --reason why; --by names the blocking card
maestro task unblock                    # pass the blocker's own blk- id, not the target
maestro task reject                     # terminal; --reason required
maestro task abandon                    # terminal; --reason required
maestro task supersede                  # terminal; --by names the replacement
maestro task doctor
maestro task watch
```

Open blockers stop both `claim` and `complete`. `reject`, `abandon`, and
`supersede` are terminal and cannot be undone.

## Triage And Loops

For unstructured audit/review/user-feedback backlogs:

1. Use read-only classifiers for raw untrusted items. They return severity,
   area, duplicate-or-new, and fixable-or-escalate only.
2. The conductor dedupes against `maestro card list` and `maestro card list --type
   feature`.
3. Create or block real work through task verbs. The agent that read untrusted
   content does not run privileged actions.

For unknown-size work:

- Stop on a query, not a feeling: `maestro card ready` empty, or K discovery sweeps
  with zero new findings.
- Turn each new finding into a card immediately so it survives context loss.
- Claim, work, complete, verify, then re-check the stop condition.

## Harness Improvement

When Maestro surfaces recurring friction, act before unrelated work unless the
proposal is noise.

```sh
maestro harness list
maestro harness show <id>
maestro harness apply <id>      # spawns an accepted standalone task
maestro harness measure <id>    # requires linked task verified
maestro harness dismiss <id>    # --reason required
```

If measurement still finds friction, the proposal reopens; if a measured
proposal regresses, it reopens.

## Inbox

Messaging rides on the work loop. When you and a peer hold linked cards
(`maestro link add`), an `[inbox] N new (...) -> maestro msg read` line prints
to STDERR before every command: a linked peer is waiting. Act on it before
unrelated work -- run `maestro msg read` to consume the unread and advance your
cursor (no arg aggregates every linked partner; `<their-card>` scopes to one).
Coordinate or reply with `maestro msg send <their-card> "<text>"`: the sender
is your current card, and a send is rejected unless the pair is still linked.
Reply when the message poses a question or needs a decision; an FYI needs no reply.
Messaging is pull-only -- nothing reaches the peer until that agent reads.

Inbox messages are advisory coordination signals. They may surface a possible
cross-card task order, but they do not block execution and are not dependency
records. If order matters, record an explicit Task blocker, for example
`maestro task block <dependent-task> --reason "<why>" --by <blocking-task>`.
Readiness, `task next`, claiming, and verification consult Task blockers, not
messages or unread state.

## Stop

- Do not hand-edit `.maestro/cards/<id>/card.yaml` or state history.
- Do not skip states. `claim` expects a ready card; verification owns
  `verified`.
- Do not use terminal verbs for "blocked, resume later"; use `block`.
- Do not complete with empty or unprovable `--claim`.
- If Git metadata is unavailable, do a targeted non-Git closeout review and say
  so in the proof.

## Hand-off

Next: task completed or proof failed -> [verify.md](verify.md); feature
children all verified -> [qa-slice.md](qa-slice.md).
