---
name: maestro-verify
version: 1.4.0
description: Verification protocol for Maestro tasks and feature work.
---

# Maestro Verify

## Goal

Prove the current task or feature with recorded evidence that Maestro can verify.

## When To Use

Use this skill when a task is in progress, needs verification, has failed proof,
or when a feature QA gate asks for baseline or slice evidence.

## Steps

1. Start with `maestro status` or `maestro task next`.
2. Inspect the task with `maestro task show <id>` and read its locked
   acceptance checks.
3. Identify the smallest checks that can falsify the change, run them from the
   repository root, and record exact commands and outcomes.
4. Complete the task with a concrete claim and observed proof:
   `maestro task complete <id> --summary "<what changed>" --claim "<claim>" --proof "<observed evidence>"`.
   The proof string is evidence text; Maestro does not execute it.
5. If verification fails, run `maestro query proof <id>`, repair the proof or
   claim, then rerun `maestro task verify <id>`.

## If Verification Fails

- Missing claim: complete the task with the exact observable claim that was
  proven.
- Missing proof: add observed proof with `maestro task complete --proof` or
  record proof using `maestro event create --task-id <id> --claim "<claim>"`.
- Stale proof: rerun the smallest falsifying checks, then rerun
  `maestro task verify <id>`.

## Adversarial fan-out (contested or high-stakes verification)

Use when a task failed verify twice, the task is high-risk, or you are about
to ship a feature on top of many verified tasks.

1. The rubric is `acceptance.yaml` - the locked checks, plus the task's
   completion claims. Never invent a softer rubric.
2. Spawn one fresh verifier per claim or check. Give each ONLY the claim,
   the acceptance check, and the repo - never the worker's reasoning or the
   conversation. Prompt it to REFUTE: "default to refuted if uncertain."
   Each verifier reports exactly one verdict line:
   `upheld|refuted: <check> - <observed evidence>`.
3. Verdicts land durably, not in conversation:
   - upheld -> record the verdict line as evidence:
     `maestro event create --task-id <id> --claim "<verdict line>"`
   - refuted (reproducibly) -> block the task with the refutation:
     `maestro task block <id> --reason "adversarial verifier refuted: <what>"`
     and send it back to work. Do NOT run `task verify` over a refutation.
4. All upheld -> `maestro task verify <id>` as normal.

Never message a running verifier mid-task - isolation is the point; new
information goes into a fresh verifier.

## Feature QA

- Feature accept gates on `qa-baseline` writing
  `.maestro/features/<id>/baseline.md`.
- Feature ship gates on `qa-slice` writing
  `.maestro/features/<id>/qa-slices.yaml`.
- Do not mark a feature shipped until `maestro feature ship <id> --outcome "..."`
  passes without QA blockers.

## Done

- Verified task: report the pass and the next command printed by Maestro.
- Blocked task: record the blocker or state the missing evidence and stop.
- Feature QA: write the requested baseline or slice artifact, then rerun the
  blocked feature command.
- If verification cannot run, state the blocker and the remaining risk instead
  of marking the work complete.

## Hand-off

maestro-design -> maestro-feature -> maestro-task -> [maestro-verify] -> feature ship

Next: task verified, more children live -> back to the `maestro-task` skill;
all children verified -> the `maestro-feature` skill (`qa-slice`, then
`feature ship --outcome "<one line>"`).
Related: `qa-baseline` / `qa-slice` (the gate artifacts this skill's Feature QA section
drives), `maestro-feature` (the ship gate).

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-verify`, and `activation_mode` set to `agent_selected`.
