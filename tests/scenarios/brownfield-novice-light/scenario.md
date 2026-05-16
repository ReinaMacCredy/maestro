# Scenario: brownfield-novice-light

- **Project:** brownfield
- **Familiarity:** novice
- **Mode:** light
- **Task shape:** feature

## User-mock script

The user does not know maestro verb names. They describe intent only. The
project directory has a v1 `.maestro/` tree; the agent must discover this
and migrate unprompted.

1. "I have an existing project here that uses maestro. I want to add a CSV
   export feature to the reporting module."
2. "Go ahead and work on it."
3. "Looks good. Ship it."

## Termination

One of:
- **verify=PASS + ship**: task reached `shipped`. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures with no state change. EXIT
  FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. File `.maestro/.migrated-v2.json` is present (migration ran).
2. File `docs/principles/legacy/legacy-rule-1.md` is present (corrections
   migrated from the v1 fixture's single correction file).
3. A `transition` row with `task_id` set and `to_state: "draft"`.
4. A `transition` row with `task_id` set and `to_state: "claimed"`.
5. A `transition` row with `task_id` set and `to_state: "ready"` and
   `verdict: "PASS"`.
6. A `transition` row with `task_id` set and `to_state: "shipped"`.
