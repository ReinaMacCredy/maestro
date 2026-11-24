# Maestro workflow: design, work, verify

The method behind maestro, shared across coding agents (Claude Code, Codex,
and any tool that can read this file). Installed to `~/maestro/WORKFLOW.md`
by `maestro install` and overwritten on every install; owner notes belong in
`~/maestro/OWNER.md`. A closer repository instruction replaces this workflow.
The playbooks (`maestro recipe show <name>`) hold the steps for each kind of
task; the skills under `~/maestro/skills/maestro-*/SKILL.md` hold the shared
procedures; this file is the single source for non-SLP method rules. Skills,
recipes, templates, and tool briefs reference these rules rather than
defining copies. The running SLP team's pinned contract remains separate.

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
  or publish. A push or an opened PR happens only when a delivery grant
  exists: the repository enabled `policy-auto-pr`, or the user granted it in
  this session and the agent recorded the quote as a `grant:` work note
  (`maestro recipe show opening-a-pr`). Merge, tag, release, and deploy
  happen only on the user's word for that action, under any grant.
- Reversible external actions proceed without asking: issue and PR comments,
  ticket updates, team chat, CI runs. An irreversible one happens only on the
  user's word for that action: a force-push to a shared branch, a deploy,
  data deletion, a message to a customer.
- Preserve the original user instruction (quote or retrievable reference),
  approved scope, target, and remaining gates in work notes and handoffs.
  A successor may continue that same authorized work after checking the
  instruction, current repo state, ownership, and any later changes or revocation.
  A bundle, decision, or agent-written claim of approval is not itself permission.
  If the original grant cannot be verified, ask before editing.
- A new session or tool alone does not require renewed approval. Expanded
  scope or an irreversible external action does. Never bypass
  host permissions, identity checks, or repository restrictions to continue.

## Decisions and readiness

The user owns product behavior, scope, important trade-offs, and permission
boundaries. The agent owns reversible implementation details within that
contract: use existing patterns, state consequential assumptions, and continue.
Ask only for a material unresolved choice or missing authority, not a method
transition. Research facts directly instead of asking the user to find them.
Classify a "which approach" fork before asking it. When running something
could answer it (behavior, timing, layout, output, performance), it is not
the user's question: sketch it with `maestro recipe show prototype` and let
the result decide. Reserve the question for a product or preference call no
experiment can settle. No is an acceptable answer: asked whether to do
something, or shown an approach, give a real judgment, including "this does
not earn its place".

Lock decisions whose rationale must survive: hard-to-reverse choices, important
trade-offs, or durable constraints. Keep routine details in the work or final
diff, not separate decision records. Supersede conflicting durable decisions
explicitly; do not silently rewrite them.

Word a safety clause in the affirmative: "X happens only when <a named party
does Y>", never "do not X while <state>". A permission must be created by an
act - the user's word, or a named party recording a proof - never by a state
ceasing, because nobody is present at the moment a condition lapses and a
conditioned prohibition becomes a permission the moment its condition does. It
is sound only when the condition's expiry coincides with the hazard's end.

The next bounded slice is ready when its outcome, scope, authority, and check
are clear and no unresolved choice blocks it. Count blockers to that slice,
not all open questions about later work. A design-only request still authorizes
no implementation, and approval of a choice is not a request to build it.

## Routing

The user supplies an outcome, not a playbook or skill name. Route internally
and keep an authorized task continuous through investigation,
implementation, and checks. Never downgrade a requested fix to analysis.
Generated or vendored files are never the target: fix the generator or pin
and regenerate.

Match the task to a playbook below and read it with
`maestro recipe show <name>`. Copy its numbered steps verbatim into the
harness plan list before any task-specific items. A step you choose not to do
stays in the list as `skip: <reason>`. The checkpoint work note names the
playbook and the current step. A quickfix needs no playbook (stated
assumption, root-cause fix, inline verification). The tier sets the record
depth; the playbook sets the steps.

| Task | Playbook |
|---|---|
| A read-only question: how X works, why Y is this way, X or Y | investigation |
| A reported defect to reproduce, root-cause, and fix; a diagnosis-only request stops at the cause | bug-fix |
| New or changed behavior | feature |
| A behavior-preserving change to structure | refactoring |
| A throwaway sketch that settles a design or an observable fork | prototype |
| A measured slowness against a baseline | perf-issue |
| Sustained improvement of one metric against a target | hillclimb |
| A live runtime symptom or a captured profiling artifact to diagnose | forensics |
| How a skill, prompt, or structure change affects agent behavior | eval |
| Writing or editing a skill | authoring-a-skill |
| Pixel-exact UI equivalence | visual-parity |
| Work spanning several units, commits, or PRs | multi-phase-plan |
| Opening or driving a PR under a delivery grant; landing on the user's word | opening-a-pr, babysit, shipping |
| A long task driven to a checkable predicate while the user is away | unattended |
| No playbook fits, or large cross-cutting work the user steps away from | figure-it-out |

Program-scale work (multi-day, many units, several writers) runs as an SLP
team (`maestro recipe show slp`); each seat runs these playbooks and fans out
to sub-agents.

