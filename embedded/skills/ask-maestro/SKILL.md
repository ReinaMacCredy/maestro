---
name: ask-maestro
version: 1.0.7
description: "Router for choosing the next Maestro skill or lifecycle recipe."
disable-model-invocation: true
---

# Ask Maestro

You do not remember every Maestro route, so ask.

This skill is a router. It chooses the next Maestro skill or lifecycle recipe;
it does not replace the chosen skill. After routing, follow that skill and the
repo harness.

## Quick Start

Run `maestro status` before routing. If implementation might overlap another
session, run `maestro active` and respect any conflict-handoff hard stop.

If the next move is still unclear, read `maestro status --json`. For an exact
bounded vNext projection, Integration must supply one canonical JSON request
with authenticated host context and expected Release/catalog identities; pass
it on stdin to `maestro packet read`. Never invent those authority facts.
Retired successor routes and legacy lifecycle recipe identifiers return a typed
refusal and must not be used as fallback routers. Do not use hidden stores,
hidden schedulers, silent recipe mutation, or proof/QA bypass.

## The Main Flow: Idea To Ship

Most Maestro work moves through this path:

1. `maestro-design` - use for unsettled behavior, brainstorm, workflow design,
   specs, PRD synthesis, domain modeling, grilling, UX shape, or skill/harness
   design. Stay here until material forks are locked and the feature handoff is
   finalized. "lock all", "all rec", and all-recommendations batches must be a
   DecisionSet or separate child decisions, not one compressed summary lock.
2. `maestro-card` - use after design approval or for already-scoped executable
   work: implement, bugfix, verify, QA, close, release, archive, or continue
   active work. Behavior-changing implementation defaults to test-first work
   unless the task is explicitly docs/config/mechanical/light/spike.
3. `maestro-card` ship path - use its explicit close, release, local install,
   archive, and proof-backed handoff gates.

Do not route approved build work back into design just because more discussion
is possible. Do route back to design when the requested outcome depends on an
unsettled product, lifecycle, schema, command, or UX decision.

## On-Ramps

- Setup, sync, install, hooks, global skills, doctor, or harness initialization:
  use `maestro-setup`.
- Repo-wide improvement, architecture review, code review, harness backlog, or
  read-only deepening proposals: use `maestro-audit`.
- Raw incoming bug report with an obvious reproduction target: use
  `maestro-card` and the bugfix loop: reproduce, root cause, smallest correct
  fix, regression coverage, verification, scoped commit.
- Raw incoming feature request with open questions: use `maestro-design`.
- External PRD or plan that is already approved enough to execute: use
  `maestro-card` intake.
- Small same-session work: use the Progress task surface through
  `maestro task setup` or `maestro task add/start/done`; proof is still
  required on completion.

## Cross-Session Continuation

Use Maestro artifacts instead of chat memory:

- Design continuation: `maestro feature reconcile <id>` writes or refreshes the
  pre-finalize receipt, then `maestro feature finalize <id>` writes the handoff;
  the next session starts from that handoff.
- Active work continuation: `maestro status`, `maestro task show <id>`, and
  `maestro card show <id>` reveal the current task, locked acceptance, and
  proof state.
- Shared-store coordination: use `maestro active`, links, messages, and the
  `conflict-handoff` recipe when sessions may overlap.

## Quick Router

- "What should I use?" -> this skill, then the closest row below.
- "Set up Maestro here" -> `maestro-setup`.
- "Brainstorm/design/grill/spec/PRD/domain model this" -> `maestro-design`.
- "Go build/implement/fix/verify/close/archive/release" -> `maestro-card`.
- "Review/audit/find improvements/propose backlog" -> `maestro-audit`.
- "Work while away/asleep/keep looping" -> `maestro-card`, subject to current
  readiness and unattended-authority gates.
- "Record a reusable lesson" -> `maestro-card` and its explicit learning
  evidence path.
- "There are active sessions or possible overlap" -> the conflict-handoff
  protocol in the repository Harness.

Completion criterion: one route is chosen with the exact next skill, recipe, or
command; if no route fits, the missing fact is explicit before fallback.

## Stop

If no route fits, say what is missing and re-read `maestro status --json`
before inventing a custom flow. Custom flows still use Maestro verbs, proof,
QA, authority gates, hard stops, and the loop grammar:
perceive -> choose -> act -> observe -> learn -> continue.
