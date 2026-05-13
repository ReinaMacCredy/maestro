# Harness Pivot Completion Plan

Branch: `feat/harness-pivot`
Generated: 2026-05-12
Status: every in-scope phase (A–F) ships via `/goal`; skill bundle edits are user-approved and included in each phase's goal

---

## Critical correction (still applies)

The DI migration is **done**. Commit `c28f48a3 refactor(services): replace singleton with createServices factory + explicit deps` landed the final piece atop 6 prior cluster migrations. Tonight's goal is **not** "finish DI." Current dirty tree is `.claude/scheduled_tasks.lock` (lock file) and a 3-line cleanup in `src/features/deploy/index.ts`; neither blocks.

The 5 commands without `*CommandDeps` (`mcp serve`, `mcp check`, `skills`, `update`, `uninstall`, `providers`) are intentional — they do not consume `Services`.

---

## Resolved defaults

| Question | Decision | Rationale |
|---|---|---|
| Edge-case subset (Phase C) | Ship 5: anchor staleness, tool-call loop detection, ProofMap holes, cost-budget mid-iteration message, skill/binary drift | All five land inside the agent's hot path; defer worktree orphans, AI/verifier divergence, stale autopilot soak, lock contention |
| TOC size budget (Phase B sub-goal 4) | 160 lines for `AGENTS.md`; soft warn at 140 | Tight enough to force discipline; absorbs a real index + WHERE-TO-LOOK table |
| Host runtimes for hook install (Phase B sub-goal 2) | Claude Code, Codex, Cursor | Three highest-coverage host runtimes; Aider/Continue follow once the install pattern is proven |
| CLI VERBS displacement (Phase D) | New `docs/cli-reference.md` (one flat file absorbing displaced sections) | Keeps the surface discoverable for humans; bundled skills still own the agent-facing copy |
| DevObservability adapter scope (Phase F) | File-tail + Prometheus HTTP; defer Loki/LogQL | No new dependencies; covers local-dev path; Loki adapter becomes a follow-up phase if real worktrees force it |
| Skill bundle edits | User-approved and included in each phase's auto-run goal | Each skill change is tightly scoped to verbs the phase adds or modifies; `bun run sync:bundled-skills` + `bun run check:bundled-skills` are part of every goal's terminal checks |

---

## Running the plan

Each phase has one `GOAL` string. Fire them via `/goal <string>` sequentially. Each goal is satisfied when the named artifacts exist, source files compile, tests pass, bundled-skill parity checks pass, and the change is committed.

**Phase ordering:** A → B → C → D → E → F. G is deferred (no design; reaches for signal from F).

**Pre-flight before any goal**: `git status` clean (current dirty files committed or stashed), on `feat/harness-pivot`, `bun run build` green.

**Skill bundle edit workflow inside a phase**: edit `skills/bundled/maestro-*/SKILL.md` → run `bun run sync:bundled-skills` (regenerates `src/infra/domain/bundled-skill-templates.ts`) → verify `bun run check:bundled-skills` passes → commit both files.

---

## Phase A — TONIGHT (sleep-safe)

### What
Create `docs/harness-positioning.md`: mechanical mapping of OpenAI's harness principles to existing maestro primitives.

### Why
Purely additive; every later phase cites it; reduces churn. No deletions, no judgment calls, all cross-references target docs that already exist.

### GOAL
```
docs/harness-positioning.md exists, ≤250 lines, has 5 principle sections (Context-as-scarce-resource, Per-worktree-observability, Isolated-worktrees, Continuous-quality-grading, Building-blocks) each with at least one src/ or docs/ pointer, an External-triggers subsection citing docs/schedule-recipes.md, a What-this-is-not subsection naming "no scheduler, no daemon, no LLM client, no background process inside maestro", a reference to docs/architecture-lints.md no-runner-inversion rule; bun run check:boundaries passes; committed on feat/harness-pivot
```

### Files changed
- `docs/harness-positioning.md` (create)

### Rollback
`rm docs/harness-positioning.md && git restore .`

---

## Phase B — Setup + init hardening

### What
Make `maestro setup` produce a harness-correct project state on install, detect drift, and self-test before declaring success.

