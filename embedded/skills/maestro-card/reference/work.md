# Work Cards

The work loop for task/bug/chore cards. A work card is the proof-gated unit of
implementation; a feature card is the product contract it may deliver against.

## Use

- Find work: `maestro ready`, `maestro list --parent <feature>`.
- Create or prepare work: `task create`, `task explore`, `task accept`.
- Pick up work: `task claim --next` (sequenced queue with dependency context)
  or `maestro claim <id>` (any ready card).
- Record progress: `task update --summary` and/or `--claim`;
  `maestro note <id> "<text>"` for running notes.
- Finish work: `task complete --summary --claim --proof`, then verify.
- Handle pauses or terminal outcomes: `block`, `unblock`, `reject`,
  `abandon`, `supersede`.
- Act on harness improvement proposals surfaced by `status`, `task next`, or
  `harness list`.

## Do

The loop, in order (signatures: [cli.md](cli.md)):

```sh
maestro task create        # mint the card; seed --check with the observable result
maestro task explore
maestro task accept        # locks acceptance, except tiny lane may skip
maestro task claim --next  # prints feature and dependency context
maestro task update        # record progress: summary and/or evidence claim
maestro task complete      # summary + claim + proof; auto-verifies
maestro task verify
```

`verify` and `show` can omit `<id>` when `MAESTRO_CURRENT_TASK` is set.

Use `--lane` as a routing convention, not a schema enum. Standard lanes are
`implement`, `explore`, `review`, `audit`, and `normal` for unrouted default
work; extend the vocabulary only by team convention. Existing states still own
workflow meaning: exploring is a task state, brainstorm/design work belongs in
a feature card or SPEC, and planning usually happens before `feature prepare`.

When a card's locked `--check` names observable behavior, that check is the
test: STOP and work it test-first per [tdd.md](tdd.md) — one failing test,
minimal code to green, repeat, then refactor — before writing implementation
code. The skip is valid only when the `--check` is non-behavioral
(docs/markdown/config-only) or the lane is explore/spike; the skip note must
name which of those two cases applies. "Non-testable" is not a free judgment
call: a locked observable `--check` is, by definition, testable.

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
maestro dep add <child> <blocker>   # child waits on blocker (card edge)
maestro task block                  # --reason why; --by names the blocking card
maestro task unblock                # pass the blocker's own blk- id, not the target
maestro task reject                 # terminal; --reason required
maestro task abandon                # terminal; --reason required
maestro task supersede              # terminal; --by names the replacement
maestro task doctor
maestro task watch
```

Open blockers stop both `claim` and `complete`. `reject`, `abandon`, and
`supersede` are terminal and cannot be undone.

## Triage And Loops

For unstructured audit/review/user-feedback backlogs:

1. Use read-only classifiers for raw untrusted items. They return severity,
   area, duplicate-or-new, and fixable-or-escalate only.
2. The conductor dedupes against `maestro list` and `maestro list --type
   feature`.
3. Create or block real work through task verbs. The agent that read untrusted
   content does not run privileged actions.

For unknown-size work:

- Stop on a query, not a feeling: `maestro ready` empty, or K discovery sweeps
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
