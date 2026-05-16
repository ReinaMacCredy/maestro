# Dev-time Observability

Per-worktree observability for the agent *while it works*: query a metric
or tail a log without leaving the run. Surfaced through `maestro task
observe`.

Distinct from `maestro runtime check` (L7 release gate, consumes
`Spec.runtime_signals`, emits `runtime-signal` Evidence into the verdict
loop). Dev observability is advisory; it produces no Verdict-bearing
Evidence by default. `--record` opts in to a `manual-note` row tagged
`[dev-observation]` for traceability.

---

## Why a separate port

The L7 monitor port (`RuntimeMonitorPort`) is shaped around
`RuntimeSignal` — query + threshold + operator + pass/fail. That shape is
exactly what `runtime check` needs and exactly wrong for ad-hoc
agent-driven inspection: an agent asking "what's the current request rate"
doesn't have a threshold yet. The dev port returns scalars and log lines,
not gate verdicts.

Sources:
- Port: `src/features/runtime/ports/dev-observability.port.ts`
- Prometheus adapter (HTTP, no new deps): `src/features/runtime/adapters/dev-prometheus.adapter.ts`
- File-tail adapter (no follow): `src/features/runtime/adapters/log-tail.adapter.ts`
- Command: `src/features/runtime/commands/task-observe.command.ts`

---

## Verb surface

```bash
maestro task observe metrics <promql> [--prometheus-url <url>] [--json] [--record --task <id>]
maestro task observe logs [--log-file <path>] [--lines <n>] [--filter <substring>] [--json] [--record --task <id>]
```

Both subcommands are read-only by default. `--record --task <id>` writes a
`manual-note` Evidence row at `agent-claimed-locally` whose payload note
begins with `[dev-observation:metrics]` or `[dev-observation:logs]`. The
note is not verdict-gating; it preserves the observation for the next
`task introspect`.

Prometheus URL precedence:
`--prometheus-url` > `MAESTRO_PROMETHEUS_URL` > error (no implicit default).

Log file path precedence:
`--log-file` > `MAESTRO_DEV_LOG_FILE` > error (no implicit default).

`--lines` must be a positive integer (default 100). `--filter <text>` is a
substring filter applied before tail.

Exit codes: 0 success, 1 config error (missing URL or log path, invalid
`--lines`, `--record` without `--task`), 2 backend error (Prometheus query
failed, log read failed, evidence record failed).

---

## When to use each

- **Pre-implement reconnaissance.** Tail the dev server log before editing
  to see baseline error rate, then again after edits to confirm no
  regression — without leaving the agent loop.
- **Post-edit smoke check.** Query a custom Prometheus counter your
  feature increments to confirm the new code path is live before
  requesting the verdict.
- **Debug a recurring loop.** Combine with `task introspect`'s
  `loopWarning` — if the agent retries the same command, the log tail can
  expose *why* a unit test keeps failing.

---

## What this is not

- Not a deploy gate. Use `maestro runtime check --task <id>` for that.
- Not a daemon. Each invocation is a single HTTP call or file read.
- Not a Loki adapter. Deferred per the phase plan; the port stays
  language-agnostic so one can land later.
- Not a substitute for context-aware reasoning. The agent decides when to
  observe; nothing schedules these.
