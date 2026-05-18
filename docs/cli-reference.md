# Maestro CLI reference

Verb-by-verb reference for the `maestro` CLI. Each section names the verb, its flags, exit codes where relevant, and the canonical doc for the full contract.

For agent-facing usage, prefer the bundled skills under `skills/bundled/maestro-*`; they cross-reference these verbs in the right order. The six bundled skills are `maestro-task`, `maestro-mission`, `maestro-design`, `maestro-verify`, `maestro-handoff`, `maestro-setup`.

---

## Build and check helpers

```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun run check:layers      # forward-only layer enforcement
bun run test
./dist/maestro mission-control --render-check --size 120x40
bun run release:local
```

`release:local` is the only path that rebuilds `dist/maestro` and installs it onto `PATH`.

---

## spec

```bash
maestro spec new <slug> [--title <text>] [--mode light|heavy]
maestro spec validate <path>
```

`spec new` scaffolds `.maestro/specs/<slug>.md` with frontmatter (`mode: light | heavy`, `work_type`, `acceptance_criteria`, `non_goals`). `spec validate` parses an existing spec and reports frontmatter errors. Heavy-mode specs are the input to `mission new --from-spec`; light-mode specs feed `task from-spec`.

Authored interactively through the `maestro-design` skill (grill protocol).

Exit codes: `spec validate` returns 1 on parse error or schema violation.

---

## task

```bash
maestro task from-spec <path>
maestro task claim <id> [--agent <id>] [--skip-worktree]
maestro task verify <id> [--json] [--verdict human|block] [--reason <text>]
maestro task block <id> --reason <text>
maestro task abandon <id> --reason <text>
maestro task ship <id> [--pr-url <url>]
```

Hot-path aliases: `maestro claim <id>`, `maestro verify <id>`, `maestro block <id>`, `maestro abandon <id>`, `maestro ship <id>`. The aliases accept the same flags as their `task` counterparts.

State machine (ADR-0003):

```
draft → claimed → doing ↔ verifying ↔ blocked → ready → shipped
                                                              ↘ (any state) → abandoned
```

Transitions are hybrid (ADR-0004): the agent enters check states manually; the harness exits them automatically on verdict.

- `task from-spec <path>` creates a `draft` task from a product-spec markdown file.
- `task claim` flips `draft → claimed`. For heavy-mode specs it auto-creates a worktree under `<parent>/<repo>-<task_id>` and records the path on the task. `--skip-worktree` opts out. `--agent <id>` is recorded on the task and on the transition evidence row.
- `task verify` runs the architecture lints. Default routing: `PASS → verifying → ready` (exit 0), `FAIL → stay at verifying` (exit 1). `--verdict human --reason <text>` stays at `verifying` (exit 2). `--verdict block --reason <text>` transitions to `blocked` (exit 3). `--json` emits `{id, state, verdict, violations}`.
- `task block` carries a human-readable blocker on `claimed | doing | verifying → blocked`.
- `task abandon` transitions any non-terminal state to `abandoned`.
- `task ship` is the manual `ready → shipped` flip with an optional PR URL.

Each transition emits an evidence row (ADR-0009) into `.maestro/evidence/<date>.jsonl`, a parallel observability row into `.maestro/runs/<task-id>/observability.jsonl`, and a handoff envelope into `.maestro/handoffs/<id>.json`.

One task = one PR (ADR-0006). Multi-PR work promotes to a mission.

---

## task observe

```bash
maestro task observe metrics <promql> [--prometheus-url <url>] [--json] [--record --task <id>]
maestro task observe logs [--log-file <path>] [--lines <n>] [--filter <text>] [--json] [--record --task <id>]
```

Dev-time per-worktree observability. Both subcommands are read-only by default; `--record --task <id>` writes a `manual-note` evidence row at witness level `agent-claimed-locally` with a `[dev-observation:metrics]` or `[dev-observation:logs]` payload note.

