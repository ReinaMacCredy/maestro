import type { RuntimeSignal, RuntimeSignalOperator } from "@/shared/domain/legacy-spec/index.js";
import type { RuntimeSignalResult } from "../domain/types.js";
import type { RuntimeMonitorPort } from "../ports/monitor.port.js";
import { assertNever } from "@/shared/lib/assert-never.js";

export class PrometheusRuntimeMonitor implements RuntimeMonitorPort {
  constructor(
    private readonly baseUrl: string,
    private readonly fetchFn: typeof fetch = fetch,
  ) {}

  async query(signal: RuntimeSignal): Promise<RuntimeSignalResult> {
    const url = `${this.baseUrl}/api/v1/query?query=${encodeURIComponent(signal.query)}`;
    const res = await this.fetchFn(url);
    if (!res.ok) throw new Error(`prometheus: HTTP ${res.status}`);
    const body = await res.json() as { status?: string; error?: string; data?: { result?: unknown[] } };
    if (body?.status !== "success") throw new Error(`prometheus: ${body?.error ?? "unknown"}`);
    const result = body?.data?.result;
    if (!Array.isArray(result) || result.length === 0) {
      throw new Error("prometheus: empty result vector");
    }
    // Prometheus instant queries return result entries as
    //   { metric: {...}, value: [<unix_ts>, "<stringified-number>"] }
    // and matrix queries return `values: [[ts, "n"], ...]`. We accept either
    // shape here and also the bare [ts, value] tuple some adapters synthesize
    // for tests.
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
      threshold: signal.threshold.value,
      operator: signal.threshold.operator,
      pass: compare(value, signal.threshold.operator, signal.threshold.value),
      sampled_at: new Date().toISOString(),
    };
  }
}

function compare(a: number, op: RuntimeSignalOperator, b: number): boolean {
  switch (op) {
    case ">":  return a > b;
    case "<":  return a < b;
    case ">=": return a >= b;
    case "<=": return a <= b;
    case "==": return a === b;
    default: return assertNever(op);
  }
}
