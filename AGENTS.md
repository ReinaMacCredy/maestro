@CLAUDE.md

# PROJECT KNOWLEDGE BASE

**Generated:** 2026-04-24 09:16:42 +0700
**Commit:** 8b4a2d76
**Branch:** main

## OVERVIEW
Maestro is a local-first conductor for multi-agent software engineering. It is a single-package Bun/TypeScript CLI with an OpenTUI dashboard, repo-owned agent surfaces, and shared project state under `.maestro/`.

## STRUCTURE
```text
maestro/
├── .factory/    # committed bootstrap/reference assets
├── .maestro/    # repo-tracked project state, plans, tasks, and context
├── hooks/       # session/tool hook entrypoints
├── scripts/     # build, version, install, release, and TUI helpers
├── skills/      # shipped built-in and bundled skill sources
├── src/         # feature-first CLI + TUI source tree
└── tests/       # unit, integration, and compiled-binary coverage
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI entry and command registration | `src/index.ts` | Commander root; keeps registration thin |
| Dependency wiring | `src/services.ts` | Composition root only |
| Feature boundaries and imports | `src/features/`, `scripts/check-feature-boundaries-lib.ts` | Cross-feature deep imports are forbidden |
| Mission Control flow | `src/infra/commands/mission-control.command.ts`, `src/tui/README.md`, `src/tui/state/snapshot.ts` | Preview, JSON, and render-check stay read-only |
| Handoff launch and pickup | `src/features/handoff/`, `src/features/task/` | Handoff packets can resume linked tasks; standalone packets stay prompt-only |
| Evidence logbook | `src/features/evidence/` | Record, list, and show task evidence rows; storage under `.maestro/evidence/` (gitignored) |
| Shipped agent skills | `skills/built-in/`, `skills/bundled/`, `scripts/sync-*-skills.ts` | Both template embeds under `src/infra/domain/` are generated |
| Release and install behavior | `scripts/build.ts`, `scripts/ci.ts`, `scripts/install-local.ts`, `.github/workflows/` | `ci.ts` is local release-prep, not a harmless smoke script |
| Daily task loop vs mission workflow | `.maestro/tasks/tasks.jsonl`, `README.md`, `.maestro/MAESTRO.md` | `task` and `mission` are separate systems |
| Compiled-binary verification | `tests/e2e/`, `tests/helpers/run-compiled-cli.ts` | Distinguish `./dist/maestro` from installed `maestro` |
| Mission Spec (acceptance criteria, non-goals) | `src/features/spec/` | `spec show/edit --mission <id>`; stored under `.maestro/missions/<id>/spec.json` |
| Policy and owners loader | `src/features/policy/` | Loads `.maestro/policies/owners.yaml`; four roles: `policy_approver`, `ratchet_approver`, `sensitive_waiver`, `deploy_approver`. Extended in L3 with `RiskPolicy`, `AutopilotPolicy`, `ReleasePolicy` loaders, asymmetric edit classifier, and effective-policy use-case. |
| Trust Verifier | `src/features/verify/` | Runs 6 checks (scope, lockfile, generated, sensitive-paths, commit-metadata, secrets) against a diff + contract |
| `maestro ci verify` (L5/L8.1)         | `src/features/ci/` | Reads CI env (`GITHUB_*`), resolves PR + diff, runs Trust Verifier, ingests CI job results as `witnessed-by-ci` Evidence, computes Verdict via existing request-verdict use-case. Posts a GitHub Check (success/failure/action_required) per verdict decision. L8.1 added cross-task conflict detection: when other open PRs touch overlapping paths, `ci verify` records a `kind=cross-task-conflict` Evidence row and the Risk Engine raises the effective risk class one tier per signal (capped at `critical`; multiple rows still raise by one tier total). |
| ProofMap builder (L3) | `src/features/verify/usecases/proof-map.ts` | Joins `Spec.acceptance_criteria` with Evidence rows to produce a per-criterion coverage map |
| Versioned contract storage and amendments | `src/features/task/domain/contract/` | `ContractVersionStorePort`; `contract show/amend/history` verbs enforce `amendmentBudget` (Rules 3–7) |
| Risk Engine (L3) | `src/features/risk/` | `computeRisk` and `deriveRiskClassFromDiff` implement the deterministic signal-to-risk-class mapping; `risk-class-order.ts` compares levels |
| Verdict types, store, and commands (L3) | `src/features/verdict/` | `Verdict` domain types, file-system store adapter, `verdict request` use-case (decision tree: PASS/FAIL/HUMAN/BLOCK), `verdict show` and `verdict request` commands |
| Plan-check use-case + CLI (L4) | `src/features/plan/` | `checkPlan` use-case at `usecases/check-plan.ts`; three deterministic checks: `scope-widens`, `missing-proof`, `risk-class-too-low`; `plan-check.command.ts` registers the `plan check` verb |
| Cost-budget run-state + budget verb (L4) | `src/features/task/adapters/fs-run-state-store.adapter.ts`, `src/features/task/usecases/check-cost-budget.ts` | `RunState` persisted under `.maestro/runs/<task-id>/state.json` (gitignored); `checkCostBudget` short-circuits Risk Engine to BLOCK when exhausted (Rule 11); `retryCount` auto-increments on FAIL/HUMAN |
| AI Reviewer + Threat-Model Risk Engine wiring (L4) | `src/features/risk/usecases/compute-risk.ts` | Applies `ai-review` error-severity raises (Rule 1 — raises only; security-reviewer error always lifts to `critical`); adds `threat-model-required` predicate (Edge Case 12) consumed by the Verdict use-case |
| Mission Control autopilot view (L4) | `src/tui/state/autopilot-screen.ts` | Mission-mode only read model; consumed by Mission Control preview/render paths |
| Auto-merge eligibility + `merge auto` | `src/features/merge/` | 8 deterministic predicates; opt-in via `autopilot.yaml`; see `docs/auto-merge-eligibility.md` |
| Review acknowledgement + `review ack` | `src/features/review/` | Records `review-ack` Evidence at `agent-claimed-locally`; required for HUMAN verdicts at `>=medium` risk |
| Deploy gate + witnessed rollback (L7) | `src/features/deploy/` | `deploy gate` runs 4 checks (feature_flag, canary_plan, rollback, owner) and records `deploy-readiness` Evidence; `deploy rollback` runs and witnesses a rollback command |
| Runtime monitor + `runtime check` (L7) | `src/features/runtime/` | `RuntimeMonitorPort` + Prometheus adapter; `runtime check` queries `Spec.runtime_signals` and records `runtime-signal` Evidence |
| Edge-case mitigation regression corpus (L8.2) | `tests/e2e/trust-benchmark/` | Seed of 9 of 32 edge-case scenarios; CI runs the full directory on every release |

## CODE STYLE
- Prefer `interface` for object shapes and `type` for unions/intersections.
- Avoid `any`; prefer `unknown` plus narrowing.
- Keep top-level/public functions named and give public APIs explicit return types.
- Prefer `undefined` over `null`; use optional chaining and nullish coalescing.
- Use `describe`/`it`; mock external dependencies, not internal modules.

## CONVENTIONS
- Bun-first, ESM, strict TypeScript. There is no repo-wide lint layer; `typecheck` is advisory in CI.
- `src/` is feature-first: `features/` owns domains, `infra/` owns plumbing, `shared/` owns generic utilities, `tui/` owns Mission Control projection/rendering.
- Keep `src/index.ts` and `src/services.ts` thin. Put behavior in the owning feature or infra use case.
- Cross-feature imports go through `@/features/<name>` public surfaces only.
- `skills/built-in/` is the source of truth for project-level shipped skills. Sync it into `src/infra/domain/built-in-skill-templates.ts`; do not hand-edit the generated embed file.
- `skills/bundled/` is the source of truth for the global maestro skill bundle (`maestro-brainstorm`, `maestro-plan`, `maestro-task`, `maestro-mission`, `maestro-handoff`, `maestro-setup`, `maestro-verify`). 7 bundled skills total as of L4.5. Sync into `src/infra/domain/bundled-skill-templates.ts` via `bun run sync:bundled-skills`; `bun run check:bundled-skills` enforces parity. `maestro install` installs these into `~/.claude/skills/` and `~/.codex/skills/`.
- `maestro-verify` (`skills/bundled/maestro-verify/SKILL.md`) is the canonical verification protocol. It documents the pre-claim ritual, witness levels, Trust Verifier scope, ProofMap, plan-check, verdict semantics, cost-budget monitoring, AI Reviewer protocol, and threat-model production. All other skills that reference verification protocol cross-reference this skill.
- When adding a new agent-facing feature or changing related agent behavior, update the relevant `skills/bundled/maestro-*/SKILL.md` in the same change so the installed skills stay current. The per-project `.maestro/AGENTS.md` template in `src/infra/domain/bootstrap-templates.ts` still governs project bootstrap content.
- `buildSnapshot()` and `buildHomeSnapshot()` are read models. Preview, JSON, and render-check paths must remain inspection-only.
- Treat `./dist/maestro` and installed `maestro` on `PATH` as different artifacts. Verify which binary was exercised.
- Repo-tracked behavior changes bump the CLI version. Docs-only/comment-only changes do not.
- Release publishing on `main` requires manual dispatch or a head commit exactly named `chore(release): v<version>`.
- The Evidence Recorder (`src/features/evidence/`) logs verifiable outputs for a task as structured rows. Storage goes to `.maestro/evidence/` (gitignored); derived run-state goes to `.maestro/runs/<task-id>/state.json` (also gitignored). Evidence rows carry a `WitnessLevel` that tracks how trustworthy the claim is. The 4-level ladder, strongest to weakest: `witnessed-by-maestro` (Maestro itself ran the command), `witnessed-by-ci` (a trusted CI gate ran and posted the result), `agent-claimed-locally` (the agent self-reported a local run; default for schema v1 rows), `agent-claimed-and-not-reproducible` (manual notes only). The Risk Engine uses the witness level to decide whether evidence clears the autopilot policy threshold for a given risk class. See `docs/witness-levels.md`. L4 added three new evidence kinds: `plan-check` (plan file checked against contract and spec before coding), `ai-review` (reviewer LLM findings; `bug | security | architecture` — consumed by Risk Engine per Rule 1, raises only), and `threat-model` (structured threat analysis required when diff intersects security-relevant paths; see `docs/threat-model-format.md` for schema and examples).
- L8.1 added `cross-task-conflict` as a new `EvidenceKind`. Recorded by `maestro ci verify` when open PRs touch overlapping file paths; payload `{ thisPr, conflictingPrs[], overlappingPaths[] }`; witness level `witnessed-by-ci`; Risk Engine raises effective class one tier per signal (capped at `critical`, multi-row clamped to one tier total). See `docs/cross-task-conflict.md`.
- Contract amendments (L2) are versioned Evidence, not silent edits. Each `contract amend` call consumes from `amendmentBudget` (Rules 3–7: budgeted, versioned, never-lower risk, plan-time proposals exempt). The amend use-case enforces the budget and writes a `contract-amended` Evidence row automatically. See `docs/sensitive-paths-defaults.md` for default sensitive-path globs and `docs/owners-yaml-format.md` for the owners schema.
- Verdict semantics (L3): `PASS` — all acceptance criteria met with evidence at or above the required witness level; `FAIL` — evidence present but below the required level or a criterion unmet; `HUMAN` — criteria met but autopilot policy requires human review for this risk class; `BLOCK` — a blocker condition is active (broken contract, critical risk class with no human signoff, or pending loosening in the soak window). The Risk Engine derives risk class from diff signals and takes the higher of agent-proposed vs Maestro-derived (Rule 1: LLM can never lower the derived class). See `docs/risk-class-derivation.md` and `docs/policy-format.md`.
- Asymmetric policy editing (L3): policy tightenings take effect immediately; loosenings soak for 30 days before becoming effective. Pending loosenings accumulate in `.maestro/policies/.pending-loosenings.json` (gitignored). Use `maestro policy pending` to inspect.
- CI is the authoritative verifier. Local Maestro is advisory; the GitHub check status posted by `maestro ci verify` is the merge gate. Verdicts are bound to (pr, tree_sha) so squashes survive but force-push to a different tree invalidates them.
- L6 auto-merge is opt-in via `policies/autopilot.yaml`. The `autoMergeAllowed.<risk-class>` field already existed from L3; L6 is the first layer that consumes it. All classes default to `false`. Set `autoMergeAllowed.<class>: true` only for risk classes your team has approved for automated merging. L6 also adds `review-ack` evidence (recorded via `maestro review ack`) and `verdict-override` evidence (recorded via `maestro verdict override`) — both consumed by the auto-merge eligibility gate. See `docs/auto-merge-eligibility.md`.
- L7 is reachable from L5 — building L7 phases does not require shipping L6 first. L7 is opt-in: producing `deploy-readiness` and `runtime-signal` Evidence does not by itself flip Verdict semantics; teams wire the new Evidence into `policies/risk.yaml` if they want it to gate. `deploy gate` runs four checks (feature_flag, canary_plan, rollback witness, owner); `runtime check` queries Prometheus signals declared in `Spec.runtime_signals` (schema v2; v1 specs forward-migrate at read time). Rollback must be witnessed at `witnessed-by-ci` or stronger before the deploy gate passes the rollback check. `owners.yaml` now has a fourth role: `deploy_approver` (required for the owner check in `deploy gate`). See `docs/deploy-gate.md` and `docs/runtime-monitoring.md`.

## ANTI-PATTERNS
- Deep imports into another feature's `commands/`, `usecases/`, `domain/`, `ports/`, or `adapters/`.
- Hidden writes or recovery logic inside Mission Control snapshot/preview paths.
- Hand-editing `src/infra/domain/built-in-skill-templates.ts` or `src/infra/domain/bundled-skill-templates.ts` (both are generated).
- Assuming `bun run ci` is a generic verification command; it performs release-prep work and may reset git state.
- Treating `task` and `mission` as interchangeable.
- Assuming installed `maestro` is the fresh build without checking `command -v maestro` and the build/install path used.

## COMMANDS
```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun run test
./dist/maestro mission-control --render-check --size 120x40
bun run release:local
./dist/maestro deploy gate --task <id>
./dist/maestro deploy rollback --task <id> --command <cmd>
./dist/maestro runtime check --task <id>
```

## CLI VERBS — EVIDENCE
```bash
maestro evidence record --task <id> --command "bun test" --exit 0
maestro evidence record --task <id> --kind manual-note --note "Verified manually"
maestro evidence record --task <id> --kind ai-review --reviewer <bug|security|architecture> --findings <inline-json-or-path> --confidence <0-1>
maestro evidence record --task <id> --kind threat-model --threat-model-file <path>
maestro evidence list --task <id>
maestro evidence show <evidence-id>
```

## CLI VERBS — CONTRACT (L2)
```bash
maestro contract show --task <id>
maestro contract show --task <id> --at-version <n>
maestro contract amend --task <id> --add-path <path> --reason "<why>"
maestro contract amend --task <id> --remove-path <path> --reason "<why>"
maestro contract history --task <id>
```

## CLI VERBS — TASK VERIFY (L2)
```bash
maestro task verify --task <id>
maestro task verify --task <id> --base <git-ref>
maestro task verify --task <id> --json
```

`task verify` runs 8 Trust Verifier checks (the 8th group is the architecture lints from `bun run lint:arch`). Architecture-lint findings at `error` severity also produce `lint-violation` evidence rows at `agent-claimed-locally`, queryable via `maestro task introspect`.

## CLI VERBS — TASK INTROSPECT (Phase 1)
```bash
maestro task introspect <id-or-slug>
maestro task introspect <id-or-slug> --json
```

Read-only digest: spec acceptance criteria + non-goals, latest verdict, cost-budget status, open lint violations, active blockers, last 5 evidence rows, and recent commits since the last `session-start` anchor. Use after context loss to re-orient on a task.

## CLI VERBS — SESSION (Phase 1)
```bash
maestro session whoami [--json] [-q]
maestro session start <taskId> [--json]
maestro session exit <taskId> [--json]
```

`session whoami` keeps the legacy detection behavior; bare `maestro session` continues to work as a `whoami` alias for backward compatibility.

`session start` writes an orient digest at `.maestro/runs/<taskId>/orient.md`, runs the baseline architecture-lint pass, optionally invokes `maestro:setup` and `maestro:verify` package-json scripts, and records a `session-start` evidence row at `witnessed-by-maestro` whose `headSha` payload anchors "recent commits" calculation. Blocks when the baseline arch lint has error-severity violations.

`session exit` re-runs the baseline arch-lint pass, reads the latest verdict, checks the working tree, writes `.maestro/runs/<taskId>/progress.md`, and records a `session-exit` evidence row. Exit codes: `0` clean, `1` baseline regressed, `2` arch-lint error-severity violations present. Warnings on dirty tree or FAIL/BLOCK verdict do not block the exit.

## CLI VERBS — LINT (Phase 1)
```bash
bun run lint:arch                        # standalone, no diff (3 file-scan rules)
bun run lint:arch -- --base main         # diff-aware (enables no-hand-edit-generated rule)
bun run lint:arch -- --json
```

Same library powers Trust Verifier's 8th check. See `docs/architecture-lints.md`.

## CLI VERBS — RECOVER (Phase 2)
```bash
maestro recover --task <id>                 # reset to last PASS verdict's tree
maestro recover --task <id> --to <commit>   # reset to an explicit ref
maestro recover --task <id> --dry-run       # plan without applying
maestro recover --task <id> --force         # ignore dirty tree (destructive)
maestro recover --task <id> --json
```

Resolves the last `PASS` verdict for the task, finds a commit whose tree
matches `verdict.subject.tree_sha`, runs `git reset --hard`, removes
`.maestro/runs/<id>/`, and records a `recovery` evidence row at
`witnessed-by-maestro`. Refuses to run on a dirty tree unless `--force`.

## CLI VERBS — RALPH (Phase 2)
```bash
maestro ralph review --task <id> [--stuck-threshold <n>] [--json]
```

Convergence oracle. Aggregates arch-lint, verifier, AI review, and
threat-model findings; computes a stable `findingsHash`; records a
`ralph-iteration` evidence row at `witnessed-by-maestro`. Exit codes: 0
converged (no error-severity findings), 1 not converged, 2 stuck (same hash
across `--stuck-threshold` iterations, default 3).

## CLI VERBS — GC (Phase 2)
```bash
maestro gc doc-gardening [--task <id>] [--json]
```

Scans `AGENTS.md`, `CLAUDE.md`, `README.md`, `docs/**/*.md`, `.maestro/**/*.md`,
`skills/**/*.md` for path references that don't resolve. Records a
`doc-gardening` evidence row when `--task` is provided. Phase 4 adds
`gc slop-cleanup` and `gc plan-regen`.

Schedule recipes for nightly GC sweeps live in `docs/schedule-recipes.md`.

## CLI VERBS — STATE (Phase 3)
```bash
maestro state since <iso> [--until <iso>] [--task <id>] [--json]
```

Streams a chronological event view across stores: every `Evidence` row and
every `Verdict` whose timestamp falls in the window. With `--task <id>`,
narrows to one task. Useful for "what happened on this branch since the last
session-start?" queries.

## CLI VERBS — MISSION CONTROL FILTER (Phase 3)
```bash
maestro mission-control --json --filter task=<id>
maestro mission-control --json --filter feature=<id> --filter task=<id>
maestro mission-control --screen <name> --size 120x40 --format plain
```

`--screen <name>` is an alias for `--preview <name>` framed as the
programmatic surface. `--filter key=value` narrows the JSON output: when
filters are present, the JSON wraps the full snapshot under `.snapshot` and
adds a `.narrow` field with just the matching task/feature row plus filtered
`progressLog` and `eventStream` entries. Filters are AND-combined.

## ARCHITECTURE LINTS — Phase 3 additions

Two new rules added in audit mode (`severity: warn`):

- `composition-only-in-services-ts` — value (non-`import type`) imports of any
  feature's `services.ts` are only allowed in the global composition root
  (`src/services.ts`). Move dependency wiring to the root.
- `task-vs-mission-separation` — `src/features/task/**` and
  `src/features/mission/**` must not import each other.

Both are warn-severity initially; promotion to error follows the audit-mode
→ soak → block ramp documented in `docs/architecture-lints.md`. The first two
plan-listed rules (`no-deep-cross-feature-imports`, `feature-public-via-index`)
are already enforced as errors by `bun run check:boundaries`; the lint
library does not duplicate those checks.

## CLI VERBS — SPEC (L2)
```bash
maestro spec show --mission <id>
maestro spec edit --mission <id>
```

## CLI VERBS — VERDICT (L3)
```bash
maestro verdict show --task <id>
maestro verdict show --task <id> --at-version <id>
maestro verdict show --task <id> --pr <number>
maestro verdict request --task <id>
maestro verdict request --task <id> --json
```

Exit codes for `verdict request`: 0 = PASS, 1 = FAIL, 2 = HUMAN, 3 = BLOCK.

L5 added `--pr <n>` to filter by PR — finds verdict by current HEAD tree SHA, so squashed commits with identical content retain their verdict and force-pushes to a different tree return no match.

## CLI VERBS — POLICY (L3)
```bash
maestro policy check --task <id>
maestro policy pending
```

## CLI VERBS — TASK PROOF (L3)
```bash
maestro task proof --task <id>
maestro task proof --task <id> --json
```

## CLI VERBS — PLAN (L4)
```bash
maestro plan check --task <id> --plan-file <path>
maestro plan check --task <id> --plan-file <path> --json
```

Exit code is always 0; agents react to findings in the output. A clean plan-check does not guarantee a passing verdict — it confirms the plan is internally consistent before coding starts.

## CLI VERBS — TASK BUDGET (L4)
```bash
maestro task budget --task <id>
maestro task budget --task <id> --json
```

Exit code is always 0; the verb is read-only. Once any budget limit is exceeded, the next `verdict request` returns `BLOCK` (exit 3).

## CLI VERBS — CI (L5)
```bash
maestro ci verify [--pr <n>] [--task <id>] [--base <ref>] [--json]
```

Reads CI env (`GITHUB_ACTIONS`, `GITHUB_REPOSITORY`, `GITHUB_REF`, `GITHUB_SHA`, `GITHUB_BASE_REF`, `GITHUB_EVENT_PATH`, `GITHUB_OUTPUT`, `GITHUB_TOKEN`) by default; flags override. Runs Trust Verifier, ingests any CI job-result file as `witnessed-by-ci` Evidence, computes the Verdict, writes `verdict_id`, `verdict_decision`, `effective_risk_class` to `$GITHUB_OUTPUT`, and (when running in GitHub Actions with a token) posts a GitHub Check via `gh api`.

Exit codes: 0 PASS / 1 FAIL / 2 HUMAN / 3 BLOCK.

See `docs/ci-integration.md` for the full reference: workflow template, env contract, witness ingestion, PR check semantics, verdict tree-SHA identity, and troubleshooting.

## CLI VERBS — MERGE AUTO (L6)
```bash
maestro merge auto --pr <number> --task <id> [--base <ref>] [--repo <owner/name>] [--json]
```

Runs 8 eligibility predicates. Exits 0 and triggers `gh pr merge --auto` if all pass; exits 1 and prints failing codes if any fail. Requires `autoMergeAllowed.<riskClass>: true` in `autopilot.yaml`.

See `docs/auto-merge-eligibility.md` for the full predicate reference and troubleshooting.

## CLI VERBS — VERDICT OVERRIDE (L6)
```bash
maestro verdict override --task <id> --pr <number> --reason "<text>" [--verdict <id>] [--base <ref>] [--json]
```

Records a `verdict-override` Evidence row at `agent-claimed-and-not-reproducible`. Requires the invoking user to be in `owners.yaml.sensitive_waiver` (loaded from the base branch — Rule 12). Does not change the PR check conclusion; override is an audit record only.

See `docs/override-flow.md` for authorization rules, audit trail semantics, and no-silent-pass guarantees.

## CLI VERBS — REVIEW ACK (L6)
```bash
maestro review ack --task <id> --verdict <id> --criterion "<text>" [--criterion "<text>" ...] [--json]
```

Records a `review-ack` Evidence row at `agent-claimed-locally`. Required when the verdict is `HUMAN` at `>=medium` risk before `maestro merge auto` can succeed. The `--criterion` flag is repeatable.

## CLI VERBS — DEPLOY (L7)
```bash
maestro deploy gate --task <id> [--base <ref>] [--json]
maestro deploy rollback --task <id> --command <cmd> [--json]
```

`deploy gate` runs 4 checks (feature_flag, canary_plan, rollback, owner) and records a `deploy-readiness` Evidence row. Exits 0 when gate=pass, 1 when gate=fail. Requires a `deploy_approver` entry in `owners.yaml` for the owner check to pass, and at least one `rollback-exercised` Evidence row at `witnessed-by-ci` or stronger for the rollback check to pass. Does NOT mutate the Verdict — teams wire it via `policies/risk.yaml` if they want it to gate.

`deploy rollback` runs the given shell command, records a `rollback-exercised` Evidence row at `witnessed-by-ci` (in CI) or `witnessed-by-maestro` (locally), and exits 1 if the command fails.

See `docs/deploy-gate.md` for the full check enumeration, `Spec.rollout_plan` reference, and troubleshooting.

## CLI VERBS — RUNTIME (L7)
```bash
maestro runtime check --task <id> [--provider-base-url <url>] [--json]
```

Queries each signal declared in `Spec.runtime_signals` via the configured provider (currently Prometheus). Records one `runtime-signal` Evidence row per signal. Exit code is always 0; `pass=false` rows are advisory at L7 (teams wire them into risk policy to make them gate). Provider base URL precedence: `--provider-base-url` flag → `MAESTRO_PROMETHEUS_URL` env → `http://localhost:9090`.

See `docs/runtime-monitoring.md` for the `RuntimeMonitorPort` reference, Prometheus adapter guide, and how to add new adapters.

## CLI VERBS — GC (Phase 2 / 4)
```bash
maestro gc doc-gardening [--task <id>] [--json]
maestro gc slop-cleanup [--min-severity info|warn|error] [--json]
maestro gc plan-regen --task <id> [--json]
```

`gc doc-gardening` scans repo docs for stale path references and broken local links; records `doc-gardening` evidence when `--task` is supplied.

`gc slop-cleanup` runs the architecture-lint corpus across the full repo and groups violations by file (top-10 offenders, by-rule counts, by-severity buckets). Read-only; no PR opened.

`gc plan-regen --task <id>` reports plan-vs-state drift: missing plan file, missing acceptance-criteria coverage, evidence post-dating the last PASS verdict, recorded lint-violations, and active blockers.

## CLI VERBS — CONTRACT SPRINT (Phase 4)
```bash
maestro contract sprint --task <id> [--propose <text>] [--proposed-by <actor>] [--json]
```

Prints a sprint snapshot (criteria progress, amendment-budget remaining, recent amendments). With `--propose <text>`, records a `manual-note` evidence row tagged as a sprint-contract proposal. Does NOT mutate the contract — agents/humans review and apply via `maestro contract amend`.

## CLI VERBS — INSPECT (Phase 5)
```bash
maestro inspect <taskId> [--tail <n>] [--json]
```

Read-only post-mortem snapshot. Reads `.maestro/runs/<taskId>/{orient,progress,plan}.md` plus `state.json`, lists last `--tail` evidence rows (default 10), and the verdict history. No mutation.

## CLI VERBS — WORKTREE (Phase 5)
```bash
maestro worktree create <slug> [--base <branch>] [--prefix <pre>] [--json]
```

Wraps `git worktree add -b <prefix>/<slug>` and provisions an isolated `.maestro/runs/` directory inside the new worktree. Default base is `main`, default prefix is `feat`.

## CLI VERBS — RECOVER / RALPH / STATE (Phase 2/3)
```bash
maestro recover --task <id> [--to <commit>] [--force] [--dry-run] [--json]
maestro ralph review --task <id> [--stuck-threshold <n>] [--json]
maestro state since <iso> [--until <iso>] [--task <id>] [--json]
maestro mission-control --json [--filter task=<id>] [--filter feature=<id>]
maestro mission-control --screen <name> [--feature <id>]
```

`recover` walks verdict history backward to the last PASS, resets the working tree to that commit's tree, drops `.maestro/runs/<id>/`, and records a `recovery` evidence row. `--dry-run` previews without mutation.

`ralph review` is the convergence oracle. Aggregates Trust Verifier + AI Reviewer + lint:arch findings, hashes them, and records a `ralph-iteration` evidence row. Exit codes: 0 converged, 1 not converged, 2 stuck (same finding-hash for N consecutive iterations).

`state since <iso>` aggregates Evidence + Verdict events across tasks, sorted chronologically.

`mission-control --filter` narrows JSON output by `task=<id>` or `feature=<id>` (supports repeated flags); `--screen <name>` is a programmatic alias for `--preview <name>`.

## ARCHITECTURE LINTS — Phase 4 (taste, audit-only)

`file-size-limit`, `no-bare-console-log`, `kebab-case-filenames` ship at `severity: info` and do not fail Trust Verifier. They surface in `bun run lint:arch` and `gc slop-cleanup`. See `docs/architecture-lints.md` for promotion criteria.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **maestro** (10765 symbols, 18180 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/maestro/context` | Codebase overview, check index freshness |
| `gitnexus://repo/maestro/clusters` | All functional areas |
| `gitnexus://repo/maestro/processes` | All execution flows |
| `gitnexus://repo/maestro/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- none (root)

Children:
- [.factory/AGENTS.md](.factory/AGENTS.md)
- [.maestro/AGENTS.md](.maestro/AGENTS.md)
- [hooks/AGENTS.md](hooks/AGENTS.md)
- [scripts/AGENTS.md](scripts/AGENTS.md)
- [skills/AGENTS.md](skills/AGENTS.md)
- [src/AGENTS.md](src/AGENTS.md)
- [tests/AGENTS.md](tests/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
