---
version: 1.27.0
---

# Maestro Harness Protocol

You are an agent working in a repo that
uses Maestro. Follow these rules.

Maestro is a loop harness: low-level Tasks are the executable loop; verify + qa
are the stop hook (no unbacked claim lands); decisions + friction + skills are
the compounding memory; `maestro loop` lists the orchestration recipes.

Work has three levels: High = Card, Mid = CardKind / workflow kind, and Low =
Task. Feature, Bug, Chore, Custom, Decision, Idea, and Progress are CardKinds,
not separate high-level objects. Progress is a lightweight CardKind that stores
many Low Tasks in `progress.yml`; use it through `maestro task
add/start/done/list` for small work. Tasks are executable units, not target
CardTypes; legacy `type: task` cards remain readable for compatibility.

## Shared protocol (all agents)
1. Start with `maestro status`; honor MAESTRO_CURRENT_TASK env or `maestro task show <id>` when a current task is set.
2. Acceptance criteria live in the card (`maestro card show <id>`) - they are locked.
3. Use the skills active for this task.
4. Exact command signatures live in `reference/cli.md` inside every installed
   maestro skill (e.g. `.maestro/skills/maestro-card/reference/cli.md`),
   generated from this binary. A verb or flag not listed there does not
   exist; read it instead of probing `--help`.
5. Never chain a guessed id: use only ids read from verb output. When a
   lookup misses, re-list and read the real id; do not retry spelling
   variations.
6. Complete tasks with `maestro task complete` (summary, claim, and proof);
   Maestro records the proof and auto-runs verification.
7. Hooks auto-record your tool calls as proof. Verification matches each `--claim` against recorded or inline proof - an empty or unbacked claim fails.
8. When the user corrects your behavior, record it with
   `maestro event intervention --note "<what was wrong>"`.
9. Before proposing an idea or re-opening a settled question, search precedent
   with `maestro grep "<topic> corpus:memory"` and cite the best matching card,
   decision, task, proof, or note. Use `maestro card list --grep <topic>
   --archived` only when the user asks for exact card rows, you are verifying
   legacy card-list behavior, or unified grep is too broad or surprising.
10. Linked-card inbox messages are advisory coordination signals only. They may
    suggest a cross-card task order, but they do not block execution. When order
    matters, record an explicit Task blocker/dependency; readiness, next, claim,
    and verification gates consult Task blockers, not messages or unread state.

## Code style

Universal principles for code written here. Project-specific
conventions live in this repo AGENTS.md, not here.

- Simplest thing that works; no speculative abstraction or
  one-caller indirection.
- Lean reach-ladder, active every session and every response (still
  active when unsure; suspended only when the user says so). Before
  and while writing, climb to the lowest rung that holds and stop:
  skip/YAGNI -> stdlib -> native platform (a DB constraint over app
  code, CSS over JS) -> installed dependency -> one-liner -> minimal
  new code. It is a reflex, not a research project; if two rungs both
  work, take the higher one. Strictness follows your `maestro lean`
  mode: full applies the cheaper version, ultra rejects what a lower
  rung already covers, lite names the lazier alternative, off suspends
  the climb. Never simplify away validation, error handling, security,
  accessibility, or anything explicitly requested; non-trivial logic
  still leaves one runnable check. The ladder governs what you build,
  not how you explain it. `maestro lean review|audit` walk a diff or
  the tree against the ladder.
- Names state intent (what/why), not type or mechanism.
- Validate at trust boundaries only; do not guard impossible states.
- Errors fail loud and early with actionable context; no silent catch.
- Match the surrounding file style, idioms, and comment density.
- Comment the non-obvious why, sparingly; names carry the what.
- Every changed line traces to the task; no drive-by edits.

