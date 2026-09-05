---
name: maestro-bundle
description: Route work into the right maestro tier and drive the SPEC/NOTES/VERIFY bundle lifecycle - open, resume, close, recall.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-bundle

Routing brain for maestro method work. Decide the tier first, then follow the
matching skill; this skill owns the bundle lifecycle itself.

## Tier rule

Decide the tier from the request alone, before any recon: no bundle files, no
code reading, no store lookups until the tier is known.

- quickfix: the diff fits in one sentence and hits no Full trigger below. Do it
  directly, verify inline (run the smallest check that can falsify it), no
  skill, no record. If it grows past one sentence, stop, `maestro work add`,
  and continue as Light.
- Light: one session, one branch, and the acceptance fits in a sentence. Work
  directly with `maestro work add|start|done` (no bundle). The work item is the
  floor because `maestro ready`, attention, and the Supervisor's brief read the
  store, not a session task list.
- Full: any trigger below. Open a bundle, in the
  store whose checkout will change: a design walk run in the Hub room still
  builds where the code lives. List Hub decisions in the SPEC as `hub:<id>`
  (`maestro bundle show` renders them) and note the bundle on the Hub map and
  the map on the bundle's work item.

Open a bundle when ANY trigger holds:
- the work spans multiple sessions or must survive a context reset
- multiple branches, worktrees, or agents touch the same scope
- the scope is high risk (schema change, wide refactor, irreversible step)
- a previous fix attempt for the same problem failed
- the user asks for one

Ceremony scales with tier. Quickfix and Light never demand a SPEC or a test;
Red tests are a Full-tier instrument for the risks the SPEC names, and nothing
beyond that list. Light work that hits a Full trigger mid-task opens the
bundle then and backfills it from what is already done; finished work is
evidence, never redone. During a production incident, fix and verify first,
then backfill and close the bundle immediately after stabilizing.

```
maestro bundle open <id> --work <workId>   # scaffold SPEC/NOTES/VERIFY, link work
```

## The trio contract

- `SPEC.md` is a pure contract: Problem, Solution, Scope, Anti-goals,
  Decisions, Red tests. Mid-flight decisions are NOT written into SPEC prose;
  record them with `maestro decision draft "<text>" --rationale "<why>" --work <id>`
  and list the ids under Decisions (`maestro bundle show <id>` renders them
  with ruling and rejected alternative). Every anti-goal gets a matching
  VERIFY.md check. Red tests exist only in Full bundles and only for the risks
  the SPEC names; the section stays empty when the SPEC names no risk. Revise
  in place; scope expansion needs the user.
- `NOTES.md` is a pure handoff: current state, next action, base commit.
  It also names Authority transferred and retained, Failed approaches, and Do
  not repeat. Overwrite it; never append. History lives in `maestro trace`,
  work notes, and decisions. Refresh it before ending any turn with work
  remaining.
- `VERIFY.md` is scenarios + results; each scenario points at a work item's
  acceptance or claim instead of restating it. Results hold the latest run
  only, stamped with date and commit.

A bundle closes when its work ships, is handed off, is cancelled, or the
investigation concludes no change is needed. Shipped or dead bundles never
stay active; an archived bundle is never reopened, follow-up work gets a new
bundle that links the old one. Never create root `SPEC-*`, `NOTES-*`, or
`VERIFY-*` files.

## Verbs

```
maestro bundle open <id> [--work <id>]  # scaffold + active row
maestro bundle close <id>               # snapshot trio into the store, archive
maestro bundle pause <id> [--reason]    # active stays open, out of the way
maestro bundle resume <id>              # paused back to active
maestro bundle list                     # states: active | paused | archived
maestro bundle show <id>                # trio + linked work + decisions
maestro bundle save <dir>               # ingest a foreign trio dir as archived
maestro search "<term>"                 # recall: hits labeled (bundle, ...)
```

## Authorization boundaries

- A request to design or plan does not authorize production edits.
- An explicit request to implement or fix authorizes only that stated scope.
- Read-only requests (answer, review, report, diagnose, explore) stay
  read-only.
- Workflow ownership never grants authority to push, merge, release, deploy,
  publish, or mutate external systems.
- Authorization does not travel with a bundle between tools or sessions: the
  receiving session needs the user's explicit ask before it edits.

## Routing

Route only work that needs a skill; a quickfix proceeds directly.

- Unsettled decisions, design questions, efforts too big for one session, or
  new scope on shipped work (review findings, follow-ups): `maestro-design`.
- Research, disposable prototype, or current-behavior baseline:
  `maestro-explore`.
- Authorized implementation or fix: `maestro-work` (a Light fix proceeds
  there directly; Full needs the bundle and its red list).
- Diagnosis-only work: `maestro-diagnose`; on a fix request, diagnosis is the
  first phase of `maestro-work`, not a separate engagement.
- Verifying and closing an open bundle: `maestro-verify`.
- The user does not understand a fork they have been asked, or wants to learn
  the concept behind a recorded decision: `maestro-coach`.
- A decision owned by someone not in the conversation:
  `maestro-questionnaire`.
- Filed lessons into the smallest doctrine edit: `maestro-improve`.

## Resume protocol

On resume, `maestro bundle list` first; pick the bundle matching this task.
Two plausible matches is a scope collision: ask. Read its NOTES.md, then
`maestro bundle show` for linked work and decisions. Never trust
conversational memory over the bundle; the files and the store are the spec.

Reconcile NOTES against live repo state before the first edit:
`git log <Base>..HEAD` and `git status` show what happened behind NOTES' back
(another tool or session may have driven the bundle meanwhile). Repo state
beats NOTES, NOTES beats memory. A bundle whose scope already shipped is
closed on sight, not resumed. An interrupted operation has unknown outcome
until checked.

## Concurrency and git

One work item and bundle per thread; concurrent threads write only disjoint,
exclusively owned paths. Dirty or untracked content visible in the checkout
is not owned by this thread merely because it is visible: leave it alone.
Stage explicit task-owned paths only, never bundle contents, and inspect the
staged diff before committing.

## Compact or hand off

Hand off instead of compacting when:

- the owner changes
- a dependency becomes its own branch
- the role changes
- the context is full of false starts

Compact only when ownership, scope, and role stay stable and the history still
helps the same writer continue.

Use break-before-make when the writer on a moving scope changes: release the
lease and overwrite NOTES.md before the new session starts. The handoff packet
must preserve the base, Current State, Next Action, Authority transferred and
retained, Failed approaches, and Do not repeat.

## Hand-off

Run `maestro handoff <bundle-id>` to seed untouched NOTES.md sections before transferring ownership.
Then decide which of three cases this is; the destination differs:

1. **Continuation with a bundle.** A future session, in any tool, continues
   this bundle in this workspace. The handoff IS the NOTES.md overwrite
   covering every section the trio contract names, with a `Driver:` line
   naming the tool expected to resume. The bundle stays active; the next
   session's resume protocol must find it.
2. **Transfer.** The work leaves this workspace, to a person, another repo,
   or an agent that will not resume the bundle. Write the standalone document
   below, then close the bundle per `maestro-verify`'s close order, citing
   the handoff target.
3. **No bundle.** If the work continues in this workspace it now spans
   sessions, a Full trigger: open a bundle and use case 1. Otherwise write
   the standalone document to the OS temporary directory, not the workspace,
   and report the path.

The standalone document (cases 2 and 3) names the suggested skills for the
next agent, references specs, decisions, commits, and diffs by path or id
instead of duplicating them, and redacts secrets and personal data.

Design lane -> use the `maestro-design` skill. Implementation -> `maestro-work`.
Verification and close -> `maestro-verify`.
