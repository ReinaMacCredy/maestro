# Scenario: greenfield-novice-heavy

- **Project:** greenfield
- **Familiarity:** novice
- **Mode:** heavy
- **Task shape:** feature (multi-PR)

## User-mock script

The user does not know maestro verb names. They describe intent only.

1. "I want to set up this project with maestro so I can track our work."
2. "We need to build a user authentication system: registration, login, and
   password reset. It's a big feature -- probably multiple PRs worth of work."
3. "That plan looks good. Break it into tasks and start on the first one."
4. "The first task looks done. Ship it."

## Termination

One of:
- **verify=PASS + ship (task 1)**: the first child task reached `shipped`. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures on the active task with no
  state change between attempts. EXIT FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. A `transition` row with `plan_id` set and `to_state: "specified"` (plan
   created from heavy-mode spec via `plan from-spec`).
2. A `transition` row with `plan_id` set and `to_state: "planned"` (plan
   decomposed into child tasks via `plan decompose`).
3. A `transition` row with `plan_id` set and `to_state: "in-progress"` (first
   child task claimed, plan auto-advanced per ADR-0011).
4. At least two `transition` rows with `task_id` set and `to_state: "draft"` --
   confirming multiple child tasks were created.
5. A `transition` row with `task_id` set and `to_state: "claimed"`.
6. A `transition` row with `task_id` set and `to_state: "shipped"`.
