---
name: maestro-design
version: 1.26.0
description: "Use when the user wants to brainstorm, design, plan, choose wording, or decide a Maestro repo change before implementation, including HARNESS, skill, workflow, architecture, or UX decisions."
---

# Maestro Design

Use this when the deliverable is the design of record, not code. The feature
stays `proposed` while the contract is still editable; `feature accept` ends
design and freezes the contract.

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

First step in a session: run `maestro active` (pull-only) to see what other
live sessions are working on -- their card, mode, and progress -- before you
open anything. If a peer session is on a related card, connect yours with
`maestro link add <your-card> <their-card>` once you have created it; maestro
never auto-links. Linked cards exchange messages with `maestro msg send
<their-card> "<text>"` and `maestro msg read`; an `[inbox] N new` line on
STDERR before any command means a linked peer is waiting. Act on the banner
before unrelated work and run `maestro msg read`, as maestro-card already does.
Reply when the message poses a question or needs a decision; an FYI needs no reply.
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
3. Put the problem and open questions on the feature:
   `maestro feature set <id> --description "<problem>" --question "<loose question>"`.
4. Decide one fork at a time. For each fork, give the concrete example, the
   options, the tradeoff, and the chosen answer. Sketch every option inline as
   ASCII before asking, so the preview is readable in the terminal.
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

## Probe Forks

When a fork hinges on runtime or state-machine behavior you cannot settle by
reading, build a throwaway runnable harness, drive it through the edge cases,
record the answer in the decision context or `notes.md`, then delete the
harness. Preserve the answer, not the code.

## Stop

- Do not implement from this skill.
- Do not batch unrelated decisions into one lock.
- Do not keep a contradicted decision silently. Reopen or supersede it in the
  Decision record.
- Do not resume from chat memory. Resume from `maestro feature spec <id>`,
  `.maestro/cards/<id>/notes.md`, and `maestro decision list --feature <id>`
  (the bare list windows to recent decisions, so scope it to your feature).

## Hand-off

Pipeline: `[maestro-design] -> maestro-card (qa-baseline -> feature accept -> prepare -> work -> verify -> qa-slice -> feature close)`

Next: decisions locked and contract authored -> `maestro-card` (its
qa-baseline reference, then `feature accept`). The hand-off is this skill
boundary, crossed once here; it is not a lifecycle gate. When the user
authorizes building, do not re-ask -- flow straight through accept -> prepare
-> work.

If another session is live as you cross into implementation, follow the
conflict-handoff protocol in HARNESS.md (worktree-isolate; link + `maestro
conflict` on a shared file; merge back then `--clear`); `maestro loop show
conflict-handoff` is the full dance.
