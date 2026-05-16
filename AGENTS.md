@CLAUDE.md

# PROJECT KNOWLEDGE BASE

**Generated:** 2026-04-24 09:16:42 +0700
**Commit:** 8b4a2d76
**Branch:** main

## OVERVIEW
Maestro is a local-first, long-running agent harness for multi-agent software
engineering. Single-package Bun/TypeScript CLI with an OpenTUI dashboard,
repo-owned agent surfaces, and shared project state under `.maestro/`.

- Principle-to-primitive mapping: `docs/harness-positioning.md`.
- Full verb reference: `docs/cli-reference.md`.
- Layered architecture rules: `docs/architecture.yaml`; binding decisions in `docs/adr/`.
- State on disk: `.maestro/{evidence,tasks,plans,runs,worktrees,handoffs}/`. Auto-advance rules in ADR-0011; heavy-mode worktrees in ADR-0008.
- Six bundled skills: `maestro-design`, `maestro-handoff`, `maestro-plan`, `maestro-task`, `maestro-verify`, `maestro-setup`.
- Default principles: `docs/principles/*.md`; promote new ones with `maestro principle promote <evd-id>`.

## STRUCTURE
```text
maestro/
├── .maestro/    # repo-tracked project state, plans, tasks, context, doc templates
├── hooks/       # session/tool hook entrypoints
├── scripts/     # build, version, install, release, TUI helpers
├── skills/      # built-in and bundled skill sources
├── src/         # feature-first CLI + TUI source tree
└── tests/       # unit, integration, compiled-binary coverage
```

`.maestro/docs/` holds canonical doc templates (HARNESS.md, FEATURE_INTAKE.md, VALIDATION_LADDER.md) that `maestro setup` copies to user projects.

## WHERE TO LOOK
| Task | Location |
|------|----------|
| CLI entry / command registration | `src/index.ts` |
| Dependency wiring | `src/services.ts` (composition root) |
| Task / plan / spec lifecycle | `src/runtime/{task,plan,spec,setup,principle}.command.ts` |
| Layered architecture | `src/{types,config,repo,service,runtime,ui,providers}/` (forward-only `types → config → repo → service → runtime → ui`; `providers` is cross-cutting) |
| Architecture lints | `src/service/architecture-lint.usecase.ts` |
| Ports + adapters | `src/repo/` (task, plan, spec, evidence, observability, worktree, handoff, principles) |
| Use cases | `src/service/` (task-claim, task-verify, plan-decompose, emit-handoff, migrate-v2, setup-check, principle-promote, ...) |
| Feature boundaries | `src/features/`, `scripts/check-feature-boundaries-lib.ts` |
| Mission Control TUI | `src/infra/commands/mission-control.command.ts`, `src/tui/state/snapshot.ts` |
| Evidence logbook | `src/features/evidence/`; see `docs/witness-levels.md` |
| Shipped agent skills | `skills/bundled/` (sync via `bun run sync:bundled-skills`) |
| Release and install | `scripts/build.ts`, `scripts/ci.ts`, `scripts/install-local.ts` |
| Compiled-binary verification | `tests/e2e/`, `tests/helpers/run-compiled-cli.ts` |
| Policy and owners loader | `src/features/policy/`; see `docs/owners-yaml-format.md` |
| Trust Verifier | `src/features/verdict/verify/` |
| Risk Engine | `src/features/risk/`; see `docs/risk-class-derivation.md` |
| Verdict types and store | `src/features/verdict/` |
| AI Reviewer + Threat-Model | `src/features/risk/usecases/compute-risk.ts`; `docs/threat-model-format.md` |
| Auto-merge | `src/features/merge/`; see `docs/auto-merge-eligibility.md` |
| Review acknowledgement | `src/features/review/` |
| Deploy gate + rollback | `src/features/deploy/`; see `docs/deploy-gate.md` |
| Runtime monitor + dev observe | `src/features/runtime/`; see `docs/runtime-monitoring.md`, `docs/dev-observability.md` |
| MCP server | `src/features/mcp/server/` |
| Edge-case regression corpus | `tests/e2e/trust-benchmark/`, `tests/e2e/edge-cases/` |
| Scenario tests + swarm tooling | `tests/scenarios/`, `scripts/scenarios/` (see `scripts/scenarios/AGENTS.md`) |

## CODE STYLE
- `interface` for object shapes; `type` for unions/intersections.
- Avoid `any`; prefer `unknown` plus narrowing. Name top-level/public functions and give public APIs explicit return types.
- Prefer `undefined` over `null`; use optional chaining and nullish coalescing.
- `describe`/`it`; mock external dependencies, not internal modules.

## CONVENTIONS
- Bun-first, ESM, strict TypeScript. `typecheck` is advisory in CI.
- `src/` is feature-first: `features/` owns domains, `infra/` plumbing, `shared/` generics, `tui/` Mission Control. Keep `src/index.ts` and `src/services.ts` thin.
- Cross-feature imports go through `@/features/<name>` public surfaces only.
- `skills/built-in/` and `skills/bundled/` are the source of truth; embeds in `src/infra/domain/*.ts` are generated. Update `SKILL.md` in the same change as the feature.
- `maestro-verify` is the canonical verification protocol; other skills cross-reference it.
- Treat `./dist/maestro` and installed `maestro` on `PATH` as different artifacts.
- Repo-tracked behavior changes bump the CLI version; docs-only do not. Release publishing on `main` requires manual dispatch or a head commit named `chore(release): v<version>`.
- Local Maestro is advisory; CI Maestro is authoritative. Verdicts bind to (PR, tree_sha).
- Policy edits are asymmetric: tightenings immediate, loosenings 30-day soak (`maestro policy pending`).
- Auto-merge is opt-in via `policies/autopilot.yaml` (`autoMergeAllowed.<class>: true`); all classes default to `false`.
- L7 deploy gate and runtime monitor produce Evidence; they don't gate Verdict unless wired via `policies/risk.yaml`.
- Mission Control snapshot read models stay inspection-only.

## ANTI-PATTERNS
- Deep imports into another feature's `commands/`, `usecases/`, `domain/`, `ports/`, or `adapters/`.
- Deep imports across layers that violate forward-only-layers.
- Hidden writes or recovery logic inside Mission Control snapshot/preview paths.
- Hand-editing generated embed files under `src/infra/domain/`.
- Treating `bun run ci` as generic smoke; it performs release-prep.
- Treating `task` and `exec-plan` as interchangeable.
- Assuming installed `maestro` is the fresh build without checking `command -v maestro`.

## COMMANDS
```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun run check:layers
bun run test
./dist/maestro mission-control --render-check --size 120x40
bun run release:local
./dist/maestro setup check
```

## CLI VERBS
See `docs/cli-reference.md` for the full verb-by-verb reference. Hot-path aliases: `claim`, `verify`, `block`, `abandon`, `ship`.

## GitNexus
Usage rules: `docs/gitnexus-usage.md`. Before editing a symbol run `gitnexus_impact`; before committing run `gitnexus_detect_changes`.

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
