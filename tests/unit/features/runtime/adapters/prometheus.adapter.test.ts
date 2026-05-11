import { describe, expect, it } from "bun:test";
import { PrometheusRuntimeMonitor } from "@/features/runtime/adapters/prometheus.adapter.js";
import type { RuntimeSignal } from "@/features/spec";

function makeSignal(overrides: Partial<RuntimeSignal> = {}): RuntimeSignal {
  return {
    name: "error-rate",
    provider: "prometheus",
    query: "rate(errors_total[5m])",
    threshold: { operator: "<", value: 0.01 },
    severity: "critical",
    ...overrides,
  };
}

function makeSuccessBody(rawValue: string): unknown {
  return {
    status: "success",
    data: {
      result: [
        [1714000000, rawValue],
      ],
    },
  };
}

function stubFetch(body: unknown, status = 200): typeof fetch {
  const fn = (async (_url: string | URL | Request) => ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as Response)) as typeof fetch;
  return fn;
}

describe("PrometheusRuntimeMonitor.query", () => {
  it("parses a valid vector response and computes pass=true for < operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("0.005")));
    const signal = makeSignal({ threshold: { operator: "<", value: 0.01 } });
    const result = await monitor.query(signal);
    expect(result.value).toBe(0.005);
    expect(result.threshold).toBe(0.01);
    expect(result.operator).toBe("<");
    expect(result.pass).toBe(true);
  });

  it("computes pass=false when value exceeds threshold for < operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("0.05")));
    const signal = makeSignal({ threshold: { operator: "<", value: 0.01 } });
    const result = await monitor.query(signal);
    expect(result.pass).toBe(false);
  });

  it("computes pass correctly for > operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("100")));
    const signal = makeSignal({ threshold: { operator: ">", value: 50 } });
    const result = await monitor.query(signal);
    expect(result.value).toBe(100);
    expect(result.pass).toBe(true);
  });

  it("computes pass correctly for >= operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("50")));
    const signal = makeSignal({ threshold: { operator: ">=", value: 50 } });
    const result = await monitor.query(signal);
    expect(result.pass).toBe(true);
  });

  it("computes pass=false for >= when below threshold", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("49")));
    const signal = makeSignal({ threshold: { operator: ">=", value: 50 } });
    const result = await monitor.query(signal);
    expect(result.pass).toBe(false);
  });

  it("computes pass correctly for <= operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("10")));
    const signal = makeSignal({ threshold: { operator: "<=", value: 10 } });
    const result = await monitor.query(signal);
    expect(result.pass).toBe(true);
  });

  it("computes pass correctly for == operator", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("42")));
    const signal = makeSignal({ threshold: { operator: "==", value: 42 } });
    const result = await monitor.query(signal);
    expect(result.pass).toBe(true);
  });

  it("throws on empty result vector", async () => {
    const body = { status: "success", data: { result: [] } };
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(body));
    await expect(monitor.query(makeSignal())).rejects.toThrow("prometheus: empty result vector");
  });

  it("throws on non-200 HTTP response", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch({}, 503));
    await expect(monitor.query(makeSignal())).rejects.toThrow("prometheus: HTTP 503");
  });

  it("throws with prometheus error message on status=error body", async () => {
    const body = { status: "error", error: "query parse error" };
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(body));
    await expect(monitor.query(makeSignal())).rejects.toThrow("prometheus: query parse error");
  });

  it("throws on non-numeric value", async () => {
    const monitor = new PrometheusRuntimeMonitor("http://prometheus", stubFetch(makeSuccessBody("NaN")));
    await expect(monitor.query(makeSignal())).rejects.toThrow("prometheus: non-numeric value");
  });
});
