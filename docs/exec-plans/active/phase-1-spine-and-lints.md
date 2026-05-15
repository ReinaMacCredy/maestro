---
phase: 1
state: in-progress
pr_count: 17
done_criteria: "dogfooded on a real maestro change, spec -> task -> ship, with transition evidence recorded, and maestro task verify runs the architecture lints as one of its checks"
v2_source_layout_decision: "v2 code lives in src/v2/{types,config,repo,service,runtime,ui,providers}/ as a parallel tree alongside the unchanged v1 src/features/ tree. Phase 4 deletes src/features (v1) after merge to main. No interleaving."
migration_test_scope: "Phase 1 ships pure state-mapping functions plus fixture tests only. The CLI verb setup --migrate-v2 ships in Phase 3. PR 16 tests the mapping tables against a frozen .maestro/ snapshot fixture, not the full 13-step orchestrator."
---

# Phase 1: Spine + Architecture Lints

Decomposed from `docs/v2-master-plan.md` §10 Phase 1. Each PR is independently mergeable with its own verify gate.

## PR ledger

### PR 01 — docs: stage planning artifacts and scaffold knowledge-primitive directories
- Purpose: land CONTEXT.md, docs/adr/, docs/v2-master-plan.md; create empty primitive directories.
- Deliverables: commit untracked docs; .gitkeep for `docs/{design-docs/learnings,exec-plans/{active,completed},references,generated,principles}/`, `.maestro/specs/`.
- Dependencies: none.
- Verify gate: `git status` clean of v2 docs; `ls docs/exec-plans/active/` shows phase-1 plan; `bun run build` passes.
- Risk: low. Size: xs.

### PR 02 — feat(v2/scaffold): establish v2 source-tree skeleton with layered architecture
- Purpose: create `src/v2/{types,config,repo,service,runtime,ui,providers}/` with barrel files; seed `docs/architecture.yaml`.
- Deliverables: 7 layer barrels; architecture.yaml with `layers`, `forward_only: true`, `passive_harness.forbidden_patterns` stub.
- Dependencies: PR 01.
- Verify gate: `bun run build` passes; all 7 dirs exist; YAML parses.
- Risk: low. Size: xs.

### PR 03 — feat(v2/types): v2 task and exec-plan state-type definitions
- Purpose: canonical state unions, transition tables, guards.
- Deliverables: `src/v2/types/task-state.ts`, `exec-plan-state.ts`, `index.ts`; unit tests.
- Dependencies: PR 02.
- Verify gate: `bun test tests/unit/v2/types/` green.
- Risk: low. Size: s.

### PR 04 — feat(v2/types): product-spec frontmatter types and ID generator
- Purpose: `ProductSpec`, `WorkType`, `RiskClass`, `SpecMode`, slug validator.
- Deliverables: `src/v2/types/product-spec.ts`, `spec-id.ts`, tests.
- Dependencies: PR 02.
- Verify gate: `bun test tests/unit/v2/types/product-spec.test.ts` green.
- Risk: low. Size: s.

### PR 05 — feat(v2/repo): evidence store port and transition-evidence emitter
- Purpose: ADR-0009 evidence emit; one row per transition.
- Deliverables: `evidence-store.port.ts`, `jsonl-evidence-store.adapter.ts`, `emit-transition-evidence.ts`; tests.
- Dependencies: PR 03.
- Verify gate: unit tests green; JSONL row format validates.
- Risk: low. Size: s.

### PR 06 — feat(v2/repo): task store port and JSONL adapter for v2 task ledger
- Purpose: persistence for v2 tasks.
- Deliverables: `Task` type, `task-store.port.ts`, `jsonl-task-store.adapter.ts`; tests.
- Dependencies: PR 03, PR 05.
- Verify gate: unit tests green.
- Risk: low. Size: s.

### PR 07 — feat(v2/repo): product-spec store port, YAML/MD parser, and spec writer
- Purpose: read/write `.maestro/specs/<slug>.md` with YAML frontmatter.
- Deliverables: `spec-store.port.ts`, `fs-spec-store.adapter.ts`, parse/serialize helpers; tests.
- Dependencies: PR 04.
- Verify gate: round-trip parse/serialize tests green; missing-field errors actionable.
- Risk: low. Size: s.

