---
name: maestro-design
version: 1.4.2
description: "Use for design or brainstorming in a Maestro repo before implementation starts. Map current behavior, decide one fork at a time, record decisions and notes, then hand the approved contract to maestro-feature."
---

# Maestro Design

Use this when the deliverable is the design of record, not code. The feature
stays `proposed` while the contract is still editable; `feature accept` ends
design and freezes the contract.

Activate:
`maestro hook record --event skill_activation --skill maestro-design`

## Do

1. Open one feature for the topic:
   `maestro feature new "<topic>"`.
2. Map the current state from real evidence before options:
   files, commands, outputs, screenshots, or repo artifacts with `file:line`
   where code is involved.
3. Put the problem and open questions on the feature:
   `maestro feature set <id> --description "<problem>" --question "<fork>"`.
4. Decide one fork at a time. For each fork, give the concrete example, the
   options, the tradeoff, and the chosen answer.
5. Lock each decision durably:
   `maestro decision new "<decision title>"`, fill the generated decision
   template, then append the reasoning to `.maestro/features/<id>/notes.md` as
   a dated line.
6. If a chosen answer removes a field, file, command, behavior, or workflow,
   enumerate consumers before locking the removal.
7. Keep feature questions current: re-issue `--question` with remaining open
   forks, or use `maestro feature set <id> --clear-questions` when none remain.
8. Author the implementation contract only after decisions are stable:
   `maestro feature set <id> --acceptance "<observable behavior>" --area "<surface>"`.

## Taste Forks

Use a generate-and-filter pass for naming, UX wording, API shape, report
structure, or other judgment-heavy forks.

1. Write a 3-5 point rubric into `notes.md` before generating options.
2. Ask 3-5 fresh-context generators for one concrete option each from different
   angles, such as minimal, user-first, or consistency-first.
3. Use a fresh judge to score against the rubric and remove duplicates.
4. If scores cluster, run pairwise matches until one option survives.
5. Lock the survivor with `maestro decision new` and record why rejected options
   lost. Generators do not become durable outputs.

## Stop

- Do not implement from this skill.
- Do not batch unrelated decisions into one lock.
- Do not keep a contradicted decision silently. Reopen or supersede it in the
  feature notes and Decision record.
- Do not resume from chat memory. Resume from `maestro feature show <id>`,
  `.maestro/features/<id>/notes.md`, and `maestro decision list`.

## Hand-off

Pipeline: `[maestro-design] -> qa-baseline -> maestro-feature -> maestro-task -> maestro-verify -> qa-slice -> feature ship`

Next: decisions locked and contract authored -> `qa-baseline`, then
`maestro-feature` for `feature accept`.
