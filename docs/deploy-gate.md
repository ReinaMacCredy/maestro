# Deploy Gate

`maestro deploy gate` runs four deterministic checks against the current task's Spec and Evidence, then records a `deploy-readiness` Evidence row. It does **not** mutate the Verdict — teams wire deploy readiness into `policies/risk.yaml` if they want it to gate merges.

## The four checks

### `feature_flag`

Passes when `Spec.rollout_plan.feature_flag` is a non-empty string.

A feature flag lets teams release the change behind a toggle, reducing blast radius if something goes wrong in production. If the Spec has no rollout plan or the flag name is empty, this check fails.

**Fix:** Add a `rollout_plan.feature_flag` entry to the Spec:

```bash
maestro spec edit --mission <id>
```

### `canary_plan`

Passes when `Spec.rollout_plan.canary.stages` contains at least one stage.

A canary plan defines the percentage rollout schedule. Without it, the deploy has no gradual-rollout strategy on record.

**Fix:** Add at least one canary stage to `Spec.rollout_plan.canary.stages`.

### `rollback`

Passes when at least one `rollback-exercised` Evidence row at `witnessed-by-ci` or stronger exists for the task.

A witnessed rollback proves that the rollback procedure was exercised before the deploy was considered safe. See [Witnessed Rollback](#witnessed-rollback) below.

**Fix:** Run `maestro deploy rollback` in your CI workflow before calling `deploy gate`:

```yaml
- name: Exercise rollback
  run: maestro deploy rollback --task $MAESTRO_TASK_ID --command "./scripts/rollback.sh"
- name: Deploy gate
  run: maestro deploy gate --task $MAESTRO_TASK_ID
```

### `owner`

Passes when `owners.yaml.deploy_approver` contains at least one entry.

This check verifies that at least one human has been designated as a deploy approver for this repo. The approver list is loaded from the **base branch** (Rule 12 pattern — same as sensitive_waiver), so self-promotion via a PR head edit is rejected.

**Fix:** Add a `deploy_approver` block to `.maestro/policies/owners.yaml`:

```yaml
deploy_approver:
  - alice@example.com
  - bob@example.com
```

## Relationship to `Spec.rollout_plan` (schema v2)

Spec schema v2 adds `rollout_plan` alongside `runtime_signals`. A v1 Spec is forward-migrated at read time with `rollout_plan: undefined` — the `feature_flag` and `canary_plan` checks will fail for v1 Specs until you edit the Spec and add rollout plan fields.

Example Spec JSON with a populated `rollout_plan`:

```json
{
  "schema_version": 2,
  "mission_id": "2026-05-10-001",
  "acceptance_criteria": [
    { "id": "crt-0000000000001-aabbccdd", "text": "API returns 200 for valid input" }
  ],
  "non_goals": [],
  "runtime_signals": [
    {
      "name": "p99_latency",
      "provider": "prometheus",
      "query": "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
      "threshold": { "operator": "<", "value": 0.5 },
      "severity": "warn"
    }
  ],
  "rollout_plan": {
    "feature_flag": "my-feature-flag",
    "canary": {
      "stages": [
        { "percent": 5,  "hold_minutes": 30 },
        { "percent": 25, "hold_minutes": 60 },
        { "percent": 100, "hold_minutes": 0  }
      ]
    },
    "rollback_command": "./scripts/rollback.sh"
  },
  "created_at": "2026-05-10T10:00:00.000Z",
  "updated_at": "2026-05-10T10:00:00.000Z"
}
```

## `deploy-readiness` Evidence payload

The `deploy-readiness` Evidence row uses the `DeployReadinessPayload` shape from `src/features/evidence/domain/types.ts`:

```typescript
interface DeployReadinessPayload {
  task_id: string;
  checks: {
    feature_flag: { ok: boolean; value?: string };
    canary_plan:  { ok: boolean; stages?: number };
    rollback:     { ok: boolean; witness_evidence_id?: string };
    owner:        { ok: boolean; approvers?: string[] };
  };
  gate: "pass" | "fail";
}
```

Example recorded payload:

```json
{
  "task_id": "tsk-abc123",
  "checks": {
    "feature_flag": { "ok": true,  "value": "my-feature-flag" },
    "canary_plan":  { "ok": true,  "stages": 3 },
    "rollback":     { "ok": true,  "witness_evidence_id": "evd-0000000000001-rollbk" },
    "owner":        { "ok": true,  "approvers": ["alice@example.com"] }
  },
  "gate": "pass"
}
```

## Troubleshooting

| Failure | Cause | Fix |
|---|---|---|
| `feature_flag: fail` | `Spec.rollout_plan.feature_flag` is absent or empty | Edit the Spec to add a rollout plan with a feature flag name |
| `canary_plan: fail` | No canary stages in `Spec.rollout_plan.canary.stages` | Add at least one canary stage to the Spec |
| `rollback: fail` | No `rollback-exercised` Evidence at `witnessed-by-ci` or stronger | Run `maestro deploy rollback` in CI before calling `deploy gate` |
| `owner: fail` | `owners.yaml.deploy_approver` is empty or absent | Add at least one email to `deploy_approver` in `owners.yaml` |
| `Task not found` | `--task` id does not exist | Run `maestro task list` to find the correct id |
| `owners.yaml not found at <base>` | Base ref does not have an `owners.yaml` | Run `maestro init`, check `--base` ref, or scaffold owners.yaml manually |

## Sample CI workflow snippet

The following is advisory. Teams must wire it themselves. Insert between your build/test step and the merge step.

```yaml
jobs:
  deploy-safety:
    runs-on: ubuntu-latest
    needs: [test, ci-verify]
    steps:
      - uses: actions/checkout@v4

      - name: Install maestro
        run: |
          curl -fsSL https://github.com/your-org/maestro/releases/latest/download/maestro-linux-x64 \
            -o /usr/local/bin/maestro && chmod +x /usr/local/bin/maestro

      # Exercise rollback first — this records rollback-exercised at witnessed-by-ci
      - name: Exercise rollback
        run: maestro deploy rollback --task "$MAESTRO_TASK_ID" --command "./scripts/rollback.sh"

      # Gate checks feature_flag, canary_plan, rollback, owner
      - name: Deploy gate
        run: maestro deploy gate --task "$MAESTRO_TASK_ID" --json
        # Exits 0 = pass, 1 = fail
```

The `deploy gate` step records a `deploy-readiness` Evidence row. To make this gate a merge blocker, add a `deploy-readiness` signal to your `policies/risk.yaml`.

## Witnessed rollback

`maestro deploy rollback` executes the provided shell command and records a `rollback-exercised` Evidence row:

- Witness level is `witnessed-by-ci` when `GITHUB_ACTIONS=true`.
- Witness level is `witnessed-by-maestro` when run locally.

Both levels satisfy the rollback check in `deploy gate`. `agent-claimed-locally` does **not** satisfy the check — the rollback must be mechanically witnessed.
