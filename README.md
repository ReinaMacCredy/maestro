# maestro

The harness OS for agent-generated codebases.

Maestro is a local-first agent harness for the spec-to-ship loop: agents claim
tasks, verify their own work, and ship. Humans steer. Agents execute. Maestro
is the substrate that gives every agent session durable primitives, lifecycle
state, evidence, and golden rules without a daemon, a database, or a network
API.

The vocabulary is `spec -> task -> verify -> ship`. One task equals one PR
(ADR-0006). Multi-PR work decomposes into an exec-plan via `maestro-plan`.
Maestro stays passive: no scheduler, no background process, no LLM call inside
maestro. "Automatic" means computed from the result of the verb the agent just
called.

---

## Install

### Requirements

- [Bun](https://bun.sh/) 1.x
- Git

### Build and install locally

```bash
git clone https://github.com/ReinaMacCredy/maestro.git
cd maestro
bun install
bun run release:local
```

`release:local` rebuilds `dist/maestro` and installs the binary to
`~/.local/bin/maestro`. This is the only path that exercises the installed
binary on `PATH`.

Verify the install:

```bash
maestro --version
```

`./dist/maestro` is the fresh repo build. `maestro` on your `PATH` is the
installed binary. Treat them as separate artifacts; `command -v maestro` tells
you which one you are running.

---

## Five-minute quickstart

The steps below mirror `scripts/v2-smoke.ts`, the canonical happy path.

### 1. Initialize a git repo (if needed)

```bash
git init -b main
```

Maestro resolves its project root from `cwd`; it does not need a flag.

### 2. Create the architecture rules file

`maestro task verify` runs the architecture lint pass and requires a rules file:

```bash
mkdir -p docs
cat > docs/architecture.yaml <<'YAML'
version: 1
forward_only: true
layers:
  - types
  - config
  - repo
  - service
  - runtime
  - ui
cross_cutting:
  - providers
lint_scope:
  - "src/config/**/*.ts"
  - "src/providers/**/*.ts"
  - "src/repo/**/*.ts"
  - "src/runtime/**/*.ts"
  - "src/service/**/*.ts"
  - "src/types/**/*.ts"
  - "src/ui/**/*.ts"
passive_harness:
  forbidden_patterns:
    - setInterval
YAML
```

For existing projects the file is usually already present. Skip this step if
`docs/architecture.yaml` exists.

### 3. Bootstrap the project

```bash
maestro setup bootstrap
```

Creates the canonical directories under `.maestro/` with `.gitkeep` placeholders.
Idempotent: safe to re-run.

### 4. Check setup

```bash
maestro setup check
```

Audits the directory layout, principles pack, and config file. Exits 1 only
when an entry is `missing`; `warn` is informational. Add `--json` for machine
output.

### 5. Author a spec

```bash
maestro spec new my-feature --title "My first feature"
```

Scaffolds `.maestro/specs/my-feature.md` with YAML frontmatter. Open the file
and fill in `acceptance`, `non_goals`, `risk_class`, `mode`, and `work_type`.
For a guided interview, run the `maestro-design` skill before this step.

The frontmatter shape:

```yaml
---
slug: my-feature
title: My first feature
status: draft
acceptance:
  - "The new endpoint returns 200 for valid input"
non_goals:
  - "Migrating existing data"
risk_class: medium
mode: light
work_type: feature
blocked_by: []
---
```

Run `maestro spec validate .maestro/specs/my-feature.md` to check frontmatter
before proceeding.

### 6. Create the task

```bash
maestro task from-spec .maestro/specs/my-feature.md
```

Creates a task in `draft` state and prints the task ID (`tsk-...`). The task
log is `.maestro/tasks/tasks.jsonl`.

### 7. Claim the task

```bash
maestro task claim <tsk-id>
# or the hot-path alias:
claim <tsk-id>
```

Transitions the task to `claimed`, records a transition evidence row, and
emits a handoff envelope at `.maestro/handoffs/<id>.json`. For heavy-mode
specs, a worktree is auto-created at this step. To skip worktree creation:

```bash
maestro task claim <tsk-id> --skip-worktree
```

### 8. Do the work, then verify

Implement the change. When ready:

```bash
maestro task verify <tsk-id>
# or:
verify <tsk-id>
```

Exit codes:

| Code | Meaning | Next action |
|---|---|---|
| `0` | PASS | Task auto-advances to `ready`. Run `ship`. |
| `1` | FAIL | Fix the cited violations. Run `verify` again. |
| `2` | HUMAN | Task stays at `verifying`. Hand off and stop. |
| `3` | BLOCK | Task transitions to `blocked`. Surface the reason. |

FAIL sends you back to the implementation loop. PASS advances automatically.

### 9. Ship

```bash
maestro task ship <tsk-id>
# or:
ship <tsk-id>
```

Transitions `ready -> shipped`. Optionally attach a PR URL:

```bash
maestro task ship <tsk-id> --pr-url https://github.com/owner/repo/pull/123
```

---

## The five skills

Maestro ships a bundle of five agent-facing skills. Agents load them at session
start. Each skill is a markdown document in `skills/bundled/`.

### maestro-design

Interview-driven product-spec authoring. Runs the grill protocol:
a one-question-at-a-time interview that walks acceptance criteria, non-goals,
risk class, mode, and work type, challenging user language against `CONTEXT.md`
and committed ADRs. Output is a committed `.maestro/specs/<slug>.md` ready for
`maestro task from-spec`. Use this skill before authoring any spec.

### maestro-plan

Heavy-mode workflow. Takes an approved `mode: heavy` product-spec and turns it
into an exec-plan with child tasks via `maestro plan from-spec` followed by
`maestro plan decompose`. The decompose step runs the grill protocol against
the spec, `CONTEXT.md`, and the architecture lint set before emitting the task
batch. The exec-plan auto-completes when every child task reaches `shipped` or
`abandoned` (ADR-0011). Use this skill when the work spans three or more
vertical slices or multiple feature directories.

### maestro-task

Single-task execution loop for light-mode specs. Guides the agent from
`task from-spec` through `claim`, the `doing <-> verifying` iteration,
blocking when stuck, and finally `ship`. Auto-activates when a `.maestro/`
directory is detected in the working tree. Every state transition emits a
handoff envelope and an evidence row. Use this skill for any single-PR
implementation task.

### maestro-verify

The canonical verification protocol. Documents exit-code routing (PASS / FAIL /
HUMAN / BLOCK), the architecture-lint corpus, the Trust Verifier checks, and
the ProofMap acceptance-criteria coverage gate. Cross-referenced by
`maestro-task` and `maestro-plan`. Read this skill before declaring any task
complete; it is the shared pre-ship ritual every agent follows.

### maestro-setup

Repository onboarding. The skill generates context docs under
`.maestro/context/`, a hierarchical `AGENTS.md`, language style guides, and a
setup report. The CLI mirrors the skill: `setup check` audits drift, `setup
bootstrap` scaffolds directories, `setup migrate-v2` performs the 11-step
upgrade from a pre-rebuild `.maestro/`, and `setup migrate-corrections` moves
legacy corrections into `docs/principles/legacy/`. Use this skill when
initializing a new project or upgrading an older `.maestro/` directory.

---

## Architecture: src/

The implementation follows a forward-only layered architecture enforced by
`docs/architecture.yaml` and checked at every `maestro task verify`:

| Layer | Path | Role |
|---|---|---|
| `types` | `src/types/` | Domain types: task state machine, exec-plan state machine, product-spec shape, evidence kinds |
| `config` | `src/config/` | Per-project and per-repo configuration loading |
| `repo` | `src/repo/` | Ports and adapters: task store, plan store, spec store, evidence store, worktree store, handoff store |
| `service` | `src/service/` | Use cases: task-claim, task-verify, plan-decompose, migrate-v2, setup-check, principle-promote |
| `runtime` | `src/runtime/` | CLI command registration: spec, task, plan, principle, setup verbs |
| `providers` | `src/providers/` | Cross-cutting service wiring (importable from any layer) |

Layer-order imports are enforced mechanically. A service may not import from
runtime; a repo adapter may not import from service. The `providers` layer is
exempt in both directions.

For the full WHERE TO LOOK table, see `AGENTS.md`.

---

## .maestro/ layout

The `.maestro/` directory is the harness-internal state layer. Agents and the
CLI write it; humans inspect it via `maestro` commands rather than editing
files directly.

```
.maestro/
  specs/              product-spec markdown files (<slug>.md, YAML frontmatter)
  tasks/
    tasks.jsonl    append-only task ledger
  plans/
    plans.jsonl    exec-plan ledger
    <slug>.md         optional human-readable plan sidecar
  evidence/
    <date>.jsonl      transition + ad-hoc evidence rows (per-machine)
  runs/
    <task-id>/
      observability.jsonl   per-task observability mirror (per-machine)
  handoffs/           handoff envelopes (<id>.json, emitted at transitions)
  worktrees/
    <task-id>.json    worktree binding records
  context/            operator-authored context docs (index, architecture, etc.)
  backups/            migration backup tarballs (pre-v2-<ts>.tar.gz)
  .migrated-v2.json   migration idempotency stamp
  config.yaml         project-local maestro settings
```

The `docs/` tree is the human-visible, maestro-managed surface: ADRs,
exec-plans, design docs, principles, architecture rules, and references. Both
trees are maestro-owned; the split is visibility, not ownership.

---

## CLI verbs

The full verb reference is at `docs/cli-reference.md`. Top-level nouns at a
glance:

| Verb | Purpose |
|---|---|
| `setup` | Bootstrap, audit, and migrate (`check`, `bootstrap`, `migrate-v2`, `migrate-corrections`) |
| `spec` | Product-spec authoring (`new`, `validate`) |
| `task` | Task lifecycle (`from-spec`, `claim`, `verify`, `ship`, `block`, `abandon`) |
| `plan` | Exec-plan workflow for heavy-mode specs (`from-spec`, `decompose`, `show`) |
| `principle` | Promote a correction to a principle markdown (`promote`) |
| `evidence` | Record and list evidence rows |
| `worktree` | Explicit worktree management |
| `recover` | Reset a working tree to its last PASS verdict's tree |
| `gc` | Garbage collection and slop-cleanup lint pass |
| `bundle` | Export a portable archive of a plan or task |
| `skills` | List and sync the bundled skill set |
| `mcp` | Start the MCP server for tool-calling agents |
| `ci` | Run the verdict pipeline in CI mode |

Hot-path aliases (first-class; identical to their `task` counterparts):
`claim`, `verify`, `ship`, `block`, `abandon`.

---

## Migrating from a pre-rebuild `.maestro/`

The current harness has no backward-compatibility shims for the
pre-rebuild `.maestro/` directory layout. To upgrade:

```bash
maestro setup migrate-v2
```

This writes a backup tarball to `.maestro/backups/pre-v2-<timestamp>.tar.gz`,
rewrites `.maestro/` to the current shape, and stamps `.migrated-v2.json` for
idempotency. Pin to the `v0.LAST` tag on `main` if you are not ready to
upgrade. Full verb-rename tables and the file-layout mapping are in
`UPGRADING.md`.

---

## Contributing and repo conventions

The authoritative guide for contributors and agents is `AGENTS.md` at the
project root. It contains the full WHERE TO LOOK table, architecture rules, and
anti-patterns. `CLAUDE.md` points at it.

Conventions at a glance:

- Bun-first, ESM, strict TypeScript. `bun run build` produces `dist/maestro`.
- Conventional commits: `feat(scope):`, `fix(scope):`, `refactor(scope):`.
  Bump the CLI version for every behavior change.
- Every skill change must update `skills/bundled/maestro-*/SKILL.md` in the
  same commit.
- Hand-editing generated embed files under `src/infra/domain/` is an
  anti-pattern; run `bun run sync:bundled-skills`.
- `bun run release:local` is the only way to test the installed binary.
- After code changes: `bun run build && ./dist/maestro --version && bun test`.

---

## Status

The current `main` reflects the harness-OS rebuild: a layered `src/`
(`types`, `config`, `repo`, `service`, `runtime`, `ui`, `providers`), a
5-skill agent-facing bundle, the `spec -> task -> verify -> ship` vocabulary,
the eight behavioral scenarios under `tests/scenarios/`, and the dispatch +
rubric runner at `scripts/scenarios/`. The decision register lives at `docs/adr/`.
