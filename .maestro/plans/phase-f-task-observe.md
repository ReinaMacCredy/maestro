# Phase F — `maestro task observe` CLI verb (Plan-agent design, opus, 2026-05-16)

> Continues the master-plan loop on `harness-os`. Port + 2 adapters + doc
> already exist; this phase wires the CLI verb + tests + skill docs.

## State on entry

- `src/features/runtime/ports/dev-observability.port.ts` — DONE.
- `src/features/runtime/adapters/dev-prometheus.adapter.ts` — DONE.
- `src/features/runtime/adapters/log-tail.adapter.ts` — DONE.
- `docs/dev-observability.md` — DONE.
- `tests/unit/features/runtime/dev-observability/*.test.ts` — DONE (adapter unit coverage).

Missing: CLI verb (`maestro task observe metrics|logs`), command-level tests, skill docs.

## Preliminary decisions

| Decision | Choice | Rationale |
|---|---|---|
| `--record` evidence kind | Write `manual-note` with payload prefix `[dev-observation:metrics]` / `[dev-observation:logs]`; do NOT add a `dev-observation` kind this PR. | `EvidenceKind` enum has no `dev-observation` member; promoting cascades to `EvidencePayloadByKind`, the MCP schemas in `schemas/inputs.ts`, the CLI `EVIDENCE_KINDS` allowlist, and serialization tests. The port doc-comment already anticipates the deferral ("`manual-note`-shaped `dev-observation` row"). Promotion is a clean follow-up once usage signal arrives. |
| Command file location | `src/features/runtime/commands/task-observe.command.ts` | Cohesion with port + adapters in `src/features/runtime/`. Mirrors `runtime-check.command.ts`. |
| Parent command shape | `task observe` parent with `metrics <promql>` and `logs` children (three-level Commander nesting). | Matches the master-plan goal string; `observe` is a stateless read-only parent, no action of its own. |
| `--filter` semantics | Substring only (no regex) — matches `LogTailAdapter.tailLogs` `.includes(filter)`. | Avoids regex-injection risk; aligns with existing adapter behavior. |
| `--follow` flag | OUT — would force `setInterval`. | Passive-harness invariant. |
| Exit codes | `0` success; `1` config error (missing URL/path, `--record` without `--task`); `2` backend unreachable / empty vector / fs read error. | Mirrors `task verify` exit-code pattern. |
| URL flag | `--prometheus-url` (overrides `MAESTRO_PROMETHEUS_URL`); flag wins. | Shorter; matches env name. |
| Output | Plain text by default; `--json` for JSON envelope. | Matches `task verify --json`. |

## CLI surface

### `maestro task observe metrics <promql>`

```
[--prometheus-url <url>]   Override MAESTRO_PROMETHEUS_URL
[--json]                   JSON envelope
[--record]                 Write a manual-note evidence row tagged [dev-observation:metrics]
[--task <id>]              Required when --record is set
```

**stdout (plain):**
```
[dev-metrics] value=42.5  source=prometheus@http://localhost:9090  sampled_at=2026-05-16T...
```

**stdout (`--json`):**
```json
{ "kind": "metrics", "query": "up", "value": 42.5, "source": "...", "sampledAt": "..." }
```

### `maestro task observe logs`

```
[--log-file <path>]   Override MAESTRO_DEV_LOG_FILE
[--lines N]           Default 100
[--filter <text>]     Substring filter
[--json]              JSON envelope
[--record]            Write a manual-note evidence row tagged [dev-observation:logs]
[--task <id>]         Required when --record is set
```

**stdout (plain):**
```
[dev-logs] source=file:/path/to/app.log  lines=3
error: ...
error: ...
error: ...
```

**stdout (`--json`):**
```json
{ "kind": "logs", "source": "file:...", "lines": [{"text":"..."},...] }
```

### `--record` payload

