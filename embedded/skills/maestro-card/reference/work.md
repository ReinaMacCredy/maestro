# Work Cards

The work loop for task/bug/chore cards. A work card is the proof-gated unit of
implementation; a feature card is the product contract it may deliver against.

## Use

- Find work: `maestro card ready`, `maestro card list --parent <feature>`.
- Create or prepare work: `task create`, `task explore`, `task accept`.
- Pick up work: `task claim --next` (sequenced queue with dependency context)
  or `maestro card claim <id>` (any ready card).
- Record progress: `task update --summary` and/or `--claim`;
  `maestro card note <id> "<text>"` for running notes.
- Finish work: `task complete --summary --claim --proof`, then verify.
- Handle pauses or terminal outcomes: `block`, `unblock`, `reject`,
  `abandon`, `supersede`.
- Act on harness improvement proposals surfaced by `status`, `task next`, or
  `harness list`.

## Do

When native Maestro MCP tools are available, use them for the normal work-card
loop:

```text
maestro_task_create -> maestro_task_explore -> maestro_task_accept
maestro_task_claim -> maestro_task_update -> maestro_task_complete
maestro_verify
```

Use `maestro_card_ready`, `maestro_card_list`, and `maestro_card_show` for
orientation. Use the CLI loop below when MCP is unavailable or a needed verb is
not exposed as an MCP tool. Signatures: [mcp.md](mcp.md), [cli.md](cli.md).

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

## Cards As A Lightweight Todo List

For simple work that does not need the full
feature->accept->verify->close pipeline, the gate-free `card` verbs are a
Claude-task-tool-style todo tracker. The whole loop is three verbs:

MCP: `maestro_card_create` -> `maestro_card_claim` -> `maestro_card_close`.

```sh
maestro card create -t task "first thing" "second thing" "third thing"  # batch-mint N open todos
maestro card claim <id>     # start one: marks it in_progress and takes ownership
maestro card close <id>     # finish it
```

`card create` takes one or more titles and mints one open card each; pass
`--id-only` to capture the ids for scripting (one per line). `--parent` and
`-t/--type` apply to every card in a batch; per-card text
(`--description`, `--active-form`) is one card's, so set it later with
`card update <id>`. No `feature`, `accept`, `verify`, or proof is involved --
these are plain todos.

Focus discipline (the task tool's one-active-item rule):

- `claim` is the "start before working" step. It records you as the owner and
  moves the card to `in_progress` in one move -- use it, not
  `card update --status in_progress` (which leaves the card unowned).
- Keep one card `in_progress` per session at a time. If you claim a second
  while one is still active, `claim` prints a STDERR advisory naming the
  already-active card; it never blocks, so close or pause the first when you
  switch.
- `close` when the work is done.

Scoped todo view -- the same `list` filters narrow to your todos:

```sh
maestro card list --type task --status open          # the backlog (pending)
maestro card list --type task --status in_progress   # what is being worked
maestro card list --assignee "<agent>#<session>"     # just yours
```

The model maps 1:1 to the task tool, so the board reads the same way:

| task tool   | maestro status | board glyph        |
| ----------- | -------------- | ------------------ |
| pending     | `open`         | `○` (ready)   |
| in_progress | `in_progress`  | `◐` (active)  |
| completed   | `closed`       | `✓` (done)    |

`maestro watch` renders the board live. A card given `--active-form "<doing
X>"` shows that present-tense label on its active row in place of the title
(display-only, like the task tool's activeForm); set it at `create` for a
single card or later with `card update <id> --active-form "..."`.

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
