---
version: 1.15.0
---

# Maestro Harness Protocol

You are an agent working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Start with `maestro status`; honor MAESTRO_CURRENT_TASK env or `maestro task show <id>` when a current task is set.
2. Acceptance criteria live in the card (`maestro show <id>`) - they are locked.
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
9. Before proposing an idea or re-opening a settled question, run `maestro list --grep <topic> --archived` and cite any precedent card in the proposal.

## Code style

Universal principles for code written here. Project-specific
conventions live in this repo AGENTS.md, not here.

- Simplest thing that works; no speculative abstraction or
  one-caller indirection.
- Names state intent (what/why), not type or mechanism.
- Validate at trust boundaries only; do not guard impossible states.
- Errors fail loud and early with actionable context; no silent catch.
- Match the surrounding file style, idioms, and comment density.
- Comment the non-obvious why, sparingly; names carry the what.
- Every changed line traces to the task; no drive-by edits.

Per-language styleguides live in `.maestro/playbook/<lang>.md` (Rust, Python,
Go, TypeScript, JavaScript, C++, C#, Dart, HTML/CSS, plus `general.md`). Before
writing or changing code, read the one file for the language you are editing;
do not load the rest.

## Where to look

    starting a session     -> maestro status
    picking up work        -> maestro-card skill (work)
    brainstorm / design    -> maestro-design skill
    proof failed / verify  -> maestro-card skill (verify)
    before accept / ship   -> maestro-card skill (qa-baseline / qa-slice)

## Task commands (the loop)

State flow: draft -> exploring -> ready -> in_progress -> needs_verification -> verified
(reject -> rejected; abandon / supersede are terminal; block overlays any state)

Orient and find work:

    maestro status        # repo handoff and next action
    maestro task next     # one best task action
    maestro ready         # claimable work (ready + unblocked)
    maestro task show     # task detail: state, claim, blockers

Make a task claimable (intake):

    maestro task create   # -> draft; seed --check with the observable result
    maestro task explore  # -> exploring
    maestro task accept   # locks acceptance -> ready

Execute:

    maestro task claim    # -> in_progress
    maestro task update   # record evidence claims as you work
    maestro task complete # summary + claim + proof; auto-verifies
    maestro query proof   # recovery path when verification fails

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
independent. The recipes live in the skills; this is the menu:

    2+ independent ready tasks on a feature  -> feature fan-out      (maestro-card)
    contested / high-stakes verification     -> adversarial fan-out  (maestro-card)
    taste-based design fork (naming, UX)     -> generate-and-filter  (maestro-design)
    unstructured backlog to triage           -> intake triage        (maestro-card)
    unknown amount of work                   -> loop until done      (maestro-card)
    user away/asleep, backlog to work        -> unattended loop      (maestro-card)

Results land through the verbs (task / decision / event), never only in conversation.
Claude Code: author a Workflow script. Codex: parallel sub-agents directly (worktree
threads when files overlap).
