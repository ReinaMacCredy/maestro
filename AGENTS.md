@CLAUDE.md

# PROJECT KNOWLEDGE BASE

**Generated:** 2026-04-24 09:16:42 +0700
**Commit:** 8b4a2d76
**Branch:** main

## OVERVIEW
Maestro is a local-first, long-running agent harness for multi-agent software
engineering. It is a single-package Bun/TypeScript CLI with an OpenTUI dashboard,
repo-owned agent surfaces, and shared project state under `.maestro/`.

Read `docs/harness-positioning.md` for the principle-to-primitive mapping.

## Maestro v2 (in flight)

Maestro v2 is a layered rewrite landing on `main` per ADR-0007 (big-bang)
and ADR-0013 (greenfield branch inlined). Phases 1, 1.5, and 2 — the v2
spine, the principles + correction-recording bridge, and the heavy-mode
plan lifecycle — are feature-complete and dogfooded; see
`docs/phase-1-done.md`, `docs/phase-1.5-done.md`, and
`docs/phase-2-done.md` for the transition-evidence records.

- **Plan:** `docs/v2-master-plan.md` is the source of truth. Layered
  architecture rules live in `docs/architecture.yaml` (loaded by the
  v2 `ArchitectureRules` port and enforced by the lint runner). ADRs
  under `docs/adr/` capture each binding decision.
- **Verbs live now:** `maestro spec new`, `maestro spec validate`,
  `maestro task from-spec`, `maestro task claim` (+ hot-path `claim`),
  `maestro task block` (+ `block`), `maestro task abandon` (+ `abandon`),
  `maestro task verify` (+ `verify`; `--verdict {human,block} --reason`
  exits 2 or 3), `maestro task ship` (+ `ship`), `maestro plan from-spec`,
  `maestro plan decompose <id> --file <path|->`, `maestro plan show`, and
  `bun run lint:arch` (v2 runner; `lint:arch:v1` keeps the v1 runner
  reachable until Phase 4 removal).
- **State + storage:** v2 task state machine in `src/v2/types/task-state.ts`;
  v2 plan state machine in `src/v2/types/exec-plan-state.ts`; append-only
  evidence in `.maestro/evidence/<date>.jsonl`; v2 tasks in
  `.maestro/tasks/tasks.v2.jsonl`; v2 plans in `.maestro/plans/plans.v2.jsonl`
  (v1 `tasks.jsonl` is unchanged). Plans auto-advance off task transitions
  per ADR-0011 (`plan:auto-start` on first claim, `plan:auto-complete` when
  every child is terminal).
- **Skill:** `skills/bundled/maestro-design` runs the grill protocol from
  ADR-0016 to author product-specs before `task from-spec` or
  `plan from-spec`. `maestro-plan` documents the v2 heavy-mode handoff
  (`plan from-spec` -> `plan decompose --file -`); `maestro-verify`
  documents the 4-exit-code routing from `task verify`.
- **Principles bridge (Phase 1.5):** four default principles ship at
  `docs/principles/*.md` (`prefer-shared-utils`, `no-yolo-data-probing`,
  `passive-harness`, `layer-order`). `maestro principle promote <evd-id>`
  materializes new principle scaffolds from `lint-violation` evidence
  rows; `maestro setup migrate-corrections` moves v1
  `.maestro/memory/corrections/*.json` into `docs/principles/legacy/`;
  `gc slop-cleanup` folds principle findings into the same per-file
  report it already emits for arch-lint violations.

## STRUCTURE
```text
maestro/
├── .maestro/    # repo-tracked project state, plans, tasks, context, and doc templates
├── hooks/       # session/tool hook entrypoints
├── scripts/     # build, version, install, release, and TUI helpers
├── skills/      # shipped built-in and bundled skill sources
├── src/         # feature-first CLI + TUI source tree
└── tests/       # unit, integration, and compiled-binary coverage
```

**Note**: `.maestro/docs/` contains canonical doc templates (HARNESS.md, FEATURE_INTAKE.md, VALIDATION_LADDER.md) that `maestro setup` copies to user projects.

