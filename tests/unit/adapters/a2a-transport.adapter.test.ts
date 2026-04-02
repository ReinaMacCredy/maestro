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

    it("parses multiline SSE data frames and tolerates telemetry callback failures", async () => {
    const sseServer = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      routes: {
        "/.well-known/agent-card.json": () => Response.json({
          name: "Multiline Worker",
          description: "Streams multiline SSE data",
          protocolVersion: "0.3.0",
          version: "0.1.0",
          url: `http://127.0.0.1:${sseServer.port}/a2a/jsonrpc`,
          capabilities: { streaming: true, pushNotifications: false },
          defaultInputModes: ["text"],
          defaultOutputModes: ["text"],
          skills: [],
        }),
        "/a2a/jsonrpc": () => new Response(
          [
            "event: message",
            "data: {\"result\":{\"kind\":\"artifact-update\",",
            "data: \"taskId\":\"task-1\",\"contextId\":\"ctx-1\",",
            "data: \"artifact\":{\"parts\":[{\"kind\":\"text\",\"text\":\"multiline ok\"}]}}}",
            "",
            "event: message",
            "data: {\"result\":{\"kind\":\"status-update\",\"taskId\":\"task-1\",\"contextId\":\"ctx-1\",\"status\":{\"state\":\"completed\"}}}",
            "",
          ].join("\n"),
          {
            headers: { "content-type": "text/event-stream" },
          },
        ),
      },
    });

    try {
      const adapter = new A2aTransportAdapter();
      const result = await adapter.spawn(
        {
          enabled: true,
          transport: "a2a",
          url: `http://127.0.0.1:${sseServer.port}`,
        },
        "implement feature",
        {
          cwd: process.cwd(),
          featureId: "f1",
          missionId: "m1",
          workerSlug: "a2a-worker",
          onEvent: async () => {
            throw new Error("event store offline");
          },
        },
      );

      expect(result.success).toBe(true);
      expect(result.parsedOutput).toContain("multiline ok");
      } finally {
        await sseServer.stop(true);
      }
    });

    it("fails fast when the A2A stream does not answer before the connect timeout", async () => {
      const hangingServer = Bun.serve({
        port: 0,
        hostname: "127.0.0.1",
        routes: {
          "/.well-known/agent-card.json": () => Response.json({
            name: "Slow Connect Worker",
            description: "Never answers the stream request",
            protocolVersion: "0.3.0",
            version: "0.1.0",
            url: `http://127.0.0.1:${hangingServer.port}/a2a/jsonrpc`,
            capabilities: { streaming: true, pushNotifications: false },
            defaultInputModes: ["text"],
            defaultOutputModes: ["text"],
            skills: [],
          }),
          "/a2a/jsonrpc": async () => {
            await Bun.sleep(200);
            return new Response("event: message\n\n", {
              headers: { "content-type": "text/event-stream" },
            });
          },
        },
      });

      try {
        const adapter = new A2aTransportAdapter({ connectTimeoutMs: 50, idleTimeoutMs: 500 });
        const events: WorkerProgressEvent[] = [];
        const result = await adapter.spawn(
          {
            enabled: true,
            transport: "a2a",
            url: `http://127.0.0.1:${hangingServer.port}`,
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
        expect(result.summary).toContain("Timed out connecting to a2a-worker");
        expect(result.stderrRaw).toContain("Timed out connecting to a2a-worker");
        expect(events.at(-1)).toMatchObject({
          kind: "status",
          runtimeState: "failed",
          text: "Timed out connecting to a2a-worker",
        });
      } finally {
        await hangingServer.stop(true);
      }
    });

    it("fails fast when the A2A stream goes idle past the timeout", async () => {
      const idleServer = Bun.serve({
        port: 0,
        hostname: "127.0.0.1",
        routes: {
          "/.well-known/agent-card.json": () => Response.json({
            name: "Idle Worker",
            description: "Accepts the stream but never emits output",
            protocolVersion: "0.3.0",
            version: "0.1.0",
            url: `http://127.0.0.1:${idleServer.port}/a2a/jsonrpc`,
            capabilities: { streaming: true, pushNotifications: false },
            defaultInputModes: ["text"],
            defaultOutputModes: ["text"],
            skills: [],
          }),
          "/a2a/jsonrpc": () => new Response(
            new ReadableStream<Uint8Array>({
              start(controller) {
                controller.enqueue(new TextEncoder().encode(": keep-alive\n\n"));
              },
            }),
            {
              headers: { "content-type": "text/event-stream" },
            },
          ),
        },
      });

      try {
        const adapter = new A2aTransportAdapter({ connectTimeoutMs: 200, idleTimeoutMs: 50 });
        const events: WorkerProgressEvent[] = [];
        const result = await adapter.spawn(
          {
            enabled: true,
            transport: "a2a",
            url: `http://127.0.0.1:${idleServer.port}`,
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
        expect(result.summary).toContain("Timed out waiting for A2A worker 'a2a-worker' output");
        expect(result.stderrRaw).toContain("Timed out waiting for A2A worker 'a2a-worker' output");
        expect(events.at(-1)).toMatchObject({
          kind: "status",
          runtimeState: "failed",
          text: "Timed out waiting for A2A worker 'a2a-worker' output",
        });
      } finally {
        await idleServer.stop(true);
      }
    });
  });
