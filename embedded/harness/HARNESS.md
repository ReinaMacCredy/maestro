---
version: 1.29.4
---

# Maestro Harness Protocol

You are an agent working in a repo that uses Maestro.

Maestro is a local loop harness. Tasks are executable loop units; verification
and QA are the stop hook; decisions, friction, and skills are durable memory.
Use Maestro artifacts and shipped recipes as the source of truth.

## Start

Run `maestro status` before acting. If `MAESTRO_CURRENT_TASK` is set, or status
names a current task, read it with `maestro task show <id>`. Read locked
acceptance from `maestro card show <id>` and use the active task skills.

Do not guess ids. Use only ids printed by Maestro output; when lookup misses,
re-list and read the real id.
For routine `task list` output, use the displayed `REF` for the next
`task show/start/done` command, or use `task list --json` when you need stable
Task ids.

## Route

Maestro's main workflow is the loop. Use `maestro status` for current state and
its compact loop hint, then use `maestro loop next` when routing is not obvious.
`loop next` is read-only: it recommends a recipe or uncertainty from local
artifacts and never writes cards, tasks, features, decisions, proof, QA, git,
releases, archives, or files. After choosing, read the recipe with
`maestro loop show <recipe>` and perform writes only through the existing
Maestro verbs named by the recipe.

Choose the closest shipped lifecycle recipe and stay inside its grammar:

- design / brainstorm: `maestro loop show design`
- executable work: `maestro loop show work`
- audit / review: `maestro loop show audit`
- close / ship / archive: `maestro loop show ship`
- unattended autonomy: `maestro loop show unattended`
- reusable learning: `maestro loop show learning`

If no shipped recipe fits, a custom card or run recipe must still use
perceive -> choose -> act -> observe -> learn -> continue, current Maestro
verbs, hard stops, continue output, and no skipped proof, QA, authority,
approval, or hard-stop gates.

## Command Truth

The generated `reference/cli.md` for installed or shipped Maestro skills
matching this binary is authoritative. A verb or flag not listed there does not
exist. Read the generated reference instead of probing or guessing.

## Work Model

Work has three levels: High = Card, Mid = CardKind / workflow kind, and Low =
Task. Feature, Bug, Chore, Custom, Decision, Idea, and Progress are CardKinds.
Progress stores small Low Tasks in `progress.yml`; use it through
`maestro task add/start/done/list`. The default board hides the backing
Progress card, shows current actor/session low Tasks by ordinal `REF`, and
keeps stable ids in `progress.yml` and `task list --json`.
When Maestro hooks are installed, the first write-like `PreToolUse` event in a
session automatically creates or reuses that session's Progress Task and starts
it unless `MAESTRO_CURRENT_TASK` is already set. Read-only hooks do not create
Progress rows. If hooks are unavailable, run `maestro task add` and
`maestro task start` yourself before editing.

Linked-card inbox messages are advisory coordination signals only. They do not
block execution. When order matters, record an explicit Task blocker or
dependency; readiness, next, claim, and verification gates consult Task
blockers, not messages or unread state.

## Proof And Corrections

Complete executable work with `maestro task complete` using summary, claim, and
proof. Maestro records the proof and runs verification.
For low-ceremony Progress Tasks, close the row with
`maestro task done <ref> --proof "<evidence>"`; proof is required there too.

Hooks auto-record tool calls as proof. Verification matches each `--claim`
against recorded or inline proof. Empty or unbacked claims fail. When proof or
verification fails, use the active recipe or `maestro task proof`.

When the user corrects your behavior, record it:

`maestro event intervention --note "<what was wrong>"`

## Design

For brainstorm or unsettled behavior, use the design loop. Map the problem from
real code and artifacts, then walk open questions one at a time. Lock each
decision and record the corresponding note. Do not batch-decide independent
forks, edit locked decisions in place, or cross into implementation before the
user approves build.

Before proposing an idea or reopening a settled question, search precedent with
`maestro grep "<topic> corpus:memory"` and cite the best matching card, decision,
task, proof, or note. Use `maestro card list --grep <topic> --archived` only for
exact legacy rows, compatibility checks, or when unified grep is too broad or
surprising.

## Concurrency

The card store is shared state. In fan-out work, the orchestrator performs
store-mutating verbs such as decision lock, task complete, verification, status,
and notes. Sub-agents return results as data unless they have isolated stores.

Serialize overlapping writers through the orchestrator, or use a separate git
worktree when sessions may edit overlapping code or the shared card store. Use
`maestro active`, `[overlap]`, `[CONFLICT]`, and `[busy]` notices to coordinate.
For full conflict handling, run `maestro loop show conflict-handoff`.

A failed multi-file store command can be partial. Re-run it so Maestro reads the
latest store and reapplies the intended change.

## Harness Improvement

Passive friction backlog: `maestro harness list / apply / measure`.

When status, next, or complete surfaces over-threshold friction, apply and claim
it before new work, or dismiss it with a reason when it is noise. The binary
counts and shows friction; the agent acts.
