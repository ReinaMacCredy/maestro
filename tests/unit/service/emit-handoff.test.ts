import { describe, expect, it } from "bun:test";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
  HandoffPickup,
} from "@/repo/handoff-emitter.port.js";
import { emitHandoff } from "@/service/emit-handoff.js";

function makeEmitter(): { emitter: HandoffEmitterPort; emitted: HandoffEnvelope[] } {
  const emitted: HandoffEnvelope[] = [];
  const pickups = new Map<string, HandoffPickup>();
  return {
    emitted,
    emitter: {
      async emit(env) {
        emitted.push(env);
      },
      async list() {
        return emitted;
      },
      async get(id) {
        return emitted.find((e) => e.id === id);
      },
      async markPickedUp(envelopeId, pickup) {
        if (pickups.has(envelopeId)) {
          throw new Error("EEXIST");
        }
        pickups.set(envelopeId, pickup);
      },
      async getPickup(envelopeId) {
        return pickups.get(envelopeId);
      },
    },
  };
}

describe("emitHandoff", () => {
  it("returns undefined when no emitter is wired", async () => {
    const result = await emitHandoff(
      {},
      { task_id: "tsk-1", trigger_verb: "task:claim" },
    );
    expect(result).toBeUndefined();
  });

  it("stamps an id, timestamp, and trigger_verb on the envelope", async () => {
    const { emitter, emitted } = makeEmitter();
    const FROZEN = new Date("2026-05-15T10:00:00.000Z");
    const result = await emitHandoff(
      { emitter, clock: () => FROZEN, idFactory: () => "hnd-static" },
      { task_id: "tsk-9", trigger_verb: "task:claim", agent_id: "agent-z" },
    );
    expect(result?.id).toBe("hnd-static");
    expect(result?.created_at).toBe(FROZEN.toISOString());
    expect(result?.trigger_verb).toBe("task:claim");
    expect(result?.agent_id).toBe("agent-z");
    expect(emitted).toHaveLength(1);
  });

  it("omits optional fields when not provided rather than writing undefined", async () => {
    const { emitter, emitted } = makeEmitter();
    await emitHandoff(
      { emitter },
      { task_id: "tsk-min", trigger_verb: "task:block", reason: "blocked-on-x" },
    );
    expect(emitted[0]).not.toHaveProperty("agent_id");
    expect(emitted[0]).not.toHaveProperty("worktree_path");
    expect(emitted[0]).not.toHaveProperty("spec_path");
    expect(emitted[0]!.reason).toBe("blocked-on-x");
  });
});