```ts
recordEvidence(evidenceStore, {
  task_id: taskId,
  kind: "manual-note",
  witness_level: "agent-claimed-locally",
  payload: {
    note: `[dev-observation:metrics] query="${query}" value=${sample.value} source=${sample.source} sampledAt=${sample.sampledAt}`,
  },
})
```

Logs variant: `[dev-observation:logs] lines=N filter=... source=...`.

## Files

**Create:**
- `src/features/runtime/commands/task-observe.command.ts` — exports `registerTaskObserveCommand(task: Command, deps?: TaskObserveCommandDeps)`.
- `tests/unit/features/runtime/commands/task-observe.command.test.ts` — command-level tests via injected stub adapters.

**Modify:**
- `src/runtime/task.command.ts` — call `registerTaskObserveCommand(task, {})` at the end of `registerTaskV2Commands`.
- `skills/bundled/maestro-task/SKILL.md` — append a "Dev-time observability" section after Discovery.
- `skills/bundled/maestro-verify/SKILL.md` — add one-line note that `task observe` is dev-time, distinct from `runtime check`.
- `src/infra/domain/bundled-skill-templates.ts` — auto via `bun run sync:bundled-skills`.

## Command deps shape

```ts
interface TaskObserveCommandDeps {
  readonly buildPrometheusAdapter?: (baseUrl: string) => DevObservabilityPort;
  readonly buildLogTailAdapter?: (filePath?: string) => DevObservabilityPort;
  readonly recordEvidence?: typeof defaultRecordEvidence;
  readonly getEvidenceStore?: (repoRoot: string) => EvidenceStorePort;
}
```

Defaults: `DevPrometheusAdapter` ctor, `LogTailAdapter` ctor, real `recordEvidence`, `buildV2Services(...).evidenceStore`. Tests inject stubs.

## Test plan

| Case | Assertion |
|---|---|
| metrics happy path | stdout contains `value=42.5`, exit 0 |
| metrics --json | parses to `{kind:"metrics", value:42.5}` |
| metrics adapter throws HTTP 502 | exit 2, stderr names error |
| metrics with no URL | exit 1, stderr names MAESTRO_PROMETHEUS_URL |
| logs happy path | 3 line texts in stdout, exit 0 |
| logs --filter threaded | stub adapter receives correct filter arg |
| logs --lines threaded | stub adapter receives correct lines arg |
| logs adapter throws ENOENT | exit 2 |
| logs with no path | exit 1 |
| --record --task metrics | recordEvidence called with manual-note + `[dev-observation:metrics]` |
| --record --task logs | recordEvidence called with manual-note + `[dev-observation:logs]` |
| --record without --task | exit 1, stderr names missing flag |
| --json shape stability | trailing newline, no stray output |

Test harness mirrors `runtime-check.command.test.ts`: build a Commander via `makeProgram(deps)` with `.exitOverride()`, inject stubs, call `program.parseAsync(...)`.

## After-edit checks

1. `bun run build`
2. `bun test`
3. `bun run sync:bundled-skills`
4. `bun run check:bundled-skills`
5. `bun run check:boundaries`

## Risks

- **R1**: Three-level Commander nesting (`task observe metrics`) help/exit paths — covered by `--help` smoke test in the test suite.
- **R2**: `--record` without `--task` silently no-ops — explicit exit-1 test.
- **R3**: Adapter constructor throws synchronously — wrap in try/catch inside the action, not at module load.
- **R4**: `bun run check:bundled-skills` fails if SKILL.md edit is malformed — run before commit.

## Out of scope

- `--follow` / live streaming (passive-harness invariant).
- Promotion of `dev-observation` to a first-class `EvidenceKind`.
- Loki/LogQL adapter.
- New npm/bun dependencies.
- MCP tool surface for `task observe` (dev-time observation is ephemeral).

## Rollback

`git revert HEAD --no-edit` — no migrations, no env-var side effects. `MAESTRO_PROMETHEUS_URL` and `MAESTRO_DEV_LOG_FILE` are read but not written.

## Status

Design complete. Ready for implementation.
