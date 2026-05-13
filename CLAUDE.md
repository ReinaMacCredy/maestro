# Project Instructions
@AGENTS.md

Harness positioning: see `docs/harness-positioning.md`. The skill bundle in
`skills/bundled/` is the source of truth for agent-facing CLI; CLI must match.

`maestro-verify` is the canonical verification protocol; other skills cross-reference it.

- When the CLI diverges from a skill, fix the CLI. Do not document around the mismatch.
- Skill drafts go through user approval before landing in `skills/bundled/`.
- Local Maestro is advisory; CI Maestro is authoritative. The PR check posted by `maestro ci verify` is the merge gate. See `docs/ci-integration.md`.
- Auto-merge requires PASS + Spec score 1.0 + `autoMergeAllowed.<class>: true`. See `docs/auto-merge-eligibility.md`.
- Deploy gate, runtime monitor, rollback witness, cross-task conflict — each emits a typed Evidence kind. See `docs/deploy-gate.md`, `docs/runtime-monitoring.md`, `docs/cross-task-conflict.md`.
- Agent-facing list verbs are lean by default; `--full` / `view: "full"` recovers the verbose shape. See `docs/token-budget.md`.

## Always release + link locally when testing
```bash
bun run release:local          # rebuild dist/maestro + install to PATH
```

`release:local` is the only way to exercise the installed binary on `PATH`.

## Quick reference
```bash
bun run build && ./dist/maestro --version
maestro mission-control --preview --size 120x40 --format plain
maestro mission-control --render-check --size 120x40
bun tui:dev --screen all --size 120x40
bun test
maestro bundle export <missionId> --out ./review.mission.tar.gz
maestro setup --check
```

Conventional Commits: `feat(scope):`, `fix(scope):`, `refactor(scope):`. Bump
the CLI version for every behavior change.

## GitNexus
See `docs/gitnexus-usage.md`. Run `gitnexus_impact` before editing a symbol;
run `gitnexus_detect_changes` before committing.
