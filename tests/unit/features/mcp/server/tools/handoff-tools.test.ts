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

function parsePickupPayload(result: CallToolResult): {
  envelope: HandoffEnvelope;
  pickup: HandoffPickup;
  warnings?: string[];
} {
  const text = result.content[0]?.text ?? "{}";
  return JSON.parse(text) as {
    envelope: HandoffEnvelope;
    pickup: HandoffPickup;
    warnings?: string[];
  };
}

describe("registerHandoffTools — handoff_pickup warn-but-allow", () => {
  function buildPickupHarness(envelopes: HandoffEnvelope[]): {
    handler: ToolHandler;
    emitter: HandoffEmitterPort;
  } {
    const { server, handlers } = makeServerStub();
    const emitter = makeEmitterWithEnvelopes([...envelopes]);
    const services = { handoffEmitter: emitter } as unknown as Services;
    registerHandoffTools(
      server as Parameters<typeof registerHandoffTools>[0],
      { getServices: () => services, sessionId: "test-session" },
    );
    const handler = handlers.get("maestro_handoff_pickup");
    if (!handler) throw new Error("handoff_pickup handler not registered");
    return { handler, emitter };
  }

  const targetedEnvelope = (id: string, to_agent: string): HandoffEnvelope => ({
    id,
    task_id: "tsk-1",
    trigger_verb: "task:claim",
    created_at: "2026-05-01T00:00:00.000Z",
    to_agent,
  });

  const legacyEnvelope = (id: string): HandoffEnvelope => ({
    id,
    task_id: "tsk-1",
    trigger_verb: "task:claim",
    created_at: "2026-05-01T00:00:00.000Z",
  });

  it("returns warnings array when picked_up_by differs from envelope.to_agent", async () => {
    const { handler } = buildPickupHarness([targetedEnvelope("hnd-1", "codex")]);
    const result = await handler({ id: "hnd-1", picked_up_by: "claude-code" });
    const payload = parsePickupPayload(result);
    expect(result.isError).toBeUndefined();
    expect(payload.warnings).toEqual([
      "Envelope was addressed to 'codex'; picked up by 'claude-code'. Pickup recorded; verify this is the envelope you intended.",
    ]);
    expect(payload.envelope).toBeDefined();
    expect(payload.pickup).toBeDefined();
  });

  it("still creates the pickup sidecar on mismatch (warn-but-ALLOW)", async () => {
    const { handler, emitter } = buildPickupHarness([targetedEnvelope("hnd-2", "codex")]);
    const result = await handler({ id: "hnd-2", picked_up_by: "claude-code" });
    const payload = parsePickupPayload(result);
    expect(Array.isArray(payload.warnings)).toBe(true);
    expect(payload.warnings?.length).toBe(1);
    const pickup = await emitter.getPickup("hnd-2");
    expect(pickup).toBeDefined();
    expect(pickup?.picked_up_by).toBe("claude-code");
  });

  it("omits the warnings field entirely when picked_up_by matches to_agent", async () => {
    const { handler } = buildPickupHarness([targetedEnvelope("hnd-3", "codex")]);
    const result = await handler({ id: "hnd-3", picked_up_by: "codex" });
    const text = result.content[0]?.text ?? "{}";
    const payload = JSON.parse(text) as Record<string, unknown>;
    expect(result.isError).toBeUndefined();
    expect("warnings" in payload).toBe(false);
  });

  it("omits the warnings field entirely for legacy envelopes with no to_agent", async () => {
    const { handler } = buildPickupHarness([legacyEnvelope("hnd-4")]);
    const result = await handler({ id: "hnd-4", picked_up_by: "claude-code" });
    const text = result.content[0]?.text ?? "{}";
    const payload = JSON.parse(text) as Record<string, unknown>;
    expect(result.isError).toBeUndefined();
    expect("warnings" in payload).toBe(false);
  });

  it("uses deps.sessionId in the warning when picked_up_by is omitted from args", async () => {
    const { handler } = buildPickupHarness([targetedEnvelope("hnd-5", "codex")]);
    const result = await handler({ id: "hnd-5" });
    const payload = parsePickupPayload(result);
    expect(result.isError).toBeUndefined();
    expect(payload.warnings).toEqual([
      "Envelope was addressed to 'codex'; picked up by 'test-session'. Pickup recorded; verify this is the envelope you intended.",
    ]);
    expect(payload.pickup.picked_up_by).toBe("test-session");
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
