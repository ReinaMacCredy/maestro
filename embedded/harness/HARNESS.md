---
version: 1.29.15
---

# Maestro Harness Protocol

Use local Maestro artifacts as source of truth. This is a router: status ->
route -> act -> proof -> learn.

## Start

Run `maestro status` before acting. If status or `MAESTRO_CURRENT_TASK` names a
current task, read `maestro task show <id>`. Read locked acceptance with
`maestro card show <id>` and use active task skills. Do not guess ids: use
printed ids, routine `task list` REF values, or `task list --json`. The generated
`reference/cli.md` for installed/shipped skills matching this binary is
authoritative; unlisted verbs or flags do not exist.

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
When the user is unavailable but has provided a bounded design mandate, use
`maestro loop show design-relay`: the main session may make only in-mandate
design decisions, subagents/advisors provide evidence only, and the relay must
return to the parent design loop.
If no shipped recipe fits, custom card/run recipes still use perceive -> choose
-> act -> observe -> learn -> continue, current Maestro verbs, hard stops,
continue output, and no skipped proof, QA, authority, approval, or hard-stop
gates.

## Work + Proof

Work levels: High = Card, Mid = CardKind/workflow kind, Low = Task. Use
Progress through `maestro task add/start/done/list` and displayed REF values.
Before write-like work, create a visible Progress breakdown with
`maestro task setup --task ... --start`: at least two rows, or one row only with
`--atomic --reason "<why one row is enough>"`. `MAESTRO_CURRENT_TASK` does not
bypass this.
Design-to-card gate: before executable work after design/brainstorm, ask:
- Am I coming from design or brainstorm?
- What card/feature owns this work?
- Is that card/feature handoff finalized and fresh?
If design started and ownership/fresh handoff is missing, stop before creating
Progress rows, running `feature prepare`, editing source, or running tests. Bind
standalone chat or Decision records to a Feature/card and refresh the handoff.
Do not let Progress tasks or source edits implicitly end the design phase.
Canonical work readiness is `maestro ready`: a task-wave projection from the
Task DAG. It shows the parallel executable wave, ready serial gates, and the
bounded blocked-next frontier. `maestro loop next` uses that projection and
does not create a second scheduler. `maestro card ready` is the explicit legacy
card-board readiness surface.
Complete executable work with `maestro task complete` using summary, claim, and
proof. Close Progress rows with `maestro task done <ref> --proof "<evidence>"`.
Verification matches each `--claim` against recorded/inline proof; empty claims
fail. Repair proof/verification failures with the active recipe or
`maestro task proof`. Corrections: `maestro event intervention --note "<what was wrong>"`.

## Design + Coordination

For brainstorm/unsettled behavior, use the design loop: map real code/artifacts,
ask one question at a time, lock each decision, record the note, and do not
implement until build is approved. Do not batch independent forks or edit locked
decisions.
Anti-MVP scope authority: if the user says anti-MVP, full, deep, complete,
make one forever, full framework, or rejects MVP, treat Full Durable Design as
the scope authority. Do not offer MVP, first-slice, or reduced product scope
unless the user explicitly asks for MVP. Stage the build, proof, or delivery
when needed; do not shrink the design target.
For "lock all", "all rec", or "all-recommendations", preserve each fork as a
DecisionSet child: use `maestro decision set draft` /
`maestro decision set lock`, or separate child decisions. Never compress to one
`maestro decision lock`; repair with `maestro decision audit --compressed` then
`maestro decision set repair`. Keep separate child decisions visible.
Before new/reopened ideas, search `maestro grep "<topic> corpus:memory"` and
cite the best card, decision, task, proof, or note. Use
`maestro card list --grep <topic> --archived` only for exact legacy rows or
compatibility checks.
Inbox messages are advisory. If order matters, record a Task blocker/dependency;
readiness, next, claim, and verification gates use blockers, not messages.
The card store is shared state. In fan-out, the orchestrator owns store writes;
sub-agents return data unless isolated. Use worktrees for overlapping code/store
writes. Coordinate with `maestro active`, `[overlap]`, `[CONFLICT]`, `[busy]`,
and `maestro loop show conflict-handoff`. If a multi-file store command fails,
re-run it so Maestro rereads current state and reapplies the change.

## Harness Improvement

Passive friction backlog: `maestro harness list / apply / measure`. When status,
next, or complete surfaces over-threshold friction, apply and claim it before
new work, or dismiss it with a reason when it is noise.