- `task observe metrics <promql>` runs a one-shot PromQL query against the dev metrics backend. Base URL precedence: `--prometheus-url` → `MAESTRO_PROMETHEUS_URL`. Missing URL exits 1.
- `task observe logs` tails the last N lines (default 100) from the dev log file declared by `--log-file` or `MAESTRO_DEV_LOG_FILE`. `--filter <text>` is a substring filter applied before tail; `--lines` must be a positive integer.
- `--json` emits `{kind, ...}` envelopes (`kind: "metrics"` or `kind: "logs"`); the plain form prints a `[dev-metrics]` / `[dev-logs]` summary line.
- `--record` without `--task` exits 1.

Exit codes: 0 success, 1 config error (missing URL or log path, invalid `--lines`, `--record` without `--task`), 2 backend error (Prometheus query failed, log read failed, evidence record failed).

See `docs/dev-observability.md`.

---

## mission

```bash
maestro mission new [title...] [--from-spec <path>] [--from-file <path>] [--template <name>] [--slug <slug>] [--list-templates]
maestro mission from-spec <path>
maestro mission decompose <id> --file <path|->
maestro mission cancel <id> [--reason <text>]
maestro mission show <id> [--json]
```

State machine (ADR-0011):

```
intake → approved → planned → in-progress ↔ paused → completed | failed
                                                              ↘ (any state) → cancelled
```

- `mission new <title>` creates a bare mission in `intake`. Pass `--from-spec` (creates at `approved`), `--from-file` (creates at `planned` with seeded tasks), or `--template <name>` (creates at `planned` with built-in or user template tasks). Flags are mutually exclusive. `--list-templates` prints built-ins and user overrides without creating a mission.
- `mission from-spec` is the spec-first shortcut (equivalent to `mission new <spec-title> --from-spec <path>`); requires `mode: heavy`.
- `mission decompose` reads a task batch (JSON file or `-` for stdin) and creates child tasks. Accepts both `intake` and `approved` input states; advances the mission to `planned`. Refuses missions that already have any tasks.
- `mission cancel` cascades active tasks to `abandoned`, then transitions the mission to `cancelled`. Idempotent on already-cancelled; errors on `completed` / `failed`.
- `mission show` prints the mission and its child tasks; `--json` emits the full record.

Auto-advance (ADR-0011) is wired into `task claim` / `task ship` / `task abandon` / `task block`. Rollup rules: `planned → in-progress` on the first non-draft task; `in-progress → paused` when every active task is `blocked`; `paused → in-progress` when any unblocks; terminal rollup to `completed` when every task `shipped`, or `failed` when any `abandoned`. Auto-rollup evidence rows carry `trigger: "rollup"` plus the matching rule and a task summary.

Built-in templates: `refactor`, `feature`, `bug`, `migration`. User templates live at `.maestro/templates/missions/<name>.yaml` and override built-ins of the same name; `name` in the YAML must match the filename.

No hot-path aliases — mission verbs are heavy and not on the loop critical path.

---

## principle

```bash
maestro principle promote <correctionId> [--json]
```

Materializes `docs/principles/<slug>.md` from a `lint-violation` evidence row (slug derived from `rule_id`). Promotion is the only path from a one-off correction to a durable golden rule.

Exit code 1 when the correction id is missing or is not a `lint-violation` row.

---

## setup

```bash
maestro setup [--global] [--dry-run] [--resync-skills] [--reset-templates] [--no-git-ok] [--json]
maestro setup check [--json]
```

`maestro setup` scaffolds the `.maestro/` layout. Idempotent: detects the current state and only touches what changed.

- Creates `.maestro/{specs,missions,tasks,runs,evidence,handoffs,worktrees}` with `.gitkeep` placeholders.
- Writes default skill bundles and context templates; `--reset-templates` overwrites user-customized files.
- `--resync-skills` reconciles `.claude/skills/` and `.codex/skills/` with shipped templates.
- `--dry-run` plans without writing.
- `--global` initializes `~/.maestro/` for the user instead of the repo.
- `--no-git-ok` allows running outside a git working tree (default refuses).
- `--json` emits the full report; otherwise prints one line per step.

`setup check` audits the `.maestro/` directory layout, the principles pack (`docs/principles/`), and `.maestro/config.yaml`. Exit 1 only when an entry is `missing`; `warn` (empty principles pack, absent config.yaml) is informational.

