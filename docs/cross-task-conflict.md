# Cross-Task Conflict Detection

`maestro ci verify` (L8.1) detects when other open PRs touch overlapping file paths and records a `kind=cross-task-conflict` Evidence row when overlap is found. The Risk Engine raises the effective risk class one tier per conflict signal.

## How detection works

Detection runs as part of the normal `ci verify` flow after the diff is resolved:

1. **Port call** — `ci verify` calls `ConflictDetectorPort.listOpenPrPaths(thisPr)`, which returns a map of `{ prNumber → changedPaths[] }` for all open PRs except the current one.
2. **Adapter** — `GhCliConflictDetectorAdapter` (at `src/features/ci/adapters/gh-cli-conflict-detector.adapter.ts`) shells out to `gh api /repos/{owner}/{repo}/pulls` and then fetches each PR's files via `gh api /repos/{owner}/{repo}/pulls/{n}/files`.
3. **Overlap check** — the use-case intersects the current PR's changed-file list against each other PR's changed-file list. A path counts as overlapping when it appears in both lists.
4. **Evidence record** — if any overlap is found, a `kind=cross-task-conflict` Evidence row is recorded at `witnessed-by-ci` and passed to the Risk Engine. If no overlap is found, no row is written.
5. **Risk raise** — the Risk Engine applies a one-tier raise per conflict signal to the effective risk class. The raise is capped at `critical`. Even if multiple conflict rows exist for the same run, the raise is clamped to one tier total.

### Port interface

```typescript
// src/features/ci/ports/conflict-detector.port.ts
interface ConflictDetectorPort {
  listOpenPrPaths(excludePr: number): Promise<Map<number, string[]>>;
}
```

### Use-case location

`src/features/ci/usecases/detect-cross-task-conflicts.usecase.ts`

## Payload schema

The `cross-task-conflict` Evidence row uses `CrossTaskConflictPayload`:

```typescript
interface CrossTaskConflictPayload {
  thisPr: number;
  conflictingPrs: number[];
  overlappingPaths: string[];
}
```

Example recorded payload:

```json
{
  "thisPr": 142,
  "conflictingPrs": [138, 141],
  "overlappingPaths": [
    "src/features/auth/session.ts",
    "src/features/auth/types.ts"
  ]
}
```

Inspect a recorded row with:

```bash
maestro evidence list --task <id> --kind cross-task-conflict
maestro evidence show <evidence-id>
```

## Risk impact

| Effective class before signal | Effective class after signal |
|---|---|
| `low` | `medium` |
| `medium` | `high` |
| `high` | `critical` |
| `critical` | `critical` (already at cap) |

The raise is applied before the Verdict decision tree. A task that was heading for `PASS` at `medium` risk may become `HUMAN` or `BLOCK` at `high` depending on the team's autopilot policy thresholds.

**Multi-row clamping:** if `ci verify` records two or more `cross-task-conflict` rows in the same run (each listing different conflicting PRs), the Risk Engine still applies only a single one-tier raise. Rows are deduplicated by kind before the raise is computed.

## What to do when you see a conflict row

1. Run `maestro evidence show <id>` to see which PRs and paths overlap.
2. Check `conflictingPrs` — open those PRs and assess whether the overlapping changes will interfere.
3. If the conflicts represent genuine contention, coordinate with the other PR author before merging. The safest resolution is to merge the lower-risk PR first, rebase this PR, and re-run `ci verify`.
4. If the overlap is incidental (for example, two PRs both update the same config comment), document the coordination outcome in a `manual-note` Evidence row. That note does not clear the conflict row, but it gives the human reviewer context.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| No `cross-task-conflict` row despite known overlapping PRs | `gh api` call failed or `GITHUB_TOKEN` lacks `pull_requests: read` scope | Check that `GITHUB_TOKEN` has PR read access; review `ci verify` log output for `[warn] conflict detector: ...` lines |
| Conflict row listed but overlapping PR is already merged | Timing race: the other PR merged after files were fetched | Rebase and re-run `ci verify`; the merged PR will no longer appear in the open-PR list |
| Risk class raised unexpectedly | A `cross-task-conflict` row exists in the Evidence store from a prior run | Run `maestro evidence list --task <id> --kind cross-task-conflict` to confirm; if the conflict is resolved, re-run `ci verify` so a fresh run produces no new rows |
| `gh api` rate-limit error | CI is making too many GitHub API calls | The check is non-fatal — `ci verify` logs a warning and continues. If rate-limiting is chronic, consider running `ci verify` less frequently or requesting a higher token rate limit |

## What counts as overlap

A path is considered overlapping when it appears in **both** this PR's changed-file list (as reported by the GitHub API) and at least one other open PR's changed-file list. The comparison is exact path-string equality — no glob expansion or directory prefix matching.

Paths that appear in a PR's changed-file list include: added, modified, renamed (both old and new path), and deleted files. Copy-only changes are included on the new path.

## Non-fatal on API errors

If the `gh api` call fails for any reason (network error, missing token, rate-limit, permissions), `ci verify` logs a warning at the `[warn]` level and skips the conflict detection step entirely. The verify step does not fail and no Evidence row is written. Treat a missing conflict row as "conflict status unknown" rather than "no conflicts."

This design prevents `ci verify` from becoming a hard dependency on GitHub API availability. Teams that want the check to be required should add a policy signal for `cross-task-conflict` Evidence to `policies/risk.yaml`.
