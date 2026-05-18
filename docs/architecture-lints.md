# Architecture lints

Architecture lints are the 8th Trust Verifier check. They enforce repo-shape
invariants that other tools (typecheck, tests, feature-boundaries) cannot
catch on their own.

The CLI surface is `bun run lint:arch` (standalone). The same library powers
the 8th check inside `maestro task verify` and `maestro ci verify`.

## Rules (Phase 1)

### `no-runner-inversion` — error

Maestro must not spawn Claude or Codex CLIs as subprocesses. The agent calls
maestro; the inverse breaks the harness model.

The rule scans `src/**/*.ts` (excluding `tests/**` and `scripts/**`) for
`Bun.spawn`, `Bun.spawnSync`, `child_process.spawn`/`exec*`, `spawn`,
`spawnSync`, `execFile`, `execFileSync`, `exec`, or `execSync` whose first
positional argument matches `claude`, `codex`, `claude-code`, or `codex-cli`.

Per-line escape: `// lint-arch-allow: no-runner-inversion`.

### `single-opentui-render` — error

OpenTUI's `root.render()` must be called at most once per process. Repeated
calls corrupt internal state. The canonical pattern is a bridge component
that renders once and uses `useState`/`setState` to update the tree —
`src/tui/opentui/app/interactive.tsx` is the reference.

The rule scans `src/tui/**/*.{ts,tsx}` excluding `**/testing/**` (the test
frame-capture path legitimately renders separately) and counts
`root.render(` call expressions per file.

### `mission-control-readonly` — warn

Mission Control snapshot, preview, and render-check paths must not write.
Agents inspect these screens assuming side-effect-free reads.

The rule scans the bodies of `buildSnapshot` and `buildHomeSnapshot` in
`src/tui/state/snapshot.ts`, plus the preview/render-check entrypoints in
`src/infra/commands/mission-control.command.ts`, for `await x.METHOD(`
expressions whose method name is on the hardcoded write-method allowlist:
`append`, `write`, `record`, `create`, `update`, `delete`, `claim`,
`unclaim`, `block`, `unblock`, `heartbeat`, `syncMetadata`, `backfillSlug`,
`backfillSlugs`, `createBatch`, `releaseOwned`, `reopen`, `increment`.

This is a heuristic — regex-based static analysis cannot reliably catch
indirect writes via aliased variables or dynamic dispatch. The severity is
`warn` for Phase 1 to avoid false-positive blocks. Phase 3 will promote it
to `error` once a TS-AST-based replacement lands.

Per-line escape: `// lint-arch-allow: mission-control-readonly`.

### `no-hand-edit-generated` — error (diff-aware)

Generated template files (`src/infra/domain/built-in-skill-templates.ts`,
`src/infra/domain/bundled-skill-templates.ts`) are produced by the
`sync:built-in-skills` and `sync:bundled-skills` scripts. Hand-edits are
silently overwritten on the next sync.

The rule fires when the diff includes one of the generated paths but no
`skills/built-in/**` or `skills/bundled/**` source path was touched.

When no diff is supplied (e.g., `bun run lint:arch` without `--base`), the
rule self-skips with a non-blocking `info` finding.

## Escape hatch

```ts
// lint-arch-allow: <ruleId>[, <ruleId>...]
problematicCallExpression();
```

The comment may sit on the same line or one line above. Multiple rules can
be allowlisted in a single comment by separating them with commas or
whitespace.

## Witness levels for `lint-violation` evidence

When a rule fires under one of the production verbs, a `lint-violation`
Evidence row is recorded:

| Verb              | Witness level              |
| ----------------- | -------------------------- |
| `task verify`     | `agent-claimed-locally`    |
| `ci verify`       | `witnessed-by-ci`          |
| `session start`   | `witnessed-by-maestro`     |
| `session exit`    | `witnessed-by-maestro`     |

`task introspect <id>` reads these rows to display "open lints" without
re-running the lint pass. See `docs/witness-levels.md` for the full ladder.

## Rules (Phase 3)

