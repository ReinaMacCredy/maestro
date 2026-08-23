# ADR-0003: One work entity + minimal decision entity

Date: 2026-08-23
Status: accepted (supersedes the four-entity clause of ADR-0002)

## Context

The Rust maestro models work as four first-class entities — card, task,
feature, decision — and that split is the primary driver of verb sprawl (41
root verbs, 14 clap-hidden). The blank-slate brainstorm (2026-08-23) asked what
an agent actually needs to answer after compaction: what is open, what blocks
what, what should I pick up. That needs one tree, one DAG, and states — not
three work-shaped tables.

## Decision

- One `work` entity: parent-child tree + `blocked_by` DAG + `kind` as plain
  data (feature/task/bug/idea/chore...), states as data, acceptance as a data
  field, notes attached, and a lease (`held_by` session; expires when the
  holder session is no longer alive). `work start` is the claim.
- One `decision` entity, separate because its lifecycle differs in kind:
  draft -> locked -> superseded(link), parent-child children for "lock all"
  (each fork stays a visible child decision — the never-compress rule is
  satisfied by structure), linked to work. Verbs: draft/lock/show/list only;
  the set/audit/repair verb family does not return.
- The old feature lifecycle (verify/accept/ship/close) becomes gate policies
  on `work done` for kind=feature — no separate feature verb family.

## Rejected

- Four entities as in Rust: verb sprawl is proven, and card-vs-task-vs-feature
  boundaries never carried their weight for a single-owner repo.
- Generic item store (still rejected, as in ADR-0002): decision keeps its own
  entity because done/blocked are meaningless for it and locked/superseded are
  meaningless for work — one table would stack two state machines.
- Folding decision into work as kind=decision: same reason.

## Consequences

- Tracking surface is two verb families (`work`, `decision`) plus read verbs.
- Policy plugins read `kind` to scope their gates (e.g. TDD gates only
  kind=feature/bug implement work).
- The stage-1 SPEC's earlier card/task/feature surface is superseded.
