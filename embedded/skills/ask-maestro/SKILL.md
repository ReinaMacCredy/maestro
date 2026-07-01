---
name: ask-maestro
version: 1.0.0
description: "Routes Maestro requests to the right shipped skill and lifecycle recipe. Use when the user asks what Maestro route, skill, loop, card, task, feature, setup, audit, design, bugfix, ship, archive, progress, or continuation workflow to use."
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

If the next move is still unclear, run `maestro loop next`. It is read-only and
routes from local artifacts. Read the selected recipe with
`maestro loop show <recipe>`.

## The Main Flow: Idea To Ship

Most Maestro work moves through this path:

1. `maestro-design` - use for unsettled behavior, brainstorm, workflow design,
   specs, PRD synthesis, domain modeling, grilling, UX shape, or skill/harness
   design. Stay here until material forks are locked and the feature handoff is
   finalized.
2. `maestro-card` - use after design approval or for already-scoped executable
   work: implement, bugfix, verify, QA, close, release, archive, or continue
   active work. Behavior-changing implementation defaults to test-first work
   unless the task is explicitly docs/config/mechanical/light/spike.
3. `maestro-card` ship path - use `maestro loop show ship` for close, release,
   local install, archive, and proof-backed handoff boundaries.

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

- Design continuation: `maestro feature finalize <id>` writes the handoff; the
  next session starts from that handoff.
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
- "Work while away/asleep/keep looping" -> `maestro-card`, then
  `maestro loop show unattended`.
- "Record a reusable lesson" -> `maestro-card`, then
  `maestro loop show learning`.
- "There are active sessions or possible overlap" -> `maestro loop show
  conflict-handoff`.

## Stop

If no route fits, say what is missing and use `maestro loop next` before
inventing a custom flow. Custom flows still use Maestro verbs, proof, QA,
authority gates, hard stops, and the loop grammar:
perceive -> choose -> act -> observe -> learn -> continue.