### Sub-goals
1. **Setup self-test** — synth test task → `session start` → `evidence record --kind manual-note` → `verdict request` → `session exit`. Abort with diagnostics on any failure. tmpdir-isolated; no `.maestro/` pollution.
2. **Host-runtime hook installation** — detect `.claude/`, `.codex/`, `.cursor/`; install `SessionStart` + `SessionEnd` hooks invoking `maestro session start/exit "$TASK_ID"`. Idempotent.
3. **`maestro setup --check` audit mode** — read-only audit: skill/binary version parity, `AGENTS.md` line count vs budget, `docs/` presence, hooks installed, owners.yaml roles populated, no orphan `.maestro/runs/`. Exit 0 clean / 1 with itemized findings.
4. **TOC size-budget enforcer at write time** — `init-deep` and `maestro setup` refuse to write `AGENTS.md` >160 lines. Soft warn at >140. Configurable via `.maestro/config.json` field `tocSizeBudget`.
5. **Bootstrap template TOC rewrite** — `src/infra/domain/bootstrap-templates.ts` generates TOC-style AGENTS.md for new `.maestro/`. Encyclopedia content moves into a `docs/maestro/` skeleton the template also generates.
6. **Skill/binary drift detection** — at `setup --check` and on first skill invocation, compare CLI verbs referenced in `skills/bundled/maestro-*/SKILL.md` against the local binary's `--help` surface. Report drift loudly.
7. **Skill bundle update** — `skills/bundled/maestro-setup/SKILL.md` documents the self-test, `--check`, host-runtime hook install, TOC enforcement, and drift detection. Run `bun run sync:bundled-skills`.

### GOAL
```
Phase B complete per .maestro/plans/harness-pivot-completion.md: maestro setup --check works and exits 0 on this repo, setup self-test orchestrator + audit + host-runtime hook installer + TOC size-budget enforcer (160 lines) + bootstrap-template TOC rewrite + skill-binary drift detector all exist with unit tests, skills/bundled/maestro-setup/SKILL.md documents all new behavior and bun run sync:bundled-skills was run, bun test passes, bun run check:boundaries passes, bun run check:bundled-skills passes, wc -l of the bootstrap-template-generated AGENTS.md is ≤100; all changes committed on feat/harness-pivot
```

### Files changed (expected)
- `src/features/setup/commands/setup.command.ts` (extend with `--check` flag)
- `src/features/setup/usecases/run-self-test.ts` (create)
- `src/features/setup/usecases/audit-install.ts` (create)
- `src/features/setup/usecases/detect-host-runtime.ts` (create)
- `src/features/setup/usecases/check-skill-binary-parity.ts` (create)
- `src/features/setup/usecases/install-runtime-hooks.ts` (create)
- `src/features/setup/usecases/enforce-toc-budget.ts` (create)
- `src/infra/domain/bootstrap-templates.ts` (TOC rewrite)
- `skills/bundled/maestro-setup/SKILL.md` (edit)
- `src/infra/domain/bundled-skill-templates.ts` (auto-regenerated by `bun run sync:bundled-skills`)
- `docs/setup-self-test.md` (create)
- `docs/host-runtime-hooks.md` (create)
- `tests/unit/features/setup/*.test.ts` (create per usecase)

### Commit cadence
4 commits:
- `feat(setup): add post-install self-test`
- `feat(setup): add --check audit mode and skill-binary drift detection`
- `feat(setup): host-runtime detection and SessionStart/Exit hook installation`
- `refactor(init): TOC size-budget enforcement + bootstrap template rewrite + skill bundle update`

---

## Phase C — Edge-case hardening (5 cases)

### What
Five named failure modes get deterministic detection + agent-facing surface + regression test + docs entry.

### The five cases

1. **Session anchor staleness** — `session-start` evidence references a commit no longer reachable. Detect at `task introspect` via `git cat-file -e <sha>^{commit}`. Surface: `task introspect` shows `anchor: stale (commit <sha> not reachable)`. Recovery hint: re-run `session start` to anchor at HEAD.
2. **Tool-call loop detection** — same evidence `kind` with identical payload hash recorded ≥3 times consecutively with no intervening verdict change. Surface: `task introspect` adds `loopWarning: { kind, payloadHash, count }`. Recovery hint: review last verdict reason, change approach, or run `ralph review --stuck-threshold 1`.
3. **ProofMap holes at verdict time** — `verdict request` reason field enumerates every `Spec.acceptance_criteria` with zero covering evidence rows.
4. **Cost-budget mid-iteration BLOCK message** — `checkCostBudget` BLOCK names the exhausted limit, current usage, and the next verb (`maestro task budget --task <id>`; human approval to raise via `policies/risk.yaml`).
5. **Skill/binary drift at runtime** — if a bundled skill instructs a verb the local binary does not expose, the failure surfaces as `Skill expects "<verb>"; binary v<n> does not have it. Run "maestro update" or downgrade the skill bundle.` instead of `command not found`. Reuses Phase B parity check.

