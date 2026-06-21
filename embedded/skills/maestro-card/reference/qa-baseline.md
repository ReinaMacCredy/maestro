# QA Baseline

Create `.maestro/cards/<id>/qa.md` before feature edits start. This is the
behavior oracle for `feature close`, not a list of tests.

## Use

- `maestro feature accept` is blocked on missing or empty `qa.md`.
- A behavioral amend added acceptance or area and the baseline must be fresh.
- The feature touches user-visible, data, security, persistence,
  compatibility, release, or workflow behavior.

## No behavioral surface

A feature with nothing to QA skips the baseline entirely — accept it with a
declaration instead of a `qa.md`:

```sh
maestro feature accept <id> --qa none --reason "<why there is nothing to QA>"
```

This waives the baseline at accept and lets the feature close with no slices; the
reason persists on the feature and prints on `feature show` / `feature spec`. The
waiver stays fresh until a *behavioral* amend lands: adding acceptance or an
affected area re-arms the full gate, so capture a real `qa.md` then (re-declare
`--qa none` only if the new scope is still non-behavioral). A non-behavioral
amend — a non-goal or open question — leaves the waiver intact.

Reach for `--qa none` only when the change is behavior-keyed light: docs- or
config-only, or mechanical/structural code with behavior held constant and
already covered by the existing suite. Any change that adds or alters observable
behavior has a surface — write a real baseline below, however small.

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
4. Give each behavioral scenario a stable `[bl-NNN]` id. These ids are the close
   coverage units. No behavioral surface means record that explicitly and use
   no ids. A scenario line may declare `(covers: ac-N)` to tie it to an
   acceptance criterion.
5. Capture the current-behavior oracle: setup, action, expected observable
   result, evidence to capture, and reproduction steps.
6. Write the contract below to `.maestro/cards/<id>/qa.md`.

MCP: when available, `maestro_qa_baseline` records the observed baseline through
the normal QA gate. If there is no behavioral surface, use
`maestro_feature_accept` with `qa.mode = "none"` and a non-empty reason.

## Freshness

The frontmatter tracks how far through the feature's amend log this baseline
is fresh:

```markdown
---
amend_log_position: 0
---
```

At first accept, use `0`. After behavioral amends, set it to the current count
of amend entries after adding the new scenarios. Missing or invalid
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

- Do not edit implementation code from this reference.
- Do not use "tests pass" as the baseline by itself. Name the observable
  behavior or artifact the tests protect.
- Do not add scenarios to pad coverage. Add `[bl-NNN]` only for real behavior
  the feature can break.
- Do not seek exhaustive QA. Cover the highest-risk behavior that can
  realistically regress.

## Hand-off

Next: baseline written -> [feature.md](feature.md) for `feature accept`.
