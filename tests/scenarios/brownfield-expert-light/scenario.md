# Scenario: brownfield-expert-light

- **Project:** brownfield
- **Familiarity:** expert
- **Mode:** light
- **Task shape:** bug

## User-mock script

The user knows maestro verb names and uses them explicitly.

1. "Run `maestro setup migrate-v2` to upgrade to v2, then author a light-mode
   spec for a bug: the search filter ignores accented characters."
2. "Run `task from-spec`, claim with `--skip-worktree`, fix the bug, verify,
   and ship."

## Termination

One of:
- **verify=PASS + ship**: task reached `shipped`. EXIT PASS.
- **fail-budget**: 3 consecutive verify failures with no state change. EXIT
  FAIL-BUDGET.
- **scenario-timeout**: 20 minutes elapsed. EXIT TIMEOUT.

## Expected evidence (informational)

The rubric checks for:

1. File `.maestro/.migrated-v2.json` present (explicit migrate-v2 ran).
2. File `docs/principles/legacy/legacy-rule-1.md` present (corrections
   migrated from v1 fixture).
3. A `transition` row with `task_id` set and `to_state: "draft"`.
4. A `transition` row with `task_id` set and `to_state: "claimed"`.
5. A `transition` row with `task_id` set and `to_state: "ready"` and
   `verdict: "PASS"`.
6. A `transition` row with `task_id` set and `to_state: "shipped"`.