Per-language styleguides are served by `maestro playbook <lang>` (Rust, Python,
Go, TypeScript, JavaScript, C++, C#, Dart, HTML/CSS, plus `general`). Before
writing or changing code, run it for the language you are editing; run
`maestro playbook` with no language to list the guides. Read the one you need,
not the rest.

For frontend or UI work, read the repository-root `DESIGN.md` when it exists
before changing the interface. If it is missing and visual direction materially
matters, run `maestro design init --dry-run` and ask or record the chosen style
before creating it. In proof or handoff, state that the UI matches `DESIGN.md`
or name the intentional deviation.

## Where to look

    starting a session     -> maestro status
    picking up work        -> maestro-card skill (work)
    brainstorm / design    -> maestro-design skill
    proof failed / verify  -> maestro-card skill (verify)
    before accept / close  -> maestro-card skill (qa-baseline / qa-slice)
    backlog / unattended   -> maestro loop show unattended-loop (keep going in place; follow imperatively)

## Task commands (the loop)

State flow: draft -> exploring -> ready -> in_progress -> needs_verification -> verified
(reject -> rejected; abandon / supersede are terminal; block overlays any state)

Orient and find work:

    maestro status        # repo handoff and next action
    maestro task next     # one best task action
    maestro card ready    # claimable work (ready + unblocked)
    maestro task show     # task detail: state, claim, blockers

Make a task claimable (intake):

    maestro task create   # -> draft; seed --check with the observable result
    maestro task explore  # -> exploring
    maestro task accept   # locks acceptance -> ready

Small work without full card ceremony:

    maestro task add      # creates/reuses a Progress card, appends a ready Task
    maestro task start    # -> in_progress, records ownership
    maestro task done     # simple claims-only verification when no gate is attached

Execute:

    maestro task claim    # -> in_progress
    maestro task update   # record evidence claims as you work
    maestro task complete # summary + claim + proof; auto-verifies
    maestro task proof    # recovery path when verification fails

When stuck:

    maestro task block    # --reason why; --by names the blocking card
    maestro task unblock  # pass the blocker's own blk- id, not the target

Terminal verbs (reject / abandon / supersede), plus doctor and watch -> see the maestro-card skill (work reference).

## Design work (the brainstorm loop)

Design lands as a feature while `proposed`. Map the problem from real code, then walk
open questions ONE at a time - lock each as a decision + a notes.md line, never batch-decide.

    maestro feature new       # scaffolds notes.md
    maestro feature set       # --description carries the problem, --question each open question
    maestro decision new      # open the fork; lock it, or --lock for a pre-decided one
    maestro feature show      # resume point
    maestro feature set       # then author the contract: acceptance + areas

Full method -> the maestro-design skill; lifecycle -> maestro-card (feature reference).

## Harness self-improvement

Passive friction backlog: `maestro harness list / apply / measure`.
Over-threshold friction surfaced by status/next/complete: apply and claim it before new work, or dismiss it with a reason when noise.
Binary only counts and shows; the agent acts. Full method -> the maestro-card skill (work reference).

## Orchestration (when work can fan out)

Parallel sub-agents pay off when contexts must stay clean or work is
independent. Each recipe's full HOW: `maestro loop show <name>`. The menu:

    2+ independent ready tasks on a feature  -> feature-fan-out
    contested / high-stakes verification     -> adversarial-fan-out
    taste-based design fork (naming, UX)     -> generate-and-filter
    unstructured backlog to triage           -> intake-triage
    unknown amount of work                   -> loop-until-done
    user away/asleep, backlog to work        -> unattended-loop

Results land through the verbs (task / decision / event), never only in conversation.
The card store is shared state every session writes, so in a fan-out the orchestrator
runs the store-mutating verbs (decision lock, task complete/verify, status, notes);
sub-agents return results as data, they do not each write the store. Two sub-agents
locking decisions on one feature collide on the shared decisions.yaml -- serialize
through the orchestrator, or give a genuinely-parallel writer its own worktree (the
store is git-tracked, merges back via conflict-handoff) even when its code files do
not overlap. A failed multi-file store command is partial: re-run it, it re-reads the
latest store and reapplies (AGENTS.md concurrent-agent contract).
Claude Code: author a Workflow script. Codex: parallel sub-agents directly (worktree
threads when files -- or the card store -- overlap).

## Concurrent sessions (conflict handoff)

maestro is a noticeboard: it surfaces who else is live but never runs git, makes
a worktree, or links cards -- you do. `maestro active` and the pre-command
`[overlap]` / `[CONFLICT]` / `[busy]` banners are how other live sessions show up.

When any other session is live as you cross from design into implementation
(`feature accept` / `prepare` print a `[worktree]` nudge):

1. Isolate -- implement in your own `git worktree` so two sessions never clobber
   one checkout.
2. Share a file with a peer? Link the cards (`maestro link add <yours> <theirs>`)
   and assert it (`maestro conflict <peer-card> "<file: why>"`) so the peer sees
   a `[CONFLICT]` naming you.
3. Peer-side: while a `[CONFLICT]` names a file, hold off editing that file until
   it clears.
4. Done -- you run the git merge back to the shared branch, then
   `maestro conflict --clear <peer-card>`. A stale asserter's notice auto-hides,
   but clear yours when you resolve.
5. Heavy runs serialize themselves: the full-suite gate takes a shared lock, so a
   second gate run waits (you see `[busy]`); let it finish rather than forcing a
   parallel suite.

Full worktree-split / merge-back dance, including a merge-back that itself
conflicts: `maestro loop show conflict-handoff`.
