---
name: maestro:mission-planning
description: "Plan and structure new missions. Brainstorm raw ideas into decomposed missions with milestones, features, worker types, boundaries, and UKI v5.2 handoffs ready for external workers to pick up."
argument-hint: "<raw idea or mission description>"
---

# Mission Planning

Maestro is the conductor. It holds the score, emits handoffs, and validates results, but it does not write code itself. External workers (Codex CLI, Claude Code children, Gemini, Aider) implement features once a plan and a UKI v5.2 handoff exist.

This skill turns a raw idea into that plan plus handoff. The input is `$ARGUMENTS` — a single sentence like "add a command palette to the TUI" or a rough paragraph describing a goal. The output is two concrete artifacts:

1. A mission persisted under `.maestro/missions/{id}/` via `maestro mission create --file plan.json`
2. A UKI v5.2 handoff string emitted via `maestro handoff create`, ready for an external worker to consume

Skip any step and the downstream worker either drifts off-scope or halts asking for clarification. All six steps are mandatory.

## The 6-step workflow at a glance

1. Brainstorm opening — clarify intent before structure
2. Decompose into milestones and features — 3-7 milestones, sprint-sized
3. Match worker types — codex-cli, claude-code, subagent, human
4. Capture boundaries — what not to touch, and why
5. Calibrate confidence — CS-work and CS-summary, honestly
6. Persist and emit handoff — mission file plus UKI string

## Step 1 — Brainstorm opening

**Trigger**: `$ARGUMENTS` contains a raw idea or problem statement. No structure yet.

**Action**:
1. Restate the idea in one sentence and read it back to the user. If the user does not confirm, ask one clarifying question and wait.
2. Ask "what does done look like?" The answer is the core goal you will carry into Step 2.
3. Surface any obvious assumptions the idea rests on. Write them down even if they feel trivial — they become calibration inputs in Step 5.
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
1. For each feature, pick a worker type from the allowed set: `codex-cli`, `claude-code`, `subagent`, `human`. Do not invent new worker types.
2. Apply the decision table in the reference file. Mechanical work goes to `codex-cli`, ambiguous work goes to `claude-code`, exploration goes to `subagent`, trust calls go to `human`.
3. For any milestone with a `code-review` or `plan-review` profile, confirm the reviewer worker type is a different instance than whatever produced the artifact being reviewed. Self-review is pathologically lenient.
4. Re-read each feature's worker-type choice and ask "would this worker actually succeed here?" If not, revise.

**Reference**: `reference/worker-type-matching.md`

**Output**: every feature has a `workerType` field. Review milestones use a different instance than the generator.

## Step 4 — Capture boundaries

**Trigger**: every feature has a worker type assigned.

**Action**:
1. For each feature, list the things a worker must not touch while executing it. Name specific files, APIs, patterns, or out-of-scope extensions.
2. For each boundary, write the reason next to it. A boundary without a reason is unenforceable at edge cases.
3. Convert each boundary to a short `BOUNDARY_STATE` token (max 4 words per `_`-link, underscores between words, no dashes inside tokens).
4. Move the reasons into `KEY_DECISIONS` tokens or the feature description so the worker can actually read them.
5. If any feature has more than 5 boundaries, the feature is too large — go back to Step 2 and split it.

**Reference**: `reference/boundary-capture.md`

**Output**: `BOUNDARY_STATE` token list per feature, reasons captured in `KEY_DECISIONS` or feature descriptions.

## Step 5 — Calibrate confidence

**Trigger**: you have a plan with worker types and boundaries.

**Action**:
1. Rate `CS-work` (0.0-1.0): is the plan correct and executable end-to-end?
2. Rate `CS-summary` (0.0-1.0): does the summary you will write capture the full intent?
3. Apply the honesty rule. Name at least one failure mode. Drop 0.1 per realistic failure mode, up to 0.2 total.
4. Check the divergent-score rules. If `CS-work` and `CS-summary` differ by more than 0.1, revise whichever is lower. If both are below 0.80, go back to Step 1 and re-scope.
5. Write the final `CS` slot: `CS-work_0.xx~summary_0.yy`.

**Reference**: `reference/confidence-calibration.md`

**Output**: two calibrated numbers ready to drop into the `CS` slot of the handoff.

## Step 6 — Persist mission and emit handoff

**Trigger**: plan, worker types, boundaries, and confidence scores are all in hand.

**Action**:
1. Write the plan to a JSON file (typically `plans/<mission-name>.json`). Include milestones, features, dependencies, verification steps, worker types, and boundaries.
2. Run `maestro mission create --file <plan.json>` to persist the mission. Capture the returned mission id.
3. Assemble the 12 UKI slots. Every slot must be present; use `DIVERGENCES-NONE` and `STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION` when nothing to report. `ARTIFACTS` must contain at least one `commit_`, `branch_`, `version_`, or `file_` token.
4. Run `maestro handoff create` with the slots as flags. Keep `SUMMARY` under 140 characters and use the `Essence-Progress-Risk` shape.
5. Return the mission id and the handoff string to the user so they can route the handoff to a worker.

**Reference**: `reference/uki-cheatsheet.md`

**Output**: a persisted mission under `.maestro/missions/{id}/` and a UKI v5.2 handoff string printed to stdout.

## Critical constraints

- Never skip the brainstorm opening (Step 1). Jumping straight into decomposition produces plans that solve the wrong problem cleanly.
- Never invent worker types or milestone profiles. The allowed sets are closed. If none fit, the work needs to be re-decomposed, not given a new category.
- Never ship a handoff without calibrated `CS-work` and `CS-summary` scores. Unscored handoffs are treated as 0.0 by downstream tooling and will be rejected.
- Never skip the final handoff emission. A plan without a UKI handoff is not executable by external workers — it is just a draft.
- The output of this skill is always a mission file plus a UKI handoff string. If either is missing, the skill has not completed.

## Example output

Input: "Refactor the auth middleware so session validation and permission checking are separate."

After Steps 1-5, the plan is persisted:

```bash
./dist/maestro mission create --file plans/auth-split.json
# mission_id: mis_01h8k2f9
```

Then the handoff is emitted:

```bash
./dist/maestro handoff create \
  --session-core auth_middleware_split \
  --summary "Auth middleware split drafted; signature preserved; 14 callers need regression pass before code-review." \
  --next-action assign_feat_001_to_codex_cli_worker \
  --driver user_report_signature_churn \
  --driver refactor_debt_audit \
  --divergence NONE \
  --decision split_validation_from_permission \
  --decision keep_middleware_signature \
  --decision defer_permission_semantics \
  --signal callers_14_stable \
  --signal unit_tests_42_target \
  --artifact branch_feat_auth_split \
  --artifact file_src_auth_middleware_ts \
  --boundary preserve_middleware_signature \
  --boundary no_session_store_changes \
  --boundary no_permission_semantics_changes \
  --execution-state plan_drafted \
  --stance-collapse NONE_DETECTED_LOW_FRICTION \
  --confidence-work 0.88 \
  --confidence-summary 0.92
```

Mission persisted, handoff printed. A worker (`codex-cli` per the worker-type assignment) can now pick up the first feature by reading the mission file and the handoff string. The skill is done.
