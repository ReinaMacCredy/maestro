---
name: maestro-design
version: 1.34.0
description: "Design Maestro changes before implementation: use for brainstorm, plan, PRD synthesis, grill me, grilling, stress-test, domain model, deepening candidate, wording, workflow, skill, harness, card/task/feature, architecture, UX, or agent-process decisions."
---

# Maestro Design

Use this when the deliverable is the design of record, not code. The feature
stays `proposed` while the contract is still editable; `feature finalize`
writes the clean continuation handoff, and `feature accept` later freezes the
contract.

Maestro work has three levels:

```text
High = Card
Mid  = CardKind / workflow kind
Low  = Task
```

Card is the only high-level durable work object. Feature, Bug, Chore, Custom,
Decision, Idea, and Progress are CardKinds / workflow kinds on cards. A Task is
the low executable unit, not a CardType in the target model; legacy `type: task`
cards stay readable for compatibility. For tiny same-session work, design
toward the low-ceremony `task add/start/done/list` surface backed by a Progress
card's `progress.yml`. Use facets (`spec.md`, `qa.md`, `notes.md`) for any card
type that needs contract, evidence, or history, not only features.

Activate with a known session id:
`maestro hook record --event skill_activation --skill maestro-design --session <session_id>`

Exact command signatures live in [reference/cli.md](reference/cli.md),
generated from the binary. A verb or flag not listed there does not exist;
read it instead of probing `--help`. Never chain a guessed id: use only ids
read from verb output, and when a lookup misses, re-list instead of retrying
spelling variations.

Native Maestro MCP tools may be used for supported orientation reads
(`maestro_status`, `maestro_feature_show`, `maestro_card_list`, and related
list/show tools) when the host exposes them. Design authoring still uses the
CLI because `feature spec`, `feature set`, and `decision lock` are not MCP
tools yet. After the design hand-off, `maestro-card` prefers MCP for supported
work-card and feature-lifecycle steps.

Routing: external PRD with open forks -> decide forks in design, then intake per maestro-card.
PRD synthesis from settled context -> use [reference/prd.md](reference/prd.md).
Grill/stress-test session -> use [reference/grilling.md](reference/grilling.md).
Domain-modeling session -> use [reference/domain-model.md](reference/domain-model.md).
Grill-with-docs session -> use [reference/grilling.md](reference/grilling.md), plus [reference/domain-model.md](reference/domain-model.md) for CONTEXT.md/ADR updates.
Chosen architecture deepening candidate -> use [reference/deepening-candidate.md](reference/deepening-candidate.md).

## Conversation Driver

Keep a working thesis from the user's latest correction, selected text,
sidechat paste, example, and preference. Before opening or revising a fork,
fold that thesis into the option framing instead of treating the new detail as
a loose comment.

Keep a short fork queue. After each locked decision or clarified user answer,
derive the next unresolved fork from the feature spec, open questions, locked
decisions, and working thesis. If a concrete next fork exists, present it in
the same turn; do not make the user send "next fork" just to continue. If no
fork remains, say that directly and name the next lifecycle gate, usually
`maestro feature finalize <id>`.

Every material fork needs a detail floor:

- the concrete problem being decided
- A/B/C options with artifact-level previews
- one real Maestro example
- the tradeoff and why rejected options lose
- a clear recommendation
- the exact answer format expected from the user

Completion criterion: a fork is ready to ask only when the user can answer in
the requested format and the eventual lock context can cite a real artifact,
command output, or explicit user statement.

If the user asks for more detail, examples, or clarification, answer by
improving the current fork or thesis first. Only open a new fork after the
clarified point is settled or explicitly becomes the next decision.

Recipe checkpoint: design work uses the shipped `design` lifecycle recipe.
Before deciding or changing the proposed contract, read or cite
`maestro loop show design` and keep the work inside perceive -> choose -> act
-> observe -> learn -> continue. Custom recipes are allowed only for the current
card/run, and only when no shipped recipe fits; they must use the same six
phases, current Maestro verbs, hard stops, and continue output.

