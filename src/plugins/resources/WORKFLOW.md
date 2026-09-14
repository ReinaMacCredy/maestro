# Maestro workflow: design, work, verify

The method behind maestro, shared across coding agents (Claude Code, Codex,
and any tool that can read this file). Installed to `~/maestro/WORKFLOW.md`
by `maestro install` and overwritten on every install; owner notes belong in
`~/maestro/OWNER.md`. A closer repository instruction replaces this workflow.
The skills under `~/maestro/skills/maestro-*/SKILL.md` hold the procedures;
this file is the single source for non-SLP method rules. Skills, recipes,
templates, and tool briefs reference these rules rather than defining copies.
The running SLP team's pinned contract remains separate.

## Tiers

Read-only reconnaissance comes before tier selection: inspect enough source,
existing checks, and relevant work to understand the next change and its risk.
Reconnaissance does not authorize implementation or unbounded exploration.

- **quickfix**: a bounded, low-risk change explainable in a sentence, with
  no Full trigger. Work directly, verify inline, no skill or record required.
  If it needs tracking, `maestro work add` and continue as Light.
- **Light, the default**: bounded work with clear acceptance and no Full
  trigger. `maestro work add|start|done` plus relevant notes is the record;
  no bundle or SPEC required. A checkpoint supports continuation across sessions.
- **Full**: use a bundle for high-risk scope (schema change, wide refactor,
  irreversible step), coordinated writers with shared scope or dependencies
  needing a durable contract, or an explicit user request for a bundle.
  `maestro bundle open <id> --work <workId>` scaffolds
  `.maestro/bundle/<id>/` with `SPEC.md`, `NOTES.md`, `VERIFY.md`.

A context reset, session change, or failed attempt alone is not a Full trigger.
Tier determines record depth, not test count. If recon or implementation finds
a Full trigger, upgrade then and reuse the evidence already collected; never
redo finished work for ceremony. During an authorized production incident,
stabilize and verify first, then backfill the required record.

## Authorization boundaries

- A request to design or plan does not authorize production edits.
- An explicit request to implement or fix authorizes only that stated scope.
- Read-only requests (answer, review, report, diagnose, explore) stay
  read-only.
- Workflow ownership never grants authority to push, merge, release, deploy,
  publish, or mutate external systems.
- Preserve the original user instruction (quote or retrievable reference),
  approved scope, target, and remaining gates in work notes and handoffs.
  A successor may continue that same authorized work after checking the
  instruction, current repo state, ownership, and any later changes or revocation.
  A bundle, decision, or agent-written claim of approval is not itself permission.
  If the original grant cannot be verified, ask before editing.
- A new session or tool alone does not require renewed approval. Expanded
  scope or an unapproved external action does. Never bypass host permissions,
  identity checks, or repository restrictions to continue.

## Decisions and readiness

The user owns product behavior, scope, important trade-offs, and permission
boundaries. The agent owns reversible implementation details within that
contract: use existing patterns, state consequential assumptions, and continue.
Ask only for a material unresolved choice or missing authority, not a method
transition. Research facts directly instead of asking the user to find them.

Lock decisions whose rationale must survive: hard-to-reverse choices, important
trade-offs, or durable constraints. Keep routine details in the work or final
diff, not separate decision records. Supersede conflicting durable decisions
explicitly; do not silently rewrite them.

The next bounded slice is ready when its outcome, scope, authority, and check
are clear and no unresolved choice blocks it. Count blockers to that slice,
not all open questions about later work. A design-only request still authorizes
no implementation, and approval of a choice is not a request to build it.

## Routing

The user supplies an outcome, not a skill name. Route internally and keep an
authorized task continuous through investigation, implementation, and checks.
Skills load by bare name in every tool (`maestro-design`, `maestro-work`,
`maestro-verify`, `maestro-bundle`, `maestro-explore`, `maestro-diagnose`,
`maestro-coach`, `maestro-questionnaire`, `maestro-improve`,
`maestro-council`). Route only work
that needs one: a quickfix or a Light production fix proceeds directly
(stated assumption, root-cause fix, inline verification). Never downgrade a
requested fix to analysis. Generated or vendored files are never the target:
fix the generator or pin and regenerate.

- Material unsettled decisions, design questions, or new scope needing
  clarification: `maestro-design` (grill, research, prototype, model,
  wayfind by unknown).
- A hard-to-reverse fork with wide blast radius, run by the Lead:
  `maestro-council`.
- Research, disposable prototype, or current-behavior baseline:
  `maestro-explore`.
- Authorized implementation or fix: `maestro-work`.
- Diagnosis-only work: `maestro-diagnose`; on a fix request, diagnosis is
  `maestro-work`'s first phase.
- Verifying and closing an open bundle: `maestro-verify`.
- A fork the user does not understand, or a recorded decision they want to
  learn from: `maestro-coach`.
