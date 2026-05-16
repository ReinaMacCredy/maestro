# Scenario: greenfield-expert-heavy

- **Project:** greenfield
- **Familiarity:** expert
- **Mode:** heavy
- **Task shape:** feature (multi-PR with one lint-violation recovery)

## User-mock script

The user knows maestro verb names and uses them explicitly.

1. "Run `maestro setup bootstrap`, then author a heavy-mode spec for a feature:
   a data pipeline that ingests CSV files, transforms rows, and writes output.
   Break it into 3 tasks."
2. "Run `plan from-spec`, then `plan decompose` with at least 3 child tasks.
   Show me the plan."
3. "Claim task-1. Before implementing, create `docs/architecture.yaml` with a
   `passive_harness` forbidden pattern `pollInterval`. Implement the ingest
   step using that pattern so verify catches it."
4. "Now fix the violation -- remove `pollInterval` from the code -- and
   re-verify task-1. Ship it when ready."

## Termination

One of:
- **verify=PASS + ship (task 1)**: after the recovery, task-1 reached `shipped`.
  EXIT PASS.
- **fail-budget**: 3 consecutive verify failures on the same task with no state
  change (excluding the intentional first FAIL). EXIT FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. A `transition` row with `plan_id` set and `to_state: "specified"`.
2. A `transition` row with `plan_id` set and `to_state: "planned"`.
3. At least 2 child task `transition` rows with `to_state: "draft"` and both
   `task_id` and `plan_id` set.
4. A `lint-violation` row (the intentional FAIL before the fix).
5. A `transition` row with `task_id` set and `to_state: "ready"` and
   `verdict: "PASS"` (the recovery verify).
6. A `transition` row with `task_id` set and `to_state: "shipped"`.
