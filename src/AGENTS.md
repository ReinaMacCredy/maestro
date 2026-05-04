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
| Evidence logbook (evidence record/list/show) | `features/evidence/` | `EvidenceKind`, `WitnessLevel`, `EvidenceRow` in `domain/types.ts`; storage adapter under `adapters/file-storage.ts` |
| Mission Spec (spec show/edit) | `features/spec/` | `Spec`, `AcceptanceCriterion`, `NonGoal` in `domain/types.ts`; commands under `commands/spec.command.ts` |
| Policy and owners loader | `features/policy/` | `Owners`, `OwnersYaml` in `domain/owners-types.ts`; `loadOwners` use-case reads `.maestro/policies/owners.yaml`. Extended in L3: `RiskPolicy`, `AutopilotPolicy`, `ReleasePolicy` types and loaders; asymmetric edit classifier (`classify-policy-edit.usecase.ts`); effective-policy use-case; `policy check` and `policy pending` commands. |
| Trust Verifier (task verify) | `features/verify/` | `runTrustVerifier` in `usecases/trust-verifier.ts`; 6 checks under `usecases/checks/`; `TrustFinding`, `TrustVerifierResult` in `domain/types.ts` |
| ProofMap builder (L3) | `features/verify/usecases/proof-map.ts` | Joins `Spec.acceptance_criteria` with Evidence rows; called by `maestro task proof --task <id>` |
| Risk Engine (L3) | `features/risk/` | `computeRisk` in `usecases/compute-risk.ts`; `deriveRiskClassFromDiff` in `usecases/derive-risk-class.ts` (signal-to-class mapping per ROADMAP table); `risk-class-order.ts` for level comparison |
| Verdict (L3) | `features/verdict/` | Domain types in `domain/`; file-system store adapter in `adapters/`; `verdict-id.ts` under `usecases/`; `verdict show` and `verdict request` commands (exit 0 PASS, 1 FAIL, 2 HUMAN, 3 BLOCK) |

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
