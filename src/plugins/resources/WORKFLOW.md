# Maestro workflow: design, work, verify

The method behind maestro, shared across coding agents (Claude Code, Codex,
and any tool that can read this file). Installed to `~/maestro/WORKFLOW.md`
by `maestro install` and overwritten on every install; owner notes belong in
`~/maestro/OWNER.md`. A closer repository instruction replaces this workflow.
The skills under `~/maestro/skills/maestro-*/SKILL.md` hold the procedures;
this file holds the rules that bind all of them.

## Tiers

Decide the tier from the request alone, before any recon.

- **quickfix**: the diff fits in one sentence and hits no Full trigger. Do it
  directly, verify inline with the smallest check that can falsify it, no
  skill, no record, no test demanded. If it grows past a sentence, `maestro
  work add` and continue as Light.
- **Light, the default**: one session, one branch, acceptance in a sentence.
  `maestro work add|start|done` is the whole record; state assumptions,
  verify the changed surface before claiming done. No bundle, no SPEC, no
  red-test list; a regression test only for a real bug being fixed.
- **Full**: open a bundle when the work spans sessions or concurrent threads,
  carries risky or destructive scope, retries a previously failed fix, or the
  user asks for one. `maestro bundle open <id> --work <workId>` scaffolds
  `.maestro/bundle/<id>/` with `SPEC.md`, `NOTES.md`, `VERIFY.md`.

Ceremony scales with tier: Red tests are a Full-tier instrument for the risks
the SPEC names and nothing beyond that list. Light work that hits a Full
trigger mid-task opens the bundle then and backfills it from what is already
done. During a production incident, fix and verify first; backfill and close
the bundle immediately after stabilizing.

## Authorization boundaries

- A request to design or plan does not authorize production edits.
- An explicit request to implement or fix authorizes only that stated scope.
- Read-only requests (answer, review, report, diagnose, explore) stay
  read-only.
- Workflow ownership never grants authority to push, merge, release, deploy,
  publish, or mutate external systems.
- Authorization does not travel between tools or sessions; the receiving
  session needs the user's explicit ask before it edits.

## Routing

Skills load by bare name in every tool (`maestro-design`, `maestro-work`,
`maestro-verify`, `maestro-bundle`, `maestro-explore`, `maestro-diagnose`,
`maestro-coach`, `maestro-questionnaire`, `maestro-improve`). Route only work
that needs one: a quickfix or a Light production fix proceeds directly
(stated assumption, root-cause fix, inline verification). Never downgrade a
requested fix to analysis. Generated or vendored files are never the target:
fix the generator or pin and regenerate.

- Unsettled decisions, design questions, efforts too big for one session, new
  scope on shipped work: `maestro-design` (grill, research, prototype, model,
  wayfind by unknown).
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
- Quality review, with whatever tooling the tool provides: Light gets a
  simplification pass after green; Full gets one correctness review after
  verify passes. Verify owns "does it meet the contract"; review owns "is the
  code good".

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
  `maestro bundle show <id>`), Red tests (named risks only). Revise in place;
  scope expansion needs the user.
- `NOTES.md` is a pure handoff, overwritten wholesale, never a log: Current
  State, Next Action, Authority transferred and retained, Failed approaches,
  Do not repeat, `Base: <commit>`, `Driver: <tool>`. Refresh before ending any
  turn with work remaining. History lives in the store: `maestro work note`,
  `maestro trace`, decisions.
- `VERIFY.md` is the scenario table drafted at design time and filled at
  verify time. Results hold the latest run only, stamped with date and commit.

A bundle closes when its work ships, is handed off, is cancelled, or the
investigation concludes no change is needed: `maestro bundle close <id>`
snapshots the trio into the store. Archived bundles are never reopened;
follow-up work gets a new bundle that links the old one. Bundle directories
are never staged or committed.

## Resume

- Resume from the store and the bundle, never conversational memory alone:
  `maestro bundle list`, pick the match, read NOTES.md, then
  `maestro bundle show`. Two plausible matches is a scope collision: ask.
- Reconcile NOTES against live repo state first: `git log <Base>..HEAD` and
  `git status` show what happened behind NOTES' back. Repo state beats NOTES,
  NOTES beats memory. A bundle whose scope already shipped is closed on
  sight. An interrupted operation has unknown outcome until checked.
- Bundles move between tools through their files and the store alone:
  refresh NOTES.md before switching, and re-check NOTES and `git status`
  before the first edit of each turn.

## Memory

- Recall: `maestro search "<term>"` hits work, decisions, notes, terms, and
  archived bundles in the repo store; the Hub index is `~/maestro/MEMORY.md`,
  written by `maestro memory render` and imported by each tool's global
  instruction file. Cite what you use; re-verify fast-drifting
  facts (branches, ports, dirty state, test counts) live.
- Record: a settled fork is a locked decision with its rejected alternative;
  a domain term is `maestro term add`; a failed pass is one
  `maestro work note <id> "failed: <one line>"`.
- Per-tool stores (Claude auto-memory under `~/.claude/projects/*/memory/`,
  Codex `~/.codex/memories/`) are write buffers their tools keep writing.
  Promotion into the Hub store is `maestro memory ingest` (dry-run first),
  which like every memory verb runs from any cwd and re-renders the index;
  only the Hub render is injected. No hand-edited global index.

## Testing discipline

In a Full bundle: one failing test per named risk, written before the
implementation at a seam a decision locked, or a VERIFY.md scenario where no
executable seam exists, or a captured baseline for behavior-preserving
change. Nothing beyond that list. Never delete, skip, or weaken a failing
test to make a suite pass. `maestro-work` holds the loop.

## Concurrency and git

One work item and bundle per thread; concurrent threads write only disjoint,
exclusively owned paths. Dirty or untracked content visible in the checkout
is not owned by this thread merely because it is visible; leave it alone.
Stage explicit task-owned paths only, and inspect the staged diff before
committing. Commit or push only when asked; push, tag, publish, and release
are the user's gates.

## Per-tool adapters

Review routing is the same in every tool, using whatever the tool ships:
Light runs a simplification pass after green, before the final commit; Full
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
