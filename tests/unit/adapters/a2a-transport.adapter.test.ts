import { afterEach, describe, expect, it } from "bun:test";
import { A2aTransportAdapter } from "../../../src/adapters/a2a-transport.adapter.js";
import { startA2aTestServer, type TestA2aServer } from "../../helpers/a2a-test-server.js";

let server: TestA2aServer | undefined;

afterEach(async () => {
  await server?.close();
  server = undefined;
});

describe("A2aTransportAdapter", () => {
  it("streams task events from a live A2A server", async () => {
    server = await startA2aTestServer("a2a worker ok");
    const adapter = new A2aTransportAdapter();

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
      },
    );

    expect(result.success).toBe(true);
    expect(result.summary).toBe("a2a worker ok");
    expect(result.parsedOutput).toContain("a2a worker ok");
    expect(result.stdoutRaw).toContain("\"kind\":\"status-update\"");
    expect(result.stdoutRaw).toContain("\"kind\":\"artifact-update\"");
  });
});