- A decision owned by someone not in the conversation:
  `maestro-questionnaire`.
- Tier, bundle lifecycle, resume, and handoff: `maestro-bundle`.
- Quality review: use Per-tool adapters below. Verify owns "does it meet
  the contract"; review owns "is the code good".

## Two coordination models

Maestro keeps two coordination models with an explicit boundary. Inside a
running SLP team (a Lead, Peer, or Team Supervisor pane after `team start`)
only the nine SLP v2 operations apply, and the retired verbs redirect to them.
Everywhere else (plain sessions, the Hub room, design and wayfinding sessions)
the classic surface is the live model: `work start|done`,
`decision draft|lock`, `ready`, `dispatch` and `handback` lane contracts,
councils, worktree lanes, and policy-dispatch. `dispatch` and `handback` are
not legacy and are not scheduled for removal; they are the lane contracts
outside a running SLP team.

## Bundle contract (tier Full)

- `SPEC.md` is a pure contract: Problem, Solution, Scope, Anti-goals (each
  with a matching VERIFY.md check), Decisions (store ids, rendered by
  `maestro bundle show <id>`), Red tests (when needed by Testing discipline,
  not a quota or an exhaustive whitelist). Record newly discovered in-scope
  risks and checks without reopening design; scope expansion needs the user.
- `NOTES.md` is a pure handoff, rendered by `maestro handoff <id>` from the
  store, never authored or appended: Current State, Next Action, Authority
  transferred and retained, Failed approaches, Do not repeat,
  `Base: <commit>`, `Driver: <tool>`. Hand-edit only the placeholders it
  leaves. Re-run at handoff or a meaningful checkpoint, not every reply. History lives in
  the store: `maestro work note`, `maestro trace`, decisions; the latest
  `checkpoint:` note (state / next / avoid) is what survives a compaction.
- `VERIFY.md` is the scenario table drafted at design time and filled at
  verify time. Results hold the latest run only, stamped with date and commit.

Close under Completion and delivery below: `maestro bundle close <id>`
snapshots the trio into the store. Archived bundles are never reopened;
follow-up work gets a new bundle that links the old one. Bundle directories
are never staged or committed.

## Resume

- Resume the identified work item with `maestro work show <id> --notes all`;
  read its latest checkpoint and original authorization. For Full, also read
  NOTES.md and `maestro bundle show <id>`; use `maestro bundle list` only to
  locate an unknown bundle. Two plausible matches is a scope collision: ask.
- Reconcile NOTES against live repo state first: `git log <Base>..HEAD` and
  `git status` show what happened behind NOTES' back. Repo state beats NOTES,
  NOTES beats memory. A bundle whose scope already shipped is closed on
  sight. An interrupted operation has unknown outcome until checked.
- Light handoff uses a work note: state, next action, base, task-owned dirty
  paths, original authorization and retained gates, failed approaches, and
  what not to repeat. No bundle is needed just to remember these facts.
- Refresh the handoff before switching; reconcile it and `git status` before
  the successor's first edit. Do not repeatedly reload unchanged history on
  every conversational turn. Release ownership before another writer takes it.

## Memory

- Recall: `maestro search "<term>"` hits work, decisions, notes, terms, and
  archived bundles in the repo store; the Hub index is `~/maestro/MEMORY.md`,
  written by `maestro memory render` and imported by each tool's global
  instruction file. Cite what you use; re-verify fast-drifting
  facts (branches, ports, dirty state, test counts) live.
- Record: a durable decision has its rationale and rejected alternative;
  a domain term is `maestro term add`; a failed pass is one
  `maestro work note <id> "failed: <one line>"`.
- Per-tool stores (Claude auto-memory under `~/.claude/projects/*/memory/`,
  Codex `~/.codex/memories/`) are write buffers their tools keep writing.
  Promotion into the Hub store is `maestro memory ingest` (dry-run first),
  which like every memory verb runs from any cwd and re-renders the index;
  only the Hub render is injected. No hand-edited global index.

## Testing discipline

Verification is required; new tests are not. Behavior and risk determine the
check, independently of tier. Honor explicit user and repository test-first
requirements without treating them as a demand for duplicate tests.

- Inspect and run relevant existing checks before adding any. Reuse or extend existing tests first.
- Before adding a test, name the gap: "Which plausible wrong implementation would this catch
  that existing checks would miss?" Assert the observable contract, not internal
  structure, mock choreography, or an expectation copied from the implementation.
- A bug needs a reproducer; use an existing failing test when it already catches
  it. When a new regression test is needed, demonstrate failure before the fix.
  A small feature may also need tests. Add them for uncovered behavior or risk,
  not because a file, function, or tier exists. Red-first is useful when a new
  executable check is needed and the contract is clear, not a paperwork gate.