### PR 08 — feat(v2/service): maestro spec new + spec validate CLI verbs
- Purpose: ADR-0010 doc-driven entry; grill lives in SKILL.md not the verb.
- Deliverables: `spec-new.usecase.ts`, `spec-validate.usecase.ts`, `spec.command.ts`; wire into `src/index.ts`; e2e test.
- Dependencies: PR 07, PR 04.
- Verify gate: `./dist/maestro spec new <slug>` creates file; `spec validate` exits 0/1 correctly.
- Risk: medium (first v2 CLI surface; mustn't disturb v1 routing). Size: m.

### PR 09 — feat(v2/service): task from-spec + task claim with transition evidence
- Purpose: light-path entry; first transition-evidence-producing verbs.
- Deliverables: `task-from-spec.usecase.ts`, `task-claim.usecase.ts`, `task.command.ts`; alias `claim`; e2e.
- Dependencies: PR 06, PR 07, PR 05.
- Verify gate: e2e test asserts task state + evidence row.
- Risk: medium. Size: m.

### PR 10 — feat(v2/service): task block / task abandon with transition evidence
- Purpose: complete the side-exit verbs.
- Deliverables: `task-block.usecase.ts`, `task-abandon.usecase.ts`; aliases `block`, `abandon`; tests.
- Dependencies: PR 09.
- Verify gate: e2e test asserts states + evidence.
- Risk: low. Size: s.

### PR 11 — feat(v2/repo): ArchitectureRules port and YAML-backed default adapter
- Purpose: ADR-0005 port; default adapter reads docs/architecture.yaml.
- Deliverables: `architecture-rules.port.ts`, `yaml-architecture-rules.adapter.ts`; update architecture.yaml `passive_harness.forbidden_patterns`; tests.
- Dependencies: PR 02.
- Verify gate: port load + rule lookup tests green.
- Risk: low. Size: s.

### PR 12 — feat(v2/service): architecture-lint runner (no-runner-inversion + v2-layer-violation)
- Purpose: two rule families; called from `task verify`.
- Deliverables: `architecture-lint-runner.ts`; fixture-based tests.
- Dependencies: PR 11, PR 05.
- Verify gate: lint-runner unit tests green on positive + negative fixtures.
- Risk: medium (regex false-positives). Size: m.

### PR 13 — feat(v2/service): task verify with architecture-lint integration + task ship
- Purpose: hybrid auto-transitions (ADR-0004); Ralph Wiggum loop on FAIL; ship terminal.
- Deliverables: `task-verify.usecase.ts`, `task-ship.usecase.ts`; aliases; full lifecycle e2e.
- Dependencies: PR 09, PR 10, PR 12, PR 05.
- Verify gate: full light-path e2e test (`spec new -> from-spec -> claim -> verify -> ship`) green; correct evidence rows; FAIL fixture triggers `verifying -> doing`.
- Risk: high (auto-transition logic). Size: m.

### PR 14 — feat(v2/service): standalone lint:arch command
- Purpose: lint runner exposed outside verify; CI gate surface.
- Deliverables: `lint-arch.command.ts`; package.json script; e2e.
- Dependencies: PR 12, PR 11.
- Verify gate: `bun run lint:arch` exits 0 on clean; 1 on injected violation.
- Risk: low. Size: s.

### PR 15 — feat(skills): maestro-design SKILL.md with grill protocol (ADR-0016)
- Purpose: spec authoring interview-driven from day 1.
- Deliverables: `skills/bundled/maestro-design/SKILL.md` (new dir); minimal v2-verb section appended to `skills/bundled/maestro-task/SKILL.md`.
- Dependencies: PR 08, PR 13.
- Verify gate: `bun run check:bundled-skills` passes; grill steps present; verb refs match v2 surface.
- Risk: low. Size: m.

### PR 16 — feat(v2/repo): v1->v2 state-mapping functions and fixture-based migration tests
- Purpose: §9 migration tables as pure functions; not the orchestrator (that's Phase 3).
- Deliverables: `migrate-v1-task-state.ts`, `migrate-v1-exec-plan-state.ts`; fixture `tests/fixtures/migrate-v2/v1-snapshot/`; `tests/e2e/migrate-v2/state-mapping.test.ts`.
- Dependencies: PR 03, PR 06.
- Verify gate: table-driven tests cover every row; legacy-bypass behavior (deferred -> abandoned) verified.
- Risk: medium (normalization bypass is subtle). Size: m.

### PR 17 — docs: AGENTS.md v2 rewrite and Phase 1 done-criteria capture
- Purpose: doc the v2 surface; close Phase 1 with dogfood evidence.
- Deliverables: AGENTS.md rewrite (v2 STRUCTURE, WHERE TO LOOK, CLI VERBS); this exec-plan moved from `active/` to `completed/` after dogfood evidence captured.
- Dependencies: PRs 01-16.
- Verify gate: AGENTS.md WHERE-TO-LOOK rows resolve to real files; `bun run build` green; full e2e suite green.
- Risk: low. Size: s.

## Done criteria capture

Phase 1 completes when:
1. All 17 PRs landed on `harness-os` branch.
2. A real maestro change has been dogfooded through the v2 light path (spec -> task -> ship) with transition evidence recorded in `.maestro/evidence/`.
3. `maestro task verify` runs `runArchitectureLints` as one of its checks (PR 13 verify gate proves this).
4. This exec-plan moves to `docs/exec-plans/completed/phase-1-spine-and-lints.md` with a final note appended.
