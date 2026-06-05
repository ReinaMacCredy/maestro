---
name: qa-slice
version: 1.1.0
description: "Use after implementation waves to replay affected baseline scenarios and record qa-slices.yaml evidence for every shipped behavioral bl-NNN."
---

# QA Slice

Replay affected baseline scenarios after an implementation wave and record the
evidence in `.maestro/features/<id>/qa-slices.yaml`. The ship gate counts only
slices with scenario ids and non-empty evidence.

Activate: record `skill_activation` for `qa-slice` with
`activation_mode=agent_selected` through `maestro hook record`.

## Use

- After a task wave changes feature behavior.
- Before `maestro feature ship`.
- When ship reports uncovered `[bl-NNN]` scenarios or stale QA evidence.

## Do

1. Read changed files/commands, `maestro feature show <id>`, and
   `.maestro/features/<id>/baseline.md`.
2. Select the affected `[bl-NNN]` scenarios. If the wave adds behavior, extend
   the baseline with a new id instead of hiding it behind a unit test.
3. Run the smallest useful probes:
   focused tests for local invariants, plus a real command/manual/API/UI flow
   when composition risk exists.
4. Compare against the baseline. Unexplained output, schema, state, permission,
   performance, compatibility, or UI drift is a blocker.
5. Append a counting slice to `qa-slices.yaml`.
6. If blocked, return a tracker entry with expected vs actual, reproduction,
   evidence, and fix path. Do not fix code from this skill.

## qa-slices.yaml

Keep this append-only shape. Scenario ids must match baseline digits exactly:
`bl-001` and `bl-1` are different.

```yaml
slices:
  - at: "2026-05-31T00:00:00Z"
    scenarios: ["bl-001", "bl-002"]
    probes: ["cargo test --test feature_domain"]
    result: pass
    evidence:
      - "feature_domain: 12 passed; 0 failed"
      - "manual: feature new -> accept -> ship round-trips on temp .maestro"
```

Required for the gate: `scenarios` and `evidence`. Other fields are optional.
If the file does not parse, the ship gate prints the path, parse error, and the
expected shape.

## Output When Blocked

```markdown
### Gate Tracker - QA Slice

- [ ] [qs-001] <severity/confidence> <surface> - <behavior drift or missing proof>
  - Scenario: [bl-NNN] <scenario name and dimensions>
  - Expected: <baseline behavior>
  - Actual: <actual behavior or missing proof>
  - Reproduction: <steps/command/manual flow>
  - Evidence: <command/output/manual check>
  - Artifact: <path/screenshot/output/log/state snapshot, or None>
  - Fix path: <recommended fix or probe>
  - Verification: <command or check>
```

If clean:

```markdown
### QA Slice

- No blocking QA findings for <wave/scope>.
- Workflow chains replayed: <chain names and steps, or None>
- Scenarios replayed: <bl-NNN scenario names>
- Probes run: `<command>`, <manual check>
- Artifacts captured: <paths/descriptions or None>
```

## Stop

- Do not count a slice without scenario ids and evidence.
- Do not let a changed journey rely only on a unit test when a real observable
  flow is feasible.
- Do not drop focused proof for safety-critical invariants.
- Do not block on broad nice-to-have coverage; record follow-up unless the
  feature goal depends on it.

## Hand-off

Next: all behavioral baseline ids covered -> `maestro-feature` for
`feature ship --outcome "<one line>"`.
