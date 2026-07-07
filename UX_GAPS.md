# Maestro UX Gaps

Use this as a lightweight backlog for out-of-scope Maestro UX gaps noticed
while doing other work. Keep entries short: date, surface, observed friction,
and why it is not part of the current fix.

## 2026-07-06

- Surface: repo harness versus installed local binary.
  Observed friction: `.maestro/harness/HARNESS.md` instructs agents to create
  Progress rows with `maestro task setup --task ... --start`, but the installed
  `maestro` on PATH in this worktree rejected `task setup` as an unrecognized
  subcommand. This blocks the harness-prescribed setup step before code work.
  Not part of this fix because the requested CloudBrief issue is status
  tolerance for malformed local cards, not local binary/resource drift.

## 2026-07-04

- Surface: `maestro task setup` harness guidance.
  Observed friction: `.maestro/harness/HARNESS.md` shows
  `maestro task setup --task ... --start`, but the repeatable `--task` shape is
  not obvious enough in the quick path; I first tried a plausible `--step` flag
  and got `unexpected argument '--step'`.
  Fixed: Harness `1.29.17` now includes the concrete repeatable form
  `maestro task setup --task "Map current behavior" --task "Implement scoped fix" --task "Verify" --start`.
- Surface: global skill sync for unmanaged local skills.
  Observed friction: installing the new binary reported that
  `/Users/reinamaccredy/.maestro/skills/maestro-design/SKILL.md` differs from
  the embedded skill and is not recorded as Maestro-managed. The warning gives a
  blunt move-aside-or-restore remediation, but there is no safe guided diff or
  adopt path for a locally edited skill.
  Fixed: `maestro sync --global-skills --adopt-unmanaged` now backs up unmanaged
  cache edits and replaces them with this binary's shipped skills; dry-run
  previews the affected files first.
- Surface: `maestro feature accept --qa`.
  Observed friction: `maestro feature accept loop-chain-readout-and-trace --qa
  cli --dry-run` failed with `unsupported --qa value 'cli'; only '--qa none' is
  accepted`, even though behavioral CLI work needs a way to name an explicit QA
  surface.
  Fixed: `maestro feature accept <id> --qa cli` records the explicit QA surface;
  `--qa none` remains gated by `--reason`.
- Surface: `maestro feature prepare` inline task flags.
  Observed friction: `maestro feature prepare ... --task ... --check ...`
  failed with `prepare plan must contain at least one explicit task entry`, while
  generated CLI reference advertises inline `--task`, `--check`, `--covers`,
  `--blocker`, and `--after` flags.
  Fixed: inline `maestro feature prepare --task "Title" --check "<check>" --covers <ac-id>`
  now renders a parseable task heading; local refs like `T1: Title` still work
  for `--after`.
- Surface: concurrent run busy notice.
  Observed friction: task completion printed `[busy]
  019f28bf-02cf-7322-b982-4d2a117a90ac is running the full-suite gate; hold
  heavy runs until it clears`. It explains the contention but gives no exact
  read command to check when the hold has cleared.
  Fixed: full-suite busy notices now include `inspect: maestro active`.
- Surface: proposed feature question cleanup.
  Observed friction: after locking a decision that answers one feature question,
  `maestro feature show` still lists the resolved question and the generated CLI
  reference does not expose an obvious single-question removal command.
  Fixed: `maestro feature set <id> --remove-question q-1 --reason "<decision>"`
  removes one open question by ref or exact text and records the reason in the
  feature notes.
- Surface: `maestro qa baseline --observed-stdin`.
  Observed friction: passing a full baseline contract through the helper nests
  that contract inside the helper scaffold and raw observed block, which makes
  baseline scenario discovery noisy before `qa slice`.
  Fixed: `maestro qa baseline <id> --observed-stdin` detects a full
  `### QA Baseline Contract`, stores it directly with normalized
  `amend_log_position`, and prints the parsed `[bl-NNN]` ids.
- Surface: proposed design next-step pointer.
  Observed friction: after locking the only design decision on
  `schema-contract-maintenance-architecture`, `maestro feature show` reported
  `next: maestro feature finalize ...` even though the `maestro-design` workflow
  requires an explicit build-approval gate and `feature reconcile` before
  `feature finalize`.
  Fixed: proposed feature next-step output now points to
  `maestro feature reconcile <id>` while the reconcile receipt is missing or
  stale; after reconcile is current, the next gate can advance to
  `maestro feature finalize <id>`.
- Surface: feature task `claim --next` after a verified child task.
  Observed friction: after completing T1 on
  `schema-contract-maintenance-architecture`, Maestro printed
  `next: maestro task claim --next`; running it selected an unrelated ready task
  from `maestro-loop-ux-closeout-fixes` instead of the current feature's ready
  T2.
  Fixed: verified child-task handoff now prints the next ready sibling as
  `maestro task start <next-child-id>` instead of the global
  `maestro task claim --next` shortcut.