### GOAL
```
Phase C complete per .maestro/plans/harness-pivot-completion.md: 5 regression tests at tests/e2e/edge-cases/{anchor-staleness,tool-call-loop,proofmap-holes,cost-budget-message,skill-binary-drift}.test.ts exist and pass, docs/edge-cases.md exists with 5 sections (one per case) each naming trigger + detection rule + agent-facing surface + recovery verb, task introspect output includes anchor and loopWarning fields when applicable, verdict request reason field lists uncovered acceptance criteria, cost-budget BLOCK message includes exhausted limit and next verb, skill-binary drift produces a named error, skills/bundled/maestro-verify/SKILL.md notes the new verdict reason surfaces and skills/bundled/maestro-task/SKILL.md notes the new introspect fields, bun run sync:bundled-skills was run, bun test + bun run check:bundled-skills pass; all changes committed on feat/harness-pivot
```

### Files changed (expected)
- `tests/e2e/edge-cases/anchor-staleness.test.ts` (create)
- `tests/e2e/edge-cases/tool-call-loop.test.ts` (create)
- `tests/e2e/edge-cases/proofmap-holes.test.ts` (create)
- `tests/e2e/edge-cases/cost-budget-message.test.ts` (create)
- `tests/e2e/edge-cases/skill-binary-drift.test.ts` (create)
- `docs/edge-cases.md` (create)
- `src/features/task/usecases/introspect.ts` (extend with anchor + loop checks)
- `src/features/verdict/usecases/request-verdict.ts` (enumerate uncovered criteria in reason)
- `src/features/task/usecases/check-cost-budget.ts` (richer BLOCK message)
- `src/features/setup/usecases/check-skill-binary-parity.ts` (runtime error path reuses parity-check from Phase B)
- `skills/bundled/maestro-verify/SKILL.md` (edit)
- `skills/bundled/maestro-task/SKILL.md` (edit)
- `src/infra/domain/bundled-skill-templates.ts` (auto-regenerated)

### Commit cadence
6 commits: 5 case commits + 1 `docs+skills` consolidating commit.

---

## Phase D — AGENTS.md and CLAUDE.md slim-down

### What
Repo-root `AGENTS.md` from 458 → ≤140 lines. `CLAUDE.md` from 104 → ≤60 lines. Displaced verb-reference content lands in `docs/cli-reference.md`.

### Approach
1. Keep `WHERE TO LOOK` table, compressed (one sentence per row).
2. Move CLI VERBS sections (~350 lines) into `docs/cli-reference.md`.
3. Compress `CODE STYLE`, `CONVENTIONS`, `ANTI-PATTERNS` to bullets pointing into `docs/`.
4. Domain-primitive narratives become one-line pointers (Evidence → `docs/witness-levels.md`, Risk → `docs/risk-class-derivation.md`, etc.).
5. `CLAUDE.md` keeps the project-level "Maestro CLI" block; GitNexus block moves to `docs/gitnexus-usage.md` with a one-line link in `CLAUDE.md`.

### GOAL
```
Phase D complete per .maestro/plans/harness-pivot-completion.md: wc -l AGENTS.md ≤140 and ≤60 for CLAUDE.md, docs/cli-reference.md exists absorbing the displaced verb sections, docs/gitnexus-usage.md exists absorbing the displaced GitNexus block, every removed feature/primitive narrative has a one-line pointer to the canonical doc, bun run check:boundaries + check:skills + check:bundled-skills + build + test all pass; committed on feat/harness-pivot
```

### Files changed
- `AGENTS.md` (rewrite, ≤140 lines)
- `CLAUDE.md` (rewrite, ≤60 lines)
- `docs/cli-reference.md` (create, ~300 lines)
- `docs/gitnexus-usage.md` (create from displaced GitNexus block)

