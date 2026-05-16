# Runtime Monitoring

`maestro runtime check` queries runtime signals declared in `Spec.runtime_signals` and records one `runtime-signal` Evidence row per signal. Exit code is always 0; downstream verdict or risk policy logic decides what to do with `pass=false` rows.

## `RuntimeMonitorPort` interface

Defined at `src/features/runtime/ports/monitor.port.ts`:

```typescript
interface RuntimeMonitorPort {
  query(signal: RuntimeSignal): Promise<RuntimeSignalResult>;
}
```

`RuntimeSignal` comes from `src/shared/domain/legacy-spec/types.ts`. `RuntimeSignalResult` is defined in `src/features/runtime/domain/types.ts`:

```typescript
interface RuntimeSignalResult {
  value: number;
  threshold: number;
  operator: RuntimeSignalOperator;
  pass: boolean;
  sampled_at: string; // ISO 8601
}
```

The port is demand-driven: add a new adapter by implementing `RuntimeMonitorPort` and registering it in the command's `buildMonitor` factory (see [Adding a new adapter](#adding-a-new-adapter) below).

## Prometheus adapter

The built-in adapter targets Prometheus instant queries (`/api/v1/query`). It is implemented at `src/features/runtime/adapters/prometheus.adapter.ts`.

### URL configuration

Provider base URL precedence (highest first):

1. `--provider-base-url <url>` CLI flag
2. `MAESTRO_PROMETHEUS_URL` environment variable
3. `http://localhost:9090` (default)

### Query format

The adapter sends the signal's `query` field as a PromQL instant query and reads the first result vector value. Non-numeric or empty results surface as errors — the Evidence row is still recorded with `pass=false` and a `note` explaining the failure.

### Threshold operators

| Operator | Meaning |
|---|---|
| `>` | value must be greater than threshold |
| `<` | value must be less than threshold |
| `>=` | value must be greater than or equal to threshold |
| `<=` | value must be less than or equal to threshold |
| `==` | value must equal threshold |

## Declaring `Spec.runtime_signals`

`runtime_signals` is an array of `RuntimeSignal` objects in the Spec. Each signal specifies the provider, query, threshold, and severity.

Schema (from `src/shared/domain/legacy-spec/types.ts`):

```typescript
interface RuntimeSignal {
  name: string;
  description?: string;
  provider: string;       // e.g. "prometheus"
  query: string;          // provider-specific query string
  threshold: {
    operator: ">" | "<" | ">=" | "<=" | "==";
    value: number;
  };
  severity: "info" | "warn" | "critical";
}
```

Example Spec snippet:

```json
{
  "schema_version": 2,
  "runtime_signals": [
    {
      "name": "p99_latency",
      "description": "99th percentile HTTP latency",
      "provider": "prometheus",
      "query": "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
      "threshold": { "operator": "<", "value": 0.5 },
      "severity": "warn"
    },
    {
      "name": "error_rate",
      "description": "5xx error rate",
      "provider": "prometheus",
      "query": "rate(http_requests_total{status=~\"5..\"}[5m]) / rate(http_requests_total[5m])",
      "threshold": { "operator": "<", "value": 0.01 },
      "severity": "critical"
    }
  ]
}
```

## `runtime-signal` Evidence payload

Each recorded Evidence row uses the `RuntimeSignalPayload` shape from `src/features/evidence/domain/types.ts`:

```typescript
interface RuntimeSignalPayload {
  signal_name: string;
  provider: string;
  query: string;
  value: number;
  threshold: number;
  operator: string;
  pass: boolean;
  sampled_at: string; // ISO 8601
  note?: string;      // present when provider is unsupported or query errored
}
```

Example recorded payload:

```json
{
  "signal_name": "p99_latency",
  "provider": "prometheus",
  "query": "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
  "value": 0.312,
  "threshold": 0.5,
  "operator": "<",
  "pass": true,
  "sampled_at": "2026-05-10T14:23:00.000Z"
}
```

## Advisory semantics at L7

`runtime check` exit code is always 0. At L7, `pass=false` signals are recorded but do not automatically flip the Verdict. To make a failing runtime signal block a PR:

1. Add the `runtime-signal` kind to the evidence gate in `policies/risk.yaml`.
2. Run `maestro runtime check` in your CI workflow before `maestro ci verify`.

## Adding a new adapter

The demand-driven pattern: implement `RuntimeMonitorPort`, then patch `src/features/runtime/commands/runtime-check.command.ts` to route the new provider name to your adapter in `buildMonitor`.

Example for a hypothetical Datadog adapter:

1. Create `src/features/runtime/adapters/datadog.adapter.ts` implementing `RuntimeMonitorPort`.
2. In `runtime-check.command.ts`, extend the `buildMonitor` factory:

```typescript
buildMonitor: (baseUrl: string, provider?: string) => {
  if (provider === "datadog") return new DatadogRuntimeMonitor(baseUrl);
  return new PrometheusRuntimeMonitor(baseUrl);
}
```

3. Signal entries with `"provider": "datadog"` will route to your adapter.
4. Unsupported providers are skipped with a `note: "unsupported provider"` in the Evidence row.

No changes to the port contract or domain types are required when adding an adapter — the port interface is stable.