- Surface: `maestro task note` on DB-backed task cards.
  Observed friction: trying to note the accidental claim on
  `task-clarify-task-setup-repeatable-task-85fc` failed because the command
  attempted to create `.maestro/store.sqlite/cards/<task>/notes.md`, treating
  the SQLite file as a directory.
  Fixed: card-backed `maestro task note` now routes through the DB-aware card
  note appender when the task lives in `.maestro/store.sqlite`; Progress task
  notes keep their existing Progress sidecar behavior.
- Surface: verification-only feature child tasks.
  Observed friction: `task-verify-resource-contract-kernel-gates-92e2` is a
  pure verification gate, but `maestro task start` classified it as `TDD
  required` and requested RED/GREEN proof even though no new implementation is
  supposed to happen in that task.
  Fixed: `maestro task start` now classifies explicit verification-only tasks
  as `TDD skipped` with `method_reason: verification-only task`, while ordinary
  behavior-changing tasks still render `TDD required`.
- Surface: `maestro active --connect` message hint for task peers.
  Observed friction: `maestro active --connect` suggested
  `maestro msg send --from <current-feature> <peer-task> "<text>"`, but
  `maestro msg send` then rejected the task as a non-inbox endpoint and told the
  agent to message the parent card instead.
  Fixed: task-bound peer sessions now render link/message hints against the
  owning parent card, while the conflict notice still points at the task doing
  the work.
- Surface: `maestro task start` verify+ missing hint after acceptance lock.
  Observed friction: a Progress task created by `maestro task setup --start`
  entered `in_progress` without checks; `maestro task start` then suggested
  `maestro task set <id> --check ...`, but the follow-up command failed because
  task acceptance was already locked.
  Fixed: check-edit remediation is now shown only while a standalone task is
  still in `draft` or `exploring`; locked ready/in-progress tasks point at the
  completion/proof path instead.

## 2026-07-05

- Surface: `maestro feature set --reason` on additive frozen-contract edits.
  Observed friction: while adding loop-readiness acceptance to a ready feature,
  `maestro feature set ... --reason "<why>" --acceptance ...` failed at argument
  parsing with a required `--remove-question` message. The useful guidance was
  the later lifecycle error that pointed to `maestro feature amend ...`; the
  first error made the command shape look wrong for the wrong reason.
  Why not part of the current fix: this session was design/card update only; the
  CLI parser and frozen-contract remediation behavior need a separate focused
  fix.
- Surface: ready DB-backed feature amend and handoff refresh path.
  Observed friction: after `maestro feature amend` grew
  `harness-engineering-map-for-maestro`, `maestro feature reconcile
  harness-engineering-map-for-maestro` refused because DB-backed ready features
  must be reopened first. The command is correct about the current mechanism,
  but the user path is unclear: after a scoped amendment, Maestro does not say
  whether the handoff is stale, whether reopen is required, or what safe next
  command refreshes the build handoff.
  Why not part of the current fix: the requested work was to record and deepen
  the loop-readiness design, not to change feature lifecycle reconciliation.
- Surface: `maestro status` during an active design-card session.
  Observed friction: `maestro status` foregrounded global work/progress and
  unrelated active task guidance, while `maestro active` showed the current
  session is owned by `Harness engineering map for Maestro`. The split makes a
  design fork feel like it has lost its card context unless the agent also runs
  `maestro active`.
  Fixed: `maestro status` now derives a `current_session` focus from
  `MAESTRO_SESSION_ID` and the run ledger, renders a `CURRENT SESSION` block
  with the bound card before repo-wide actions, labels the global queue as
  `REPO NEXT`, and exposes the same focus in `status --json`.
- Surface: `maestro feature design --section` readback.
  Observed friction: after appending a design section, running
  `maestro feature design <id> --section "<section>"` failed with `--section
  needs the text to write`, even though the generated reference says the verb
  can render a feature's design or fill one section.
  Fixed: `maestro feature design <id> --section "<section>"` and the `feature
  spec` alias now read that section when no `--append` or `--replace` text is
  supplied, keep write behavior for explicit text, and report available sections
  plus read-all/append commands when the requested section is missing.
- Surface: `maestro feature prepare --from` with a reopened feature workbench.
  Observed friction: placing the reviewable prepare plan in
  `.maestro/workbench/harness-engineering-map-for-maestro/prepare-plan.md`
  made the feature handoff fingerprint stale again, so `maestro feature prepare
  --from ...` refused even though the plan is an input to prepare rather than a
  design-contract change.
  Why not part of the current fix: the current work is moving the approved
  design into implementation tasks; the feature fingerprint rules and safe
  scratch-plan location need a separate lifecycle UX fix.
- Surface: `maestro feature reopen` after an emptied workbench directory.
  Observed friction: after the workbench-local prepare plan was removed, the
  empty `.maestro/workbench/harness-engineering-map-for-maestro` directory still
  blocked `maestro feature reopen`, while `maestro feature finalize` continued
  to say the fix was to run `feature reopen`.
  Fixed: `maestro feature reopen <id>` now removes only an empty stale
  `.maestro/workbench/<id>` directory before exporting the DB-backed card.
  Non-empty directories still fail and preserve their contents, so user scratch
  work is not overwritten.
