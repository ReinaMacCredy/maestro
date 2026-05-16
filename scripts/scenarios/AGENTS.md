# scripts/scenarios/

Phase 6 swarm tooling: sandbox prep, rubric dispatch, and result evaluation.

## Files

| File | Purpose |
|------|---------|
| `_scenarios.ts` | Single source of truth: the 8 scenario names + `projectTypeOf()` |
| `swarm.ts` | Prepares sandboxes, fills agent-brief placeholders, writes `last-run.json`, prints dispatch instructions |
| `check.ts` | Single-scenario rubric runner: `bun scripts/scenarios/check.ts <name> <dir>` |
| `check-all.ts` | Multi-scenario runner: reads `last-run.json`, prints summary table |

## Pre-flight

```bash
bun run release:local
```

This rebuilds `dist/maestro` and installs it to `~/.local/bin/maestro`. The
greenfield sandbox prep calls `maestro setup bootstrap` via PATH. Stale or
missing binary -> step fails. Run this before every swarm.

## Swarm workflow

1. Prepare sandboxes and print instructions:

   ```bash
   bun scripts/scenarios/swarm.ts --all
   # or a subset:
   bun scripts/scenarios/swarm.ts --scenarios greenfield-novice-light,brownfield-novice-light
   ```

2. Read what `swarm.ts` printed. For each scenario, open an Agent tool call
   with `run_in_background: true` and paste the contents of the `brief_path`
   shown in the table as the agent prompt.

   **swarm.ts does NOT spawn agents.** That step is always the operator's
   manual action inside an interactive Claude Code session.

3. After all sub-agents finish:

   ```bash
   bun scripts/scenarios/check-all.ts
   ```

## Interpreting check-all.ts output

```
SCENARIO                    STATUS  CHECKS
greenfield-novice-light     PASS    5/5
greenfield-novice-heavy     FAIL    3/4    [task-reached-ready FAIL]
...

OVERALL: 7/8 PASS
```

- PASS = all checks in the rubric matched the `.maestro/evidence/<date>.jsonl`
  trail left by the sub-agent.
- FAIL = one or more checks did not match. The bracketed ids are the failing
  check IDs.

For detailed per-check output, run the single-scenario runner:

```bash
bun scripts/scenarios/check.ts <name> <sandbox-dir>
```

## Triage categories

When a check fails, identify which category applies:

**A - Maestro bug.** The evidence trail shows maestro did not produce the
expected transition or emitted the wrong state. Fix in `src/v2/`. Most common.

**B - Rubric bug.** The predicate is wrong (e.g., wrong field name, wrong
`to_state` value). Fix in `tests/scenarios/<name>/rubric.ts`. Re-run
`check.ts` against the same sandbox -- no re-dispatch needed.

**C - Brief bug.** The sub-agent misunderstood the scenario because the
`agent-brief.md` was ambiguous. Fix the brief, then re-dispatch swarm for
that scenario only.

**D - Agent reasoning failure.** The brief is clear and maestro is correct
but the sub-agent took a wrong path. Re-dispatch swarm for the scenario.
If it fails repeatedly, sharpen the brief (treat as C).

## Re-dispatching failed scenarios only

```bash
bun scripts/scenarios/swarm.ts --scenarios <failed-name>,<other-failed-name>
```

This overwrites `last-run.json` with a new run containing only the
re-dispatched scenarios. Sandboxes from prior runs stay on disk under
`/tmp/maestro-scenario-*`; they are never deleted automatically.

## Cleanup

Sandboxes live under `/tmp/maestro-scenario-*`. Remove manually when done:

```bash
rm -rf /tmp/maestro-scenario-*
```

Do NOT clean up between re-dispatch loops; old sandboxes are inert.
