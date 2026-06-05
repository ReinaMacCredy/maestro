---
name: qa-baseline
version: 1.1.0
description: "Use before feature accept to capture baseline.md: a short real-scenario behavior contract with stable bl-NNN ids for the later ship gate."
---

# QA Baseline

Create `.maestro/features/<id>/baseline.md` before feature edits start. This is
the behavior oracle for `feature ship`, not a list of tests.

Activate: record `skill_activation` for `qa-baseline` with
`activation_mode=agent_selected` through `maestro hook record`.

## Use

- `maestro feature accept` is blocked on missing or empty `baseline.md`.
- A behavioral amend added acceptance or area and the baseline must be fresh.
- The feature touches user-visible, data, security, persistence,
  compatibility, release, or workflow behavior.

## Do

1. Read `maestro feature show <id>` for acceptance criteria and areas.
2. Identify the smallest real user/operator scenarios that could regress.
   Cover risk dimensions that matter: actor, entrypoint, state/lifecycle, data
   shape, environment/channel, integration boundary, permissions/trust, failure
   recovery, and non-functional behavior.
3. Include a workflow chain when the feature changes a trunk journey, for
   example setup -> create work -> record proof -> verify -> inspect output.
   Keep isolated probes for safety-critical local invariants such as rollback,
   auth, parsers, schemas, install ownership, migration, and destructive guards.
4. Give each behavioral scenario a stable `[bl-NNN]` id. These ids are the ship
   coverage units. No behavioral surface means record that explicitly and use
   no ids.
5. Capture the current-behavior oracle: setup, action, expected observable
   result, evidence to capture, and reproduction steps.
6. Write the contract below to `.maestro/features/<id>/baseline.md`.

## Freshness

The frontmatter tracks how far through `amend-log.yaml` this baseline is fresh:

```markdown
---
amend_log_position: 0
---
```

At first accept, use `0`. After behavioral amends, set it to the current count
of amend-log entries after adding the new scenarios. Missing or invalid
frontmatter is treated as `0`.

## Output Shape

```markdown
---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: <feature id and surface>
- Critical workflow chains:
  - <chain name or None>
    - Steps: <setup -> action -> downstream consumer -> recovery/cleanup if relevant>
    - Touched link: <link changed by this feature, or None>
    - Minimal proof: <command/manual flow/artifact comparison>
- Scenario Matrix:
  - [bl-001] <scenario name>
    - Dimensions: <actor/entrypoint/state/data/environment/integration/trust/failure/non-functional>
    - Setup: <state, fixture, account, repo, command, or manual precondition>
    - Action: <real command/click/API call/user flow>
    - Oracle: <observable pass condition>
    - Evidence to capture: <output, screenshot, artifact, state file, log, response>
    - Reproduction: <steps a developer/operator can rerun>
- Preserved behaviors:
  - <behavior> -> Proof: `<command>` or <manual artifact/check>
- Changed behaviors:
  - <intentional change, or None>
- Critical probes before commit:
  - <probe> -> `<command>` or <manual check>
- Required artifacts:
  - <path/description, or None>
- Baseline gaps:
  - <gap> -> Proposed probe: <smallest useful check>
```

## Stop

- Do not edit implementation code from this skill.
- Do not use "tests pass" as the baseline by itself. Name the observable
  behavior or artifact the tests protect.
- Do not add scenarios to pad coverage. Add `[bl-NNN]` only for real behavior
  the feature can break.
- Do not seek exhaustive QA. Cover the highest-risk behavior that can
  realistically regress.

## Hand-off

Next: baseline written -> `maestro-feature` for `feature accept`.
