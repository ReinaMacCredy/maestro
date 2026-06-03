---
name: maestro-verify
version: 1.1.0
description: Verification protocol for Maestro tasks and feature work.
---

# Maestro Verify

Use this skill when proving a task or feature is complete.

## Task Runbook

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

## Feature QA

- Feature accept gates on `qa-baseline` writing
  `.maestro/features/<id>/baseline.md`.
- Feature ship gates on `qa-slice` writing
  `.maestro/features/<id>/qa-slices.yaml`.
- Do not mark a feature shipped until `maestro feature ship <id> --outcome "..."`
  passes without QA blockers.

If verification cannot run, state the blocker and the remaining risk instead of
marking the work complete.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-verify`, and `activation_mode` set to `agent_selected`.
