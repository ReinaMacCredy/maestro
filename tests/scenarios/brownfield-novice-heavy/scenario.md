# Scenario: brownfield-novice-heavy

- **Project:** brownfield
- **Familiarity:** novice
- **Mode:** heavy
- **Task shape:** feature (multi-PR)

## User-mock script

The user does not know maestro verb names. They describe intent only. The
project has a v1 `.maestro/` tree that must be migrated.

1. "I have an older maestro project here. I want to build a notification system:
   email alerts, in-app banners, and a preferences panel. It's a big feature."
2. "That plan looks right. Break it into tasks and start on the first one."
3. "The first task is done. Ship it."

## Termination

One of:
- **verify=PASS + ship (task 1)**: first child task shipped. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures on active task with no state
  change. EXIT FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. File `.maestro/.migrated-v2.json` present (migration ran unprompted).
2. File `docs/principles/legacy/legacy-rule-1.md` present (corrections migrated).
3. A `transition` row with `mission_id` set and `to_state: "approved"`.
4. A `transition` row with `mission_id` set and `to_state: "planned"`.
5. At least 2 child task `transition` rows with `to_state: "draft"` and both
   `task_id` and `mission_id` set.
6. A `transition` row with `task_id` set and `to_state: "claimed"`.
7. A `transition` row with `task_id` set and `to_state: "shipped"`.
