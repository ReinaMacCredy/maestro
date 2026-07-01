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

Recipe checkpoint: feature implementation uses `maestro loop show work` until
all child tasks verify. Before close, local install, push, release, publish,
archive, or any other ship-style gate, switch to `maestro loop show ship` and
fail closed unless authority, target, allowed actions, hard stops, and evidence
are explicit.

```sh
maestro feature finalize <id>    # writes/refreshes the finalized handoff authority
maestro feature accept            # -> ready, requires qa-baseline
maestro feature prepare --draft   # reviewable child-task plan
maestro feature prepare --from    # create/explore/accept tasks from a plan file
maestro feature close              # -> closed, requires qa-slice; --outcome required
maestro feature auto-archive <id> --authority-ref <ref> --authority-target <id> --authority-head <sha> --authority-state current --tested-head <sha> --qa-result pass --qa-evidence "<proof>" --run <run> --multi-agent "<merge/evidence disposition>" --canonical-store <path-to/.maestro> --worker-source "<branch/worktree or none>"  # final cleanup when bounded ship/auto-archive authority exists
maestro card archive <id>          # explicit terminal archive when no auto-archive authority exists
```

Design owns proposed-contract authoring. After accept, use `feature amend` to
append to an existing list without resending it.

Use `feature show <id>` for the everyday lifecycle summary. Use
`feature list` to orient across live feature cards. Open decisions are for real
forks; `--question` is for loose questions not yet forks, both handled in
`maestro-design`.

At the approval moment, read `maestro feature show <id>` and
`maestro feature spec <id>` first. They are the authority-aware continuation
index after finalize, including DB-backed finalized cards that no longer have a
live `.maestro/cards/<id>/` directory. Use raw editable files only while a card
folder or workbench is the current authority surface. If the handoff is missing
or stale, run `maestro feature finalize <id>`.

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
- a fresh finalized handoff from `maestro feature finalize <id>`, readable with
  `maestro feature spec <id>` / `maestro feature show <id>`
- a non-empty QA baseline from [qa-baseline.md](qa-baseline.md), readable through
  `maestro feature spec <id>` and the QA verbs

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

Auto-archive is a separate evidence-gated archive action, not a blind side
effect of `close` or `cancel`. After a successful `feature close` or auto-close,
do not finish at "closed" when a durable user/SPEC/run authority grants bounded
ship or auto-archive authority for this target. Complete the separately
authorized push, publish, release, local install, or handoff boundary first; then
run `maestro feature auto-archive <id>` without asking again once the delivered
commit hash is known. The helper must see the exact current `HEAD` in
`--tested-head`, matching current target-scoped authority in `--authority-target`,
`--authority-head`, and `--authority-state current`, a passing QA verdict,
bounded QA evidence, a canonical owning store in `--canonical-store`, and a
multi-agent/worktree disposition (`none`, or workers merged back and conflicts
clear). A linked implementation worktree may auto-archive when its current
`.maestro` store owns the live target card and the work is done and verified on
the exact current `HEAD`. A worktree whose store is missing the target card,
stale, or merely copied from another checkout provides commits and evidence
only; the checkout that owns the target store runs the helper. The helper then
runs the normal terminal feature archive preflight, writes an `auto_archive` run
event, and adds an archive-index receipt that
records the canonical store path, invoking checkout path, worker branch/worktree
source, final target head, tested head, authority, merge-back/evidence
disposition, run id, event hash/path, archive path, and restore command. If any
check fails, stop and report the blocker instead of falling back to manual
archive. Use `maestro card archive <id>` only for explicit terminal archive when
no auto-archive authority exists.

## Fan-out

Use feature fan-out only when 2+ ready work cards are independent. Full
orchestration HOW (dispatch, worktree isolation, collection): `maestro loop
show feature-fanout`.

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
[qa-slice.md](qa-slice.md), then `feature close --outcome "<one line>"`. After
close, run `maestro feature auto-archive <id> ...` when bounded ship or
auto-archive authority plus exact-HEAD QA evidence are satisfied; otherwise use
`maestro card archive <id>` only for explicit terminal retirement.
