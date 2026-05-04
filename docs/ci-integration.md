# CI Integration

Maestro is local-first, but **CI is the authoritative verifier.** Local `maestro task verify` and `maestro verdict request` runs are advisory; the verdict computed by `maestro ci verify` inside GitHub Actions and posted as a GitHub Check is the merge gate.

## Workflow template

The `maestro-setup` skill installs a starter workflow at `.github/workflows/maestro-verify.yml` (copied from `skills/bundled/maestro-setup/reference/github-workflow/maestro-verify.yml.template`). It triggers on `pull_request`, installs the maestro binary via curl-extract, and runs `maestro ci verify`.

To customize:
- pin `MAESTRO_VERSION` to your team's chosen release tag.
- adjust the trigger paths if you don't want every PR verified.
- add additional CI jobs upstream of `maestro ci verify` and pass their outputs via `CI_TEST_RESULTS_FILE` (see "Witness ingestion" below).

Required permissions:
- `contents: read` — for `actions/checkout@v4`.
- `pull-requests: write` — to attach to the PR.
- `checks: write` — to post the GitHub Check.

## `maestro ci verify` CLI

```
maestro ci verify [--pr <n>] [--task <id>] [--base <ref>] [--json]
```

By default reads CI env (`GITHUB_ACTIONS`, `GITHUB_REPOSITORY`, `GITHUB_REF`, `GITHUB_SHA`, `GITHUB_BASE_REF`, `GITHUB_EVENT_PATH`, `GITHUB_OUTPUT`, `GITHUB_TOKEN`). PR number is read from `GITHUB_EVENT_PATH` JSON when present; flags override. Task ID is required (one task per PR is the L5 contract).

Steps:
1. Resolve PR + base/head from env or flags.
2. Run Trust Verifier against the diff.
3. Ingest CI job results from `CI_TEST_RESULTS_FILE` if set, recording each as `kind=command` or `kind=verifier` Evidence at `witnessed-by-ci`.
4. Compute the Verdict via the existing `requestVerdict` use-case.
5. Write `verdict_id`, `verdict_decision`, `effective_risk_class` to `$GITHUB_OUTPUT`.
6. When running in GitHub Actions with a token, POST a GitHub Check (success / failure / action_required) via `gh api` (PATCH on subsequent runs to the same `(PR, tree_sha)`).

Exit codes mirror `verdict request`:
- 0 — PASS
- 1 — FAIL
- 2 — HUMAN
- 3 — BLOCK

## Witness ingestion

`witnessed-by-ci` is the second-strongest level on the 4-level witness ladder (only `witnessed-by-maestro` is stronger). Evidence ingested via `maestro ci verify` carries this level when the job actually ran inside the GitHub Actions container; locally-claimed evidence stays at `agent-claimed-locally`.

The Risk Engine compares the witness level against the autopilot policy threshold for the diff's effective risk class. A diff that needs `witnessed-by-ci` and only has `agent-claimed-locally` evidence yields `FAIL` until the missing witness lands.

## Verdict identity by tree SHA

Verdicts are bound to `(pr?, tree_sha)`. The `tree_sha` is `git rev-parse HEAD^{tree}` — the SHA of the file tree, not the commit. Two consequences:

- **Squash survives.** Rebasing a clean diff into a single commit preserves the tree SHA, so the prior `PASS` verdict still matches the PR head.
- **Force-push invalidates.** Rewriting the diff to different content produces a different tree SHA; the prior verdict no longer matches `--pr <n>`. CI re-runs and re-computes.

Inspect locally:
```
maestro verdict show --pr <n>
```
Returns the latest verdict whose `subject.tree_sha` matches the current `HEAD^{tree}`.

## Troubleshooting

- **Check posted but my PR is stuck on "expected" status:** Confirm the GitHub branch protection rule lists "Maestro Verdict" as a required check. The check name is fixed.
- **`maestro ci verify` fails with `No contract found for task <id>`:** Ensure the contract was created on the PR branch via `maestro task new` or `maestro contract amend` before `maestro ci verify` runs. The CI workflow runs against the PR branch's checked-out source.
- **Verdict was PASS earlier but FAIL after squash:** Should not happen — `tree_sha` is content-addressed, so a clean squash retains the prior verdict. If it does happen, the squash actually changed file content (whitespace, line endings, etc.). Use `git show -p HEAD` to confirm.
- **Verdict re-computed even though I only renamed the branch:** Branch ref doesn't enter `tree_sha`. The recompute came from a content change, even if the rename made it look superficial. Inspect `git diff <prior-head> HEAD`.

## Reference

- L5 phases — `ROADMAP.md` §L5.
- Witness ladder — `docs/witness-levels.md`.
- Risk derivation — `docs/risk-class-derivation.md`.
- Source — `src/features/ci/`.
