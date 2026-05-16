# Scenario: brownfield-expert-heavy

- **Project:** brownfield
- **Familiarity:** expert
- **Mode:** heavy
- **Task shape:** feature (with `--verdict block` recovery cycle)

## User-mock script

The user knows maestro verb names and uses them explicitly.

1. "Migrate to v2 first: run `maestro setup migrate-v2`."
2. "Author a heavy spec for a reporting module: weekly digest, PDF export, and
   email delivery."
3. "Run `plan from-spec` then `plan decompose` -- break into at least 3 tasks."
4. "Claim task-1. Implement it. Then run verify with `--verdict block --reason
   'email service not configured'`."
5. "Email service is now available. Re-verify task-1 and ship it."

## Termination

One of:
- **verify=PASS + ship (task 1)**: task-1 shipped after block recovery. EXIT PASS.
- **fail-budget**: 3 consecutive verify FAILs on task-1 (after the intentional
  BLOCK) with no state change. EXIT FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. File `.maestro/.migrated-v2.json` present.
2. File `docs/principles/legacy/legacy-rule-1.md` present.
3. A `transition` row with `plan_id` set and `to_state: "specified"`.
4. A `transition` row with `plan_id` set and `to_state: "planned"`.
5. At least 2 child task rows with `to_state: "draft"` and `plan_id` set.
6. A `transition` row with `task_id` set and `to_state: "blocked"` and
   `verdict: "BLOCK"` (the explicit block from message 4).
7. A `transition` row with `task_id` set and `to_state: "ready"` and
   `verdict: "PASS"` (the recovery verify from message 5).
8. A `transition` row with `task_id` set and `to_state: "shipped"`.
