# Project Instructions
@AGENTS.md

## The skill bundle is the spec

`skills/bundled/` (the 7 `maestro-*` skills installed to `~/.claude/skills/` and `~/.codex/skills/` by `maestro install`) is the **source of truth** for maestro's agent-facing CLI. The CLI must match what those skills describe.

`maestro-verify` is the canonical verification protocol. It is the single source of truth for the pre-claim ritual, witness levels, plan-check, AI Reviewer protocol, threat-model production, and cost-budget monitoring. Other skills (`maestro-task`, `maestro-plan`, `maestro-handoff`) cross-reference it. When verification behavior is unclear, read `maestro-verify` first.

- **When the CLI diverges from a skill, fix the CLI.** Do not "document around" the mismatch in the skill. The skills are what agents read; they must stay clean and authoritative.
- **Never edit a skill to match surprising CLI behavior.** If a skill needs to change (new section, new flag, renamed verb, semantic shift), stop and ask the user first. Skill content is behavioral contract, not scratch space.
- **Skill drafts belong in conversation, not in files.** Adjustments go through user approval before landing in `skills/bundled/`.

Local Maestro is advisory; CI Maestro is authoritative. The PR check status posted by `maestro ci verify` is the merge gate — see `docs/ci-integration.md`.

Auto-merge (`maestro merge auto`) requires a `PASS` verdict, a Spec quality score of 1.0 (when a Spec is associated with the task), and `autoMergeAllowed.<riskClass>: true` in `policies/autopilot.yaml`. See `docs/auto-merge-eligibility.md` for the full 8-predicate reference.

Deploy gate (`maestro deploy gate`) emits `kind=deploy-readiness` Evidence; runtime monitoring (`maestro runtime check`) emits `kind=runtime-signal` Evidence; rollback witness (`maestro deploy rollback`) emits `kind=rollback-exercised` Evidence. See `docs/deploy-gate.md` and `docs/runtime-monitoring.md`. Cross-task conflict detection emits `kind=cross-task-conflict` Evidence when other open PRs touch overlapping paths; the Risk Engine raises class one tier per signal. See `docs/cross-task-conflict.md`.

## Always release + link locally when testing

Every test/verification loop:
```bash
bun run release:local          # rebuild dist/maestro + install to PATH
```
Not `bun run build` alone. `release:local` is the only way to exercise the installed binary (`maestro` on `PATH`) against the current source. Testing only `./dist/maestro` can miss install-path or binary-packaging regressions.

## Quick Reference

### Build and verify
```bash
bun run build && ./dist/maestro --version
bun run release:local          # rebuild + install to PATH (preferred for verification)
```

### TUI preview (agent-friendly)
```bash
maestro mission-control --preview --size 120x40 --format plain
maestro mission-control --preview all --size 120x40 --format plain
maestro mission-control --render-check --size 120x40
bun tui:dev --screen all --size 120x40
```

### Test
```bash
bun test                       # full suite
bun test tests/unit/tui/       # TUI unit tests only
```

### Mission bundles
```bash
maestro bundle export <missionId> --out ./review.mission.tar.gz
maestro bundle export <missionId> --redact memory,prompts --json
maestro bundle inspect ./review.mission.tar.gz --json
```

### Conventions
- Conventional Commits: `feat(scope):`, `fix(scope):`, `refactor(scope):`
- Bump version for every behavior change (minor=feature, patch=fix, major=breaking)
- Verify against `./dist/maestro`, not `maestro` on PATH, unless specifically testing the installed binary

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **maestro** (10765 symbols, 18180 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/maestro/context` | Codebase overview, check index freshness |
| `gitnexus://repo/maestro/clusters` | All functional areas |
| `gitnexus://repo/maestro/processes` | All execution flows |
| `gitnexus://repo/maestro/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
