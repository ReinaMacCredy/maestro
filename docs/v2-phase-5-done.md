# Phase 5 Done: v1 Source-Code Sunset

**Status:** Phase 5 complete on 2026-05-16.

---

## What landed

### Pre-Phase-5 prep (docs split + Phase 4 close)

- `4f84793b` `docs(plan)`: split Phase 4 into Phase 4 (docs/skills) + Phase 5 (v1 source-code sunset). Updated master plan with ADR-0018/0019 references and Phase 6-7 scope.
- `763448d3` `docs(phase-4)`: Phase 4 done doc and dogfood evidence.

These shipped before Phase 5 proper but are counted in the `c042936a..HEAD` window.

---

### PR-A: Extract reply + principle stores out of `src/features/mission/`

- `10969752` `refactor(harness-os)`: extract principle + reply features out of mission (Phase 5 PR-A).

Moved the v1 principle JSONL store and reply store into sibling features `src/features/principle/` and `src/features/reply/`. Mission barrel kept backward-compat re-exports so PR-C could delete mission cleanly. Direct consumers (init usecase, index registrations, TUI reply-projection) rewired to the new locations.

---

### PR-B: Rewire TUI Mission Control onto exec-plan shapes

- `e0563a4f` `refactor(harness-os)`: move v1 mission domain machinery to `shared/domain/legacy-mission/` (Phase 5 PR-B).

The v1 mission types, state machine, validators, IDs, workflows, ports, adapters, and missions usecase were moved from `src/features/mission/` into `src/shared/domain/legacy-mission/`. Bundle, handoff, reply, and TUI snapshot paths consume `Missions` (read-only) from this shared home. Mission Control TUI continued to work through the PR-C deletion.

---

### PR-C: Delete `src/features/mission/` + rewire bundle + handoff off mission types

- `cc83001a` `feat(harness-os)`: delete `src/features/mission/` + rewire init to v2 principles (Phase 5 PR-C).

With all consumers off it, `src/features/mission/` was deleted wholesale. Init was rewired to the v2 principles path. Bundle and handoff continue to consume the legacy-mission store ports from `src/shared/domain/legacy-mission/`.

Also in this window (MCP task_list filter alignment):

- `fa707ebf` `refactor(mcp)`: rename `missionId` to `plan_id` on task_list filter.

---

### PR-D0: Rehome cross-cutting types out of v1 spec/task to `src/v2/`

- `a0905731` `refactor(harness-os)`: rehome cross-cutting types to `v2/legacy-spec` (Phase 5 D0).

Spec and task domain types that were entangled with v1 feature paths were extracted into `src/v2/` so the subsequent feature deletions (D-setup, D-verify, D-spec, D-task-rehome) would not break v2 consumers.

---

### Bulk feature deletions: v1 source-code sunset

- `f9e30e0a` `feat(harness-os)`: delete v1 `memory`, `memory-ratchet`, `agent` features.
- `88f19303` `feat(harness-os)`: delete v1 `graph`, `session`, `notes`, `intake` features.
- `08b4e7bb` `feat(harness-os)`: delete v1 `ralph`, `inspect`, `state` features + `lint:arch` cleanup.

These three commits deleted the ADR-0015 absorbed/dropped directories: the memory+correction store (migrated to `docs/principles/legacy/` in production), project graph (migrated to `docs/references/`), session (absorbed into handoff + worktree), intake/brainstorm (folded into product-spec authoring), and the v1 diagnostic features that v2 does not expose.

---

### PR-D-setup, PR-D-verify, PR-D-spec: Delete v1 `src/features/{setup,verify,spec}/`

- `e8f879f0` `feat(harness-os)`: delete v1 `src/features/setup/` (Phase 5 D-setup).
- `4eb3cc80` `feat(harness-os)`: delete v1 `src/features/verify/` (Phase 5 D-verify).
- `735fcfb0` `feat(harness-os)`: delete v1 `src/features/spec/` (Phase 5 D-spec).

The three v1 feature dirs whose verbs v2 owns outright were deleted. v2 runtime commands (`src/v2/runtime/`) are the sole implementation for setup, verify, and spec.

---

### PR-D-task-rehome: Rehome v1 task surface to legacy-task + delete `src/features/task/`

- `c4958a4b` `refactor(task)`: rehome v1 task surface to `legacy-task` and delete `src/features/task/`.

The v1 task surface (contract state, run-state, task types, legacy ID pattern) was moved to `src/features/legacy-task/` to serve MCP and contract consumers that still needed it. `src/features/task/` was then deleted. v2 task lifecycle lives exclusively in `src/v2/`.

Bugfix follow-on in this group:

- `63a0c4f6` `fix(legacy-task)`: accept v2 task IDs in legacy contract + run-state adapters.

---

### PR-D-task-MCP: MCP server v2 verb pass + port contract usecases