Phase 3 ships two new rules at `severity: warn` (audit mode). They share the
library and emit `lint-violation` evidence at the same witness levels.

### `composition-only-in-services-ts` — warn

Cross-feature value imports of another feature's `services.ts` (e.g.
`import { x } from "@/features/foo/services.js"`) belong in the global
composition root `src/services.ts`. Other files should depend on ports, not
directly compose another feature's services.

The rule scans `src/**/*.ts` and ignores `import type` declarations (those
do not import code). `src/services.ts` itself is exempt.

Per-line escape: `// lint-arch-allow: composition-only-in-services-ts`.

### `task-vs-mission-separation` — warn (historical)

This rule was defined for `src/features/task/**` and `src/features/mission/**`,
which no longer exist. The rule is retained in the document for historical
reference; it is a no-op at runtime because neither directory exists. Shared logic
now belongs in `src/` or `src/shared/`.

The first two plan-listed Phase 3 rules (`no-deep-cross-feature-imports`,
`feature-public-via-index`) are already enforced as errors by
`bun run check:boundaries` — the lint library does not duplicate those.

## Adding a new rule

1. Add a new `ArchitectureRuleId` literal in
   `src/shared/lib/arch-rules.ts`.
2. Add a remediation string to `REMEDIATION` (used in violation output and
   Trust Verifier finding `details`).
3. Implement the rule as an async function returning
   `ArchitectureViolation[]` and add it to the `Promise.all` in
   `checkArchitectureRules`.
4. Wire severity intent: `error` for hard repo-shape violations,
   `warn` for heuristics with known false-positive risk, `info` for
   advisory-only.
5. Add tests under `tests/unit/shared/lib/arch-rules.test.ts`.
6. Update this document.

## Rules (Phase 4 — taste)

Phase 4 ships three taste-level rules at `severity: info` (audit-only). They
flag stylistic drift; CI does not fail on `info` findings, but they show up in
`bun run lint:arch` output and in `gc slop-cleanup` reports.

### `file-size-limit` — info

Files in `src/**/*.{ts,tsx}` over 800 lines are flagged. Generated template
files (`built-in-skill-templates.ts`, `bundled-skill-templates.ts`) are
exempt. Per-line escape: `// lint-arch-allow: file-size-limit`.

### `no-bare-console-log` — info

Bare `console.log/info/debug/warn` calls under `src/**` (excluding `src/tui/**`
where the OpenTUI render loop legitimately uses console output). Use the
`output()` helper or stderr for diagnostics. Per-line escape:
`// lint-arch-allow: no-bare-console-log`.

### `kebab-case-filenames` — info

`.ts` files under `src/**` should be kebab-case. `.tsx` files are exempt
(PascalCase component filenames are conventional in React/JSX).

## Phase 4 verbs that consume the lint library

- `maestro gc slop-cleanup` — aggregates all rule violations into a per-file
  slop report. Optional `--min-severity warn|error|info`.
- `maestro gc plan-regen --task <id>` — checks plan-vs-state drift (no plan
  file, missing acceptance-criteria coverage, evidence after last PASS,
  blockers active, recorded lint violations).
- `maestro contract sprint --task <id>` — sprint snapshot (criteria progress,
  amendment budget, recent amendments). `--propose <text>` records a proposal
  as `manual-note` evidence; does not mutate the contract.

## Phase 5 verbs

- `maestro inspect run <taskId>` — post-mortem snapshot (run-dir artifacts,
  recent evidence, verdict history). Read-only.
- `maestro worktree create <slug> [--base <branch>] [--prefix <pre>]` — wraps
  `git worktree add` and provisions an isolated `.maestro/runs/` directory.

## Audit mode → soak → block

New rules ship at `severity: warn` (Phase 3) or `severity: info` (Phase 4) so
they appear in lint output and `task introspect` "open lints" without
failing Trust Verifier or blocking a verdict. After a soak window where main
has zero violations under the rule, promote to `severity: error` by editing
the rule's `severity` field and updating the remediation.
