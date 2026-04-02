import { afterEach, describe, expect, it } from "bun:test";
import { getWorkerHealthRows, probeA2aWorkerReadiness } from "../../../src/usecases/worker-health.usecase.js";
import { startA2aTestServer, type TestA2aServer } from "../../helpers/a2a-test-server.js";

let server: TestA2aServer | undefined;

afterEach(async () => {
  await server?.close();
  server = undefined;
});

describe("getWorkerHealthRows", () => {
  it("marks enabled workers ready when probes succeed", async () => {
    const rows = await getWorkerHealthRows(
      {
        codex: {
          enabled: true,
          transport: "cli",
          command: "codex",
          outputMode: "raw",
        },
      },
      {
        probeCli: async () => ({
          status: "ready",
          checks: [
            { label: "command found", ok: true },
            { label: "launch test", ok: true },
          ],
        }),
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "codex",
      status: "ready",
    });
    expect(rows[0]?.checks.some((check) => check.label === "launch test" && check.ok)).toBe(true);
  });

  it("marks workers degraded when the probe fails after the command is found", async () => {
    const rows = await getWorkerHealthRows(
      {
        codex: {
          enabled: true,
          transport: "cli",
          command: "codex",
          outputMode: "raw",
        },
      },
      {
        probeCli: async () => ({
          status: "degraded",
          detail: "auth/session check failed",
          checks: [
            { label: "command found", ok: true },
            { label: "auth/session", ok: false, detail: "auth/session check failed" },
          ],
        }),
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "codex",
      status: "degraded",
      detail: "auth/session check failed",
    });
  });

  it("marks disabled workers without probing them", async () => {
    const rows = await getWorkerHealthRows(
      {
        gemini: {
          enabled: false,
          transport: "cli",
          command: "gemini",
          outputMode: "stream-json",
        },
      },
      {
        probeCli: async () => {
          throw new Error("should not be called");
        },
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "gemini",
      status: "disabled",
    });
  });

  it("skips active probes in passive mode", async () => {
    const rows = await getWorkerHealthRows(
      {
        codex: {
          enabled: true,
          transport: "cli",
          command: "codex",
          outputMode: "raw",
        },
      },
      {
        probe: false,
        probeCli: async () => {
          throw new Error("should not be called");
        },
      },
    );

    expect(rows[0]).toMatchObject({
      slug: "codex",
      status: "ready",
      detail: "configured; not checked in read-only mode",
    });
    expect(rows[0]?.checks).toEqual([
      { label: "probe skipped", ok: true, detail: "read-only mode" },
    ]);
  });

  it("confirms the A2A agent card and JSON-RPC endpoint when the worker is reachable", async () => {
    server = await startA2aTestServer("a2a worker ok");

    const result = await probeA2aWorkerReadiness({
      enabled: true,
      transport: "a2a",
      url: server.baseUrl,
    });

    expect(result.status).toBe("ready");
    expect(result.detail).toContain("JSON-RPC endpoint");
    expect(result.checks).toEqual([
      { label: "agent card", ok: true, detail: "reachable" },
      expect.objectContaining({ label: "json-rpc endpoint", ok: true }),
    ]);
  });

  it("degrades A2A readiness when the agent card omits a JSON-RPC endpoint", async () => {
    const badCardServer = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      routes: {
        "/.well-known/agent-card.json": () => Response.json({
          name: "Broken Worker",
          description: "Missing endpoint",
          protocolVersion: "0.3.0",
          version: "0.1.0",
          capabilities: {
            streaming: true,
            pushNotifications: false,
          },
          defaultInputModes: ["text"],
          defaultOutputModes: ["text"],
          skills: [],
        }),
      },
    });

    try {
      const result = await probeA2aWorkerReadiness({
        enabled: true,
        transport: "a2a",
        url: `http://127.0.0.1:${badCardServer.port}`,
      });

      expect(result.status).toBe("degraded");
      expect(result.detail).toContain("JSON-RPC endpoint");
      expect(result.checks).toEqual([
        {
          label: "json-rpc endpoint",
          ok: false,
          detail: "Agent card does not expose a JSON-RPC endpoint",
        },
      ]);
    } finally {
      await badCardServer.stop(true);
    }
  });
});
