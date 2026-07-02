---
version: 1.29.11
---

# Maestro Harness Protocol

Use local Maestro artifacts as source of truth. This is a router: status ->
route -> act -> proof -> learn.

## Start

Run `maestro status` before acting. If status or `MAESTRO_CURRENT_TASK` names a
current task, read `maestro task show <id>`. Read locked acceptance with
`maestro card show <id>` and use active task skills. Do not guess ids: use
printed ids, routine `task list` REF values, or `task list --json`.

## Route

Maestro's main workflow is the loop. Use `maestro status` for current state and
`maestro loop next` when routing is unclear. `loop next` is read-only: it
recommends from local artifacts and never writes cards, tasks, features,
decisions, proof, QA, git, releases, archives, or files. Read
`maestro loop show <recipe>` and write only through existing Maestro verbs.

Rule: loop next recommends; outcome/proof/memory verbs write. Use
`maestro loop outcome` after action/proof/repair. Use `maestro loop improve` for
read-only proposals; apply only the explicit memory, harness, proof, or QA
command it prints. No hidden stores, hidden schedulers, silent recipe mutation,
or proof/QA bypass.

Use the closest shipped lifecycle recipe: `maestro loop show design`,
`maestro loop show work`, `maestro loop show audit`, `maestro loop show ship`,
`maestro loop show unattended`, or `maestro loop show learning`.

If no shipped recipe fits, custom card/run recipes still use perceive -> choose
-> act -> observe -> learn -> continue, current Maestro verbs, hard stops,
continue output, and no skipped proof, QA, authority, approval, or hard-stop
gates.

## Command Truth

The generated `reference/cli.md` for installed or shipped Maestro skills
matching this binary is authoritative. Unlisted verbs or flags do not exist.

## Work Model

Work levels: High = Card, Mid = CardKind/workflow kind, Low = Task. Use
Progress through `maestro task add/start/done/list` and displayed REF values.

Before write-like work, create a visible Progress breakdown with
`maestro task setup --task ... --start`: at least two rows, or one row only with
`--atomic --reason "<why one row is enough>"`. `MAESTRO_CURRENT_TASK` does not
bypass this.

Design-to-card gate: before executable work after a design or brainstorm
session, ask:

- Am I coming from design or brainstorm?
- What card/feature owns this work?
- Is that card/feature handoff finalized and fresh?

If design started and the owning card/feature or fresh handoff is missing, stop
before creating Progress rows, running `feature prepare`, editing source, or
running tests. Bind standalone chat or Decision records to a Feature/card and
refresh the handoff. Do not let Progress tasks or source edits implicitly end the
design phase.

Canonical work readiness is `maestro ready`: a task-wave projection from the
Task DAG. It shows the parallel executable wave, ready serial gates, and the
bounded blocked-next frontier. `maestro loop next` uses that projection and does
not create a second scheduler. `maestro card ready` is the explicit legacy
card-board readiness surface.

## Proof And Corrections

Complete executable work with `maestro task complete` using summary, claim, and
proof. Close Progress rows with `maestro task done <ref> --proof "<evidence>"`.

Verification matches each `--claim` against recorded or inline proof. Empty
claims fail. Repair proof/verification failures with the active recipe or
`maestro task proof`.

When the user corrects your behavior, record it:

`maestro event intervention --note "<what was wrong>"`

## Design

For brainstorm or unsettled behavior, use the design loop. Map real code and
artifacts, ask one open question at a time, lock each decision, and record the
note. Do not batch-decide independent forks, edit locked decisions in place, or
cross into implementation before the user approves build.

When the user says "lock all", "all rec", "all-recommendations", or otherwise
settles multiple forks at once, preserve each fork as its own DecisionSet child
record. Use `maestro decision set draft` / `maestro decision set lock` for the
batch, or lock separate child decisions. Never compress a batch into one summary
`maestro decision lock`; if one exists, run `maestro decision audit --compressed`
and repair it with `maestro decision set repair`.

Before proposing an idea or reopening a settled question, search precedent with
`maestro grep "<topic> corpus:memory"` and cite the best matching card, decision,
task, proof, or note. Use `maestro card list --grep <topic> --archived` only for
exact legacy rows or compatibility checks.

## Coordination

Linked-card inbox messages are advisory only. When order matters, record an
explicit Task blocker or dependency; readiness, next, claim, and verification
gates consult Task blockers, not messages or unread state.

The card store is shared state. In fan-out, the orchestrator performs
store-mutating verbs; sub-agents return data unless isolated. Use worktrees for
overlapping code or shared-store writes. Coordinate with `maestro active`,
`[overlap]`, `[CONFLICT]`, `[busy]`, and `maestro loop show conflict-handoff`.

A failed multi-file store command can be partial. Re-run it so Maestro reads the
latest store and reapplies the intended change.

## Harness Improvement

Passive friction backlog: `maestro harness list / apply / measure`. When status,
next, or complete surfaces over-threshold friction, apply and claim it before
new work, or dismiss it with a reason when it is noise.