First step in a session: run `maestro active` (pull-only) to see what other
live sessions are working on -- their card, mode, and progress -- before you
open anything. If a peer session is on a related card, connect yours with
`maestro link add <your-card> <their-card>` once you have created it; maestro
never auto-links. Linked cards exchange messages with `maestro msg send
<their-card> "<text>"` and `maestro msg read`; an `[inbox] N new` line on
STDERR before any command means a linked peer is waiting. Act on the banner
before unrelated work and run `maestro msg read`, as maestro-card already does.
Inbox is advisory coordination: it can suggest a cross-card Task dependency,
but it never blocks execution by itself. Record explicit Task
blockers/dependencies when ordering matters. Reply when the message poses a
question or needs a decision; an FYI needs no reply.
maestro never auto-reads or auto-replies; you do.

## Do

1. Open one feature for the topic: `maestro feature new "<topic>"`,
   seeding `--description` with the problem.
2. Map the current state from real evidence before options:
   files, commands, outputs, screenshots, or repo artifacts with `file:line`
   where code is involved. Write what you map into the spec as you go:
   `maestro feature spec <id> --section "Current state" --append "<finding>"`.
   The same verb fills `Problem` and creates any new section; `--replace`
   rewrites a section wholesale. For a complex core domain, model it here: run
   the DDD fitness gate in [reference/ddd.md](reference/ddd.md) before reaching
   for domain-driven design (most cards do not need it; stop at the first NO).
   When the design turns on domain language, bounded contexts, or business
   concepts, run the domain-model branch in
   [reference/domain-model.md](reference/domain-model.md).
3. Put the problem and open questions on the feature:
   `maestro feature set <id> --description "<problem>" --question "<loose question>"`.
   When the request is to turn settled context into a PRD, use
   [reference/prd.md](reference/prd.md) and do not open a new discovery interview.
4. Decide one fork at a time. For each fork, give the concrete example, the
   options, the tradeoff, and the chosen answer. Sketch every option inline as
   ASCII before asking, so the preview is readable in the terminal.
   When the user asks to grill, stress-test, or challenge a plan before build,
   use [reference/grilling.md](reference/grilling.md): walk branches one by one,
   give a recommended answer, and ask exactly one question at a time.
5. Lock each decision durably: `maestro decision new` (with `--feature` and
   `--context`) opens the fork; `maestro decision lock` records the chosen
   answer, the rejected options, and optionally a preview and superseded
   decisions. A fork the user already settled opens and locks in one call:
   `maestro decision new --lock --decision "<chosen>"`. Put the chosen ASCII
   sketch into `--preview` as multiline text. The lock echoes the entry and
   appends the dated feature-note pointer automatically; do not add a manual
   duplicate note.
6. If a chosen answer removes a field, file, command, behavior, or workflow,
   enumerate consumers before locking the removal.
7. Before locking a material or hard-to-reverse fork, get an independent
   adversarial review from a fresh context. Use an advisor-class tool or a
   skeptic sub-agent as peers, then incorporate or explicitly rebut its points
   in the lock context.
8. Keep feature questions current: open decisions are for real forks;
   `--question` is for loose questions not yet forks. A question that becomes a
   fork is opened as a decision and removed from questions.
9. Author the implementation contract only after decisions are stable:
   `maestro feature set <id> --acceptance "<observable behavior>" --area "<surface>"`.
10. When design is done, write the canonical clean handoff:
   `maestro feature finalize <id>`. The next agent starts at
   `.maestro/cards/<id>/handoff.md`; raw `spec.md`, `notes.md`, and decision
   cards stay preserved for audit.

Completion criterion: design is done only when every material fork is locked or
left as an explicit feature question, acceptance criteria and affected areas
exist, and `feature finalize` has refreshed `handoff.md`.