`maestro init` is a hidden alias for `maestro setup` retained for muscle memory.

---

## evidence

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
maestro evidence record --task <id> --kind manual-note --note "Verified manually"
maestro evidence record --task <id> --kind ai-review --reviewer <bug|security|architecture> --findings <inline-json-or-path> --confidence <0-1>
maestro evidence record --task <id> --kind threat-model --threat-model-file <path>
maestro evidence list --task <id>
maestro evidence show <evidence-id>
```

Evidence rows are append-only and witnessed (L0–L7). See `docs/witness-levels.md`.

---

## contract

```bash
maestro contract show --task <id> [--at-version <n>]
maestro contract amend --task <id> --add-path <path> --reason "<why>"
maestro contract amend --task <id> --remove-path <path> --reason "<why>"
maestro contract history --task <id>
```

Amendments consume from `amendmentBudget`. Each amend writes a `contract-amended` evidence row. See `docs/sensitive-paths-defaults.md` and `docs/owners-yaml-format.md`.

---

## verdict

```bash
maestro verdict show --task <id> [--at-version <id>] [--pr <number>]
maestro verdict request --task <id> [--json]
maestro verdict override --task <id> --pr <number> --reason "<text>" [--verdict <id>] [--base <ref>] [--json]
```

Exit codes for `verdict request`: 0 = PASS, 1 = FAIL, 2 = HUMAN, 3 = BLOCK.

`--pr <n>` filters by current HEAD tree SHA.

`verdict override` records `verdict-override` evidence at `agent-claimed-and-not-reproducible`. Requires the invoker in `owners.yaml.sensitive_waiver` (loaded from base branch). Does NOT flip the PR check conclusion. See `docs/override-flow.md`.

---

## policy

```bash
maestro policy check --task <id>
maestro policy pending
```

`policy pending` lists pending loosenings still in their 30-day soak.

---

## ci

```bash
maestro ci verify [--pr <n>] [--task <id>] [--base <ref>] [--json]
```

Reads CI env (`GITHUB_*`) by default; flags override. Runs the Trust Verifier, ingests CI job-result file as `witnessed-by-ci` evidence, computes Verdict, writes outputs, posts a GitHub Check (when token present).

Exit codes: 0 PASS / 1 FAIL / 2 HUMAN / 3 BLOCK.

CI Maestro is authoritative; the PR check is the merge gate. See `docs/ci-integration.md`.

---

## merge auto

```bash
maestro merge auto --pr <number> --task <id> [--base <ref>] [--repo <owner/name>] [--json]
```

Runs 8 eligibility predicates. Exit 0 + `gh pr merge --auto` on pass; exit 1 listing failing codes on fail. Requires `autoMergeAllowed.<riskClass>: true` in `autopilot.yaml`. See `docs/auto-merge-eligibility.md`.

---

## review ack

```bash
maestro review ack --task <id> --verdict <id> --criterion "<text>" [--criterion "<text>" ...] [--json]
```

Records `review-ack` evidence at `agent-claimed-locally`. Required when the verdict is `HUMAN` at `>=medium` risk before `merge auto` can succeed. `--criterion` is repeatable.

---

## deploy

```bash
maestro deploy gate --task <id> [--base <ref>] [--json]
maestro deploy rollback --task <id> --command <cmd> [--json]
```

`deploy gate` runs 4 checks (feature_flag, canary_plan, rollback, owner) and records `deploy-readiness` evidence. Exit 0 pass, 1 fail. Does NOT mutate Verdict by default; wire via `policies/risk.yaml`.

`deploy rollback` runs the command and records `rollback-exercised` evidence at `witnessed-by-ci` (in CI) or `witnessed-by-maestro` (locally).

See `docs/deploy-gate.md`.

---

## runtime

```bash
maestro runtime check --task <id> [--provider-base-url <url>] [--json]
```

Queries each signal declared in `Spec.runtime_signals` via the configured provider (Prometheus today). Records `runtime-signal` evidence per signal. Exit code always 0; `pass=false` rows are advisory at L7.

Provider base URL precedence: `--provider-base-url` → `MAESTRO_PROMETHEUS_URL` → `http://localhost:9090`. See `docs/runtime-monitoring.md`.

