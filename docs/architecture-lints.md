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

## Adding a new rule

1. Add a new `ArchitectureRuleId` literal in
   `src/features/verify/usecases/checks/check-architecture-lints.ts`.
2. Add a remediation string to `REMEDIATION` (used in violation output and
   Trust Verifier finding `details`).
3. Implement the rule as an async function returning
   `ArchitectureViolation[]` and add it to the `Promise.all` in
   `checkArchitectureRules`.
4. Wire severity intent: `error` for hard repo-shape violations,
   `warn` for heuristics with known false-positive risk, `info` for
   advisory-only.
5. Add tests under `tests/unit/features/verify/usecases/checks/check-architecture-lints.test.ts`.
6. Update this document.

Phase 2 and beyond add Tier 2 (advisory-promote) and Tier 3 (taste) rules
under the same library.
