# Scenario: greenfield-novice-light

- **Project:** greenfield
- **Familiarity:** novice
- **Mode:** light
- **Task shape:** feature

## User-mock script

The user does not know maestro verb names. They describe intent only.

1. "I want to set up this project with maestro so I can track my work."
2. "I need to add a greeting endpoint that returns 'Hello, <name>'."
3. "Go ahead and get started on it."
4. "Looks good. Ship it."

## Termination

One of:
- **verify=PASS + ship**: task reached `shipped` state. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures with no state change between
  attempts. EXIT FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. A `transition` row with `to_state: "draft"` and `task_id` set (task created
   from spec).
2. A `transition` row with `to_state: "claimed"` and `task_id` set.
3. A `transition` row with `to_state: "ready"` and `verdict: "PASS"` (verify
   succeeded).
4. A `transition` row with `to_state: "shipped"` and `task_id` set.
5. No `lint-violation` row (clean run expected).
