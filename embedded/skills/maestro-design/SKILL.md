---
name: maestro-design
version: 1.3.0
description: Use when the work is design or brainstorming rather than implementation - turning a rough idea into decided design-of-record before any task is built. Covers mapping the problem from the real code, walking open questions one decision at a time, and locking each fork as a Decision record with running reasoning in the feature's notes.md. Reach for it on design, architecture, brainstorm, or spec-authoring requests in a Maestro repo.
---

# Maestro Design

Use this skill when the work is design or brainstorming, not implementation: turning a rough
idea into decided design-of-record before any task is built. The deliverable is the decided
design, captured in the feature and its notes.md, not a side conversation.

The whole loop runs while the feature is `proposed` - that is the design state, the only status
where its contract is freely editable (`feature set`). `accept` ends the brainstorm and freezes it
into `ready`.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-design`, and `activation_mode` set to `agent_selected`.

## The design loop

1. **One feature per topic.** `maestro feature new "<topic>"` opens it (proposed) and scaffolds
   notes.md. The feature holds the problem; notes.md holds the running reasoning.
2. **Map before deciding.** Read the real code first. Write the current state grounded in
   `file:line`, then an honest list of the open questions, before proposing anything:
   `maestro feature set <id> --description "<problem>" --question "<open question>" ...`.
3. **Walk decisions one at a time.** For each open question: describe it plainly with a concrete
   example, present the options, then lock it. Never batch-decide a pile of forks at once.
4. **Record each lock; the two lists are the status.** On lock: `maestro decision new "<the fork>"`
   for the durable record, append the reasoning to `.maestro/features/<id>/notes.md` at the moment
   you decide, and re-issue `--question` with the remaining list (`feature set` replaces the field,
   so drop the answered one). Still listed as a question = open; has a decision record = locked.
5. **Surface tradeoffs, don't silently bank.** If a locked choice contradicts something downstream,
   stop and surface it rather than writing it down and moving on.
6. **Itemize impact before any drop.** Before designing the removal of a field, file, or behavior,
   enumerate every consumer first.
7. **Resume from the feature, not memory.** `maestro feature show <id>` (open questions + notes) and
   `maestro decision list` are where you pick back up after any break.
8. **Author the contract, then hand to the lifecycle.** Once the decisions are locked you know the
   acceptance criteria and affected areas: `maestro feature set <id> --acceptance "..." --area "..."`.
   From there it is the feature lifecycle (maestro-feature skill): `feature accept` (which also needs
   a behavior baseline) freezes the contract into `ready`, then `maestro task create ... --feature <id>`
   builds it.
9. **On ship, capture the outcome and ask before archiving.** `maestro feature ship <id> --outcome
   "<one line>"`, then `maestro feature archive <id>` only when you mean to retire it.

## Taste forks: generate-and-filter (tournament variant)

Use when an open question is taste-based - naming, UX wording, API shape,
report structure - where comparing concrete options beats reasoning to one,
and a single context window would bias toward its own first idea.

1. Write the rubric BEFORE generating: 3-5 observable criteria drawn from
   the feature's description and acceptance lines, not invented on the fly.
   Append it to notes.md - the rubric is part of the reasoning record.
2. Spawn N generators (3-5), fresh context each, one option apiece, from
   deliberately different angles (minimal / user-first / consistency-first).
   Each returns one concrete option: the name, the wording, the sketch.
3. Filter: a fresh judge scores options against the rubric and dedupes
   near-identical ones. Discard weak options - don't iterate them.
4. Tournament variant, when options are many or scores cluster: judge
   pairwise (A vs B, fresh judge per match) instead of absolute scoring -
   comparative judgment is more reliable. Bracket until one survives.
5. Lock the survivor like any other fork: `maestro decision new "<the
   locked fork>"`, reasoning to notes.md including one line per losing
   option on why it lost, and drop the answered --question.

Generators explore; the Decision record stays the only durable output.
Never message a running generator with another generator's option.

## Hand-off

[maestro-design] -> maestro-feature -> maestro-task -> maestro-verify -> feature ship

Next: decisions locked + contract authored -> the `maestro-feature` skill (`feature accept`;
the gate also needs a `qa-baseline` baseline).
Related: `maestro-task` (the loop that builds the decided design), `maestro-verify` (the
evidence gate those tasks must pass).
