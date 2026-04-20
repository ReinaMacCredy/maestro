---
name: maestro:mission-planning
description: "Plan and structure new missions. Brainstorm raw ideas into decomposed missions with milestones, features, worker types, constraints, and the exact `maestro handoff` command for the first external worker."
argument-hint: "<raw idea or mission description>"
---

# Mission Planning

Maestro is the conductor. It persists the mission, keeps shared context on disk, and can launch a fresh Codex or Claude handoff with a self-contained markdown brief. This skill turns a raw idea into that plan plus the exact handoff command the operator should run next.

The input is `$ARGUMENTS` — a single sentence like "add a command palette to the TUI" or a rough paragraph describing a goal. The output is two concrete artifacts:

1. A mission persisted under `.maestro/missions/{id}/` via `maestro mission create --file plan.json`
2. The exact `maestro handoff ...` command for the first external worker

Do not auto-launch the handoff in this skill. Planning stops once the mission exists and the launch command is drafted. Skip any step and the downstream worker either drifts off-scope or halts asking for clarification.

## The 5-step workflow at a glance

1. Brainstorm opening — clarify intent before structure
2. Decompose into milestones and features — 3-7 milestones, sprint-sized
3. Match worker types — codex-cli, claude-code, subagent, human
4. Capture constraints — what not to touch, and why
5. Persist mission and draft the first handoff command

## Step 1 — Brainstorm opening

**Trigger**: `$ARGUMENTS` contains a raw idea or problem statement. No structure yet.

**Action**:
1. Restate the idea in one sentence and read it back to the user. If the user does not confirm, ask one clarifying question and wait.
2. Ask "what does done look like?" The answer is the core goal you will carry into Step 2.
3. Surface any obvious assumptions the idea rests on. Write them down even if they feel trivial — they become launch-readiness checks in Step 5.
4. If the idea is genuinely ambiguous (two or more valid interpretations), stop and ask the user to pick one before proceeding. Do not guess.

**Reference**: none — this is a conversational step, not a structural one.

**Output**: one-sentence core goal, list of known assumptions, user confirmation.

## Step 2 — Decompose into milestones and features

**Trigger**: you have a confirmed core goal from Step 1.

**Action**:
1. Extract the core goal into a single sentence: "when this mission is done, X is true."
2. Identify phases by asking "what has to happen before the next thing can happen?" Each answer is a milestone candidate.
3. Break each phase into 1-5 features. Each feature is sprint-sized (30 minutes to 2 hours of focused work) and named as an outcome, not an implementation detail.
4. Assign `dependsOn` edges between features. Empty is fine; cycles are not.
5. Draft 2-5 `verificationSteps` per feature. Each step must be observable and repeatable.
6. Pick one milestone `profile` per milestone from the allowed set: `planning`, `plan-review`, `implementation`, `code-review`, `bug-hunt`, `simplify`, `validation`, `custom`. Do not invent new profiles.

**Reference**: `reference/decomposition.md`

**Output**: a draft plan with 3-7 milestones, 1-5 features per milestone, dependencies, and verification steps. No worker types yet.

## Step 3 — Match worker types

**Trigger**: you have a decomposed plan from Step 2.

**Action**:
1. For each feature, pick a worker type from the allowed set: `codex-cli`, `claude-code`, `subagent`, `human`. Do not invent new worker types. If the work genuinely requires a worker type outside the allowed set, invoke `maestro:define-mission-skills` to register the new skill, then return here to assign it.
2. Apply the decision table in the reference file. Mechanical work goes to `codex-cli`, ambiguous work goes to `claude-code`, exploration goes to `subagent`, trust calls go to `human`.
3. For any milestone with a `code-review` or `plan-review` profile, confirm the reviewer worker type is a different instance than whatever produced the artifact being reviewed. Self-review is pathologically lenient.
4. Re-read each feature's worker-type choice and ask "would this worker actually succeed here?" If not, revise.
5. Identify the first feature that should be launched through `maestro handoff`. It must be assigned to `codex-cli` or `claude-code`. If your first execution feature is `subagent` or `human`, either re-scope the plan or choose the next feature that should run as a fresh external worker.

