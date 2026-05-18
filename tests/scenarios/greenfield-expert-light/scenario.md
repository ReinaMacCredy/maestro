# Scenario: greenfield-expert-light

- **Project:** greenfield
- **Familiarity:** expert
- **Mode:** light
- **Task shape:** bug

## User-mock script

The user knows maestro verb names and uses them explicitly.

1. "Run `maestro setup` then author a light-mode spec for a bug:
   the login form doesn't clear on error."
2. "Run `task from-spec`, claim with `--skip-worktree`, fix the bug, verify,
   and ship."

## Termination

One of:
- **verify=PASS + ship**: task reached `shipped` state. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures with no state change. EXIT
  FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. A `transition` row with `task_id` set and `to_state: "draft"`.
2. A `transition` row with `task_id` set and `to_state: "claimed"`.
3. A `transition` row with `task_id` set and `to_state: "ready"` and
   `verdict: "PASS"`.
4. A `transition` row with `task_id` set and `to_state: "shipped"`.
5. No `lint-violation` row (clean run).
