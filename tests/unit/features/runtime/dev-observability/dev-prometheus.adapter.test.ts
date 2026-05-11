import { describe, it, expect } from "bun:test";
import { DevPrometheusAdapter } from "@/features/runtime/index.js";

function stubFetch(body: unknown, ok = true, status = 200): typeof fetch {
  return (async () => {
    return {
      ok,
      status,
      json: async () => body,
    } as Response;
  }) as unknown as typeof fetch;
}

describe("DevPrometheusAdapter", () => {
  it("parses an instant-query scalar and tags the source", async () => {
    const adapter = new DevPrometheusAdapter(
      "http://prom:9090",
      stubFetch({
        status: "success",
        data: { result: [{ metric: {}, value: [1700000000, "42.5"] }] },
      }),
    );
    const sample = await adapter.queryMetric("up");
    expect(sample.value).toBe(42.5);
    expect(sample.source).toBe("prometheus@http://prom:9090");
    expect(typeof sample.sampledAt).toBe("string");
  });

  it("throws on HTTP error", async () => {
    const adapter = new DevPrometheusAdapter("http://prom:9090", stubFetch({}, false, 502));
    await expect(adapter.queryMetric("up")).rejects.toThrow(/HTTP 502/);
  });

  it("throws on prometheus error status", async () => {
    const adapter = new DevPrometheusAdapter(
      "http://prom:9090",
      stubFetch({ status: "error", error: "bad query" }),
    );
    await expect(adapter.queryMetric("up")).rejects.toThrow(/bad query/);
  });

  it("throws on empty result vector", async () => {
    const adapter = new DevPrometheusAdapter(
      "http://prom:9090",
      stubFetch({ status: "success", data: { result: [] } }),
    );
    await expect(adapter.queryMetric("up")).rejects.toThrow(/empty result vector/);
  });

  it("rejects tailLogs to surface adapter scope", async () => {
    const adapter = new DevPrometheusAdapter("http://prom:9090", stubFetch({}));
    await expect(adapter.tailLogs()).rejects.toThrow(/does not support tailLogs/);
  });
});
