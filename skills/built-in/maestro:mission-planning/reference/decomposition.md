# Decomposition: Raw Idea to Milestones and Features

This is the second step of mission planning. You have a clarified goal from the brainstorm opening. You need to turn it into a structured plan that a maestro mission file can hold.

The output of this step is a draft plan with 3-7 milestones, each with 1-5 features. No worker types yet, no boundaries yet, no confidence yet — those come in later steps.

## The rule of 3-7 milestones

Plans with fewer than 3 milestones usually hide missing phases ("it's just implementation"). Plans with more than 7 exceed working memory — the human coordinating terminals cannot hold 8+ distinct phases in their head. If you end up with 8+, collapse adjacent phases or push some to a follow-up mission.

A milestone is sprint-sized: a worker (or a small chain of workers) should be able to finish one milestone's features before the human needs to step in and redirect. If a single milestone has 15 features, it is not one milestone.

## Milestone profiles

Every milestone has a `profile` that tells workers what mode they are in. Use one of these exactly — do not invent new ones.

- **`planning`**: design and decomposition work. Output is a plan, not code. Example: "sketch the data model" or "write the ADR for the auth change."
- **`plan-review`**: another agent reviews a plan before implementation starts. Catches ambiguity, missing deps, unrealistic scope. Different instance than the planner.
- **`implementation`**: the default. Write code, write tests, make it work. Most milestones are this.
- **`code-review`**: another agent reviews the implementation. Different instance than the implementer — self-review is pathologically lenient.
- **`bug-hunt`**: find and fix a specific bug. Has a reproducer, a root cause, a regression test.
- **`simplify`**: reduce code, remove duplication, improve clarity without adding behavior. Scope-constrained refactor.
- **`validation`**: run the validators, confirm the acceptance criteria, ship-gate. Usually short.
- **`custom`**: escape hatch when none of the above fit. Explain why in the milestone description.

If you find yourself wanting a profile that does not exist, the answer is almost always: split the work into two milestones that do fit.

## The decomposition sequence

Work in this order. Skipping ahead produces plans that look complete but do not execute.

1. **Extract the core goal.** Write one sentence: "When this mission is done, X is true." If you cannot write that sentence, go back to Step 1 (brainstorm opening).
2. **Identify phases.** Ask "what has to happen before the next thing can happen?" Each answer is a milestone candidate. Research before design, design before implementation, implementation before review, review before validation.
3. **Break phases into features.** For each milestone, list the concrete outcomes needed. Each outcome is a feature.
4. **Assign dependencies.** Features inside a milestone and across milestones have a `dependsOn` list. Empty is fine. Cycles are not.
5. **Draft verification steps.** For each feature, write 2-5 observable verifications. See the next section.

## Feature sizing: 30 minutes to 2 hours

A feature is a worker's unit of attention. If a feature would take a worker more than 2 hours of focused effort, split it. If it would take less than 30 minutes, merge it with a neighbor or promote it to a verification step.

Common split points:
- "Implement X and document it" -> two features: implement, then document
- "Refactor A, B, and C" -> three features, one per target
- "Add feature and the test for it" -> one feature (tests are part of the feature, not a separate one)

## `verificationSteps`: 2-5 concrete, observable checks per feature

Verification steps are what the worker runs to prove the feature is done. They must be observable — a human or another agent has to be able to repeat the check and get the same answer.

Good verification:
- `bun test tests/unit/auth/login.test.ts passes`
- `./dist/maestro handoff create --help shows --boundary flag`
- `grep -r "TODO.*auth" src/ returns zero hits`

Bad verification:
- `code is clean` (not observable)
- `feature works` (not specific)
- `tests pass` (which tests?)

2-5 is the target. 1 is usually not enough to catch regressions. More than 5 means the feature is too big.

## Anti-patterns to avoid

- **Label-milestones instead of profile-milestones.** A milestone called "auth stuff" is a label, not a phase. Ask what profile it would have — if you cannot pick one, the milestone is not real.
- **Implementation-descriptions instead of outcomes.** "Add a function that calls X" describes how. "Users can log in with email" describes what. Features are outcomes.
- **Missing dependencies.** If feature B reads state that feature A writes, B must `dependsOn` A. Missing deps manifest as workers blocked mid-stream because something they assumed is absent.
- **The 15-feature monster milestone.** If one milestone has more than 5 features, it is actually two or three milestones that were collapsed to hide scope. Uncollapse them.
- **Plans that start with `implementation`.** Almost no real mission starts cold on implementation. There is always at least a planning or plan-review phase hiding.

## Worked example

Raw idea: "Add a command palette to the TUI so users can jump between screens without memorizing keybindings."

Decomposed:

1. **`planning` milestone — `palette-design`**
   - `feat-1` (title: "Survey existing TUI command structure") — list every screen, modal, and keybinding currently available. Verification: file exists at `plans/palette/screen-inventory.md`; inventory has at least one entry per registered screen in `preview-state.ts`.
   - `feat-2` (title: "Sketch palette UX") — wireframe the palette open, filter, select, close flow. Verification: `plans/palette/ux-sketch.md` exists; lists the 4 flow states.

2. **`implementation` milestone — `palette-core`** (depends on `palette-design`)
   - `feat-3` (title: "Add palette overlay component") — new file under `src/tui/opentui/components/`. Verification: component renders in `--preview all`; snapshot test passes.
   - `feat-4` (title: "Wire global keybinding") — press `cmd+k` opens palette. Verification: `tests/unit/tui/palette.test.ts` asserts open on keypress.
   - `feat-5` (title: "Populate palette with registered screens") — reads from `preview-state.ts`. Verification: palette shows every screen name; test enumerates expected list.

3. **`code-review` milestone — `palette-review`** (depends on `palette-core`)
   - `feat-6` (title: "Review palette implementation") — different agent than implementer. Verification: review notes exist in `plans/palette/review.md`; at least one concern raised or explicit "no concerns."

4. **`validation` milestone — `palette-validate`** (depends on `palette-review`)
   - `feat-7` (title: "Run render-check across all screens with palette open") — `./dist/maestro mission-control --render-check --size 120x40` green. Verification: exit code 0; zero warnings.

Four milestones, seven features, covers planning through validation, each feature is sprint-sized.