- Surface: `maestro task list --json` for prepared tasks with unresolved
  after-dependencies.
  Observed friction: during loop-readiness buildout, `maestro task list --json`
  reported the follow-up prepared tasks as `state: ready` with `blocked_by: []`,
  while `maestro ready --json` correctly kept them out of the parallel wave and
  showed `remaining_blockers: ["impediment blockers"]`.
  Why not part of the current fix: the current task is adding loop readiness
  pattern contracts; task-list readiness parity needs a separate card-store
  read-model fix.
- Surface: top-level `maestro note` discoverability.
  Observed friction: `maestro note <ID> <TEXT>` exists and works, but it is
  hidden enough that an agent answered as if only `card note`, `feature note`,
  and `task note` existed until checking the live command.
  Why not part of the current fix: this task is adding implementation-note
  guidance to shipped harness/card resources; command visibility and help
  hierarchy need a separate CLI UX decision.

## 2026-07-06

- Surface: `maestro decision new --lock` / `maestro decision supersede` with
  command-shaped prose in shell arguments.
  Observed friction: while locking a design decision, backticked command text in
  the shell argument was evaluated by the shell before Maestro received it, so
  the locked decision lost the literal command phrase and had to be repaired
  with a superseding decision. Maestro accepted the already-corrupted argument;
  the only reliable catch was a manual `decision show` readback.
  Why not part of the current fix: this session is designing the
  repository-harness-native Maestro layer, not changing decision authoring UX.
  A focused follow-up should consider stdin/file input or safer generated
  guidance for multiline decision text that contains commands.
- Surface: `maestro feature set` next-step output after adding an open question.
  Observed friction: after adding a new open design question to a proposed
  feature, `maestro feature set` still printed `next: maestro feature reconcile
  <id>`, even though the design loop should not reconcile/finalize while a
  material fork remains unresolved.
  Why not part of the current fix: this session is still designing the
  repository-harness-native Maestro layer. The feature-next readout needs a
  separate lifecycle UX fix so open questions point agents back to decision
  resolution instead of reconcile.
- Surface: `maestro feature set --acceptance/--area` during design contract
  updates.
  Observed friction: adding one acceptance or area line replaced the entire
  existing list, while the generated CLI reference says the verb can "replace
  or append fields." Restoring the list with literal `[ac-n]` prefixes also
  double-numbered the readout because Maestro owns acceptance numbering.
  Why not part of the current fix: this session is locking the fanout routing
  packet design, not changing feature-contract authoring UX. A focused follow-up
  should make append-vs-replace behavior explicit and prevent accidental list
  loss during design updates.

## 2026-07-07 demo-project dogfood audit

- Surface: `maestro status` in a fresh git repo before the first commit.
  Observed friction: `git status --short --branch` reported `No commits yet on
  main`, but `maestro status` rendered the repo as `git: detached`, making a
  normal unborn branch look like a detached checkout.
  Fix in this session: preserve the unborn branch name from `.git/HEAD` when
  building the git snapshot.
- Surface: `maestro install --agent codex --dry-run`.
  Observed friction: dry-run previewed repo mirror writes but omitted the global
  skill-sync side effects that the real install performed afterward.
  Fix in this session: include the global skill-sync dry-run renderer in install
  dry-run output without writing the global lock or cache.
- Surface: shipped Harness running-note guidance.
  Observed friction: the installed harness told agents to run
  `maestro note <card-or-task-id>`, but low-ceremony task ids require
  `maestro task note`; the top-level note command is card-store scoped.
  Fix in this session: update shipped and local Harness plus maestro-card skill
  guidance to use `maestro task note <task-id>` for task implementation notes.
- Surface: `maestro task list` after serial Progress setup.
  Observed friction: the installed binary showed dependency-blocked ready tasks
  as claimable in the compact list even though `maestro ready --plan` and
  `maestro task claim` enforced the dependency.
  Fix in this session: make the compact task-list renderer use the readiness
  dependency check and render blocked ready rows as `ready / blocked` with an
  inspect-blocker next action.
- Surface: `maestro task start <id>` on low-ceremony Progress tasks.
  Observed friction: after starting a low-ceremony task, the handoff pointed at
  `maestro task complete`, but that command is rejected for tasks without an
  explicit verification gate and the working closure path is `maestro task done`.
  Fix in this session: reuse the simple-done contract predicate for claim/start
  handoff output and print `maestro task done <id> --proof "<evidence>"` for
  low-ceremony tasks.

## 2026-07-07 witness design dogfood

- Surface: `maestro decision show <id>` during a design session.
  Observed friction: a read command printed `global skills resynced to this
  maestro` output, which made a read-only inspection look like it had mutated
  global skill state.
  Why not part of the current fix: this session is designing the witness
  approval gate, not changing passive sync output. A focused follow-up should
  make read-command side effects explicit, suppress unrelated sync chatter, or
  move it behind a separate actionable notice.
