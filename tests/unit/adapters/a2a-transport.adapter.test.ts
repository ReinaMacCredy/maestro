import { afterEach, describe, expect, it } from "bun:test";
import { A2aTransportAdapter } from "../../../src/adapters/a2a-transport.adapter.js";
import { startA2aTestServer, type TestA2aServer } from "../../helpers/a2a-test-server.js";

let server: TestA2aServer | undefined;

afterEach(async () => {
  await server?.close();
  server = undefined;
});

describe("A2aTransportAdapter", () => {
  it("streams task events from a live A2A server and propagates the remote task handle", async () => {
    server = await startA2aTestServer("a2a worker ok");
    const adapter = new A2aTransportAdapter();
    const events: Array<{
      kind: "status" | "stdout" | "stderr" | "heartbeat";
      runtimeState?: string;
      text?: string;
      sessionId?: string;
    }> = [];

    const result = await adapter.spawn(
      {
        enabled: true,
        transport: "a2a",
        url: server.baseUrl,
      },
      "implement feature",
      {
        cwd: process.cwd(),
        featureId: "f1",
        missionId: "m1",
        workerSlug: "a2a-worker",
        onEvent: async (event) => {
          events.push(event);
        },
      },
    );

    expect(result.success).toBe(true);
    expect(result.summary).toBe("a2a worker ok");
    expect(result.parsedOutput).toContain("a2a worker ok");
    expect(result.stdoutRaw).toContain("\"kind\":\"status-update\"");
    expect(result.stdoutRaw).toContain("\"kind\":\"artifact-update\"");
    expect(events[0]).toMatchObject({
      kind: "status",
      runtimeState: "starting",
      text: "Connecting to a2a-worker",
    });
    const taskHandleEvents = events.filter((event) => event.sessionId);
    expect(taskHandleEvents.length).toBeGreaterThan(0);
    expect(new Set(taskHandleEvents.map((event) => event.sessionId)).size).toBe(1);
    expect(taskHandleEvents.some((event) => event.kind === "stdout" && event.text === "a2a worker ok")).toBe(true);
    expect(events.at(-1)).toMatchObject({
      kind: "status",
      runtimeState: "completed",
      text: "a2a-worker completed",
    });
  });

  it("emits a terminal failed status event when the stream errors", async () => {
    const adapter = new A2aTransportAdapter();
    const events: Array<{
      kind: "status" | "stdout" | "stderr" | "heartbeat";
      runtimeState?: string;
      text?: string;
      sessionId?: string;
    }> = [];

    const result = await adapter.spawn(
      {
        enabled: true,
        transport: "a2a",
        url: "http://127.0.0.1:1",
      },
      "implement feature",
      {
        cwd: process.cwd(),
        featureId: "f1",
        missionId: "m1",
        workerSlug: "a2a-worker",
        onEvent: async (event) => {
          events.push(event);
        },
      },
    );

    expect(result.success).toBe(false);
    expect(result.failureClass).toBe("infrastructure");
    expect(result.summary).toContain("Failed to communicate with A2A worker");
    expect(result.stderrRaw.length).toBeGreaterThan(0);
    expect(events.at(-1)).toMatchObject({
      kind: "status",
      runtimeState: "failed",
    });
    expect(events[0]).toMatchObject({
      kind: "status",
      runtimeState: "starting",
      text: "Connecting to a2a-worker",
    });
  });
});
