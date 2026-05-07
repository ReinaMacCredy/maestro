import { describe, expect, it } from "bun:test";
import { checkRuntimeSignals } from "@/features/runtime/usecases/check-runtime-signals.usecase.js";
import type { RuntimeMonitorPort } from "@/features/runtime/ports/monitor.port.js";
import type { RuntimeSignal } from "@/features/spec";
import type { RuntimeSignalResult } from "@/features/runtime/domain/types.js";

function makeSignal(overrides: Partial<RuntimeSignal> = {}): RuntimeSignal {
  return {
    name: "p99-latency",
    provider: "prometheus",
    query: "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
    threshold: { operator: "<", value: 0.5 },
    severity: "critical",
    ...overrides,
  };
}

function makeResult(pass: boolean): RuntimeSignalResult {
  return {
    value: pass ? 0.1 : 0.9,
    threshold: 0.5,
    operator: "<",
    pass,
    sampled_at: "2026-05-05T00:00:00.000Z",
  };
}

function stubMonitor(result: RuntimeSignalResult | Error): RuntimeMonitorPort {
  return {
    query: async () => {
      if (result instanceof Error) throw result;
      return result;
    },
  };
}

describe("checkRuntimeSignals", () => {
  it("returns result outcome for a prometheus signal", async () => {
    const signal = makeSignal();
    const result = makeResult(true);
    const outcomes = await checkRuntimeSignals({
      signals: [signal],
      monitor: stubMonitor(result),
      now: () => new Date("2026-05-05T00:00:00.000Z"),
    });

    expect(outcomes).toHaveLength(1);
    expect(outcomes[0]!.signal).toBe(signal);
    expect(outcomes[0]!.result).toEqual(result);
    expect(outcomes[0]!.note).toBeUndefined();
  });

  it("returns unsupported provider outcome without calling monitor", async () => {
    const signal = makeSignal({ provider: "datadog" });
    let queryCalled = false;
    const monitor: RuntimeMonitorPort = {
      query: async () => { queryCalled = true; return makeResult(true); },
    };

    const outcomes = await checkRuntimeSignals({
      signals: [signal],
      monitor,
      now: () => new Date(),
    });

    expect(outcomes).toHaveLength(1);
    expect(outcomes[0]!.result).toBeUndefined();
    expect(outcomes[0]!.note).toBe("unsupported provider");
    expect(queryCalled).toBe(false);
  });

  it("returns error outcome when monitor.query throws", async () => {
    const signal = makeSignal();
    const outcomes = await checkRuntimeSignals({
      signals: [signal],
      monitor: stubMonitor(new Error("connection refused")),
      now: () => new Date(),
    });

    expect(outcomes).toHaveLength(1);
    expect(outcomes[0]!.result).toBeUndefined();
    expect(outcomes[0]!.note).toBe("error: connection refused");
  });

  it("handles a mix of prometheus and unknown provider signals", async () => {
    const prom = makeSignal({ name: "prom-signal", provider: "prometheus" });
    const dd = makeSignal({ name: "dd-signal", provider: "datadog" });
    const result = makeResult(true);
    const outcomes = await checkRuntimeSignals({
      signals: [prom, dd],
      monitor: stubMonitor(result),
      now: () => new Date(),
    });

    expect(outcomes).toHaveLength(2);
    expect(outcomes[0]!.result).toEqual(result);
    expect(outcomes[1]!.note).toBe("unsupported provider");
  });

  it("returns empty array for no signals", async () => {
    const outcomes = await checkRuntimeSignals({
      signals: [],
      monitor: stubMonitor(makeResult(true)),
      now: () => new Date(),
    });
    expect(outcomes).toHaveLength(0);
  });
});