- `7373e067` `feat(mcp)`: rewire MCP tools to v2 use cases and delete `src/features/task/` shim.

MCP tool surface aligned with v2: `task_complete` renamed to `task_ship`; `task_unblock` deleted; `task_create`/`task_plan` replaced by `task_from_spec`; `principle_promote`, `setup_check`, `setup_migrate_v2` added; kept tools (`task_claim`, `task_block`, `task_get`, `task_list`, `evidence_*`, `verdict_*`, `policy_*`, `handoff_*`, `contract_*`) rewired to v2 use cases.

---

### PR-G: Install-smoke workflow + release.yml v2 verbs

- `596ed395` `ci(workflows)`: exercise v2 hot-path verbs in install-smoke and release.
- `e5386f8d` `fix(build)`: tolerate missing `skills/built-in/` directory in `collectSkills`.

Install-smoke workflow extended from `maestro --version` + render-check only to a full `setup bootstrap`, `setup check`, `spec new`, `task from-spec`, `task claim`, `task verify`, `task ship` sequence. A broken v2 binary now fails the smoke job instead of escaping to a release. Build fix: `collectSkills` no longer throws if `skills/built-in/` was never populated (regression from the colon-tier deletion in Phase 4).

---

### PR-H: README v2 sweep + AGENTS.md WHERE-TO-LOOK rewrite + §9 audit

- `0ebd0ed0` `docs`: Phase 5 PR-H sweep - v2 verb audit, WHERE-TO-LOOK rewrite, UPGRADING.md, §9 audit.

All root-level documentation scanned and updated for v2-only verb references. AGENTS.md WHERE-TO-LOOK table rewritten to point at surviving `src/v2/` and retained `src/features/` paths (deleted paths removed). `UPGRADING.md` finalized as the user-facing v1 to v2 upgrade guide. §9 artifact-coverage table in the master plan brought to 100% coverage - all dirs found in real `.maestro/` trees classified as in-mapping or skip-silently/preserved.

---

### README v2 rewrite (PR 53.5)

- `6f2685c7` `docs(readme)`: rewrite for v2 (spec to task to verify to ship).

`README.md` rewritten end-to-end around v2 primitives and the `spec -> task claim -> task verify -> task ship` lifecycle. All v1 verb references (mission, intake, session) removed.

---

### Dogfood bug fix (Phase 5 closing PR)

- `06529ef5` `fix(v2/verify)`: treat missing `docs/architecture.yaml` as zero lint violations.

Discovered during Phase 5 external-project dogfood: external projects without a `docs/architecture.yaml` hit an uncaught `ArchitectureRulesNotFoundError` when running `task verify`. Fixed by catching `ArchitectureRulesNotFoundError` specifically in `task-verify.usecase.ts` and treating it as an empty lint report (PASS). A project with no architecture rules configured has nothing to violate. One unit test added pinning the new behavior.

---

## Done criteria check

From master plan §10, Phase 5:

**v2 e2e green (all `tests/e2e/v2-*.test.ts` pass):**
✓ DONE. `bun test` runs 1835 pass / 0 fail across 229 files. v2 e2e tests (`tests/e2e/v2-task-verify-ship.test.ts`, `tests/e2e/v2-task-verify-verdict.test.ts`) green.

**Kept-feature e2e green:**
✓ DONE. No regressions in retained feature tests (evidence, contract, verdict, policy, ci, merge, bundle, handoff, worktree, legacy-task). 0 fail.

**Install-smoke workflow green against v2 verbs:**
✓ DONE. `596ed395` extended `.github/workflows/install-smoke.yml` to exercise `setup bootstrap`, `setup check`, `spec new`, `task from-spec`, `task claim`, `task verify`, `task ship`. The workflow now fails on a broken v2 binary rather than passing vacuously.

**No dead imports:**
✓ DONE. Build succeeds (`bun run build` compiles 686 modules without error). Feature deletions cleaned up cross-feature imports. `bun run check:boundaries` passes.

**README and root docs reference v2 verbs only:**
✓ DONE. PR-H sweep plus the README rewrite removed all v1-only verb references (mission, intake, session verbs) from `README.md`, `AGENTS.md`, `CLAUDE.md`, `UPGRADING.md`, and `docs/harness-positioning.md`.

**Maestro completes one dogfooded spec to task to ship cycle on itself using only v2 verbs:**
✓ DONE. Phase 4 dogfood evidence at `docs/phase-4-done.md`. Phase 5 development itself was conducted with v2 verbs for task lifecycle tracking.

**One Phase 5 external-project dogfood completed:**
✓ DONE. See transcript below.

---

## External-project dogfood transcript

**Sandbox path:** `/var/folders/63/s3x44l1935l9dkthd410t7mr0000gn/T/maestro-dogfood2-XXXXXX.alyudUmsbB`

**Maestro version:** `0.83.0.1778891833-g6f2685c` (built 2026-05-16, then `06529ef5` fix applied and reinstalled).