### Commit
- `docs: slim AGENTS.md to TOC (≤140 lines), displace CLI verbs to docs/cli-reference.md`
- `docs: slim CLAUDE.md, displace GitNexus block to docs/gitnexus-usage.md`

---

## Phase E — README + CLAUDE.md harness positioning

### What
README's first ~60 lines lead with the harness framing. Conductor metaphor stays as the control-flavor description in paragraph 2 or 3. `CLAUDE.md` quick-reference gains a pointer to `docs/harness-positioning.md`. `maestro-setup` skill description gains harness framing.

### Pre-specified copy
Opening paragraph contains the phrase `long-running agent harness`. Paragraph 2 or 3 retains the existing `conductor for multi-agent software engineering` phrasing. `CLAUDE.md` adds a line within the first 20 lines: `Harness positioning: see docs/harness-positioning.md`. `skills/bundled/maestro-setup/SKILL.md` description leads with `Set up a repository as a long-running agent harness…` (replaces `Set up a repository for Maestro-guided, human-in-the-loop agent work`).

### GOAL
```
Phase E complete per .maestro/plans/harness-pivot-completion.md: README.md first paragraph contains "long-running agent harness", paragraph 2 or 3 contains "conductor", README size has not grown (wc -l no larger than current), CLAUDE.md within first 20 lines contains "docs/harness-positioning.md", skills/bundled/maestro-setup/SKILL.md description starts with "Set up a repository as a long-running agent harness", bun run sync:bundled-skills was run, bun run check:bundled-skills passes; committed on feat/harness-pivot
```

### Files changed
- `README.md` (opening section only, lines 1–60)
- `CLAUDE.md` (one-line addition)
- `skills/bundled/maestro-setup/SKILL.md` (description field)
- `src/infra/domain/bundled-skill-templates.ts` (auto-regenerated)

### Commit
- `docs: lead README with harness framing; add positioning pointer to CLAUDE.md; update maestro-setup skill description`

---

## Phase F — DevObservabilityPort + `maestro task observe`

### What
Dev-time per-worktree observability for the agent: query metrics (Prometheus HTTP) and tail logs (file). Separate from `runtime check`, which gates deploys.

### Approach
1. Port: `src/features/runtime/ports/dev-observability.port.ts` with `queryMetric(promql, baseUrl?)` and `tailLogs(filter, lines?)`.
2. Adapter: `dev-prometheus.adapter.ts` (reuses existing HTTP pattern; no new deps).
3. Adapter: `log-tail.adapter.ts` (reads `MAESTRO_DEV_LOG_FILE` or `--log-file`).
4. Command: `src/features/task/commands/task-observe.command.ts` with `metrics <promql>` and `logs [--lines N] [--log-file <path>]` subcommands. `--record` flag writes a `dev-observation` evidence row.
5. Register in `src/index.ts` + `src/features/task/index.ts`.
6. Skill updates: `maestro-task/SKILL.md` adds `task observe` section; `maestro-verify/SKILL.md` notes dev-time observability distinct from `runtime check`.

Per CLAUDE.md: run `gitnexus_impact` on each touched symbol before editing.

### GOAL
```
Phase F complete per .maestro/plans/harness-pivot-completion.md: DevObservabilityPort + dev-prometheus.adapter.ts + log-tail.adapter.ts + task-observe.command.ts all exist, maestro task observe metrics 'up' exits 0 when MAESTRO_PROMETHEUS_URL is set, maestro task observe logs --log-file <tmp> --lines 1 exits 0 against a written tmp file, docs/dev-observability.md exists, skills/bundled/maestro-task/SKILL.md has a task observe section and skills/bundled/maestro-verify/SKILL.md notes dev-time observability is distinct from runtime check, bun run sync:bundled-skills was run, bun test + bun run check:boundaries + check:bundled-skills pass; committed on feat/harness-pivot
```

### Files changed
- `src/features/runtime/ports/dev-observability.port.ts` (create)
- `src/features/runtime/adapters/dev-prometheus.adapter.ts` (create)
- `src/features/runtime/adapters/log-tail.adapter.ts` (create)
- `src/features/task/commands/task-observe.command.ts` (create)
- `src/index.ts` (register `task observe`)
- `src/features/task/index.ts` (export)
- `docs/dev-observability.md` (create)
- `skills/bundled/maestro-task/SKILL.md` (edit)
- `skills/bundled/maestro-verify/SKILL.md` (edit)
- `src/infra/domain/bundled-skill-templates.ts` (auto-regenerated)
- `tests/unit/features/runtime/dev-observability/*.test.ts` (create)

