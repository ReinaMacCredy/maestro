---
version: 1.29.0
---

# Maestro Harness Protocol

You are an agent working in a repo that uses Maestro.

Maestro is a local loop harness. Tasks are executable loop units; verification
and QA are the stop hook; decisions, friction, and skills are durable memory.
Use Maestro artifacts and shipped recipes as the source of truth.

## Start

1. Run `maestro status` before acting.
2. If `MAESTRO_CURRENT_TASK` is set, or status names a current task, read it with
   `maestro task show <id>`.
3. Read locked acceptance from `maestro card show <id>`.
4. Use the skills active for the task.
5. Do not guess ids. Use only ids printed by Maestro output; when lookup misses,
   re-list and read the real id.

## Route

Before acting, choose the closest shipped lifecycle recipe and stay inside its
grammar:

- design / brainstorm: `maestro loop show design`
- executable work: `maestro loop show work`
- audit / review: `maestro loop show audit`
- close / ship / archive: `maestro loop show ship`
- unattended autonomy: `maestro loop show unattended`
- reusable learning: `maestro loop show learning`

If no shipped recipe fits, a custom card or run recipe must still use
perceive -> choose -> act -> observe -> learn -> continue, current Maestro
verbs, hard stops, proof, QA, authority checks, and continue output.

## Command Truth

Exact command signatures live in `reference/cli.md` inside every installed
Maestro skill, for example:

`.maestro/skills/maestro-card/reference/cli.md`

A verb or flag not listed there does not exist. Read the generated reference
instead of probing or guessing.

## Work Model

Work has three levels:

- High: Card
- Mid: CardKind / workflow kind
- Low: Task

Feature, Bug, Chore, Custom, Decision, Idea, and Progress are CardKinds.
Progress stores small Low Tasks in `progress.yml`; use it through
`maestro task add/start/done/list`. Legacy `type: task` cards remain readable
for compatibility.

Linked-card inbox messages are advisory coordination signals only. They do not
block execution. When order matters, record an explicit Task blocker or
dependency; readiness, next, claim, and verification gates consult Task
blockers, not messages or unread state.

## Proof

Complete executable work with `maestro task complete` using summary, claim, and
proof. Maestro records the proof and runs verification.

Hooks auto-record tool calls as proof. Verification matches each `--claim`
against recorded or inline proof. Empty or unbacked claims fail.

When proof or verification fails, use the recovery path from the active recipe
or `maestro task proof`.

## Design

For brainstorm or unsettled behavior, use the design loop.

Map the problem from real code and artifacts, then walk open questions one at a
time. Lock each decision and record the corresponding note. Do not batch-decide
independent forks, edit locked decisions in place, or cross into implementation
before the user approves build.

Before proposing an idea or reopening a settled question, search precedent:

`maestro grep "<topic> corpus:memory"`

Cite the best matching card, decision, task, proof, or note. Use
`maestro card list --grep <topic> --archived` only for exact legacy rows,
compatibility checks, or when unified grep is too broad or surprising.

## Corrections And Learning

When the user corrects your behavior, record it:

`maestro event intervention --note "<what was wrong>"`

Durable learning belongs in Maestro artifacts: decisions, notes, events,
friction, tasks, proof, skills, or recipes. Do not leave important workflow
changes only in chat.

## Concurrency

The card store is shared state. In fan-out work, the orchestrator performs
store-mutating verbs such as decision lock, task complete, verification, status,
and notes. Sub-agents return results as data unless they have isolated stores.

Use a separate git worktree when sessions may edit overlapping code or the
shared card store. Use `maestro active`, `[overlap]`, `[CONFLICT]`, and `[busy]`
notices to coordinate. For full conflict handling, run:

`maestro loop show conflict-handoff`

A failed multi-file store command can be partial. Re-run it so Maestro reads the
latest store and reapplies the intended change.

## Harness Improvement

Passive friction backlog: `maestro harness list / apply / measure`.

When status, next, or complete surfaces over-threshold friction, apply and claim
it before new work, or dismiss it with a reason when it is noise. The binary
counts and shows friction; the agent acts.