---

## gc

```bash
maestro gc doc-gardening [--task <id>] [--json]
maestro gc slop-cleanup [--min-severity info|warn|error] [--json]
maestro gc plan-regen --task <id> [--json]
```

`gc slop-cleanup` is the principles scanner: it walks `docs/principles/*.md`, runs each rule's `Scan Command` (a ripgrep one-liner), and reports violations with the rule's `Fix Recipe`. See `docs/architecture-lints.md`.

`doc-gardening` scans repo docs for stale path references. `plan-regen` reports plan-vs-state drift.

---

## recover

```bash
maestro recover --task <id> [--to <commit>] [--force] [--dry-run] [--json]
```

Resolves the last `PASS` verdict, finds a commit whose tree matches `verdict.subject.tree_sha`, runs `git reset --hard`, removes `.maestro/runs/<id>/`, records a `recovery` evidence row at `witnessed-by-maestro`. Refuses dirty trees unless `--force`.

---

## bundle

```bash
maestro bundle export <missionId> --out <path>
```

Exports a mission bundle for review.

---

## skills

```bash
maestro skills list
maestro skills sync
```

Lists the six bundled skills and syncs their embedded templates with the source under `skills/bundled/`.

---

## worktree

```bash
maestro worktree create <slug> [--base <branch>] [--prefix <pre>] [--json]
```

Wraps `git worktree add -b <prefix>/<slug>` and provisions `.maestro/runs/` inside the worktree. Default base `main`, default prefix `feat`. State persists at `.maestro/worktrees/<task-id>.json` on the primary repo (PD-3).

`task claim` auto-creates a worktree for heavy-mode specs; this verb is for manual / off-spec cases.

---

## mission-control (TUI)

```bash
maestro mission-control [--screen <name>] [--size <wxh>] [--format plain|ansi] [--preview]
maestro mission-control --render-check --size 120x40
maestro mission-control --json [--filter task=<id>] [--filter feature=<id>]
```

`--filter key=value` narrows JSON output; filters are AND-combined. `--screen <name>` is an alias for `--preview <name>`. The TUI snapshot read model is inspection-only.

---

## mcp

```bash
maestro mcp serve [--project-root <path>] [--transport stdio]
maestro mcp check [--json]
```

The `mcp__maestro__*` tool surface ships with the binary and is consumed by host agents through MCP over stdio. Tools mirror the CLI verbs; the filter field on `maestro_task_list` is `mission_id`. Surfaces:

| Surface | Tools |
|---------|-------|
| Task | `maestro_task_list`, `maestro_task_get`, `maestro_task_from_spec`, `maestro_task_claim`, `maestro_task_block`, `maestro_task_ship` |
| Evidence | `maestro_evidence_record`, `maestro_evidence_list` |
| Contract | `maestro_contract_show`, `maestro_contract_amend` |
| Verdict | `maestro_verdict_show`, `maestro_verdict_request` |
| Policy | `maestro_policy_check` |
| Handoff | `maestro_handoff_list`, `maestro_handoff_show`, `maestro_handoff_emit`, `maestro_handoff_pickup` |
| Principle | `maestro_principle_promote` |
| Setup | `maestro_setup_check` |

Project root resolves by walking up for `.maestro/`; override with `--project-root` or `MAESTRO_PROJECT_ROOT`. `mcp check` exits 1 when the installed binary is missing or stale.

See `docs/mcp-server.md` for tool I/O schemas and error codes, and `docs/mcp-setup.md` for client wiring.

---

## Exit code summary

| Code | Meaning                              | Verbs                        |
| ---- | ------------------------------------ | ---------------------------- |
| 0    | PASS / success / advisory clean      | All verbs                    |
| 1    | FAIL / error / missing requirement   | All verbs                    |
| 2    | HUMAN — needs human attention        | `task verify`, `verdict request`, `ci verify` |
| 3    | BLOCK — blocked at the source        | `task verify`, `verdict request`, `ci verify` |

Local Maestro is advisory; CI Maestro is authoritative.
