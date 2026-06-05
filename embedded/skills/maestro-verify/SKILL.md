---
name: maestro-verify
version: 1.5.0
description: "Use to prove a Maestro task or feature gate with recorded evidence, repair failed proof, and run adversarial verification for high-risk claims."
---

# Maestro Verify

Use this when a task needs verification, proof failed, or a feature gate asks
for QA evidence.

Activate: record `skill_activation` for `maestro-verify` with
`activation_mode=agent_selected` through `maestro hook record`.

## Do

1. Start with `maestro status` or `maestro task next`.
2. Inspect the work:
   `maestro task show <id>` and the locked acceptance checks, or
   `maestro feature show <id>` for feature QA blockers.
3. Run the smallest checks that can falsify the claim from the repo root.
   Record exact command/manual probe and outcome.
4. Complete with matching claim and proof when the task is not yet completed:

   ```sh
   maestro task complete <id> \
     --summary "<what changed>" \
     --claim "<claim>" \
     --proof "<observed evidence>"
   ```

5. If verification fails, run `maestro query proof <id>`, repair the claim or
   evidence, then `maestro task verify <id>`.

## Repair

- Missing claim: complete or update the task with the exact observable claim
  that was proven.
- Missing proof: add `--proof` through `task complete`, or record manual proof:
  `maestro event create --task-id <id> --claim "<claim>"`.
- Stale proof: rerun the falsifying checks and verify again.
- Failed command: fix the work or acceptance command; do not mark verified by
  narration.

## Adversarial Fan-out

Use when a task failed verify twice, risk is high, or many tasks support a
feature ship.

1. Rubric is `acceptance.yaml` plus completion claims. Do not invent softer
   checks.
2. Spawn one fresh verifier per claim/check. Give only the claim, the check, and
   the repo. Ask it to refute and default to refuted if uncertain.
3. Each verifier returns one line:
   `upheld|refuted: <check> - <observed evidence>`.
4. Record upheld verdicts as evidence:
   `maestro event create --task-id <id> --claim "<verdict line>"`.
5. For reproducible refutations, block the task:
   `maestro task block <id> --reason "adversarial verifier refuted: <what>"`.
   Do not run `task verify` over a refutation.
6. All upheld -> `maestro task verify <id>`.

Never message a running verifier with new context. Start a fresh verifier.

## Feature QA

- Accept blocker says `qa-baseline` -> write
  `.maestro/features/<id>/baseline.md` with that skill, then rerun accept.
- Ship blocker says `qa-slice` -> write/update
  `.maestro/features/<id>/qa-slices.yaml` with that skill, then rerun ship.
- Do not report a feature shipped until
  `maestro feature ship <id> --outcome "<one line>"` passes.

## Stop

- Do not replace evidence with confidence.
- Do not weaken acceptance checks after implementation to make proof pass.
- Do not continue silently when verification cannot run; state the blocker and
  remaining risk.

## Hand-off

Pipeline: `maestro-design -> qa-baseline -> maestro-feature -> maestro-task -> [maestro-verify] -> qa-slice -> feature ship`

Next: verified task with live siblings -> `maestro-task`; all children verified
-> `qa-slice`, then `maestro-feature`.