**Reference**: `reference/worker-type-matching.md`

**Output**: every feature has an `agentType` field. Review milestones use a different instance than the generator, and the first external worker candidate is identified.

## Step 4 — Capture constraints

**Trigger**: every feature has a worker type assigned.

**Action**:
1. For each feature, list the things a worker must not touch while executing it. Name specific files, APIs, patterns, or out-of-scope extensions.
2. For each constraint, write the reason next to it. A constraint without a reason is unenforceable at edge cases.
3. Store the reason somewhere the worker will later see it: `preconditions`, feature description, or a concrete verification note.
4. Keep each feature to 1-4 real constraints. If a feature needs more than that, it is too large — go back to Step 2 and split it.
5. Make sure the first external worker's feature has explicit constraints. Those become the `## Constraints` section of the eventual handoff brief.

**Reference**: `reference/boundary-capture.md`

**Output**: a concrete constraint list per feature, with reasons captured in `preconditions`, descriptions, or verification notes.

## Step 5 — Persist mission and draft the first handoff command

**Trigger**: plan, worker types, and constraints are all in hand.

**Action**:
1. Write the plan to a JSON file (typically `plans/<mission-name>.json`). Include milestones, features, dependencies, verification steps, worker types, and constraints.
2. Run `maestro mission create --file <plan.json>` to persist the mission. Capture the returned mission id.
3. Run the readiness check. If the plan is still missing a launchable first worker, go back to Steps 2-4 instead of drafting a bad command.
4. Map the first worker's `agentType` to a provider:
   - `codex-cli` -> `--provider codex`
   - `claude-code` -> `--provider claude`
5. Draft the exact handoff command. The task string must name the mission id, feature id or title, expected outcome, and the requirement to run the listed verification steps before stopping.
6. Add `--worktree <slug>` when the worker should operate in an isolated sibling checkout, especially for risky review or parallel implementation slices.
7. Return the mission id and the exact handoff command to the user. Do not run the command inside this skill.

**Reference**: `reference/readiness-check.md`, `reference/handoff-command-cheatsheet.md`

**Output**: a persisted mission under `.maestro/missions/{id}/` and an exact `maestro handoff ...` command ready for the operator to launch.

**Next step**: once the mission is persisted, the human operator can approve the mission if needed and run the drafted handoff command. This skill ends at plan creation plus launch-command drafting; execution is separate.

## Critical constraints

- Never skip the brainstorm opening (Step 1). Jumping straight into decomposition produces plans that solve the wrong problem cleanly.
- Never invent worker types or milestone profiles. The allowed sets are closed. If none fit, the work needs to be re-decomposed, not given a new category.
- Never draft the first handoff command for a `subagent` or `human` feature. Native handoff launching is only for fresh Codex or Claude runs.
- Never auto-launch the handoff from this skill. The output is the exact command, not the running child process.
- The output of this skill is always a mission file plus the exact next `maestro handoff ...` command. If either is missing, the skill has not completed.

## Example output

Input: "Refactor the auth middleware so session validation and permission checking are separate."

After Steps 1-4, the plan is persisted:

```bash
maestro mission create --file plans/auth-split.json
# mission_id: mis_01h8k2f9
```

Then the next launch command is drafted:

```bash
maestro handoff \
  "Implement feature auth-impl for mission mis_01h8k2f9. Split session validation from permission checks while preserving the existing middleware signature, keeping session-store changes out of scope, and running the listed verification steps before stopping." \
  --provider codex
```

Mission persisted, launch command drafted. The operator can now run that command to start the first Codex worker with a persisted markdown handoff brief. The skill is done.
