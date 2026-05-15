# Phase 1.5 — Done

Principles + correction-recording bridge (ADR-0015) is feature-complete on
`main`. This document captures the dogfood run satisfying the master-plan
done criterion for Phase 1.5:

> a real correction captured during Phase 1 dogfooding gets promoted to a
> principle markdown via the new verb, and `gc slop-cleanup` finds at least
> one violation in the maestro codebase and prints its fix recipe.

## PRs

| PR  | Scope                                                                    | Task |
| --- | ------------------------------------------------------------------------ | ---- |
| 25  | Principle type + FS store + ProcessRunner + 4 default principles         | #25  |
| 26  | `principlesScan` usecase                                                 | #26  |
| 27  | `maestro principle promote <evd-id>` verb + scaffold renderer            | #27  |
| 28  | `maestro setup migrate-corrections` (v1 memory → docs/principles/legacy) | #28  |
| 29  | `gc slop-cleanup` rewire to call `principlesScan` + this capture         | #29  |

## What landed

- **Storage spine.** `Principle` type (slug, rule, rationale, scan_command,
  fix_recipe), `PrinciplesStorePort` with `list / get / exists / write`, and
  `FsPrinciplesStore` reading `docs/principles/*.md` via `## Rule | Rationale
  | Scan Command | Fix Recipe` sections. `list()` skips `legacy/` so migrated
  v1 corrections stay quarantined until an operator promotes them.
- **Process runner.** `ProcessRunnerPort` + `BunProcessRunner` (Bun.spawn
  with stdout/stderr pipes; no `setTimeout`, no daemons — passive-harness
  invariant intact).
- **Default principle pack.** Four committed principles ship with the
  binary: `prefer-shared-utils`, `no-yolo-data-probing`, `passive-harness`,
  `layer-order`. Each owns a real scan command that exits 0 on clean and
  non-zero with stdout when violations are present.
- **Scan usecase.** `principlesScan({principlesStore, processRunner,
  repoRoot, only?})` runs each principle's scan command and returns
  `PrincipleScanFinding[]` with `kind` ∈ {`violation`, `scan-error`}. Lines
  parsed as `<file>:<line>: <message>` when matching, raw otherwise.
- **Promote verb.** `maestro principle promote <evd-id>` materializes
  `docs/principles/<slug>.md` from a `lint-violation` evidence row. Slug
  derives from `rule_id` (underscores → hyphens, lower-cased), collisions
  walk `-2`, `-3`, … via `exists()`. Refuses non-lint-violation kinds.
- **Migrate verb.** `maestro setup migrate-corrections` walks
  `.maestro/memory/corrections/*.json` and writes
  `docs/principles/legacy/<id>.md`, mapping `rule → ## Rule`, severity →
  rationale note, and `trigger.keywords / fileGlobs` → scan-command TODOs.
  No-op when the source dir is absent. Skip-on-collision by default;
  `--overwrite` forces replacement.
- **gc rewire.** `scanSlopCleanup` now folds principle-scan findings into
  the same `SlopCleanupResult` shape: principle slugs land in `byRule`,
  findings without a file path still bump `bySeverity.error`, and files
  with both arch-lint and principle findings merge into one group.
  External shape preserved per PD-7; consumers continue to read
  `byRule[slug]`, `groups[].ruleIds`, `bySeverity.error` unchanged.

## ADRs that constrain Phase 1.5

- ADR-0015 absorbed `memory` + `memory-ratchet` + `agent` into principles.
- ADR-0017 cross-cutting layer semantics (the v1 `gc` feature legitimately
  imports `@/v2/service/principle-scan.usecase.js` because v2 layer order
  only enforces the chain within `src/v2/**`).

## Test suite

After PR 29: 2989 pass / 0 fail / 112 skip across 3101 tests / 358 files
(local `bun test`). New Phase 1.5 coverage adds **53 tests**:

- 6 unit (`tests/unit/v2/types/principle.test.ts`)
- 10 unit (`tests/unit/v2/repo/fs-principles-store.test.ts`)
- 3 unit (`tests/unit/v2/repo/bun-process-runner.test.ts`)
- 2 unit (`tests/unit/v2/providers/build-services.test.ts`)
- 11 unit (`tests/unit/v2/service/principle-scan.usecase.test.ts`)
- 11 unit (`tests/unit/v2/service/principle-promote.usecase.test.ts`)
- 9 unit (`tests/unit/v2/service/migrate-corrections.usecase.test.ts`)
- 5 e2e (`tests/e2e/v2-principle-promote.test.ts`)
- 4 e2e (`tests/e2e/v2-setup-migrate-corrections.test.ts`)
- 3 unit added to the existing `slop-cleanup.usecase.test.ts`

Architecture lint stays clean at 52 v2 files scanned.

## Dogfood capture (real binary, real evidence)

The deliberate-FAIL flow from PD-6:

1. Seed a synthetic correction in today's evidence log:

   ```json
   {
     "id": "evd-phase1p5-dogfood",
     "kind": "lint-violation",
     "timestamp": "2026-05-15T15:00:00Z",
     "rule_id": "prefer_shared_utils_demo",
     "severity": "error",
     "file": "src/v2/repo/fs-principles-store.adapter.ts",
     "line": 1,
     "message": "Demo correction: duplicate slug-normalization helper used in two adapters",
     "remediation": "Move slug normalization into src/v2/types/principle.ts and re-export."
   }
   ```

2. Promote it through the installed binary:

   ```
   $ maestro principle promote evd-phase1p5-dogfood
   prefer-shared-utils-demo -> docs/principles/prefer-shared-utils-demo.md
     from evd-phase1p5-dogfood (rule_id=prefer_shared_utils_demo)
   ```

   The generated scaffold (verified by inspection) carries the message as
   `## Rule`, the `(file, line, timestamp)` triple as `## Rationale`, and
   the `remediation` body as `## Fix Recipe`. The `## Scan Command` section
   ships as a `# TODO:` stub; promotion is intentionally a scaffold, not a
   finished principle.

3. Re-run `gc slop-cleanup` against the committed default principle pack
   (no scratch row in the log):

   ```
   $ maestro gc slop-cleanup --min-severity error --json
   { "totalViolations": 0, "filesAffected": 0,
     "bySeverity": { "error": 0, "warn": 0, "info": 0 },
     "byRule": {}, "groups": [], "principleFindings": [] }
   ```

   With `--min-severity info`, the same run surfaces real findings (148
   `no-bare-console-log`, 7 `file-size-limit`, 1 `no-hand-edit-generated`)
   across 31 v1 files. Each rule's remediation is the live fix recipe
   wired through the arch-lint corpus.

Per PD-6, the scratch evidence row and the `prefer-shared-utils-demo.md`
file are **not committed** — they were created locally, exercised, and
reverted before this commit. The IDs above are reproducible verbatim
against any fresh checkout.

## What's next

Phase 1.5 closes the correction-recording bridge. Phase 3 (observability +
setup hardening) and Phase 4 (v1 surface removal) remain on the master plan.

Refs: `docs/v2-master-plan.md` §10 Phase 1.5, ADR-0015.
