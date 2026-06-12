---
name: maestro-design
version: 1.10.0
description: "Use for design or brainstorming in a Maestro repo before implementation starts. Map current behavior, decide one fork at a time, record decisions and notes, then hand the approved contract to maestro-card."
---

# Maestro Design

Use this when the deliverable is the design of record, not code. The feature
stays `proposed` while the contract is still editable; `feature accept` ends
design and freezes the contract.

Activate:
`maestro hook record --event skill_activation --skill maestro-design`

Exact command signatures live in [reference/cli.md](reference/cli.md),
generated from the binary. A verb or flag not listed there does not exist;
read it instead of probing `--help`. Never chain a guessed id: use only ids
read from verb output, and when a lookup misses, re-list instead of retrying
spelling variations.

Routing: external PRD with open forks -> decide forks in design, then intake per maestro-card.

## Do

1. Open one feature for the topic: `maestro feature new "<topic>"`,
   seeding `--description` with the problem.
2. Map the current state from real evidence before options:
   files, commands, outputs, screenshots, or repo artifacts with `file:line`
   where code is involved. Write what you map into the spec as you go:
   `maestro feature spec <id> --section "Current state" --append "<finding>"`.
   The same verb fills `Problem` and creates any new section; `--replace`
   rewrites a section wholesale.
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
structure, or other judgment-heavy forks.

1. Write a 3-5 point rubric into `notes.md` before generating options.
2. Ask 3-5 fresh-context generators for one concrete option each from different
   angles, such as minimal, user-first, or consistency-first.
3. Use a fresh judge to score against the rubric and remove duplicates.
4. If scores cluster, run pairwise matches until one option survives.
5. Lock the survivor with `maestro decision new` then `maestro decision lock`;
   record why rejected options lost. Generators do not become durable outputs.

## Stop

- Do not implement from this skill.
- Do not batch unrelated decisions into one lock.
- Do not keep a contradicted decision silently. Reopen or supersede it in the
  Decision record.
- Do not resume from chat memory. Resume from `maestro feature spec <id>`,
  `.maestro/cards/<id>/notes.md`, and `maestro decision list`.

## Hand-off

Pipeline: `[maestro-design] -> maestro-card (qa-baseline -> feature accept -> work -> verify -> qa-slice -> feature ship)`

Next: decisions locked and contract authored -> `maestro-card` (its
qa-baseline reference, then `feature accept`).