Note: `spec new <slug> --title "..."` is the canonical non-interactive spec path per `scripts/v2-smoke.ts` header. The master plan's earlier reference to `--from-file` mode is documented as doc drift in the smoke script; that flag does not exist.

```
$ git init -q && echo "# Dogfood test project" > README.md
$ git add README.md && git commit -q -m "init"

$ maestro setup bootstrap
created 5, skipped 0
  created .maestro/tasks
  created .maestro/plans
  created .maestro/evidence
  created .maestro/runs
  created docs/principles
exit=0

$ maestro setup check
[ok]   .maestro/tasks
[ok]   .maestro/plans
[ok]   .maestro/evidence
[ok]   .maestro/runs
[ok]   docs/principles
[warn] docs/principles/*.md -- no principles found; run `maestro setup bootstrap` to seed the default pack
[warn] .maestro/config.yaml -- config.yaml not present (optional)
setup check: OK
exit=0

$ maestro spec new dogfood-add-greeting --title "Add greeting endpoint"
Created spec at .maestro/specs/dogfood-add-greeting.md
exit=0

$ maestro task from-spec .maestro/specs/dogfood-add-greeting.md
tsk-mp7mh4wh-m1e0da draft (dogfood-add-greeting)
exit=0

$ maestro task claim tsk-mp7mh4wh-m1e0da --skip-worktree
tsk-mp7mh4wh-m1e0da claimed
exit=0

$ maestro task verify tsk-mp7mh4wh-m1e0da
tsk-mp7mh4wh-m1e0da verified -> ready (PASS)
exit=0

$ maestro task ship tsk-mp7mh4wh-m1e0da
tsk-mp7mh4wh-m1e0da shipped
exit=0
```

**Result: PASS.** Full cycle `setup -> spec new -> task from-spec -> claim -> verify -> ship` completed on a fresh greenfield project with no pre-existing `docs/architecture.yaml`.

**Bug found during dogfood (first run, since fixed):**

First dogfood run (sandbox `maestro-dogfood-XXXXXX.sLf5IJhDga`) hit an uncaught `ArchitectureRulesNotFoundError` at `task verify`. Root cause: `task-verify.usecase.ts` did not handle a missing `docs/architecture.yaml`. External projects (and any internal project that has not authored one) would crash instead of verifying.

Fix: `06529ef5` catches `ArchitectureRulesNotFoundError` in `task-verify.usecase.ts` and treats it as an empty lint report (PASS). Second dogfood run on a fresh sandbox passed end-to-end.

---

## What is next

Phase 6: Scenario testing.

Eight behavioral scenarios across project x familiarity x workflow, driven by a swarm-fix-loop. Concrete deliverables: scenario markdown files under `tests/scenarios/<name>/`, deterministic rubric runners, fixture directories (greenfield and brownfield v1 fixture), agent brief files, rubric runner (`bun scripts/scenarios/check.ts`), and swarm dispatcher (`bun scripts/scenarios/swarm.ts`). Done criteria: all 8 scenarios green in a single swarm pass with zero fixes in between.

See master plan §10 Phase 6 for the full loop shape and per-scenario contract.

---

## Open follow-ups

These were flagged during Phase 5 but not landed. Deferred to Phase 7 cleanup unless they block Phase 6.

**`legacy-task` contract-state ANY_TASK_ID_PATTERN alias:**
`src/features/legacy-task/domain/contract-state.ts` exports `ANY_TASK_ID_PATTERN` as an alias for `TASK_ID_PATTERN` (the v2 ID format). The alias exists for backward compat with MCP contract consumers; it is dead code if no caller uses it. Audit at Phase 7 and delete if unused.

**`docs/architecture-lints.md` task-vs-mission-separation rule may still reference deleted paths:**
The `task-vs-mission-separation` architecture lint rule was added in Phase 1 to prevent mission types from leaking into task code. After Phase 5, `src/features/mission/` no longer exists. The rule in `docs/architecture-lints.md` (if present) may now point at deleted paths and should be reviewed or removed. `src/shared/lib/arch-rules.ts` may still carry the rule definition; audit whether it is still meaningful against the surviving `src/shared/domain/legacy-mission/` layout.

**`setup bootstrap` does not warn about missing `docs/architecture.yaml`:**
External projects discover the requirement only when `task verify` runs. A `setup check` warning or a `setup bootstrap` seed for a minimal `docs/architecture.yaml` would surface this earlier. Deferred to Phase 7; the dogfood fix (`06529ef5`) makes the absence non-fatal, so this is a UX improvement, not a correctness issue.

**Phase 7 RC dogfood is a separate run:**
Phase 5's external-project dogfood (recorded above) satisfies the Phase 5 done criterion. Phase 7 requires a second dogfood pass against the `v2.0.0-rc.1` tag, conducted during the 7-day RC soak window.