## Taste Forks

Use a generate-and-filter pass for naming, UX wording, API shape, report
structure, or other judgment-heavy forks. Full orchestration HOW (generator
angles, judge, pairwise): `maestro loop show generate-and-filter`.

1. Write a 3-5 point rubric into `notes.md` before generating options.
2. Ask 3-5 fresh-context generators for one concrete option each from different
   angles, such as minimal, user-first, or consistency-first.
3. Use a fresh judge to score against the rubric and remove duplicates.
4. If scores cluster, run pairwise matches until one option survives.
5. Lock the survivor with `maestro decision new` then `maestro decision lock`;
   record why rejected options lost. Generators do not become durable outputs.
   Only the conductor locks, one decision at a time: parallel `decision lock`
   calls on one feature collide on the shared `decisions.yaml` (HARNESS
   Orchestration). Generators return their option as data; they never lock.

## Grilling Forks

When the user says "grill me", "grilling", "stress-test this plan", or asks for
relentless challenge before building, run a grilling session inside the design
loop. Ask one question at a time, provide your recommended answer, and answer
explorable questions from code/docs/artifacts instead of asking. If the user asks
for docs while grilling, combine this with the domain-model branch. Full branch:
[reference/grilling.md](reference/grilling.md).

## PRD Synthesis

When the user asks to turn the current conversation into a PRD, synthesize only
from existing conversation, codebase evidence, domain language, and decisions.
Do not interview for new scope. Full branch: [reference/prd.md](reference/prd.md).

## Architecture Deepening

When the user picks a deepening opportunity from `maestro-audit`, design the
chosen module, interface, seam, dependency strategy, and surviving tests in
`maestro-design`. Full branch:
[reference/deepening-candidate.md](reference/deepening-candidate.md).

## Probe Forks

When a fork hinges on runtime or state-machine behavior you cannot settle by
reading, build a throwaway runnable harness, drive it through the edge cases,
record the answer in the decision context or `notes.md`, then delete the
harness. Preserve the answer, not the code.

## Domain Model Forks

When a fork hinges on project language or boundaries, challenge the terms before
locking the design. Read existing `CONTEXT.md`, `CONTEXT-MAP.md`, and ADRs when
present; if code can answer a question, inspect code instead of asking the user.
Resolved domain terms are captured inline in the right `CONTEXT.md`; ADRs are
offered only for hard-to-reverse, surprising trade-offs. Full branch:
[reference/domain-model.md](reference/domain-model.md).

## Stop

- Do not implement from this skill.
- Do not batch unrelated decisions into one lock.
- Do not keep a contradicted locked decision silently. To reopen a locked
  ruling, supersede it with `maestro decision supersede`; never edit or unlock
  the locked Decision record.
- Do not resume from chat memory. Resume from `maestro feature spec <id>`,
  `.maestro/cards/<id>/handoff.md` first when it exists, then raw
  `spec.md`, `notes.md`, and `maestro decision list --feature <id>` for audit
  or deeper context (the bare list windows to recent decisions, so scope it to
  your feature).

## Hand-off

Pipeline: `[maestro-design: feature finalize] -> maestro-card (qa-baseline -> feature accept -> prepare -> work -> verify -> qa-slice -> feature close)`

Next: decisions locked and contract authored -> run `maestro feature finalize
<id>`, then hand to `maestro-card` (its qa-baseline reference, then
`feature accept`). The hand-off is a lifecycle gate: `feature accept` and
`feature prepare` fail when `.maestro/cards/<id>/handoff.md` is missing or
stale. When the user authorizes building, do not re-ask -- flow straight
through finalize -> qa-baseline -> accept -> prepare -> work.

If another session is live as you cross into implementation, follow the
conflict-handoff protocol in HARNESS.md (worktree-isolate; link + `maestro
conflict` on a shared file; merge back then `--clear`); `maestro loop show
conflict-handoff` is the full dance.