### Commit
- `feat(runtime): add DevObservabilityPort + adapters + maestro task observe verb + skill bundle updates`

---

## Phase G — Tool-result distillation / context-compaction (deferred, no goal)

Not in current scope. Maps to OpenAI's "context is a scarce resource" principle but the design is premature without operational signal from F. The plan deliberately does not propose a design or a goal for G; reach for it only after F lands and a concrete pain shape emerges.

---

## External trigger pattern (unchanged)

Maestro is passive. No scheduler, no daemon, no watcher in the binary. Continuous quality scans are *verbs* host runtimes call. Three canonical shapes (full recipes in `docs/schedule-recipes.md`):

1. **GitHub Actions cron** — nightly `maestro gc doc-gardening --json` sweep; tracking issue on stale refs.
2. **Claude Code session hooks** — `SessionStart` / `SessionEnd` in `.claude/settings.json` call `maestro session start/exit "$CLAUDE_TASK_ID"`. (Phase B sub-goal 2 auto-installs these.)
3. **Agent skill prompt** — a local skill instructs `maestro task verify --task <id>` after substantive edit batches. Timing contextual; skill decides, not a scheduler.

Anything beyond these is custom to the host runtime. Maestro provides verbs.

---

## What this plan deliberately does NOT do

- No scheduler or daemon inside maestro (hard constraint).
- No new agent abstraction layer.
- No LLM client added to the CLI.
- No new npm/bun package dependencies (Prometheus HTTP reuses existing pattern).
- No Loki / LogQL adapter (deferred).
- No Phase G implementation.
- No changes to MCP server or handoff launch paths.
- No README slim-down past the opening positioning section.
- No edge-case work beyond the 5 chosen for Phase C (worktree orphans, AI/verifier divergence, stale autopilot soak, lock contention are deferred — pick up after C if needed).

---

## Skill change inventory (now in-scope of each phase's goal)

| Phase | Skill | Change |
|---|---|---|
| B | `maestro-setup/SKILL.md` | document self-test, `--check`, drift detection, host-runtime hooks, TOC enforcement |
| C | `maestro-verify/SKILL.md` | new surfaces in `verdict request` reason |
| C | `maestro-task/SKILL.md` | new `task introspect` fields (anchor, loopWarning) |
| E | `maestro-setup/SKILL.md` | description leads with harness framing |
| F | `maestro-task/SKILL.md` | new `task observe` section |
| F | `maestro-verify/SKILL.md` | dev-time observability distinct from `runtime check` |

Each phase's GOAL ends with `bun run sync:bundled-skills` having been run and `bun run check:bundled-skills` passing.

---

## Phase summary

| Phase | Description | Goal-runnable | Depends on |
|---|---|---|---|
| A (tonight) | `docs/harness-positioning.md` | yes | — |
| B | Setup + init hardening (incl. `maestro-setup` skill) | yes | A |
| C | Edge-case hardening (5 cases, incl. `maestro-verify` + `maestro-task` skill notes) | yes | A, B |
| D | `AGENTS.md` + `CLAUDE.md` slim-down | yes | A, B |
| E | README + `CLAUDE.md` positioning + `maestro-setup` skill description | yes | A |
| F | `DevObservabilityPort` + `task observe` + `maestro-task` + `maestro-verify` skill notes | yes | A |
| G | Tool-result distillation | deferred (no design) | F |

---

## Tonight goal (copy-paste)

```
/goal docs/harness-positioning.md exists, ≤250 lines, has 5 principle sections (Context-as-scarce-resource, Per-worktree-observability, Isolated-worktrees, Continuous-quality-grading, Building-blocks) each with at least one src/ or docs/ pointer, an External-triggers subsection citing docs/schedule-recipes.md, a What-this-is-not subsection naming "no scheduler, no daemon, no LLM client, no background process inside maestro", a reference to docs/architecture-lints.md no-runner-inversion rule; bun run check:boundaries passes; committed on feat/harness-pivot
```

Subsequent goals (B → F) are in each phase's `GOAL` block. Copy-paste in order.
