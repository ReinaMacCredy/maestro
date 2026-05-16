# Maestro v2: Master Plan

> Status: locked architecture, pending implementation. This document is the durable spine of the v2 rebuild; the 13 ADRs in `docs/adr/0001..0013` carry the per-decision rationale.

## 1. Identity

**Taglines (both adopted):**

- *Humans steer. Agents execute. Maestro is the substrate.*
- *The harness OS for agent-generated codebases.*

Maestro v2 is the harness OS that LLM coding agents (Claude Code, Codex, Cursor) operate against. The agent is the worker; maestro is the durable, local-first substrate that gives it primitives, lifecycle, evidence, observability, and golden rules. Maestro v2 stays passive: no daemon, no scheduler, no LLM call inside maestro. "Automatic" always means "computed from the result of a verb the agent just called."

The brand stays `maestro` (CLI binary) / `conductor` (informal). The repositioning is to the harness-OS layer; the name does not change.

## 2. Primitives

Eight knowledge primitives (directories the agent reads) and four execution primitives (the agent's verb-shaped surface). See `CONTEXT.md` for the canonical glossary.

### Knowledge primitives

| Primitive | One-line role | Source of truth |
|---|---|---|
| `design-docs` | Human-written strategic / architectural documents | `docs/design-docs/` |
| `exec-plans` | First-class decomposition artifacts for multi-PR work | `docs/exec-plans/active/` (move to `completed/`) |
| `product-specs` | Feature specifications with acceptance criteria and non-goals | `.maestro/specs/<slug>.md` (see §6 layout split) |
| `references` | LLM-targeted condensations of upstream library docs | `docs/references/` |
| `generated` | Auto-generated reference docs (wire-contracts, schema dumps) | `docs/generated/` |
| `architecture` | Mechanically-enforced layering and dependency rules | `docs/architecture.yaml` + lints |
| `quality-score` | Per-domain grade tracking gaps over time | `.maestro/quality-score.json`, updated by `gc grade` |
| `principles` | Named golden rules with scan command + fix recipe | `docs/principles/*.md`, enforced by `gc slop-cleanup` |

### Execution primitives

| Primitive | One-line role |
|---|---|
| `worktree` | Isolated execution environment per task. Per-worktree `.maestro/runs/`, scoped telemetry and logs. |
| `loop` | Default execution mode after a task is claimed. Try → verify → evidence → iterate until PASS or stuck-threshold. |
| `task` | A unit of PR-shaped work with a lifecycle. Strictly 1:1 task↔PR (ADR-0006). |
| `handoff` | Artifact emitted at cross-session state transitions, carrying context for the next agent session. |

`task` and `handoff` are retained from v1 unchanged in shape; they are the well-formed execution primitives today (ADR-0002). Everything else is renamed or absorbed.

### Vocabulary disappearances (no aliases)

`mission` → `exec-plan` · `spec` → `product-spec` · `intake` / `brainstorm` → folded into `design-docs` reading + `product-spec` authoring · `session` / `notes` → folded into `handoff` (session-detect absorbed into `worktree` per ADR-0015).

### Conductor-era features absorbed (ADR-0015)

Three v1 feature directories disappear because their job is now done by knowledge primitives the agent reads at session start:

- `memory` + `memory-ratchet` + `agent` → corrections live in `docs/principles/<rule>.md`; learnings live in `docs/design-docs/learnings/`; agent-prompt synthesis collapses into AGENTS.md. No correction store, no learning store, no recall verb. Matches "what Codex can't see doesn't exist".
- `graph` (project-to-project edges) → `docs/references/project-graph.yaml`. No link/context verbs.
- `session` → notes folded into handoff; detect-portion folded into the `worktree` primitive (worktree metadata records agent identity at claim time).

## 3. Lifecycles

Two state machines, hybrid transitions (ADR-0003, ADR-0004).

### Task lifecycle

```
draft → claimed → doing ↔ verifying ↔ blocked → ready → shipped
                                                              ↘ (any state) → abandoned
```

- **Agent enters** check states manually: `maestro task claim`, `maestro task verify`, `maestro task block`.
- **Harness auto-exits** based on the result of the verb just called:
  - `verifying → doing` on FAIL verdict (the Ralph Wiggum Loop falls out)
  - `verifying → ready` on PASS
  - `ready → shipped` when merge is detected at the next verb call
- One task = one PR (ADR-0006). Multi-PR work promotes to an exec-plan.

### Exec-plan lifecycle

```
intake → specified → planned → in-progress → completed
                                                       ↘ (any state) → cancelled
```

- Auto-completes when every child task is in `shipped` or `abandoned` (ADR-0011). Completion record captures the breakdown ("4 shipped, 1 abandoned"). No manual `plan complete` verb.

### Evidence on every transition

Every lifecycle transition writes one `kind=transition` evidence row recording from-state, to-state, trigger verb, verdict, and witness level (ADR-0009). Agents and adapters may still write ad-hoc evidence outside transitions. Witness levels L0–L7 carry over unchanged.

## 4. Verbs (agent-facing surface)

The five-skill bundle is the agent's whole surface (ADR-0012):

| Skill | Purpose | Primary verbs |
|---|---|---|
| `maestro-setup` | One-time per-project init + v2 migration + QA install | `maestro setup`, `maestro setup --migrate-v2`, `maestro setup --check`, `maestro setup --qa` (absorbed v1 maestro-qa, ADR-0015) |
| `maestro-design` | Structured spec authoring driven by the **grill protocol** (ADR-0016): Q&A walks acceptance, non-goals, risk class, mode, work-type, and challenges spec language against CONTEXT.md + ADRs | `maestro spec new`, `maestro spec validate` (work-type classification absorbed from v1 maestro-classify, ADR-0015) |
| `maestro-plan` | Heavy-mode workflow; `decompose` step runs the **grill protocol** (ADR-0016) against the spec, CONTEXT.md, and the architecture lint set before emitting the task batch | `maestro plan from-spec <path>`, `maestro plan decompose`, `maestro plan show` |
| `maestro-task` | Light-mode workflow | `maestro task from-spec <path>`, `task claim` (alias `claim`), `task block` (alias `block`), `task ship` (alias `ship`), `task abandon` (alias `abandon`) |
| `maestro-verify` | Verify subroutine, called from task/plan | `maestro task verify` / alias `verify` (PASS / FAIL / HUMAN / BLOCK); `maestro principle promote <correction-id>` to materialize a principle markdown from a FAIL evidence row (ADR-0015) |

**Verb-naming convention (locked, ADR-0014):** git-style `<noun> <verb>` is the primary form for entity-shaped actions (`task claim`, `plan decompose`, `spec validate`); harness-shaped actions stay single-verb (`setup`, `recover`, `bundle`). Short hot-path aliases are first-class: `claim` → `task claim`, `verify` → `task verify`, `ship` → `task ship`, `block` → `task block`, `abandon` → `task abandon`. Aliases only exist for the curated task-hot-path list; plan/spec/harness verbs use the noun-verb form exclusively.

**`maestro plan` namespace is shared (advisor flag).** Two conceptual groups live under one noun: v1's plan-check / cost-budget (`plan check`, `plan budget`, kept as §11 non-goals) and v2's exec-plan workflow (`plan from-spec`, `plan decompose`, `plan show`). The shared namespace is intentional (both groups operate on "the plan for this work") but subcommands must disambiguate cleanly. v2 will not introduce a new plan subcommand that collides with `check` or `budget`.

**Observability and handoff are baked in, not separate skills:**
- `observe` / `see` collapse into `maestro-verify` on FAIL (auto-emits log/metric/trace evidence pointing into the worktree-scoped LogQL/PromQL/TraceQL queries).
- `handoff` emission collapses into `maestro-task` at session boundaries (the existing handoff machinery stays; only the skill wrapper is absorbed).

Today's 10-skill bundle collapses to these five. The rest become either documentation or absorbed (full mapping in ADR-0015): `maestro-brainstorm`+`maestro-intake` → `maestro-design`; `maestro-classify` → `maestro-design`; `maestro-mission` → `maestro-plan`; `maestro-handoff` → `maestro-task`; `maestro-qa` → `maestro-setup --qa`. The 7 colon-namespaced `skills/built-in/maestro:*` skills migrate piecemeal: `agent-base` → task startup; `mission-planning` → plan; `scrutiny-validator` + `user-testing-validator` → verify; `conduct`, `blueprint`, `define-mission-skills` deleted. `skills/built-in/` disappears.

## 5. Ports + default adapters

Three ports (ADR-0005) bind maestro to the three reference diagrams from the article:

| Port | Surface | Default adapter |
|---|---|---|
| `ObservabilityPort` | LogQL / PromQL / TraceQL queries, scoped per worktree | Vector + VictoriaLogs + VictoriaMetrics + VictoriaTraces (default; scope deferred, see §10 open items) |
| `ArchitectureRules` | YAML schema declaring layered domains + forward-only dependencies | Types → Config → Repo → Service → Runtime → UI, with `Providers` as the only cross-cutting boundary |
| `PrinciplesSchema` | Markdown format: rule + rationale + scan command + fix recipe | Default principle pack consumed by `gc slop-cleanup` |

Consumers can override adapters per project. Maestro's own repo dogfoods the defaults.

## 6. File layout

```
maestro/
├── CONTEXT.md                       # canonical glossary
├── AGENTS.md                        # ~100-line table of contents
├── CLAUDE.md                        # → AGENTS.md
├── docs/
│   ├── adr/0001..0013-*.md          # locked architectural decisions
│   ├── v2-master-plan.md            # this file
│   ├── design-docs/                 # human-written strategic docs (read-only to agents)
│   │   └── learnings/               # agent-writable carve-out (ADR-0015): durable learnings + migrated v1 memory
│   ├── exec-plans/
│   │   ├── active/                  # in-progress exec-plans
│   │   └── completed/               # archived
│   ├── references/                  # opentui-llms.txt, etc.
│   ├── generated/                   # wire-contract snapshots
│   ├── architecture.yaml            # layered rules (lint source of truth)
│   ├── principles/                  # named golden rules
│   └── harness-positioning.md       # mapping article → primitives
├── .maestro/
│   ├── specs/<slug>.md              # product-specs with YAML frontmatter (ADR-0010)
│   ├── tasks/tasks.jsonl            # task ledger
│   ├── plans/<id>/                  # exec-plan state
│   ├── evidence/                    # evidence rows (transition + ad-hoc)
│   ├── runs/                        # per-worktree runtime artifacts
│   ├── quality-score.json
│   └── handoffs/
└── src/                             # implementation (article-layered)
```

**Resolved layout split:** product-specs live at `.maestro/specs/` (versioned with the harness state, grep-able by `maestro spec` verbs). All other knowledge primitives (`design-docs`, `exec-plans`, `references`, `generated`, `architecture`, `principles`) live under `docs/` because they are documents humans read and edit, not raw harness state. ADR-0010 captures the product-spec location decision.

**Both directories are maestro-managed; the split is about visibility, not ownership.** Maestro CLI reads and writes both trees. The distinction:

- `docs/` is the **human-visible, maestro-managed** surface. Knowledge primitives a human edits in their editor; ADRs a human authors directly; lint-enforced; read by every skill at session start. GitHub renders it; IDEs index it; the file tree shows it by default.
- `.maestro/` is the **harness-internal** surface. State the agent and the harness own: live specs, task ledger, plan state, evidence rows, runtime scratch, handoffs, quality-score. Written almost exclusively by maestro verbs; humans inspect via `maestro` commands rather than editing files directly.

Mental model: `docs/` is where decisions live; `.maestro/` is where work-in-flight lives. Both are maestro's substrate; only one is meant to be edited by hand.

## 7. Document-driven workflow

Both light and heavy mode start with a spec markdown file (ADR-0010). There is no `maestro task new "<title>"` shortcut and no interactive intake verb. Specs live at `.maestro/specs/<slug>.md` with YAML frontmatter:

```yaml
---
slug: improve-handoff-pickup-error
acceptance_criteria:
  - "Pickup of a missing handoff returns a recoverable error and exit code 2"
  - "Tests cover both missing-file and corrupt-frontmatter cases"
non_goals:
  - "Migrating existing handoffs"
risk_class: medium
mode: light    # light | heavy
work_type: change-request    # new-spec | spec-slice | change-request | initiative | maintenance | harness-improvement (ADR-0015)
---
# Improve handoff pickup error path
<freeform body: context, design rationale, decisions>
```

Then `maestro task from-spec .maestro/specs/<slug>.md` (light) or `maestro plan from-spec <path>` (heavy) ingests the spec and creates the task or plan entity. AGENTS.md explicitly points agents at `.maestro/specs/`.

## 8. Operating modes

**Light path** (one PR, no exec-plan):

```
spec → task from-spec → claim → loop (doing ↔ verifying) → ready → shipped → handoff
```

Worktree creation is opt-in (`--worktree`). Default stays in the current checkout (ADR-0008).

**Heavy path** (multi-PR feature):

```
spec → plan from-spec → plan decompose → N child tasks (each runs the light path internally) → plan auto-completes → quality-score update
```

Worktree creation is automatic per child task at `task claim` time, because heavy mode implies parallel/concurrent work where the article's per-worktree isolation pays off.

Mode is chosen by the agent at spec authoring time (frontmatter `mode:`). The maestro-design skill walks the agent through the choice when authoring.

## 9. Migration (big-bang 2.0)

Maestro ships v2 as a single 2.0 major release (ADR-0007). Old verbs are removed. No aliasing, no deprecation warnings, no parallel binary. `maestro setup --migrate-v2` is the single migration path.

What `setup --migrate-v2` does on a v1 repo:

1. Write a backup tarball to `.maestro/backups/pre-v2-<timestamp>.tar.gz` covering the entire `.maestro/` directory. Restore is manual (`tar -xzf` over `.maestro/`); v2 does not ship an automated restore verb.
2. Rename `.maestro/missions/` → `.maestro/plans/`.
3. Rewrite mission frontmatter to exec-plan frontmatter (mechanical key map).
4. Move stale `intake/` / `brainstorm/` artifacts into `docs/design-docs/legacy/` (preserve, don't delete).
5. Convert any `session/notes/` artifacts into handoffs.
6. Rewrite `tasks.jsonl` state column per the table below.
7. Rewrite mission status fields per the table below.
8. Migrate v1 `.maestro/memory/` entries to knowledge primitives per ADR-0015: corrections → `docs/principles/legacy/<id>.md`, learnings → `docs/design-docs/learnings/<id>.md`. Delete `.maestro/memory/` and `.maestro/memory/ratchet/` after migration.
9. Migrate v1 `.maestro/graph.json` (if present) to `docs/references/project-graph.yaml`. Delete the source file.
10. Strip the `session` feature directory tree; preserve last-known session-detect output as `.maestro/runs/<id>/agent.json` for any in-flight worktree so identity survives the flip.
11. Normalize `.maestro/qa/` sub-skill paths to v2 layout (directory itself is preserved if present).
12. Stamp `.maestro/v2-migrated.flag` with timestamp, source v1 version, and backup tarball path.
13. Idempotent on re-run: the flag short-circuits steps 2–11; the backup step still runs each invocation so the user can capture a fresh restore point.

### Artifact coverage table (Phase 5 audit)

Every `.maestro/` artifact type found in a real project after Phase 5, classified against the migration steps above. Artifacts not in the numbered steps default to **skip-silently / preserved** unless a v1 contract exists.

| Artifact | Classification | Notes |
|---|---|---|
| `.maestro/missions/` | **in-mapping** | Step 2: renamed to `.maestro/plans/` |
| `.maestro/memory/` | **in-mapping** | Step 8: corrections → `docs/principles/legacy/`, learnings → `docs/design-docs/learnings/`, then deleted |
| `.maestro/graph.json` | **in-mapping** | Step 9: migrated to `docs/references/project-graph.yaml`, then deleted |
| `.maestro/session/` | **in-mapping** | Step 10: notes → handoffs; session-detect → `runs/<id>/agent.json` |
| `.maestro/sessions/` | **in-mapping** | Plural form of `session/` (same treatment as step 10) |
| `.maestro/intake/` | **in-mapping** | Step 4: moved to `docs/design-docs/legacy/` |
| `.maestro/brainstorm/` | **in-mapping** | Step 4: moved to `docs/design-docs/legacy/` |
| `.maestro/qa/` | **in-mapping** | Step 11: paths normalized to v2 layout |
| `.maestro/tasks/tasks.jsonl` | **in-mapping** | Step 6: state column rewritten per task state table |
| `.maestro/MAESTRO.md` | **in-mapping** | Deleted at Phase 5 sunset; it was a v1 operational compass with v1-only verbs. Users with this file: the migration deletes it and notes the deletion in the migration log. |
| `.maestro/wisdom/` | **skip-silently / preserved** | User-owned reference material; no v1 contract. The harness does not read or write this directory. Preserved as-is. |
| `.maestro/doctrine/` | **skip-silently / preserved** | User-owned strategic docs. No v1 contract. Preserved as-is. |
| `.maestro/tracks/` | **skip-silently / preserved** | User-owned planning artifacts. No v1 contract. Preserved as-is. |
| `.maestro/tracks.md` | **skip-silently / preserved** | User-authored index. No v1 contract. Preserved as-is. |
| `.maestro/launches/` | **skip-silently / preserved** | User-owned launch records (possibly handoff archives). No v1 migration contract. Preserved as-is. |
| `.maestro/bootstrap/` | **skip-silently / preserved** | Init-time scaffolding artifacts. No v1 contract. Preserved as-is. |
| `.maestro/drafts/` | **skip-silently / preserved** | In-progress long-form docs. No v1 contract. Preserved as-is. |
| `.maestro/archive/` | **skip-silently / preserved** | Historical reference material. No v1 contract. Preserved as-is. |
| `.maestro/context/` | **skip-silently / preserved** | Durable operator guidance. No v1 migration needed; compatible with v2 as-is. Preserved as-is. |
| `.maestro/principles.jsonl` | **skip-silently / preserved** | Append-only principles log. v2 reads this format unchanged. Preserved as-is. |
| `.maestro/feedback.jsonl` | **skip-silently / preserved** | Agent feedback log. No v1 migration contract. Preserved as-is. |
| `.maestro/retrieval-index.json` | **skip-silently / preserved** | Generated/derived index. Regenerated by the harness on next run. Preserved as-is (or deleted and regenerated). |
| `.maestro/notepad.md` | **skip-silently / preserved** | User scratch space. No v1 contract. Preserved as-is. |
| `.maestro/tmp/` | **skip-silently / preserved** | Ephemeral scratch dir. Safe to delete or preserve; no v1 contract. |
| `.maestro/handoffs/` | **skip-silently / preserved** | v1 handoff envelopes are compatible with v2 handoff read path. Preserved as-is. |
| `.maestro/plans/` | **skip-silently / preserved** | v2 plans dir. Created by step 2 or by `maestro plan from-spec`. Preserved as-is post-migration. |
| `.maestro/specs/` | **skip-silently / preserved** | Per-mission product-spec files. v2 reads `.maestro/specs/<slug>.md` natively. Preserved as-is. |
| `.maestro/contracts/` | **skip-silently / preserved** | Versioned contract snapshots. v2 reads these unchanged. Preserved as-is. |
| `.maestro/policies/` | **skip-silently / preserved** | Policy YAML files. v2 reads these unchanged. Preserved as-is. |
| `.maestro/docs/` | **skip-silently / preserved** | Canonical doc templates (HARNESS.md, FEATURE_INTAKE.md, VALIDATION_LADDER.md). v2 setup copies these; preserved as-is. |
| `.maestro/skills/` | **skip-silently / preserved** | Project-local skill overrides. v2 runtime lookup prefers these first. Preserved as-is. |
| `.maestro/verdicts/` | **skip-silently / preserved** | Gitignored derived state. Preserved as-is (or regenerated). |
| `.maestro/runs/` | **skip-silently / preserved** | Per-task observability and run-state. Gitignored; per-machine. Preserved as-is. |
| `.maestro/evidence/` | **skip-silently / preserved** | Per-machine evidence rows. Gitignored. Preserved as-is. |
| `.maestro/settings.json` | **skip-silently / preserved** | Project-local Maestro settings. Preserved as-is. |
| `.maestro/config.yaml` | **skip-silently / preserved** | Project-level Maestro config. Preserved as-is. |

### v1 → v2 task state mapping

v1 task surface today is the trio `pending` / `in_progress` / `completed`, with legacy `open` / `blocked` / `deferred` / `closed` normalized by `src/features/task/domain/task-state.ts`. **The migration reads raw legacy state from `tasks.jsonl` and bypasses `normalizeStoredTaskStatus` so the legacy rows below can map directly to v2 states** (otherwise `deferred` would normalize to `pending` and lose the abandonment signal). After v2 fills in the normalized + legacy rows below:

| v1 state (after normalization) | Condition | v2 state |
|---|---|---|
| `pending` | no `assignee`, no `blockedBy` | `draft` |
| `pending` | no `assignee`, has unresolved `blockedBy` | `blocked` |
| `pending` | has `assignee` (rare; mostly legacy data) | `claimed` |
| `in_progress` | any | `doing` |
| `completed` | task has merged PR recorded (`pr.mergedAt` non-null) | `shipped` |
| `completed` | no merged PR recorded | `abandoned` |
| legacy `open` | any | `draft` (via normalization) |
| legacy `blocked` | any | `blocked` (via normalization, then mapped) |
| legacy `deferred` | any | `abandoned` (no v2 deferred state; conservative pick) |
| legacy `closed` | any | `shipped` if `pr.mergedAt` present, else `abandoned` |

The `verifying` and `ready` v2 states have no v1 origin (v1 task surface does not track these); they exist for forward use only. A migration never produces a task in `verifying` or `ready`.

### v1 → v2 mission/exec-plan state mapping

v1 mission surface from `src/features/mission/domain/mission-state.ts` is `draft` / `approved` / `rejected` / `executing` / `paused` / `validating` / `completed` / `failed`. v2 exec-plan surface is `intake` / `specified` / `planned` / `in-progress` / `completed` / `cancelled`. Mapping:

| v1 mission state | v2 exec-plan state | Notes |
|---|---|---|
| `draft` | `intake` | acceptance criteria not yet locked |
| `approved` | `specified` | if mission has no decomposed features yet |
| `approved` | `planned` | if mission already has feature list (decomposed) |
| `executing` | `in-progress` | |
| `paused` | `in-progress` | v2 drops the pause concept; emit a transition evidence row noting "v1 paused" so the audit trail survives |
| `validating` | `in-progress` | v2 plan-level validating collapses into the per-task verify loop |
| `completed` | `completed` | direct |
| `rejected` | `cancelled` | |
| `failed` | `cancelled` | |

Milestone, Feature, and Assertion sub-states (also in `mission-state.ts`) are not exposed as separate entities in v2's exec-plan model; they migrate into the child task ledger or are dropped if unmapped. A future ADR can revisit if the loss matters in practice.

### Migration tests

Phase 1 must ship a fixture-based migration test suite at `tests/fixtures/v1-maestro/` that:

- runs `setup --migrate-v2` against a frozen v1 `.maestro/` snapshot;
- asserts every row in both mapping tables above;
- asserts the ADR-0015 deletions: `memory/` corrections land in `docs/principles/legacy/`, learnings land in `docs/design-docs/learnings/`, `.maestro/memory/` is removed; `.maestro/graph.json` lands in `docs/references/project-graph.yaml`; `session` directory tree is gone; in-flight worktrees have `agent.json` preserved;
- asserts the backup tarball restores byte-identical state when extracted over a fresh dir;
- asserts idempotency: second `--migrate-v2` call is a no-op except for the backup write.

## 10. Phasing on the greenfield v2 branch

v2 work lives on a long-lived `harness-os` branch off main (ADR-0013). v1 stays on main and may receive bug fixes during the rebuild window. When v2 is feature-complete, the branch merges to main as the 2.0 release. The merge IS the big-bang flip.

Estimated 6–7 months solo (revised after Phases 5/6/7 split off the original Phase 4 per ADR-0018, ADR-0019). Phases inside the branch (each phase ends at a working maestro-on-itself milestone):

### Phase 1: Spine + architecture lints (target: month 1)

- New primitive directories, CONTEXT.md, AGENTS.md rewrite.
- Two-lifecycle state machines implemented end-to-end.
- `maestro task from-spec` + `task claim` + `task verify` + `task block` + `task ship` + `task abandon` working with transition evidence.
- Hot-path aliases (`claim`, `verify`, `ship`, `block`, `abandon`) wired per ADR-0014.
- `maestro spec new` + `spec validate` working.
- **Grill protocol baked into `maestro-design`** (ADR-0016): the SKILL.md ships with the grill steps so spec authoring is interview-driven from day 1, challenging spec language against CONTEXT.md + ADRs.
- **Architecture lints + `ArchitectureRules` port + default layered adapter (moved from Phase 2 per advisor flag).** Lints are mechanical and pair naturally with the spine; landing them in Phase 1 means dogfooding from day 1 enforces layering as the spine settles.
- Maestro can run a light-mode task end-to-end on itself.

**Done criteria:** dogfooded on a real maestro change (e.g. fix a v1 bug), spec → task → ship, with transition evidence recorded, and `maestro task verify` runs the architecture lints as one of its checks.

### Phase 1.5: Principles + correction-recording bridge (target: ~weeks 5–6)

Inserted between Phase 1 and Phase 2 to close the correction-recording gap that ADR-0015's memory deletion creates. Without this bridge, Phase 1 dogfooding would run for ~4 weeks with no way to capture corrections; Phase 2's heavy plan/verify work would inherit that backlog.

- `PrinciplesSchema` port + default principle pack (the canonical golden rules: prefer shared utils, no YOLO data probing, etc.).
- `gc slop-cleanup` consumes the principle pack and reports violations + fix recipes.
- `maestro principle promote <correction-id>` verb (ADR-0015): materializes a principle markdown file at `docs/principles/<slug>.md` from a FAIL correction.
- Migrate any v1 `.maestro/memory/` corrections from the current main branch into `docs/principles/legacy/` as a smoke test of the migration path.

**Done criteria:** a real correction captured during Phase 1 dogfooding gets promoted to a principle markdown via the new verb, and `gc slop-cleanup` finds at least one violation in the maestro codebase and prints its fix recipe.

### Phase 2: Plan lifecycle + verify routing (target: month 2, reduced scope)

- Exec-plan lifecycle implemented end-to-end with auto-complete-on-terminal (ADR-0011).
- `maestro plan from-spec`, `plan decompose`, `plan show`.
- **Grill protocol extended to `maestro-plan`** (ADR-0016): the `decompose` step interviews against the spec, CONTEXT.md, and the architecture lint set before emitting the task batch.
- `maestro-verify` skill with PASS / FAIL / HUMAN / BLOCK routing and auto-transition wiring.

(Architecture lints moved to Phase 1; principles + gc moved to Phase 1.5; both removed from Phase 2 per advisor.)

**Done criteria:** a heavy-mode 3-task plan ships end-to-end on maestro itself, with auto-complete on terminal.

### Phase 3: Observability + setup (target: month 3)

- `ObservabilityPort` + Vector/Victoria default adapter (scope decided here; see open items).
- `maestro setup` + `setup --check` + `setup --migrate-v2`.
- Worktree-per-task for heavy mode wired (ADR-0008).
- Handoff emission baked into `maestro-task` at session boundaries.

**Done criteria:** a fresh consumer project bootstraps via `setup`, runs a spec → task → ship with observability evidence, and migrates a v1 fixture via `setup --migrate-v2`.

### Phase 4: Docs polish + skill-surface cleanup (target: month 4)

Light cleanup of the *agent-facing* surface (skills + docs). Heavy source-code cleanup is Phase 5 per ADR-0018.

- Documentation pass: all five bundled SKILL.md files finalized.
- CLI reference (`docs/cli-reference.md`) rewritten against v2 verbs only.
- `docs/harness-positioning.md` rewritten around v2 primitives.
- Docs root + setup templates v2 pass.
- Absorbed skill files deleted; `skills/built-in/maestro:*` colon-namespaced tier deleted.

**Done criteria:** PRs 36–40 landed (docs cli-reference rewrite, harness-positioning rewrite, docs+setup templates, SKILL.md finalize + delete absorbed skills, colon-tier deletion).

### Phase 5: v1 source-code sunset (target: month 5)

Per ADR-0018, v2-displaces rule: walk each `src/features/<x>/` and delete if v2 owns the verb; keep §11 non-goals.

- Delete v1 feature dirs that v2 owns: `task`, `spec`, `verify`, `setup`, `mission`, `intake`.
- Split `src/features/plan/`: delete exec-plan workflow code, keep plan-check + cost-budget.
- Delete ADR-0015 absorbed/dropped dirs: `memory`, `memory-ratchet`, `agent`, `graph`, `session`, `ralph`, `notes`, `inspect`, `state`.
- Resolve judgment-call dirs (`handoff`, `worktree`) at phase kickoff with an import-graph audit; port supporting machinery into `src/v2/` as needed.
- Delete v1 CLI verb registrations from `src/index.ts`.
- Delete v1 e2e tests for deleted features. Concrete list at kickoff includes `tests/e2e/memory-compiled-e2e.test.ts`, `notes-compiled-e2e.test.ts`, `session-compiled-e2e.test.ts`, `ratchet-compiled-e2e.test.ts`, `graph-compiled-e2e.test.ts`, `agent-loop-compiled-e2e.test.ts` (verify against the `src/features/` deletion list — anything covering a deleted feature goes with it).
- Clean up cross-feature imports referencing deleted dirs.
- **MCP server v2 pass** (revised §11 scope): rename `task_complete` → `task_ship`; delete `task_unblock`; replace `task_create` / `task_plan` with `task_from_spec`; add hot-path v2 MCP tools (`principle_promote`, `setup_check`, `setup_migrate_v2`); verify kept tools (`task_claim`, `task_block`, `task_get`, `task_list`, `evidence_*`, `verdict_*`, `policy_*`, `handoff_*`, `contract_*`) call v2 use cases. Grill-driven verbs (`spec new`, `plan from-spec`, `plan decompose`) stay CLI-only.
- Extend `.github/workflows/install-smoke.yml` and `release.yml` to exercise v2 hot-path verbs end-to-end after install — both currently only call `maestro --version` + `mission-control --render-check`, which passes vacuously against either v1 or v2. Add a scripted `setup check`, `spec new` (non-interactive `--from-file` mode), `task from-spec`, `claim`, `verify`, `ship` smoke so a broken v2 binary fails the install-smoke job instead of escaping to a release.
- README v2 sweep: after sunset, verify `README.md` and any other root-level docs reference v2 verbs only. This is a sweep, not a rewrite — Phase 4 PR 38 (`docs root + setup templates v2 pass`) owns the primary rewrite; this is the post-sunset verification that no v1 verb survived.
- §9 migration mapping coverage audit: maestro's own `.maestro/` (and a survey of any other v1 project state we have access to) contains dirs that may not yet be in §9's mapping table (`wisdom/`, `doctrine/`, `tracks/`, `launches/`, `bootstrap/`, `drafts/`, `archive/`, `MAESTRO.md`, `principles.jsonl`, `feedback.jsonl`, `retrieval-index.json`). At Phase 5 kickoff, walk each one and decide: in-mapping (add a §9 row), skip-silently (preserve as-is, no v1 contract), or error-out (migration refuses to run). Goal: no v1 user hits an artifact with no §9 row and a confusing migration outcome.
- AGENTS.md sweep: the `WHERE TO LOOK` table currently points at v1 paths Phase 5 deletes (`src/features/spec/`, `src/features/setup/`, `src/features/verify/`, `src/features/handoff/` if displaced, `.maestro/MAESTRO.md`, plus mission/intake/plan rows). Rewrite the table against surviving `src/v2/` + retained `src/features/` paths. Resolve the fate of `.maestro/MAESTRO.md` at the same time — delete, rename to `EXEC-PLANS.md`, or fold into the v2 plan store.
- Verify `scripts/check-feature-boundaries-lib.ts` still describes truthfully which dirs are features after deletions; update the allow/deny lists at kickoff.
- Verify `bun run check:bundled-skills` and the embed generator under `src/infra/domain/*.ts` still build after absorbed-skill deletions (Phase 4 owned the deletes; Phase 5 owns the source-tree cleanup).

**Done criteria:** v2 e2e green (all `tests/e2e/v2-*.test.ts` pass); kept-feature e2e green; install-smoke workflow green against v2 verbs; no dead imports; README and root docs reference v2 verbs only; maestro completes one dogfooded spec → task → ship cycle on itself using only v2 verbs; **one Phase 5 external-project dogfood completed** (add v2 maestro to a small unrelated repo, run spec → task → ship — tests the install + happy path on a non-maestro codebase). Phase 7's RC-tag dogfood is a separate run against the release candidate.

### Phase 6: Scenario testing (target: month 6)

Behavioral gate before release per ADR-0019. Eight scenarios across project × familiarity × workflow, driven by a **swarm-fix-loop** during Phase 6 itself.

**Loop shape (developer-driven, interactive Claude Code session):**

1. Author the 8 scenarios + deterministic rubrics + fixtures.
2. **Swarm** — dispatch all 8 (or all-failing subset) as sub-agents in parallel via the Claude Code `Agent` tool with `run_in_background: true`. Each sub-agent gets a scenario brief + a fresh sandbox copy of greenfield or brownfield fixture.
3. **Wait** — sub-agents emit pass/fail + rubric trace + evidence-trail dump when they finish.
4. **Triage** — for each failure: read the evidence trail, identify whether the bug is in maestro, in the scenario rubric, or in the sub-agent's harness instructions. Fix maestro (most common) or sharpen the rubric.
5. **Re-dispatch** — send a new sub-agent for each previously-failed scenario; passing scenarios stay green. Loop to step 3.
6. **Exit** — all 8 green in a single pass with no fix in between.

The Agent tool works here precisely *because* Phase 6 runs in an interactive Claude Code session — not in CI. Phase 6 is a development phase, not a perpetual gate. Post-release ongoing monitoring can use whatever transport (CI cron, SDK, manual replay) — that's a v2.x concern, not a v2.0 blocker.

**Concrete deliverables:**

- **Scenarios.** 8 authored under `tests/scenarios/<name>/`. Each contains: `scenario.md` (user-mock script + familiarity tier + termination), `rubric.ts` (deterministic must-happen / must-not-happen against `.maestro/evidence/<date>.jsonl`), `fixture/` (or symlink to `tests/fixtures/v1-maestro/` for brownfield), and `agent-brief.md` (the system+initial-user prompt that gets handed to each spawned sub-agent).
- **User-mock contract.** Each scenario carries an ordered script of N user messages (typically 2–6) plus a termination condition. User-mock is the *initial brief + scripted follow-ups* the spawned sub-agent simulates; determinism lives here so non-determinism is concentrated in the coding-agent's reasoning.
- **Coding-agent surface = production skill bundle.** Each sub-agent loads maestro CLI + the 5 bundled `SKILL.md` files verbatim, no test-only system prompt. Otherwise the rubric measures the test harness, not maestro.
- **Per-scenario project dir.** Before dispatching each sub-agent, the swarm script prepares a fresh project sandbox:
  - **Greenfield:** `mktemp -d` → `cd` in → `git init` → `maestro setup` (the same flow a real new-project user runs). Sub-agent receives the dir path in its brief.
  - **Brownfield:** `mktemp -d` → `cp -R tests/fixtures/v1-maestro/.maestro <tmpdir>/.maestro` + `git init`. The user-mock opening prompt differs by familiarity tier: **novice** opens with intent only ("help me with this project") — the coding-agent must discover from the v1 `.maestro/` artifacts that a migration is required and run `setup --migrate-v2` itself; **expert** opens with the explicit upgrade prompt. Novice tests discovery; expert tests execution. Both exercise the §9 migration spec under live agent conditions; only novice tests whether the harness signals its own upgrade need.
  - Sub-agent operates *inside* its project dir (its `agent-brief.md` opens with "Your working directory is `<path>`. All `maestro` commands run from there"). maestro v2's `resolveMaestroProjectRoot(process.cwd())` (see `src/index.ts:95`) makes cwd the project root — no `--project-root` flag plumbing needed, no env-var hacks.
  - Sandboxes survive the run; rubric runner reads `<tmpdir>/.maestro/evidence/*.jsonl` afterwards. Parallel scenarios never collide because each has its own `mktemp -d`.
- **Rubric runner.** A `bun scripts/scenarios/check.ts <scenario-name> <project-dir>` that reads `<project-dir>/.maestro/evidence/<date>.jsonl` and prints PASS/FAIL with per-line evidence. Project dir is an explicit arg, not inferred — both sub-agent (final step) and dispatcher (triage) pass it.
- **Loop dispatch harness.** `bun scripts/scenarios/swarm.ts` spawns the N sub-agents, **records the `scenario-name → tmpdir` map for the run** (writes it to `.maestro/scenarios/last-run.json`), invokes the rubric runner per scenario with the recorded tmpdir, and prints a pass/fail table + per-failure trace pointer. Each sub-agent's `agent-brief.md` includes an explicit termination contract: exit on `verify=PASS` *or* on 3 consecutive verify failures *or* on a 20-minute timeout — whichever fires first. The dispatcher enforces its own outer per-scenario timeout (e.g., 25 min) so a hung sub-agent can't wedge the swarm.

**Done criteria:** 8 scenarios + rubrics + fixtures authored; swarm dispatcher works; **one full swarm pass completes with all 8 green and zero fixes in between** (i.e., the loop terminated). No nightly cron, no API-key-in-CI dance, no flake aging — those are post-2.0 work if and when scheduled scenario runs are added.

### Phase 7: 2.0 release (target: month 6.5)

- **First action — before any v2 merge:** tag `v0.LAST` on `main` at the last v1 commit (HEAD of `main` before the `harness-os` merge). The tag must point at v1 code, not v2; cutting it post-merge would silently point users at v2. This is sequencing-critical and must precede the RC cut.
- Author `UPGRADING.md` at repo root: user-facing v1 → v2 upgrade guide (what breaks, what to run, where to read more). Separate from §9, which is the *internal* migration spec for maestro maintainers. A DRAFT exists; Phase 7 finalizes against the actual breaking-changes list.
- Hand-curate the `v2.0.0` `CHANGELOG.md` header: the existing 136KB file is auto-generated per-commit. Phase 7 adds a top-level `## v2.0.0 — 2026-MM-DD` section summarizing the breaking changes (mirrors `UPGRADING.md`'s tables), so existing users see the v1→v2 cliff in changelog form too.
- Cut `chore(release): v2.0.0-rc.1` from `harness-os` at least 7 days before the merge. During the soak, re-run the Phase 6 swarm against the RC tag — same dispatcher, same rubrics — and dogfood maestro on at least one real external project. Soak window catches issues real users hit that fixtures don't.
- After RC soak passes: release `chore(release): v2.0.0`, merge `harness-os` → main.
- Delete `harness-os` branch post-merge.
- Stop v1 main bug-fix backports (`v0.LAST` tag stands as the final v1).

**Done criteria:** `v0.LAST` tagged on the last v1 commit of `main` *before* the v2 merge; `UPGRADING.md` finalized; `CHANGELOG.md` carries a hand-curated `v2.0.0` header; `v2.0.0-rc.1` cut and soaked ≥7 days; one swarm pass against the RC tag completes all-green with zero fixes; one external-project dogfood completed; 2.0 ships from main; v1 backport branches closed.

### Items locked in Phase 3

- **Observability adapter scope** (Q12): locked at **Option (C) — minimal log-only default**. `JsonlObservabilityStore` writes per-task lines to `.maestro/runs/<task-id>/observability.jsonl`; every transition produces a row mirroring the evidence-store record. Metric/trace adapters deferred to v2.x. See `docs/phase-3-done.md`.
- **v1 backport policy**: defined in Phase 7 — `v0.LAST` tag stands as the final v1; no further main backports after the tag is cut.

## 11. Non-goals for v2

Kept as-is from v1 (no rework in v2.0 scope; revisit post-2.0):

- **Mission Control TUI** (`src/tui/`, OpenTUI dashboard): keep current rendering, only rename mission → exec-plan in the snapshot read model.
- **MCP server** (`mcp__maestro__*` tools): keep the existing tool *surface area*, but the tool-by-tool work is non-trivial and lands in Phase 5. Renames: `task_complete` → `task_ship`. Deletions: `task_unblock` (v2 routes unblock via verify PASS). Replacements: `task_create` / `task_plan` → `task_from_spec`. Additions for v2 hot-path: `principle_promote`, `setup_check`, `setup_migrate_v2`. Grill-driven verbs (`spec new`, `plan from-spec`, `plan decompose`) stay CLI-only — MCP cannot sustain the interactive grill protocol.
- **Hooks** (`hooks/` session/tool hook entrypoints): keep as-is.
- **CI integration** (`maestro ci verify`, PR check, auto-merge eligibility): keep authoritative; only the verb surface changes if the noun changes.
- **GitNexus** (`gitnexus_impact`, `gitnexus_detect_changes`): keep as-is; reference from new principles pack.
- **`gc` / `recover` / `bundle`**: keep verbs and behavior; only rebrand the user-facing copy from mission to exec-plan vocabulary.
- **Verdict types, witness levels L0–L7, policy YAML, risk class derivation**: unchanged surface; v2 reuses them under the new lifecycle.
- **`skills`** (ADR-0015): keep `skills list` + `skills sync` for the 5-skill bundle. Tiny surface; no other verbs.
- **`deploy`** (ADR-0015): keep L7 deploy-gate evidence verbs unchanged.

Deleted in v2 (ADR-0015):

- **`memory`** + **`memory-ratchet`** + **`agent`** feature directories: deleted as runtime stores; corrections → `docs/principles/*.md`, learnings → `docs/design-docs/learnings/*.md`, agent prompt → AGENTS.md. One verb survives: `maestro principle promote` (materializes a principle markdown from a FAIL correction). See ADR-0015 for the tradeoff statement.
- **`graph`** feature directory: absorbed into references.
- **`session`** feature directory: notes → handoff, detect → worktree.
- **`ralph` / `ralph-review`** verb: dropped (loop primitive owns iterate-until-PASS).
- **`note` / `notes`** standalone verb: dropped (notes live as markdown files).
- **`inspect`** and **`state`** verbs: dropped (per-primitive show verbs cover everything).
- **`skills/built-in/maestro:*` colon-namespaced tier**: piecemeal migration into the 5-skill bundle; directory removed.

Explicitly NOT in v2.0 (post-2.0 work):

- New observability backend beyond the chosen Phase 3 scope.
- LLM call inside maestro (still forbidden by no-runner-inversion lint).
- Daemon/scheduler/cron (forbidden by passive-harness rule).
- Backward compatibility shims for v1 verbs.
- Cross-task dependency graph beyond exec-plan's own decomposition.
- Memory/correction store as a runtime port (ADR-0015 absorbed it into principles).

## 12. Decision register

The 13 locked decisions, in order:

| # | ADR | Decision |
|---|---|---|
| 1 | 0001 | Full rebuild over agent-surface re-skin |
| 2 | 0002 | Adopt article vocabulary; retain task + handoff |
| 3 | 0003 | Two-lifecycle model (task + exec-plan) |
| 4 | 0004 | Hybrid transitions: manual entry, automatic exit on verdict |
| 5 | 0005 | Ports + default adapters + setup recipes |
| 6 | 0006 | Task↔PR strictly 1:1 |
| 7 | 0007 | Big-bang 2.0 release, no backward compatibility |
| 8 | 0008 | Worktree binding is mode-driven |
| 9 | 0009 | Evidence emits on every state transition + ad-hoc |
| 10 | 0010 | Document-driven workflow; spec markdown is source of truth |
| 11 | 0011 | Exec-plan auto-completes when all child tasks reach a terminal state |
| 12 | 0012 | Five-skill agent-facing bundle |
| 13 | 0013 | Greenfield long-lived branch for v2 implementation |
| 14 | 0014 | Verb naming: git-style noun-verb with hot-path aliases |
| 15 | 0015 | v1 feature gap closure (memory/graph/session absorbed; ralph/notes/inspect/state dropped; classify→design; qa→setup; colon-tier migrated) |
| 16 | 0016 | Grill protocol baked into design + plan skills (no new verbs, no sixth skill) |
| 17 | 0017 | Cross-cutting layers (`providers`) are universally importable; layer-order exempt in both directions |
| 18 | 0018 | v1 sunset scope = v2-displaces (delete every `src/features/` dir v2 owns; keep §11 non-goals) |
| 19 | 0019 | Scenario test architecture: 8 scenarios, sub-agent swarm-fix-loop via Agent tool, deterministic rubric against evidence trail |

Open decisions (lock during phase noted):

| Item | Phase | Status |
|---|---|---|
| Observability adapter scope (A/B/C) | Phase 3 | Deferred during grill; pick before Phase 3 starts |
| v1 bug-fix backport policy | Phase 4 | Stop on 2.0 ship date |

## 13. Why this shape works

Three falsifiable claims about the v2 design:

1. **The chaos disappears at its root.** The user's "chaos" complaint named the symptom (verb sprawl) and the cause (no lifecycle). v2 addresses both: the verb surface collapses to five skills, and every primitive sits inside a state machine with auto-transitions and evidence. Test: count the agent-facing verbs after Phase 1; if it exceeds 20, the rebuild has regressed to v1's shape.

2. **The article's three diagrams map onto maestro primitives 1:1.** Observability stack → ObservabilityPort + Vector/Victoria adapter; "what Codex can't see doesn't exist" → design-docs + references + principles directories; layered architecture → architecture.yaml + lints. Test: a new contributor reading the article should find every term either in maestro's primitive set or as an explicit non-goal.

3. **Maestro stays passive.** No daemon. No scheduler. No LLM call inside maestro. The no-runner-inversion lint enforces this. "Automatic" only ever means "computed from the result of the verb the agent just called." Test: grep for `setInterval`, `setTimeout`, `cron`, `daemon` in `src/`. If any appear in non-test code, the passive-harness invariant is broken.