## WHERE TO LOOK
| Task | Location |
|------|----------|
| CLI entry / command registration | `src/index.ts` |
| Dependency wiring | `src/services.ts` (composition root) |
| Feature boundaries | `src/features/`, `scripts/check-feature-boundaries-lib.ts` |
| Mission Control flow | `src/infra/commands/mission-control.command.ts`, `src/tui/state/snapshot.ts` |
| Handoff launch and pickup | `src/features/handoff/`, `src/features/task/` |
| Evidence logbook | `src/features/evidence/`; see `docs/witness-levels.md` |
| Shipped agent skills | `skills/built-in/`, `skills/bundled/` (sync via `bun run sync:bundled-skills`) |
| Release and install | `scripts/build.ts`, `scripts/ci.ts`, `scripts/install-local.ts` |
| Task vs mission systems | `.maestro/tasks/tasks.jsonl`, `.maestro/MAESTRO.md` |
| Compiled-binary verification | `tests/e2e/`, `tests/helpers/run-compiled-cli.ts` |
| Mission Spec | `src/features/spec/`; `.maestro/missions/<id>/spec.json` |
| Policy and owners loader | `src/features/policy/`; see `docs/owners-yaml-format.md` |
| Trust Verifier | `src/features/verify/` |
| Risk Engine | `src/features/risk/`; see `docs/risk-class-derivation.md` |
| Verdict types and store | `src/features/verdict/` |
| Plan-check + cost-budget | `src/features/plan/`, `src/features/task/usecases/check-cost-budget.ts` |
| AI Reviewer + Threat-Model | `src/features/risk/usecases/compute-risk.ts`; `docs/threat-model-format.md` |
| Auto-merge | `src/features/merge/`; `docs/auto-merge-eligibility.md` |
| Review acknowledgement | `src/features/review/` |
| Deploy gate + rollback | `src/features/deploy/`; `docs/deploy-gate.md` |
| Runtime monitor | `src/features/runtime/`; `docs/runtime-monitoring.md` |
| Setup / init hardening | `src/features/setup/`; `skills/bundled/maestro-setup/SKILL.md` |
| Edge-case regression corpus | `tests/e2e/trust-benchmark/`, `tests/e2e/edge-cases/` |

## CODE STYLE
- Prefer `interface` for object shapes and `type` for unions/intersections.
- Avoid `any`; prefer `unknown` plus narrowing.
- Name top-level/public functions; give public APIs explicit return types.
- Prefer `undefined` over `null`; use optional chaining and nullish coalescing.
- Use `describe`/`it`; mock external dependencies, not internal modules.

## CONVENTIONS
- Bun-first, ESM, strict TypeScript. There is no repo-wide lint layer; `typecheck` is advisory in CI.
- `src/` is feature-first: `features/` owns domains, `infra/` plumbing, `shared/` generics, `tui/` Mission Control.
- Keep `src/index.ts` and `src/services.ts` thin. Behavior belongs in features or infra use cases.
- Cross-feature imports go through `@/features/<name>` public surfaces only.
- `skills/built-in/` and `skills/bundled/` are the source of truth; their embeds in `src/infra/domain/*.ts` are generated.
- `maestro-verify` (`skills/bundled/maestro-verify/SKILL.md`) is the canonical verification protocol; other skills cross-reference it.
- When adding agent-facing features, update `skills/bundled/maestro-*/SKILL.md` in the same change.
- Mission Control snapshot read models stay inspection-only.
- Treat `./dist/maestro` and installed `maestro` on `PATH` as different artifacts.
- Repo-tracked behavior changes bump the CLI version; docs-only do not.
- Release publishing on `main` requires manual dispatch or a head commit named `chore(release): v<version>`.
- Local Maestro is advisory; CI Maestro is authoritative. Verdicts bind to (PR, tree_sha).
- Asymmetric policy editing: tightenings immediate; loosenings 30-day soak. `maestro policy pending` inspects.
- Auto-merge is opt-in via `policies/autopilot.yaml` (`autoMergeAllowed.<class>: true`). All classes default to `false`.
- L7 deploy gate and runtime monitor produce Evidence; they don't gate Verdict unless wired via `policies/risk.yaml`.

## ANTI-PATTERNS
- Deep imports into another feature's `commands/`, `usecases/`, `domain/`, `ports/`, or `adapters/`.
- Hidden writes or recovery logic inside Mission Control snapshot/preview paths.
- Hand-editing generated embed files under `src/infra/domain/`.
- Treating `bun run ci` as generic smoke; it performs release-prep.
- Treating `task` and `mission` as interchangeable.
- Assuming installed `maestro` is the fresh build without checking `command -v maestro`.

## COMMANDS
```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun run test
./dist/maestro mission-control --render-check --size 120x40
bun run release:local
./dist/maestro setup --check
```

## CLI VERBS
The full verb reference lives in [`docs/cli-reference.md`](docs/cli-reference.md).
Verbs at a glance: `evidence`, `contract`, `task verify`, `task introspect`,
`session`, `recover`, `ralph`, `gc`, `state`, `mission-control`, `spec`,
`verdict`, `policy`, `plan`, `task budget`, `ci`, `merge auto`, `review ack`,
`deploy`, `runtime`, `inspect`, `worktree`, `setup`.

## GitNexus
GitNexus code-intelligence usage rules live in
[`docs/gitnexus-usage.md`](docs/gitnexus-usage.md). Before editing a symbol,
run `gitnexus_impact`; before committing, run `gitnexus_detect_changes`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- none (root)

Children:
- [.maestro/AGENTS.md](.maestro/AGENTS.md)
- [hooks/AGENTS.md](hooks/AGENTS.md)
- [scripts/AGENTS.md](scripts/AGENTS.md)
- [skills/AGENTS.md](skills/AGENTS.md)
- [src/AGENTS.md](src/AGENTS.md)
- [tests/AGENTS.md](tests/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
