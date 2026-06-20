# Feature Cards

The feature contract and its guarded lifecycle. Work cards deliver the work;
the QA baseline and slice evidence in `qa.md` prove the feature gates.

## Use

- Author or inspect a feature: `new`, `set`, `show`, `spec`, `list`.
- Freeze a proposed contract: `accept`.
- Turn an accepted contract into work cards: `prepare`.
- Grow a frozen contract: `amend`.
- Finish or retire the feature: `close`, `cancel`, `archive`, `unarchive`.

## Do

The lifecycle, in order (signatures: [cli.md](cli.md)):

```sh
maestro feature new               # -> proposed
maestro feature set               # author acceptance and affected areas
maestro feature accept            # -> ready, requires qa-baseline
maestro feature prepare --draft   # reviewable child-task plan
maestro feature prepare --from    # create/explore/accept tasks from a plan file
maestro feature close              # -> closed, requires qa-slice; --outcome required
maestro card archive <id>         # terminal features only; archives children too
```

`set` works only while `proposed`. Repeating a base field replaces its full
list; the `--add-*` spellings append to an existing list without resending
it. After accept, use `feature amend`.

Use `feature show <id>` for the everyday lifecycle summary. Use
`feature spec <id>` when the agent needs the narrative spec, open decisions,
locked decisions, contract, and recent notes in one view. Open decisions are
for real forks; `--question` is for loose questions not yet forks.

At the approval moment, record constraints before `accept`. Scope constraints go
into the frozen contract with `feature set <id> --add-non-goal "<constraint>"`.
Directive or sequencing constraints, plus the dated authorization line, go into
one `maestro card note <id> "<date + authorization + constraints>"`. Then run
`feature accept`; `accept` itself does not grow approval fields.

`prepare --from` expects a visible plan:

```markdown
## Task T1: Scaffold project
check: package manifest exists and tests run
blocker: dependency approval required for aws-cdk-lib

## Task T2: Implement API handlers
after: T1
check: GET /articles satisfies the API contract
```

`blocker:` creates an approval blocker. `after:` creates a task dependency.
Prepare starts the feature only when at least one child task is accepted and
unblocked.

When the user authorizes building, flow straight through -- `accept`, then
`prepare --from`, then work -- with no implement-now/stop confirmation between
steps; the authorization already chose to build. Decompose the contract into
dependency-ordered tasks yourself (the auto `--draft` lumps every criterion
into one), mint them in one `prepare --from`, then report the split. `ready`
and the `prepare` verb stay for the explicit freeze-and-park path: accept a
contract, stop, and decompose later when asked.

## Gates

Accept passes only when the feature has:

- at least one acceptance criterion
- at least one affected area
- a non-empty `.maestro/cards/<id>/qa.md` from
  [qa-baseline.md](qa-baseline.md)

On pass, the contract and baseline freeze. Later growth uses:

```sh
maestro feature amend <id> --add-acceptance "<check>" --reason "<why>"
```

Behavioral amends, meaning added acceptance or area, make the close gate require
fresh baseline/slice coverage.

Close passes only when:

- no live child work cards remain
- the baseline is fresh for behavioral amends
- every behavioral `[bl-NNN]` in the baseline has a counting slice in the
  fenced `slices:` block of `qa.md`

Use `accept --dry-run` or `close --dry-run` to preview a gate without changing
state.

## Fan-out

Use feature fan-out only when 2+ ready work cards are independent. Full
orchestration HOW (dispatch, worktree isolation, collection): `maestro loop
show feature-fan-out`.

1. Confirm with `maestro card ready <feature>` and each card's locked acceptance
   checks. Same files, dependency edges, or contended card-store writes (every
   worker's `claim`/`complete` writes the store) mean serialize, or isolate each
   worker in its own worktree -- the funnel rule in HARNESS Orchestration.
2. Spawn one fresh sub-agent per card. Each owns:
   `maestro card claim <id> -> work -> task complete --summary --claim --proof`.
3. The conductor collects completions, runs `maestro task verify <id>`, commits
   verified slices, then runs the [qa-slice.md](qa-slice.md) pass before close.

## Stop

- Do not hand-edit `card.yaml` or `qa.md`. Use verbs so guards and the amend
  audit trail stay intact.
- Do not use `set` after accept; use `amend`.
- Do not cancel a feature you only mean to pause. `cancel` is terminal and
  abandons live child work.
- Do not close around QA blockers. Fix the work, baseline, or slice evidence.

## Hand-off

Next: accepted feature -> [work.md](work.md); all children verified ->
[qa-slice.md](qa-slice.md), then `feature close --outcome "<one line>"`;
closed -> `maestro card archive <id>` if you mean to retire it.
