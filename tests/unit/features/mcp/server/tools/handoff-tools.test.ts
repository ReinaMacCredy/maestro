import { describe, expect, it } from "bun:test";
import { compareEnvelopesByCreatedAt } from "@/features/mcp/server/tools/handoff-tools.js";
import type { HandoffEnvelope } from "@/repo/handoff-emitter.port.js";

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