- Docs may need readback and link checks; refactors may use existing suites and
  before/after evidence. Add a characterization test only for an actual gap.
  Avoid new fixtures, mocks, files, or frameworks when existing tools suffice.
- Newly discovered risks inside approved scope may justify checks beyond a
  SPEC's original list. Record the gap, not a new design ceremony. Changed
  product behavior or scope still needs approval.
- Stop when acceptance and in-scope risks have sufficient evidence. Do not
  start a coverage campaign, unrelated edge-case sweep, or blanket mutation
  testing. There is no arbitrary numerical cap on tests.
- Never delete, skip, or weaken a failing test to manufacture a pass. Correct
  a wrong test openly against the approved contract and record the reason.

## Recovery and verification

Repeated failures are a signal to stop repeating the approach, not proof of a design flaw.
Compare attempts and evidence; distinguish implementation, environment, flaky
checks, and a mistaken contract. After two or three failures on the same
mechanism, record what changed, the shared assumption, and the smallest new
fact needed. Return to design only when evidence challenges the contract or
reveals a material choice; otherwise continue the authorized fix.

Verify acceptance and relevant risks with sufficient evidence, not one new
test per behavior. Repair a broken command or use an equivalent measurement
inside the same acceptance: record the old/new check and why it proves the
same outcome, then run it. Do not weaken thresholds, drop required behavior,
or substitute source evidence for a required live check. If equivalence is
uncertain or the contract must change, surface the gap before claiming PASS.
After code changes, rerun affected checks; changed shared boundaries may
require broader checks. Use focused mutation checks only for a concrete
assertion-strength concern, never as a blanket close gate.

## Completion and delivery

Distinguish implementation verified, awaiting delivery approval, and delivered.
These are reported outcomes, not new work states. `work done` means the item's
accepted scope is complete; it does not assert commit, push, install, or deploy.
If delivery is part of that acceptance, the item remains unfinished until it
is authorized and proven. Otherwise record the pending delivery action and
retained gate, close the implementation item, and report it without taking it.

A Full bundle closes after its accepted scope is verified, or on explicit
transfer/cancellation or a completed no-change investigation. A commit is not
a prerequisite unless acceptance requires it. Save final evidence and pending
gates before closing; a continuation of unfinished work stays active or paused.
Keep a failed acceptance open, even if some checks passed. Never claim remote
delivery from local evidence.

## Worked examples

- **Small Save bug, existing reproducer:** inspect and reproduce, fix the
  handler, rerun the relevant check. Do not add a duplicate test or a bundle
  just because the work is a bug. Discovery of a data-loss risk changes the tier.
- **Small feature, uncovered behavior:** reuse a nearby test for the new
  observable result; add a case if it catches a concrete wrong implementation.
  Light does not prohibit that test or require unrelated edge cases.
- **Session change mid-fix:** read the checkpoint and original grant, reconcile
  the checkout and ownership, then continue. No automatic Full upgrade or
  renewed approval; an unverifiable or revoked grant still blocks edits.
- **Third failure, environment differs:** compare logs and environment before
  another code edit. A stale command may be repaired with equivalent evidence;
  do not redesign the product or relax acceptance based on the count alone.
- **Implementation verified, no push approval:** if acceptance was the local
  fix, close it and report the pending push gate. If acceptance included remote
  delivery, keep that work open. Neither case authorizes a push.

## Concurrency and git

One work item per tracked thread, with a bundle only when Full is warranted;
concurrent threads write only disjoint, exclusively owned paths. Dirty or
untracked content visible in the checkout
is not owned by this thread merely because it is visible; leave it alone.
Stage explicit task-owned paths only, and inspect the staged diff before
committing. Commit or push only when asked; push, tag, publish, and release
are the user's gates.

## Per-tool adapters

Review routing is the same in every tool, using whatever the tool ships:
Light runs a simplification pass after green, before reporting completion; Full
runs one correctness review of the frozen task-owned diff after verify
passes, a broader multi-reviewer pass when the diff touches trust boundaries,
schemas or migrations, or several subsystems, plus a security review when it
touches auth, secrets, or input handling. Any simplification pass on Full
runs after green and before verify, so the reviewed diff is final.

- **Claude Code**: skills load from `~/.claude/skills/maestro-*` symlinks
  that `maestro install` maintains; `/simplify` and `/code-review` are the
  built-in simplification and correctness passes. Forks go to the user as
  question cards: one decision per card, "what this does" first, a `my rec:`
  line.
- **Codex**: skills load from `~/.codex/skills/maestro-*` symlinks that
  `maestro install` maintains (restart Codex after a new link); the Full
  review runs in a fresh context. In project work Codex reads
  `~/.codex/AGENTS.md`, which must point here; inside the Hub it reads
  `~/maestro/AGENTS.md`.
