# Source Tree

Use this file with the repo-root [AGENTS.md](../AGENTS.md). `src/` is feature-first and keeps CLI plumbing, generic utilities, and Mission Control as separate seams.

## STRUCTURE
```text
src/
├── features/    # bounded contexts
├── infra/       # CLI plumbing and shared adapters
├── shared/      # generic utilities only
├── tui/         # Mission Control projection + rendering
├── index.ts     # commander root
└── services.ts  # composition root
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add or inspect a user command | `index.ts`, `features/*/index.ts`, `infra/commands/` | Keep registration thin |
| Wire dependencies | `services.ts`, `features/*/services.ts` | Composition only |
| Change product behavior | `features/` | Owning feature first |
| Change generic filesystem/shell/yaml helpers | `shared/` | No product-domain logic here |
| Change Mission Control | `tui/`, `infra/commands/mission-control.command.ts` | Read `tui/README.md` first |
| Evidence logbook (evidence record/list/show) | `features/evidence/` | `EvidenceKind`, `WitnessLevel`, `EvidenceRow` in `domain/types.ts`; storage adapter under `adapters/file-storage.ts`. L4 added `plan-check`, `ai-review`, and `threat-model` kinds with their record-verb flags. |
| Product-spec authoring (spec new/validate) | `src/runtime/spec.command.ts` | Spec lifecycle; product-spec files live at `.maestro/specs/<slug>.md` |
| Policy and owners loader | `features/policy/` | `Owners`, `OwnersYaml` in `domain/owners-types.ts`; `loadOwners` use-case reads `.maestro/policies/owners.yaml`. Extended in L3: `RiskPolicy`, `AutopilotPolicy`, `ReleasePolicy` types and loaders; asymmetric edit classifier (`classify-policy-edit.usecase.ts`); effective-policy use-case; `policy check` and `policy pending` commands. |
| Trust Verifier (task verify) | `features/verdict/verify/` | `runTrustVerifier` in `trust-verifier.ts`; 7 checks under `checks/`; `TrustFinding`, `TrustVerifierResult` in `@/types/trust.ts` |
| Arch-rules lint (9 rules) | `shared/lib/arch-rules.ts` | `checkArchitectureRules`; consumed by `gc/slop-cleanup` and `ci/run-ci-verify` |
| Task lifecycle | `src/runtime/task.command.ts`, `src/service/` | `task claim`, `task verify`, `task block`, `task ship`, `task abandon`; state machine at `src/types/task-state.ts` |
| Risk Engine (L3/L4) | `features/risk/` | `computeRisk` in `usecases/compute-risk.ts`; `deriveRiskClassFromDiff` in `usecases/derive-risk-class.ts` (signal-to-class mapping per ROADMAP table); `risk-class-order.ts` for level comparison. L4: applies `ai-review` error-severity raises (Rule 1) and `threat-model-required` predicate (Edge Case 12). |
| Verdict (L3) | `features/verdict/` | Domain types in `domain/`; file-system store adapter in `adapters/`; `verdict-id.ts` under `usecases/`; `verdict show` and `verdict request` commands (exit 0 PASS, 1 FAIL, 2 HUMAN, 3 BLOCK) |
| Plan-check use-case + CLI (L4) | `features/mission/` | `checkMission` in `usecases/check-mission.ts` runs three deterministic checks (`scope-widens`, `missing-proof`, `risk-class-too-low`); `commands/mission-check.command.ts` registers `maestro plan check` (verb kept under the `plan` parent for artifact validation). Records a `plan-check` Evidence row on each run. |
| Cost-budget run-state | `features/mission/` | `RunState`, `RunStateStorePort` live under `features/mission/`. Run-state persisted under `.maestro/runs/<task-id>/state.json` (gitignored). |
| Mission Control autopilot view | `tui/state/autopilot-screen.ts` | Read model for the autopilot screen in Mission Control. |
| `maestro ci verify` (L5/L8.1) | `features/ci/` | `maestro ci verify` CLI verb, GitHub Actions env reader (`readCiEnv`), GitHub API port + gh-cli adapter, post-PR-check use-case. L8.1 added cross-task conflict detection: queries open PRs for overlapping changed paths and records `kind=cross-task-conflict` Evidence when overlap is found. |
| Auto-merge eligibility + `merge auto` (L6) | `features/merge/` | `usecases/auto-merge-eligible.usecase.ts` runs 8 deterministic predicates; `commands/merge-auto.command.ts` registers `merge auto`; `domain/eligibility-types.ts` defines reason codes. Consumed `autoMergeAllowed` from `AutopilotPolicy` (field existed since L3). |
| Review acknowledgement + `review ack` (L6) | `features/review/` | `commands/review-ack.command.ts` registers `review ack`; records `review-ack` Evidence at `agent-claimed-locally`; required by eligibility gate when verdict is `HUMAN` at `>=medium` risk. |
| Deploy gate + witnessed rollback (L7) | `features/deploy/` | `commands/deploy-gate.command.ts` registers `deploy gate`; `commands/deploy-rollback.command.ts` registers `deploy rollback`; `usecases/check-deploy-readiness.usecase.ts` runs 4 checks. |
| Runtime monitor + `runtime check` (L7) | `features/runtime/` | `ports/monitor.port.ts` defines `RuntimeMonitorPort`; `adapters/prometheus.adapter.ts` is the Prometheus implementation; `commands/runtime-check.command.ts` registers `runtime check`; `domain/types.ts` defines `RuntimeSignalResult`. |

## CONVENTIONS
- Cross-feature imports go through `@/features/<name>` only.
- Keep feature logic in the owning feature, plumbing in `infra/`, and generic helpers in `shared/`.
- `src/index.ts` and `src/services.ts` stay thin.
- Mission Control snapshot builders remain read-only; recovery and workflow mutation stay out of `buildSnapshot()` and `buildHomeSnapshot()`.
- `skills/built-in/` syncs into `src/infra/domain/built-in-skill-templates.ts`; do not hand-edit the generated file from inside `src/`.

## COMMON CHECKS
- `bun run build`
- `bun run typecheck`
- `bun run check:boundaries`
- For TUI work:
  - `./dist/maestro mission-control --preview --size 120x40 --format plain`
  - `./dist/maestro mission-control --render-check --size 120x40`

## ANTI-PATTERNS
- Deep imports into another feature's internal folders.
- Feature logic in the composition root.
- Domain logic in `shared/`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [features/AGENTS.md](features/AGENTS.md)
- [infra/AGENTS.md](infra/AGENTS.md)
- [shared/AGENTS.md](shared/AGENTS.md)
- [tui/AGENTS.md](tui/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