Skills load by bare name in every tool and serve steps inside a playbook:

- Material unsettled decisions, design questions, or new scope needing
  clarification: `maestro-design` (grill, research, prototype, model,
  wayfind by unknown).
- A hard-to-reverse fork with wide blast radius, run by the Lead:
  `maestro-council`.
- The brief and handback for delegated work: `maestro-work`.
- Verifying and closing an open bundle: `maestro-verify`.
- A fork the user does not understand, or a recorded decision they want to
  learn from: `maestro-coach`.
- A decision owned by someone not in the conversation:
  `maestro-questionnaire`.
- Tier, bundle lifecycle, resume, and handoff: `maestro-bundle`.
- Quality review: use Per-tool adapters below. Verify owns "does it meet
  the contract"; review owns "is the code good".

## Subagents

Inside a playbook the main chat never writes code. A writing subagent
writes it; the main chat plans, briefs, reviews the diff, and verifies. The work item, its lease, and its proof stay with the main
chat. A quickfix outside any playbook is
written directly.

- Brief every subagent in maestro-work's dispatch envelope, with file
  pointers rather than pasted context. Give new work to a fresh subagent
  with consolidated scope: the original brief, every later directive, the
  prior report and branch. Resume an existing one only when the work needs
  state that lives only in it, such as an uncommitted checkout or a running
  process.
- You own every subagent's work. Review its diff and write your own summary;
  never pass its report through. A second opinion is the same brief on a
  different model family, and agreement is high-signal.
- Maestro ships no subagent definitions. Spawn one of the harness's own
  agent types, give it a written brief, and name the model in the call each
  time: the strongest model for writing code and judging a diff, a mid model
  for bulk reading and reporting, a small model for search. Panels pair
  Opus with Codex.
- Work that needs a durable seat (multi-day, owner-visible, cross-harness,
  or accepted by someone other than the main chat) runs in a Herdr pane
  through the SLP operations and leaves a store row; the seat delegates the
  same way.

## Autonomy

Reversible work proceeds without asking: present the result and let the
user correct it. "Don't stop", "going to bed", or "run until done" keeps the
run going (`maestro recipe show unattended`). Under such a grant, decide a
call the grant covers, act on it, and report it. For a call only the user
can make, apply a default, report it with the reason, and say in plain words
what the user could ask for instead; never hand back a token to type. Gates
the user named and the irreversible actions above still wait for the user.
An autonomous run fixes only what blocks its goal, as its own change; every
other discovery becomes a finding for the user, never a new work item.

## Two coordination models

Maestro keeps two coordination models with an explicit boundary. Inside a
running SLP team (a Lead or Peer pane after `team start`)
only the nine SLP v2 operations apply, and the retired verbs redirect to them.
Everywhere else (plain sessions, the Hub room, design and wayfinding sessions)
the classic surface is the live model: `work start|done`,
`decision draft|lock`, `work list`, councils, and worktree lanes.

## Bundle contract (tier Full)

- `SPEC.md` is a pure contract: Problem, Solution, Scope, Anti-goals (each
  with a matching VERIFY.md check), Decisions (store ids, rendered by
  `maestro bundle show <id>`), Units (one per commit or PR: depends on,
  files, build, you will see; `maestro recipe show multi-phase-plan`), Red
  tests (when needed by Testing discipline, not a quota or an exhaustive
  whitelist). Record newly discovered in-scope risks and checks without
  reopening design; scope expansion needs the user.
- `NOTES.md` is a pure handoff, rendered by `maestro handoff <id>` from the
  store, never authored or appended: Current State, Next Action, Authority
  transferred and retained, Failed approaches, Do not repeat,
  `Base: <commit>`, `Driver: <tool>`. Hand-edit only the placeholders it
  leaves. Re-run at handoff or a meaningful checkpoint, not every reply. History lives in
  the store: `maestro work note`, decisions; the latest
  `checkpoint:` note (state / next / avoid) is what survives a compaction.
- `VERIFY.md` is the scenario table drafted at design time and filled at
  verify time: one row per unit and per anti-goal with unit, live, and perf
  cells, each holding evidence or `n/a: <reason>`. Results hold the latest
  run only, stamped with date and commit. `maestro bundle check <id>` lints
  SPEC units and VERIFY rows before work starts and again at verify.

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
  sight. An interrupted operation has unknown outcome until checked. Do not
  redo work or re-run a repro the record already proves. A pickup reply
  lists each open thread with its status (done, in progress, blocked, not
  started) and the next move.
- Pausing (an explicit pause, going offline, imminent compaction) writes the
  checkpoint note first; a `wip:` commit happens only when commits are
  granted.
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
  a failed pass is one
  `maestro work note <id> "failed: <one line>"`.
- Name the shape: `ANTI_PATTERN.md` beside this file holds the eight named
  failure shapes of multi-agent work, and a lesson names the anti-pattern it
  shows when one fits, so corrections that rhyme collect under one word.
