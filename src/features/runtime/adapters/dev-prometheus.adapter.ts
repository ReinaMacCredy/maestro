import type {
  DevLogTail,
  DevMetricSample,
  DevObservabilityPort,
} from "../ports/dev-observability.port.js";

/**
 * Dev-time Prometheus adapter for `maestro task observe metrics`. Reuses the
 * Prometheus instant-query HTTP shape used by the deploy-gate adapter, but
 * returns a single scalar sample with a source label suitable for
 * agent-facing output instead of a release-gate `RuntimeSignalResult`.
 *
 * Tail behavior is not supported here. The command wires the file-tail
 * adapter for that surface.
 */
export class DevPrometheusAdapter implements DevObservabilityPort {
  constructor(
    private readonly baseUrl: string,
    private readonly fetchFn: typeof fetch = fetch,
  ) {}

  async queryMetric(query: string): Promise<DevMetricSample> {
    const url = `${this.baseUrl}/api/v1/query?query=${encodeURIComponent(query)}`;
    const res = await this.fetchFn(url);
    if (!res.ok) throw new Error(`prometheus: HTTP ${res.status}`);
    const body = await res.json() as {
      status?: string;
      error?: string;
      data?: { result?: unknown[] };
    };
    if (body?.status !== "success") {
      throw new Error(`prometheus: ${body?.error ?? "unknown"}`);
    }
    const result = body?.data?.result;
    if (!Array.isArray(result) || result.length === 0) {
      throw new Error("prometheus: empty result vector");
    }
    const sample = result[0];
    const tuple = Array.isArray(sample)
      ? (sample as [unknown, unknown])
      : (sample as { value?: [unknown, unknown]; values?: [unknown, unknown][] }).value
        ?? (sample as { values?: [unknown, unknown][] }).values?.[0];
    const rawValue = Array.isArray(tuple) ? tuple[1] : undefined;
    const value = typeof rawValue === "string" || typeof rawValue === "number"
      ? parseFloat(String(rawValue))
      : NaN;
    if (Number.isNaN(value)) throw new Error("prometheus: non-numeric value");
    return {
      value,
      sampledAt: new Date().toISOString(),
      source: `prometheus@${this.baseUrl}`,
    };
  }

  async tailLogs(): Promise<DevLogTail> {
    throw new Error("DevPrometheusAdapter does not support tailLogs; use the log-tail adapter.");
  }
}
