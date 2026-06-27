# Feature Cards

The feature contract and its guarded lifecycle. Work cards deliver the work;
the QA baseline and slice evidence in `qa.md` prove the feature gates.

## Use

- Inspect a feature after design: `show`, `list`.
- Finalize a clean design handoff: `finalize`.
- Freeze a proposed contract: `accept`.
- Turn an accepted contract into work cards: `prepare`.
- Grow a frozen contract: `amend`.
- Finish or retire the feature: `close`, `cancel`, `archive`,
  `auto-archive`, `unarchive`.

## Do

For feature lifecycle work after the design contract is authored, prefer native
MCP when available:

```text
maestro feature finalize <id> -> maestro_qa_baseline -> maestro_feature_accept -> maestro_feature_prepare
maestro_feature_verify -> maestro_qa_slice -> maestro_feature_close
```

Use the CLI for lifecycle and maintenance verbs not yet exposed as MCP tools
(`feature finalize`, `feature amend`, archive, and unarchive), or when MCP is unavailable. Design
authoring (`feature new`, `feature set`, and `feature spec`) belongs in
`maestro-design`. MCP tool schemas come from the host; CLI signatures live in
[cli.md](cli.md).

```sh
maestro feature finalize <id>    # writes/refreshes .maestro/cards/<id>/handoff.md
maestro feature accept            # -> ready, requires qa-baseline
maestro feature prepare --draft   # reviewable child-task plan
maestro feature prepare --from    # create/explore/accept tasks from a plan file
maestro feature close              # -> closed, requires qa-slice; --outcome required
maestro card archive <id>         # explicit terminal archive; archives children too
maestro feature auto-archive <id> --authority-ref <ref> --tested-head <sha> --qa-result pass --qa-evidence "<proof>" --run <run> --multi-agent "<disposition>"
```

Design owns proposed-contract authoring. After accept, use `feature amend` to
append to an existing list without resending it.

Use `feature show <id>` for the everyday lifecycle summary. Use
`feature list` to orient across live feature cards. Open decisions are for real
forks; `--question` is for loose questions not yet forks, both handled in
`maestro-design`.

At the approval moment, read `.maestro/cards/<id>/handoff.md` first. It is the
clean continuation index; use raw `spec.md`, `notes.md`, and
`maestro decision list --feature <id>` only for audit or deeper context. If
the handoff is missing or stale, run `maestro feature finalize <id>`.

Record directive or sequencing constraints, plus the
dated authorization line, in one `maestro card note <id> "<date + authorization
+ constraints>"`. If the approval changes scope before accept, return to
`maestro-design` to update the proposed contract first, then rerun
`feature finalize`. Then run `feature accept`; `accept` itself does not grow
approval fields.

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

When the user authorizes building, flow straight through -- `finalize`,
`qa-baseline`, `accept`, then `prepare --from`, then work -- with no
implement-now/stop confirmation between steps; the authorization already chose
to build. Decompose the contract into dependency-ordered tasks yourself (the
auto `--draft` lumps every criterion into one), mint them in one
`prepare --from`, then report the split. `ready` and the `prepare` verb stay
for the explicit freeze-and-park path: accept a contract, stop, and decompose
later when asked.

## Gates

Accept passes only when the feature has:

- at least one acceptance criterion
- at least one affected area
- a fresh `.maestro/cards/<id>/handoff.md` from `maestro feature finalize <id>`
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

Auto-archive is a separate evidence-gated archive action, not a side effect of
`close` or `cancel`. Use it only when a durable user/SPEC/run authority says
archive is preauthorized and the delivered commit hash is known. The helper
must see the exact current `HEAD` in `--tested-head`, a passing QA verdict,
bounded QA evidence, a clean worktree, and a multi-agent/worktree disposition
(`none`, or workers merged back and conflicts clear). It then runs the normal
terminal feature archive preflight, writes an `auto_archive` run event, and
adds an archive-index receipt that records authority, tested head, QA result,
run id, event hash/path, archive path, and restore command. If any check fails,
stop and report the blocker instead of falling back to manual archive.

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
- Do not reshape the accepted contract from this skill; use additive `amend`.
- Do not cancel a feature you only mean to pause. `cancel` is terminal and
  abandons live child work.
- Do not close around QA blockers. Fix the work, baseline, or slice evidence.

## Hand-off

Next: accepted feature -> [work.md](work.md); all children verified ->
[qa-slice.md](qa-slice.md), then `feature close --outcome "<one line>"`;
closed -> `maestro card archive <id>` for explicit retirement, or
`maestro feature auto-archive <id> ...` when the commit/QA authority gate is
already satisfied.