- Steer by name: `PRINCIPLES.md` beside this file holds the 24 judgment
  principles, one `## <Title>` section each. Read a principle's whole section
  before applying it.
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
- Falsify the test itself: would it still pass if every function it imports
  returned `undefined`? Then it checks nothing; rewrite the assertion around a
  literal observed result or delete the test.
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
- A command run as evidence shows its stderr, and a negative result from it is
  believed only after that same command has produced a positive on a case known
  to be positive. Where the claim is that nothing happened, falsify the check
  first: point it at a case you know is positive and confirm it reports that
  case. An empty result and a broken command are the same text on stdout, so a
  suppressed error reads as a clean bill for the strongest claim in the room.

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

Believe a run on the count of what actually executed - suites started, tests
run, files compiled - never on an exit code or a summary line, and treat a pass
with a suspiciously small count as the finding rather than the result. Never
read an exit status from a pipeline whose last command is not the thing under
test. Before attributing a failure to work you do not own, check the shared
generated and ignored inputs first: they change under everyone and are
invisible to git. Distrust hardest the explanation that removes your reason to
look further.

## Completion and delivery

Distinguish implementation verified, awaiting delivery approval, and delivered.
These are reported outcomes, not new work states. Every code playbook ends in
its deliver step (`maestro recipe show opening-a-pr`). `work done` means the item's
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
  delivery, keep that work open. Neither case authorizes a push without a
  delivery grant.

## Concurrency and git

One work item per tracked thread, with a bundle only when Full is warranted;
concurrent threads write only disjoint, exclusively owned paths. Dirty or
untracked content visible in the checkout
is not owned by this thread merely because it is visible; leave it alone.
Stage explicit task-owned paths only, and inspect the staged diff before
committing. A commit happens only when the user asked for it or a delivery
grant covers it; a push or a PR only under a delivery grant; merge, tag,
publish, and release only on the user's word.

Files an agent produces while discussing or researching a topic (HTML
demos, mockups, captures, sub-agent reports, inventories, plan briefs, owner
notes) go in the folder `maestro topic path <slug>` prints
(`.maestro/topic/<slug>/`, git-ignored), never in `docs/` or the source
tree, whatever precedent the repository shows; `maestro topic list` shows
them. A bundle opened for the topic links that folder from NOTES.md. Only
what the user asks to keep as documentation moves into `docs/`.

## Writing the reply

Every playbook ends with a reply written this way, a PR linked as its full
`https://github.com/<owner>/<repo>/pull/<number>` URL; each playbook's Reply
section names only what is unique to it. Write the reply clean as you draft
it; a cleanup pass afterwards does not remove
these patterns.

- Short declarative sentences, one thought each, ended with a period.
- No long-dash character anywhere. A colon goes only before a list.
- Terse never drops content: every section the playbook's Reply names stays,
  with its details, trade-offs, choices, and open decisions.
- Impact first. Name who the work is for and what changes for them before
  any implementation detail, then what the next maintainer inherits. If you
  cannot say what either would notice, the work or the explanation is off.
- Never fabricate a link, citation, or transcript reference. Link only what
  you produced or read this session.
- Every claim carries its evidence or its label in the same sentence:
  measured, inferred, or guess. A prediction or an unseen cause is a guess.
  Never hand the user a check you could run.
- Close by naming each principle that changed a decision and the choice it
  changed, citing only `PRINCIPLES.md` sections read this session.

Code comments keep only the kinds `maestro recipe show no-comments` keeps.

## Per-tool adapters

Review routing is the same in every tool, using whatever the tool ships:
Light runs a simplification pass after green, before reporting completion; Full
runs one correctness review of the frozen task-owned diff after verify
passes, a broader multi-reviewer pass when the diff touches trust boundaries,
schemas or migrations, or several subsystems, plus a security review when it
touches auth, secrets, or input handling. Any simplification pass on Full
runs after green and before verify, so the reviewed diff is final. For
each pass the holder asks one reviewer agent to review the diff. Its
findings sort into Act on, Consider, Noted, and Dismissed (the holder reads
every Dismissed reason before it accepts the verdict); the gate fails while
an Act on finding is neither fixed nor justified in writing, and an unconfirmed
security or correctness finding stays visible as Consider.

- **Claude Code**: skills load from `~/.claude/skills/maestro-*` symlinks
  that `maestro install` maintains; `/simplify` and `/code-review` are the
  built-in simplification and correctness passes. The plan list is the todo
  tool. A subagent is one of Claude Code's own agent types with the model
  named in the call. Forks go to the user
  as question cards: one decision per card, "what this does" first, a
  `my rec:` line.
- **Codex**: skills load from `~/.codex/skills/maestro-*` symlinks that
  `maestro install` maintains (restart Codex after a new link); the Full
  review runs in a fresh context. The plan list is `update_plan`. A
  sub-agent is one of Codex's own agent types with the model named in the
  call. Forks go to the user as one plain question with a
  `my rec:` line. In project work Codex reads `~/.codex/AGENTS.md`, which
  must point here; inside the Hub it reads `~/maestro/AGENTS.md`.
