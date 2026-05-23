import { describe, expect, it } from "bun:test";
import {
  compareEnvelopesByCreatedAt,
  registerHandoffTools,
} from "@/features/mcp/server/tools/handoff-tools.js";
import type { CallToolResult } from "@/features/mcp/server/errors.js";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
  HandoffPickup,
} from "@/repo/handoff-emitter.port.js";
import type { Services } from "@/services.js";

const envelope = (id: string, created_at: unknown): HandoffEnvelope =>
  ({
    id,
    task_id: "tsk-1",
    trigger_verb: "task:claim",
    created_at: created_at as string,
  }) as HandoffEnvelope;

describe("compareEnvelopesByCreatedAt", () => {
  it("orders well-formed envelopes ascending by created_at", () => {
    const a = envelope("a", "2026-05-01T00:00:00.000Z");
    const b = envelope("b", "2026-05-02T00:00:00.000Z");
    const sorted = [b, a].slice().sort(compareEnvelopesByCreatedAt);
    expect(sorted.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("does not throw when an envelope is missing created_at", () => {
    const a = envelope("a", undefined);
    const b = envelope("b", "2026-05-02T00:00:00.000Z");
    expect(() => [b, a].slice().sort(compareEnvelopesByCreatedAt)).not.toThrow();
  });

  it("places envelopes with missing created_at first", () => {
    const a = envelope("a", undefined);
    const b = envelope("b", "2026-05-02T00:00:00.000Z");
    const sorted = [b, a].slice().sort(compareEnvelopesByCreatedAt);
    expect(sorted.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("handles non-string created_at values defensively", () => {
    const a = envelope("a", 42);
    const b = envelope("b", "2026-05-02T00:00:00.000Z");
    expect(() => [b, a].slice().sort(compareEnvelopesByCreatedAt)).not.toThrow();
  });
});

// Test harness: registerHandoffTools calls server.registerTool(name, config, cb).
// Capturing the handlers lets the test invoke them directly without spinning up
// a real MCP server transport.
type ToolHandler = (args: Record<string, unknown>) => Promise<CallToolResult>;

function makeServerStub(): { server: unknown; handlers: Map<string, ToolHandler> } {
  const handlers = new Map<string, ToolHandler>();
  const server = {
    registerTool: (
      name: string,
      _config: unknown,
      cb: ToolHandler,
    ): void => {
      handlers.set(name, cb);
    },
  };
  return { server, handlers };
}

function makeEmitterWithEnvelopes(envelopes: HandoffEnvelope[]): HandoffEmitterPort {
  const pickups = new Map<string, HandoffPickup>();
  return {
    async emit(env) {
      envelopes.push(env);
    },
    async list() {
      return envelopes;
    },
    async get(id) {
      return envelopes.find((e) => e.id === id);
    },
    async markPickedUp(envelopeId, pickup) {
      pickups.set(envelopeId, pickup);
    },
    async getPickup(envelopeId) {
      return pickups.get(envelopeId);
    },
    async listPickups() {
      return [...pickups.values()];
    },
  };
}

function parseListPayload(result: CallToolResult): {
  items: { id: string; to_agent?: string }[];
} {
  const text = result.content[0]?.text ?? "{}";
  return JSON.parse(text) as {
    items: { id: string; to_agent?: string }[];
  };
}

describe("registerHandoffTools — handoff_list to_agent filter", () => {
  const baseEnvelopes: HandoffEnvelope[] = [
    {
      id: "hnd-a",
      task_id: "tsk-1",
      trigger_verb: "task:claim",
      created_at: "2026-05-01T00:00:00.000Z",
      to_agent: "codex",
    },
    {
      id: "hnd-b",
      task_id: "tsk-1",
      trigger_verb: "task:claim",
      created_at: "2026-05-02T00:00:00.000Z",
      to_agent: "claude-code",
    },
    {
      id: "hnd-c",
      task_id: "tsk-1",
      trigger_verb: "task:claim",
      created_at: "2026-05-03T00:00:00.000Z",
      // legacy: no to_agent
    },
  ];

  function buildHandler(): ToolHandler {
    const { server, handlers } = makeServerStub();
    const emitter = makeEmitterWithEnvelopes([...baseEnvelopes]);
    const services = { handoffEmitter: emitter } as unknown as Services;
    registerHandoffTools(
      server as Parameters<typeof registerHandoffTools>[0],
      { getServices: () => services, sessionId: "test-session" },
    );
    const handler = handlers.get("maestro_handoff_list");
    if (!handler) throw new Error("handoff_list handler not registered");
    return handler;
  }

  it("filtered call returns exactly the envelope addressed to the matching tool", async () => {
    const handler = buildHandler();
    const result = await handler({ to_agent: "codex" });
    const data = parseListPayload(result);
    expect(data.items.map((i) => i.id)).toEqual(["hnd-a"]);
    expect(data.items[0]?.to_agent).toBe("codex");
  });

  it("excludes legacy envelopes (no to_agent) from a filtered query", async () => {
    const handler = buildHandler();
    const result = await handler({ to_agent: "codex" });
    const data = parseListPayload(result);
    expect(data.items.map((i) => i.id)).not.toContain("hnd-c");
  });

  it("unfiltered call returns all envelopes including legacy", async () => {
    const handler = buildHandler();
    const result = await handler({});
    const data = parseListPayload(result);
    expect(data.items.map((i) => i.id).sort()).toEqual(["hnd-a", "hnd-b", "hnd-c"]);
  });
});

describe("registerHandoffTools — handoff_emit to_agent passthrough", () => {
  it("persists to_agent onto the emitted envelope", async () => {
    const { server, handlers } = makeServerStub();
    const stored: HandoffEnvelope[] = [];
    const emitter = makeEmitterWithEnvelopes(stored);
    const services = { handoffEmitter: emitter } as unknown as Services;
    registerHandoffTools(
      server as Parameters<typeof registerHandoffTools>[0],
      { getServices: () => services, sessionId: "test-session" },
    );
    const handler = handlers.get("maestro_handoff_emit");
    if (!handler) throw new Error("handoff_emit handler not registered");

    const result = await handler({
      task_id: "tsk-abc-def123",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });

    const text = result.content[0]?.text ?? "{}";
    const payload = JSON.parse(text) as { envelope: HandoffEnvelope };
    expect(result.isError).toBeUndefined();
    expect(payload.envelope.to_agent).toBe("codex");
    expect(stored).toHaveLength(1);
    expect(stored[0]?.to_agent).toBe("codex");
  });
});
